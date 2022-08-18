use crate::{
    msg::{HandleMsg, InitMsg, QueryMsg},
    state::{config, config_read, users, users_read, Config, User},
    utils,
};
use cosmwasm_std::{
    to_vec, Api, Coin, CosmosMsg, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, InitResult, Querier, QueryResult, StakingMsg, StdError, Storage, Uint128,
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

    let owner = msg.owner.unwrap_or(env.message.sender);
    let initial_state = Config {
        owner: deps.api.canonical_address(&owner)?,
        validator: deps.api.canonical_address(&msg.validator)?,
        deposits,
        lock_periods,
    };

    config(&mut deps.storage).save(&initial_state)?;

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
        HandleMsg::Claim => try_claim(deps, env),
        HandleMsg::WithdrawRewards => try_withdraw_rewards(deps, env),
        HandleMsg::Redelegate { validator_address } => try_redelegate(deps, env, validator_address),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> QueryResult {
    match msg {
        QueryMsg::TierOf { address: _ } => todo!(),
        QueryMsg::TierInfo { tier: _ } => todo!(),
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
    let sender_bytes = to_vec(&sender)?;

    let user_option = users_read(&deps.storage).may_load(&sender_bytes)?;
    let mut user_state = user_option.unwrap_or(User {
        deposit_amount: Uint128(0),
        deposit_time: env.block.time,
    });

    let deposit_amount = user_state.deposit_amount.0;
    let new_deposit_amount = deposit_amount.checked_add(deposit).unwrap();
    user_state.deposit_amount = Uint128(new_deposit_amount);

    users(&mut deps.storage).save(&sender_bytes, &user_state)?;

    let config_state = config_read(&deps.storage).load()?;
    let coin = Coin::new(deposit, USCRT);

    let delegate_msg = StakingMsg::Delegate {
        validator: deps.api.human_address(&config_state.validator)?,
        amount: coin,
    };

    let stake_msg = CosmosMsg::Staking(delegate_msg);

    Ok(HandleResponse {
        messages: vec![stake_msg],
        ..Default::default()
    })
}

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
) -> HandleResult {
    Ok(HandleResponse::default())
}

pub fn try_claim<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
) -> HandleResult {
    Ok(HandleResponse::default())
}

pub fn try_withdraw_rewards<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
) -> HandleResult {
    Ok(HandleResponse::default())
}

pub fn try_redelegate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    validator_address: HumanAddr,
) -> HandleResult {
    let config_state = config_read(&deps.storage).load()?;
    utils::assert_owner(&deps.api, &env, &config_state)?;

    let old_validator = deps.api.human_address(&config_state.validator)?;
    let query = deps
        .querier
        .query_delegation(env.contract.address, old_validator.clone())?;

    if query.is_none() {
        return Err(StdError::generic_err("Cannot query delegation"));
    }

    let query = query.unwrap();
    let coin = Coin::new(query.can_redelegate.amount.0, USCRT);
    let redelegate_msg = StakingMsg::Redelegate {
        src_validator: old_validator,
        dst_validator: validator_address.clone(),
        amount: coin,
    };

    config(&mut deps.storage).update(|mut state| {
        state.validator = deps.api.canonical_address(&validator_address)?;
        Ok(state)
    })?;

    let messages = vec![CosmosMsg::Staking(redelegate_msg)];
    Ok(HandleResponse {
        messages,
        ..Default::default()
    })
}
