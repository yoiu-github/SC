use crate::{
    msg::{HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg, ResponseStatus},
    state::{config, config_read, users, users_read, Config, User, UserState},
    utils,
};
use cosmwasm_std::{
    from_binary, to_binary, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, InitResult, Querier, QueryResult, StakingMsg, StdError,
    Storage, Uint128,
};

pub const USCRT: &str = "uscrt";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    let deposits = msg.deposits;
    let lock_periods = msg.lock_periods;

    if deposits.is_empty() || lock_periods.is_empty() {
        return Err(StdError::generic_err("Array is empty"));
    }

    if deposits.len() != lock_periods.len() {
        return Err(StdError::generic_err("Arrays have different length"));
    }

    let is_sorted = deposits.as_slice().windows(2).all(|v| v[0] > v[1]);
    if !is_sorted {
        return Err(StdError::generic_err(
            "Specify deposits in increasing order",
        ));
    }

    let owner = msg.owner.unwrap_or(env.message.sender);
    let initial_config = Config {
        owner: deps.api.canonical_address(&owner)?,
        validator: deps.api.canonical_address(&msg.validator)?,
        deposits,
        lock_periods,
    };

    config(&mut deps.storage).save(&initial_config)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    match msg {
        HandleMsg::Deposit => try_deposit(deps, env),
        HandleMsg::Withdraw => try_withdraw(deps, env),
        HandleMsg::Claim { recipient } => try_claim(deps, env, recipient),
        HandleMsg::WithdrawRewards { recipient } => try_withdraw_rewards(deps, env, recipient),
        HandleMsg::Redelegate { validator_address } => try_redelegate(deps, env, validator_address),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::TierOf { address } => query_tier_of(deps, address),
        QueryMsg::DepositOf { address } => query_deposit_of(deps, address),
        QueryMsg::WhenCanWithdraw { address } => query_when_can_withdraw(deps, address),
        QueryMsg::WhenCanClaim { address } => query_when_can_claim(deps, address),
    }
}

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let deposit = utils::get_deposit(&env)?;
    if deposit == 0 {
        return Err(StdError::generic_err("Deposit zero tokens"));
    }

    // todo: deposit after withdraw
    let sender = deps.api.canonical_address(&env.message.sender)?;
    let user_option = users_read(&deps.storage).may_load(sender.as_slice())?;
    let mut user_state = user_option.unwrap_or(User {
        state: UserState::Deposit,
        deposit_amount: Uint128(0),
        deposit_time: env.block.time,
        withdraw_time: None,
    });

    let deposit_amount = user_state.deposit_amount.u128();
    let new_deposit_amount = deposit_amount.checked_add(deposit).unwrap();
    user_state.deposit_amount = Uint128(new_deposit_amount);

    users(&mut deps.storage).save(sender.as_slice(), &user_state)?;

    let config_state = config_read(&deps.storage).load()?;
    let coin = Coin::new(deposit, USCRT);

    let delegate_msg = StakingMsg::Delegate {
        validator: deps.api.human_address(&config_state.validator)?,
        amount: coin,
    };

    let msg = CosmosMsg::Staking(delegate_msg);
    let status = to_binary(&HandleAnswer::Deposit {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(status),
        ..Default::default()
    })
}

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let sender = deps.api.canonical_address(&env.message.sender)?;
    let mut user = users_read(&deps.storage).load(sender.as_slice())?;
    if user.state == UserState::Withdraw {
        return Err(StdError::generic_err("You have already withdrawn tokens"));
    }

    let when_can_withdraw = query_when_can_withdraw(deps, env.message.sender)?;
    let withdraw_time = match from_binary(&when_can_withdraw)? {
        QueryAnswer::CanWithdraw { time } => Ok(time),
        _ => Err(StdError::generic_err("Query error")),
    }?;

    let current_time = env.block.time;
    if current_time < withdraw_time.unwrap() {
        return Err(StdError::generic_err("You cannot withdraw tokens yet"));
    }

    user.state = UserState::Withdraw;
    user.withdraw_time = Some(current_time);
    users(&mut deps.storage).save(sender.as_slice(), &user)?;

    let config_state = config_read(&deps.storage).load()?;
    let validator = deps.api.human_address(&config_state.validator)?;
    let coin = Coin::new(user.deposit_amount.u128(), USCRT);
    let withdraw_msg = StakingMsg::Undelegate {
        validator,
        amount: coin,
    };

    let msg = CosmosMsg::Staking(withdraw_msg);
    let status = to_binary(&HandleAnswer::Withdraw {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(status),
        ..Default::default()
    })
}

pub fn try_claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: Option<HumanAddr>,
) -> HandleResult {
    let sender = deps.api.canonical_address(&env.message.sender)?;
    let user = users_read(&deps.storage).load(sender.as_slice())?;
    if user.state != UserState::Withdraw {
        return Err(StdError::generic_err("You have not withdrawn your tokens"));
    }

    let withdraw_time = user.withdraw_time.unwrap();
    let claim_time = utils::claim_time(withdraw_time);
    let current_time = env.block.time;

    if current_time < claim_time {
        return Err(StdError::generic_err("Wait for tokens undelegation"));
    }

    let recipient = recipient.unwrap_or(env.message.sender);
    let coin = Coin::new(user.deposit_amount.into(), USCRT);
    let send_msg = BankMsg::Send {
        from_address: env.contract.address,
        to_address: recipient,
        amount: vec![coin],
    };

    users(&mut deps.storage).remove(sender.as_slice());

    let msg = CosmosMsg::Bank(send_msg);
    let status = to_binary(&HandleAnswer::Claim {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(status),
        ..Default::default()
    })
}

pub fn try_withdraw_rewards<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: Option<HumanAddr>,
) -> HandleResult {
    let config_state = config_read(&deps.storage).load()?;
    utils::assert_owner(&deps.api, &env, &config_state)?;

    let validator = deps.api.human_address(&config_state.validator)?;
    let delegation = utils::query_delegation(&deps.querier, &env, &validator)?;

    let can_withdraw = delegation.accumulated_rewards;
    if can_withdraw.amount.u128() == 0 {
        return Err(StdError::generic_err("There is nothing to withdraw"));
    }

    let owner = deps.api.human_address(&config_state.owner)?;
    let recipient = recipient.unwrap_or(owner);
    let withdraw_msg = StakingMsg::Withdraw {
        validator,
        recipient: Some(recipient),
    };

    let msg = CosmosMsg::Staking(withdraw_msg);
    let status = to_binary(&HandleAnswer::WithdrawRewards {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(status),
        ..Default::default()
    })
}

pub fn try_redelegate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    validator_address: HumanAddr,
) -> HandleResult {
    let config_state = config_read(&deps.storage).load()?;
    utils::assert_owner(&deps.api, &env, &config_state)?;

    let old_validator = deps.api.human_address(&config_state.validator)?;
    let delegation = utils::query_delegation(&deps.querier, &env, &old_validator)?;

    config(&mut deps.storage).update(|mut state| {
        state.validator = deps.api.canonical_address(&validator_address)?;
        Ok(state)
    })?;

    let coin = Coin::new(delegation.can_redelegate.amount.u128(), USCRT);
    let redelegate_msg = StakingMsg::Redelegate {
        src_validator: old_validator,
        dst_validator: validator_address,
        amount: coin,
    };

    let msg = CosmosMsg::Staking(redelegate_msg);
    let status = to_binary(&HandleAnswer::Redelegate {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(status),
        ..Default::default()
    })
}

pub fn query_tier_of<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let mut tier = 0;
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = users_read(&deps.storage).may_load(canonical_address.as_slice())?;
    if let Some(user) = user {
        let config = config_read(&deps.storage).load()?;
        for (index, deposit) in config.deposits.iter().enumerate() {
            if user.deposit_amount.u128() >= deposit.u128() {
                tier = index.checked_add(1).unwrap();
            }
        }
    }

    let answer = QueryAnswer::TierOf { tier: tier as u8 };
    to_binary(&answer)
}

pub fn query_deposit_of<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = users_read(&deps.storage).may_load(canonical_address.as_slice())?;

    let deposit = user.map(|u| u.deposit_amount).unwrap_or(Uint128(0));
    let answer = QueryAnswer::DepositOf { deposit };
    to_binary(&answer)
}

pub fn query_when_can_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let tier_binary = query_tier_of(deps, address.clone())?;
    let tier = match from_binary(&tier_binary)? {
        QueryAnswer::TierOf { tier } => Ok(tier),
        _ => Err(StdError::generic_err("Query error")),
    }?;

    if tier == 0 {
        let answer = QueryAnswer::CanWithdraw { time: None };
        return to_binary(&answer);
    }

    let tier_index = tier.checked_sub(1).unwrap();
    let config = config_read(&deps.storage).load()?;
    let months = config.lock_periods[tier_index as usize];

    let canonical_address = deps.api.canonical_address(&address)?;
    let user = users_read(&deps.storage).may_load(canonical_address.as_slice())?;
    let deposit_time = user.map(|u| u.deposit_time).unwrap_or(0);
    let withdraw_time = utils::withdraw_time(deposit_time, months);

    let answer = QueryAnswer::CanWithdraw {
        time: Some(withdraw_time),
    };

    to_binary(&answer)
}

pub fn query_when_can_claim<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = users_read(&deps.storage).may_load(canonical_address.as_slice())?;
    let claim_time = user.and_then(|u| u.withdraw_time).map(utils::claim_time);

    let answer = QueryAnswer::CanClaim { time: claim_time };
    to_binary(&answer)
}