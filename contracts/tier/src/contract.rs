use crate::{
    msg::{HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg, ResponseStatus},
    state::{Config, Tier, User, UserState},
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

    let is_sorted = deposits.as_slice().windows(2).all(|v| v[0] < v[1]);
    if !is_sorted {
        return Err(StdError::generic_err(
            "Specify deposits in increasing order",
        ));
    }

    let owner = msg.owner.unwrap_or(env.message.sender);
    let initial_config = Config {
        owner: deps.api.canonical_address(&owner)?,
        validator: deps.api.canonical_address(&msg.validator)?,
    };
    initial_config.save(&mut deps.storage)?;

    let length = deposits.len().try_into().unwrap();
    Tier::set_len(&mut deps.storage, length)?;

    for i in 0..length {
        let tier_state = Tier {
            index: i,
            deposit: deposits[i as usize],
            lock_period: lock_periods[i as usize],
        };

        tier_state.save(&mut deps.storage)?;
    }

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

    let sender = deps.api.canonical_address(&env.message.sender)?;
    let user_option = User::may_load(&deps.storage, &sender)?;

    if let Some(ref user) = user_option {
        if user.state != UserState::Deposit {
            return Err(StdError::generic_err("Claim your tokens first"));
        }
    }

    let mut user_state = user_option.unwrap_or(User {
        state: UserState::Deposit,
        deposit_amount: Uint128(0),
        deposit_time: env.block.time,
        withdraw_time: None,
        address: sender,
    });

    let deposit_amount = user_state.deposit_amount.u128();
    let new_deposit_amount = deposit_amount.checked_add(deposit).unwrap();
    user_state.deposit_amount = Uint128(new_deposit_amount);
    user_state.deposit_time = env.block.time;
    user_state.save(&mut deps.storage)?;

    let config_state = Config::load(&deps.storage)?;
    let validator = deps.api.human_address(&config_state.validator)?;
    let amount = Coin::new(deposit, USCRT);

    let delegate_msg = StakingMsg::Delegate { validator, amount };
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
    let mut user = User::load(&deps.storage, &sender)?;

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
    user.save(&mut deps.storage)?;

    let config_state = Config::load(&deps.storage)?;
    let validator = deps.api.human_address(&config_state.validator)?;
    let amount = Coin::new(user.deposit_amount.u128(), USCRT);

    let withdraw_msg = StakingMsg::Undelegate { validator, amount };
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
    let user = User::load(&deps.storage, &sender)?;

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

    user.remove(&mut deps.storage);

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
    let config_state = Config::load(&deps.storage)?;
    utils::assert_owner(&deps.api, &env, &config_state)?;

    let validator = deps.api.human_address(&config_state.validator)?;
    let delegation = utils::query_delegation(&deps.querier, &env, &validator)?;

    let can_withdraw = delegation.accumulated_rewards.amount.u128();
    if can_withdraw == 0 {
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
    let mut config_state = Config::load(&deps.storage)?;
    utils::assert_owner(&deps.api, &env, &config_state)?;

    let old_validator = deps.api.human_address(&config_state.validator)?;
    let delegation = utils::query_delegation(&deps.querier, &env, &old_validator)?;

    let can_redelegate = delegation.can_redelegate.amount.u128();
    let delegated_amount = delegation.amount.amount.u128();

    if can_redelegate != delegated_amount {
        return Err(StdError::generic_err(
            "Cannot redelegate full delegation amount",
        ));
    }

    config_state.validator = deps.api.canonical_address(&validator_address)?;
    config_state.save(&mut deps.storage)?;

    let coin = Coin::new(can_redelegate, USCRT);
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
    let mut tier = 0u8;
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = User::may_load(&deps.storage, &canonical_address)?;

    if let Some(user) = user {
        let user_deposit = user.deposit_amount.u128();
        let tier_len = Tier::len(&deps.storage)?;

        for i in 0..tier_len {
            let tier_state = Tier::load(&deps.storage, i)?;
            if user_deposit < tier_state.deposit.u128() {
                break;
            } else {
                tier = tier.checked_add(1).unwrap();
            }
        }
    }

    let answer = QueryAnswer::TierOf { tier };
    to_binary(&answer)
}

pub fn query_deposit_of<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = User::may_load(&deps.storage, &canonical_address)?;

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
    let tier_state = Tier::load(&deps.storage, tier_index)?;
    let months = tier_state.lock_period;

    let canonical_address = deps.api.canonical_address(&address)?;
    let user = User::may_load(&deps.storage, &canonical_address)?;
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
    let user = User::may_load(&deps.storage, &canonical_address)?;
    let claim_time = user.and_then(|u| u.withdraw_time).map(utils::claim_time);

    let answer = QueryAnswer::CanClaim { time: claim_time };
    to_binary(&answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        coins,
        testing::{mock_dependencies, mock_env, MockApi, MockQuerier},
        Decimal, MemoryStorage, StdResult, Validator,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn current_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn init_contract(
        init_msg: InitMsg,
    ) -> Result<Extern<MemoryStorage, MockApi, MockQuerier>, StdError> {
        let balance = coins(1000, USCRT);
        let mut deps = mock_dependencies(20, &[]);
        let mut env = mock_env("admin", &balance);

        let validators = vec![
            Validator {
                address: HumanAddr::from("validator"),
                commission: Decimal::percent(1),
                max_commission: Decimal::percent(10),
                max_change_rate: Decimal::percent(15),
            },
            Validator {
                address: HumanAddr::from("validator1"),
                commission: Decimal::percent(5),
                max_commission: Decimal::percent(30),
                max_change_rate: Decimal::percent(2),
            },
            Validator {
                address: HumanAddr::from("validator2"),
                commission: Decimal::percent(10),
                max_commission: Decimal::percent(50),
                max_change_rate: Decimal::percent(4),
            },
        ];

        deps.querier.update_staking(USCRT, &validators, &Vec::new());
        env.block.time = current_time();

        init(&mut deps, env, init_msg).map(|_| deps)
    }

    fn init_with_default() -> Extern<MemoryStorage, MockApi, MockQuerier> {
        let owner = HumanAddr::from("admin");
        let validator = HumanAddr::from("validator");
        let deposits = vec![100u128, 750, 5000, 20000]
            .into_iter()
            .map(Into::into)
            .collect();

        let lock_periods = vec![1, 3, 6, 12];
        let init_msg = InitMsg {
            owner: Some(owner),
            validator,
            deposits,
            lock_periods,
        };

        init_contract(init_msg).unwrap()
    }

    fn extract_error<T>(response: StdResult<T>) -> String {
        match response {
            Ok(_) => panic!("Response is not an error"),
            Err(err) => match err {
                StdError::GenericErr { msg, .. } => msg,
                _ => panic!("Unexpected error"),
            },
        }
    }

    #[test]
    fn initialization() {
        let owner = HumanAddr::from("admin");
        let validator = HumanAddr::from("validator");

        let lock_periods = vec![1, 3, 6, 12];
        let deposits: Vec<Uint128> = vec![100u128, 750, 5000, 20000]
            .into_iter()
            .map(Into::into)
            .collect();

        // Wrong order
        let wrong_deposits = vec![750u128, 100, 5000, 20000]
            .into_iter()
            .map(Into::into)
            .collect();
        let wrong_lock_periods = vec![3, 1, 6, 12];

        let init_msg = InitMsg {
            owner: Some(owner.clone()),
            validator: validator.clone(),
            deposits: wrong_deposits,
            lock_periods: wrong_lock_periods,
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Specify deposits in increasing order"));

        // Zero elements in deposits
        let init_msg = InitMsg {
            owner: Some(owner.clone()),
            validator: validator.clone(),
            deposits: vec![],
            lock_periods: lock_periods.clone(),
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Array is empty"));

        // Zero elements in lock periods
        let init_msg = InitMsg {
            owner: Some(owner.clone()),
            validator: validator.clone(),
            deposits: deposits.clone(),
            lock_periods: vec![],
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Array is empty"));

        // Elements amount mismatch
        let init_msg = InitMsg {
            owner: Some(owner.clone()),
            validator: validator.clone(),
            deposits: deposits[1..].to_owned(),
            lock_periods: lock_periods.clone(),
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Arrays have different length"));

        // Init with sender
        let init_msg = InitMsg {
            owner: None,
            validator: validator.clone(),
            deposits: deposits.clone(),
            lock_periods: lock_periods.clone(),
        };

        let deps = init_contract(init_msg).unwrap();
        let config = Config::load(&deps.storage).unwrap();
        let canonical_owner = deps.api.canonical_address(&owner).unwrap();
        let canonical_validator = deps.api.canonical_address(&validator).unwrap();
        let length = Tier::len(&deps.storage).unwrap();

        assert_eq!(config.owner, canonical_owner);
        assert_eq!(config.validator, canonical_validator);
        assert_eq!(length, deposits.len() as u8);

        for index in 0..length {
            let tier_state = Tier::load(&deps.storage, index).unwrap();
            let expected_deposit = deposits[index as usize];
            let expected_lock_period = lock_periods[index as usize];
            assert_eq!(tier_state.deposit, expected_deposit);
            assert_eq!(tier_state.lock_period, expected_lock_period);
        }

        // Init with custom owner
        let alice = HumanAddr::from("alice");
        let init_msg = InitMsg {
            owner: Some(alice.clone()),
            validator: validator.clone(),
            deposits,
            lock_periods,
        };

        let deps = init_contract(init_msg).unwrap();
        let config = Config::load(&deps.storage).unwrap();
        let canonical_alice = deps.api.canonical_address(&alice).unwrap();
        let canonical_validator = deps.api.canonical_address(&validator).unwrap();

        assert_eq!(config.owner, canonical_alice);
        assert_eq!(config.validator, canonical_validator);
    }

    #[test]
    fn deposit() {
        let alice = HumanAddr::from("alice");
        let amount = 30;
        let sent = coins(amount, USCRT);

        let mut deps = init_with_default();
        let mut env = mock_env(alice.clone(), &sent);
        env.block.time = current_time();

        let msg = HandleMsg::Deposit;
        handle(&mut deps, env.clone(), msg).unwrap();

        let alice_canonical = deps.api.canonical_address(&alice).unwrap();
        let user = User::load(&deps.storage, &alice_canonical).unwrap();

        assert_eq!(user.state, UserState::Deposit);
        assert_eq!(user.deposit_amount.u128(), amount);
        assert_eq!(user.deposit_time, env.block.time);
        assert_eq!(user.withdraw_time, None);
    }
}
