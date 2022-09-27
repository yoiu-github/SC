use crate::{
    msg::{
        ContractStatus, HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg, ResponseStatus,
    },
    state::{self, Config, TierInfo, UserWithdrawal},
    utils,
};
use cosmwasm_std::{
    coin, coins, to_binary, Api, BankMsg, CosmosMsg, Env, Extern, HandleResponse, HandleResult,
    HumanAddr, InitResponse, InitResult, Querier, QueryResult, StakingMsg, StdError, Storage,
    Uint128,
};
use secret_toolkit_utils::{pad_handle_result, pad_query_result};

pub const BLOCK_SIZE: usize = 256;
pub const UNBOUND_LATENCY: u64 = 21 * 24 * 60 * 60;
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

    let admin = msg.admin.unwrap_or(env.message.sender);
    let initial_config = Config {
        status: ContractStatus::Active as u8,
        admin: deps.api.canonical_address(&admin)?,
        validator: msg.validator,
    };

    initial_config.save(&mut deps.storage)?;

    let length = deposits.len();
    let tier_list = state::tier_info_list();
    for i in 0..length {
        let tier = TierInfo {
            deposit: deposits[i].u128(),
            lock_period: lock_periods[i],
        };

        tier_list.push(&mut deps.storage, &tier)?;
    }

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    let response = match msg {
        HandleMsg::ChangeAdmin { admin, .. } => try_change_admin(deps, env, admin),
        HandleMsg::ChangeStatus { status, .. } => try_change_status(deps, env, status),
        HandleMsg::Deposit { .. } => try_deposit(deps, env),
        HandleMsg::Withdraw { .. } => try_withdraw(deps, env),
        HandleMsg::Claim {
            recipient,
            start,
            limit,
            ..
        } => try_claim(deps, env, recipient, start, limit),
        HandleMsg::WithdrawRewards { recipient, .. } => try_withdraw_rewards(deps, env, recipient),
        HandleMsg::Redelegate {
            validator_address,
            recipient,
            ..
        } => try_redelegate(deps, env, validator_address, recipient),
    };

    pad_handle_result(response, BLOCK_SIZE)
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    let response = match msg {
        QueryMsg::Config {} => query_config(deps),
        QueryMsg::UserInfo { address } => query_user_info(deps, address),
        QueryMsg::Withdrawals {
            address,
            start,
            limit,
        } => query_withdrawals(deps, address, start, limit),
    };

    pad_query_result(response, BLOCK_SIZE)
}

pub fn try_change_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    new_admin: HumanAddr,
) -> HandleResult {
    let mut config = Config::load(&deps.storage)?;
    utils::assert_admin(&deps.api, &env, &config)?;

    let canonical_admin = deps.api.canonical_address(&new_admin)?;
    config.admin = canonical_admin;
    config.save(&mut deps.storage)?;

    let answer = to_binary(&HandleAnswer::ChangeAdmin {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        data: Some(answer),
        ..Default::default()
    })
}

pub fn try_change_status<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    status: ContractStatus,
) -> HandleResult {
    let mut config = Config::load(&deps.storage)?;
    utils::assert_admin(&deps.api, &env, &config)?;

    config.status = status as u8;
    config.save(&mut deps.storage)?;

    let answer = to_binary(&HandleAnswer::ChangeStatus {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        data: Some(answer),
        ..Default::default()
    })
}

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let config = Config::load(&deps.storage)?;
    config.assert_contract_active()?;

    let deposit = utils::get_deposit(&env)?;
    if deposit == 0 {
        return Err(StdError::generic_err("Deposit zero tokens"));
    }

    let sender = deps.api.canonical_address(&env.message.sender)?;
    let user_infos = state::user_infos();
    let mut user_info = user_infos.get(&deps.storage, &sender).unwrap_or_default();

    let current_tier = user_info.tier;
    let user_old_deposit = user_info.deposit;
    let user_new_deposit = user_old_deposit.checked_add(deposit).unwrap();

    let new_tier = utils::get_tier_by_deposit(&deps.storage, user_new_deposit)?;
    let tier_list = state::tier_info_list();
    let max_tier = tier_list.get_len(&deps.storage)? as u8;

    if current_tier == new_tier {
        if current_tier == max_tier {
            return Err(StdError::generic_err("Reached max tear"));
        }

        let next_tier_index = current_tier;
        let next_tier_state = tier_list.get_at(&deps.storage, next_tier_index.into())?;

        let min_deposit = next_tier_state.deposit;
        let expected_deposit = min_deposit.checked_sub(user_old_deposit).unwrap();

        let err_msg = format!("You should deposit at least {} USCRT", expected_deposit);
        return Err(StdError::generic_err(&err_msg));
    }

    let mut messages = Vec::with_capacity(2);
    let new_tier_index = new_tier.checked_sub(1).unwrap();
    let new_tier_info = tier_list.get_at(&deps.storage, new_tier_index.into())?;

    let new_tier_deposit = new_tier_info.deposit;
    let refund = user_new_deposit.checked_sub(new_tier_deposit).unwrap();

    if refund != 0 {
        let send_msg = BankMsg::Send {
            from_address: env.contract.address,
            to_address: env.message.sender,
            amount: coins(refund, USCRT),
        };

        let msg = CosmosMsg::Bank(send_msg);
        messages.push(msg);
    }

    let lock_period = new_tier_info.lock_period;
    let withdraw_time = env.block.time.checked_add(lock_period).unwrap();

    user_info.deposit = new_tier_deposit;
    user_info.timestamp = env.block.time;
    user_info.tier = new_tier;
    user_info.withdraw_time = withdraw_time;
    user_infos.insert(&mut deps.storage, &sender, &user_info)?;

    let deposited = deposit.checked_sub(refund).unwrap();
    let delegate_msg = StakingMsg::Delegate {
        validator: config.validator,
        amount: coin(deposited, USCRT),
    };

    let msg = CosmosMsg::Staking(delegate_msg);
    messages.push(msg);

    let answer = to_binary(&HandleAnswer::Deposit {
        deposit: Uint128(user_info.deposit),
        tier: utils::normalize_tier(new_tier, max_tier),
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages,
        data: Some(answer),
        ..Default::default()
    })
}

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let config = Config::load(&deps.storage)?;
    config.assert_contract_active()?;

    let sender = deps.api.canonical_address(&env.message.sender)?;
    let user_infos = state::user_infos();
    let user_info = user_infos
        .get(&deps.storage, &sender)
        .ok_or_else(|| StdError::not_found("user"))?;

    let current_time = env.block.time;
    if current_time < user_info.withdraw_time {
        return Err(StdError::generic_err("You cannot withdraw tokens yet"));
    }

    let amount = user_info.deposit;
    user_infos.remove(&mut deps.storage, &sender)?;

    let claim_time = current_time.checked_add(UNBOUND_LATENCY).unwrap();
    let withdrawal = UserWithdrawal {
        amount,
        timestamp: current_time,
        claim_time,
    };

    let withdrawals = state::withdrawals_list(&sender);
    withdrawals.push_back(&mut deps.storage, &withdrawal)?;

    let config = Config::load(&deps.storage)?;
    let validator = config.validator;
    let amount = coin(amount, USCRT);

    let withdraw_msg = StakingMsg::Undelegate { validator, amount };
    let msg = CosmosMsg::Staking(withdraw_msg);

    let answer = to_binary(&HandleAnswer::Withdraw {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(answer),
        ..Default::default()
    })
}

pub fn try_claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: Option<HumanAddr>,
    start: Option<u32>,
    limit: Option<u32>,
) -> HandleResult {
    let config = Config::load(&deps.storage)?;
    config.assert_contract_active()?;

    let sender = deps.api.canonical_address(&env.message.sender)?;
    let withdrawals = state::withdrawals_list(&sender);
    let length = withdrawals.get_len(&deps.storage)?;

    if length == 0 {
        return Err(StdError::generic_err("Nothing to withdraw"));
    }

    let recipient = recipient.unwrap_or(env.message.sender);
    let start = start.unwrap_or(0) as usize;
    let limit = limit.unwrap_or(50) as usize;
    let withdrawals_iter = withdrawals.iter(&deps.storage)?.skip(start).take(limit);

    let current_time = env.block.time;
    let mut remove_indices = Vec::new();
    let mut withdraw_amount = 0u128;

    for (index, withdrawal) in withdrawals_iter.enumerate() {
        let withdrawal = withdrawal?;
        let claim_time = withdrawal.claim_time;

        if current_time >= claim_time {
            remove_indices.push(index);
            withdraw_amount = withdraw_amount.checked_add(withdrawal.amount).unwrap();
        }
    }

    if withdraw_amount == 0 {
        let answer = to_binary(&HandleAnswer::Claim {
            amount: Uint128(0),
            status: ResponseStatus::Success,
        })?;

        return Ok(HandleResponse {
            data: Some(answer),
            ..Default::default()
        });
    }

    for (shift, index) in remove_indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        withdrawals.remove(&mut deps.storage, position as u32)?;
    }

    let send_msg = BankMsg::Send {
        from_address: env.contract.address,
        to_address: recipient,
        amount: coins(withdraw_amount, USCRT),
    };

    let msg = CosmosMsg::Bank(send_msg);
    let answer = to_binary(&HandleAnswer::Claim {
        amount: withdraw_amount.into(),
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(answer),
        ..Default::default()
    })
}

pub fn try_withdraw_rewards<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: Option<HumanAddr>,
) -> HandleResult {
    let config = Config::load(&deps.storage)?;
    utils::assert_admin(&deps.api, &env, &config)?;

    let validator = config.validator;
    let delegation = utils::query_delegation(&deps.querier, &env, &validator)?;

    let can_withdraw = delegation
        .map(|d| d.accumulated_rewards.amount.u128())
        .unwrap_or(0);

    if can_withdraw == 0 {
        return Err(StdError::generic_err("There is nothing to withdraw"));
    }

    let admin = deps.api.human_address(&config.admin)?;
    let recipient = recipient.unwrap_or(admin);
    let withdraw_msg = StakingMsg::Withdraw {
        validator,
        recipient: Some(recipient),
    };

    let msg = CosmosMsg::Staking(withdraw_msg);
    let answer = to_binary(&HandleAnswer::WithdrawRewards {
        amount: Uint128(can_withdraw),
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![msg],
        data: Some(answer),
        ..Default::default()
    })
}

pub fn try_redelegate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    validator_address: HumanAddr,
    recipient: Option<HumanAddr>,
) -> HandleResult {
    let mut config = Config::load(&deps.storage)?;
    utils::assert_admin(&deps.api, &env, &config)?;

    let old_validator = config.validator;
    let delegation = utils::query_delegation(&deps.querier, &env, &old_validator)?;

    if old_validator == validator_address {
        return Err(StdError::generic_err("Redelegation to the same validator"));
    }

    if delegation.is_none() {
        config.validator = validator_address;
        config.save(&mut deps.storage)?;

        let answer = to_binary(&HandleAnswer::Redelegate {
            amount: Uint128(0),
            status: ResponseStatus::Success,
        })?;

        return Ok(HandleResponse {
            data: Some(answer),
            ..Default::default()
        });
    }

    let delegation = delegation.unwrap();
    let can_withdraw = delegation.accumulated_rewards.amount.u128();
    let can_redelegate = delegation.can_redelegate.amount.u128();
    let delegated_amount = delegation.amount.amount.u128();

    if can_redelegate != delegated_amount {
        return Err(StdError::generic_err(
            "Cannot redelegate full delegation amount",
        ));
    }

    config.validator = validator_address.clone();
    config.save(&mut deps.storage)?;

    let mut messages = Vec::with_capacity(2);
    if can_withdraw != 0 {
        let admin = deps.api.human_address(&config.admin)?;
        let recipient = recipient.unwrap_or(admin);
        let withdraw_msg = StakingMsg::Withdraw {
            validator: old_validator.clone(),
            recipient: Some(recipient),
        };

        messages.push(CosmosMsg::Staking(withdraw_msg));
    }

    let coin = coin(can_redelegate, USCRT);
    let redelegate_msg = StakingMsg::Redelegate {
        src_validator: old_validator,
        dst_validator: validator_address,
        amount: coin,
    };

    messages.push(CosmosMsg::Staking(redelegate_msg));
    let answer = to_binary(&HandleAnswer::Redelegate {
        amount: Uint128(can_redelegate),
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages,
        data: Some(answer),
        ..Default::default()
    })
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> QueryResult {
    let config = Config::load(&deps.storage)?;
    let tier_info = state::tier_info_list();
    let length = tier_info.get_len(&deps.storage)?;
    let tier_list = tier_info.paging(&deps.storage, 0, length)?;

    let serialized_tier_list = tier_list.into_iter().map(|t| t.to_serialized()).collect();
    let admin = deps.api.human_address(&config.admin)?;

    let answer = QueryAnswer::Config {
        admin,
        validator: config.validator,
        status: config.status.into(),
        tier_list: serialized_tier_list,
    };

    to_binary(&answer)
}

pub fn query_user_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let canonical_address = deps.api.canonical_address(&address)?;
    let user_infos = state::user_infos();
    let user_info = user_infos
        .get(&deps.storage, &canonical_address)
        .unwrap_or_default();

    let answer = user_info.to_answer(&deps.storage)?;
    to_binary(&answer)
}

pub fn query_withdrawals<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    start: Option<u32>,
    limit: Option<u32>,
) -> QueryResult {
    let canonical_address = deps.api.canonical_address(&address)?;
    let withdrawals = state::withdrawals_list(&canonical_address);
    let amount = withdrawals.get_len(&deps.storage)?;

    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(50);

    let withdrawals = withdrawals.paging(&deps.storage, start, limit)?;
    let serialized_withdrawals = withdrawals.into_iter().map(|w| w.to_serialized()).collect();

    let answer = QueryAnswer::Withdrawals {
        amount,
        withdrawals: serialized_withdrawals,
    };

    to_binary(&answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::UserInfo;
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env, MockApi, MockQuerier},
        FullDelegation, MemoryStorage, StdResult,
    };
    use rand::{thread_rng, Rng};
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
        env.block.time = current_time();

        init(&mut deps, env, init_msg).map(|_| deps)
    }

    fn init_with_default() -> Extern<MemoryStorage, MockApi, MockQuerier> {
        let admin = HumanAddr::from("admin");
        let validator = HumanAddr::from("validator");
        let deposits = vec![100u128, 750, 5000, 20000]
            .into_iter()
            .map(Into::into)
            .collect();

        let day = 24 * 60 * 60;
        let lock_periods = vec![2 * day, 5 * day, 14 * day, 31 * day];
        let init_msg = InitMsg {
            admin: Some(admin),
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
                StdError::Unauthorized { .. } => "Unauthorized".into(),
                StdError::NotFound { .. } => "Not found".into(),
                _ => panic!("Unexpected error"),
            },
        }
    }

    fn config_info<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> Config {
        let msg = QueryMsg::Config {};
        let response = query(deps, msg).unwrap();

        match from_binary(&response).unwrap() {
            QueryAnswer::Config {
                admin,
                validator,
                status,
                ..
            } => Config {
                admin: deps.api.canonical_address(&admin).unwrap(),
                validator,
                status: status as u8,
            },
            _ => unreachable!(),
        }
    }

    fn tier_info<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> Vec<TierInfo> {
        let msg = QueryMsg::Config {};
        let response = query(deps, msg).unwrap();

        match from_binary(&response).unwrap() {
            QueryAnswer::Config { tier_list, .. } => tier_list
                .into_iter()
                .map(|t| TierInfo {
                    deposit: t.deposit.u128(),
                    lock_period: t.lock_period,
                })
                .collect(),
            _ => unreachable!(),
        }
    }

    fn user_info<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
        address: HumanAddr,
    ) -> UserInfo {
        let msg = QueryMsg::UserInfo { address };
        let response = query(deps, msg).unwrap();

        match from_binary(&response).unwrap() {
            QueryAnswer::UserInfo {
                tier,
                deposit,
                withdraw_time,
                timestamp,
            } => UserInfo {
                tier,
                deposit: deposit.u128(),
                withdraw_time,
                timestamp,
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn initialization() {
        let admin = HumanAddr::from("admin");
        let validator = HumanAddr::from("secretvaloper1l92u46n0d33mhkknwm7zpg0twlqqxg826990re");

        let day = 24 * 60 * 60;
        let lock_periods = vec![2 * day, 5 * day, 14 * day, 31 * day];
        let deposits: Vec<Uint128> = vec![100u128, 750, 5000, 20000]
            .into_iter()
            .map(Into::into)
            .collect();

        // Wrong order
        let wrong_deposits = vec![750u128, 100, 5000, 20000]
            .into_iter()
            .map(Into::into)
            .collect();

        let init_msg = InitMsg {
            admin: Some(admin.clone()),
            validator: validator.clone(),
            deposits: wrong_deposits,
            lock_periods: lock_periods.clone(),
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Specify deposits in increasing order"));

        // Zero elements in deposits
        let init_msg = InitMsg {
            admin: Some(admin.clone()),
            validator: validator.clone(),
            deposits: vec![],
            lock_periods: lock_periods.clone(),
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Array is empty"));

        // Zero elements in lock periods
        let init_msg = InitMsg {
            admin: Some(admin.clone()),
            validator: validator.clone(),
            deposits: deposits.clone(),
            lock_periods: vec![],
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Array is empty"));

        // Elements amount mismatch
        let init_msg = InitMsg {
            admin: Some(admin.clone()),
            validator: validator.clone(),
            deposits: deposits[1..].to_owned(),
            lock_periods: lock_periods.clone(),
        };

        let response = init_contract(init_msg);
        let error = extract_error(response);
        assert!(error.contains("Arrays have different length"));

        // Init with sender
        let init_msg = InitMsg {
            admin: None,
            validator: validator.clone(),
            deposits: deposits.clone(),
            lock_periods: lock_periods.clone(),
        };

        let deps = init_contract(init_msg).unwrap();
        let config = Config::load(&deps.storage).unwrap();
        let canonical_admin = deps.api.canonical_address(&admin).unwrap();
        let tier_list = state::tier_info_list();
        let length = tier_list.get_len(&deps.storage).unwrap();

        assert_eq!(config.admin, canonical_admin);
        assert_eq!(config.validator, validator);
        assert_eq!(config, config_info(&deps));
        assert_eq!(length, deposits.len() as u32);

        for index in 0..length {
            let tier = tier_list.get_at(&deps.storage, index).unwrap();
            let expected_deposit = deposits[index as usize];
            let expected_lock_period = lock_periods[index as usize];
            assert_eq!(tier.deposit, expected_deposit.u128());
            assert_eq!(tier.lock_period, expected_lock_period);
        }

        // Init with custom admin
        let alice = HumanAddr::from("alice");
        let init_msg = InitMsg {
            admin: Some(alice.clone()),
            validator: validator.clone(),
            deposits,
            lock_periods,
        };

        let deps = init_contract(init_msg).unwrap();
        let config = Config::load(&deps.storage).unwrap();
        let canonical_alice = deps.api.canonical_address(&alice).unwrap();

        assert_eq!(config.admin, canonical_alice);
        assert_eq!(config.validator, validator);
        assert_eq!(config, config_info(&deps));
    }

    #[test]
    fn change_admin() {
        let mut deps = init_with_default();
        let admin = HumanAddr::from("admin");
        let alice = HumanAddr::from("alice");
        let new_admin = HumanAddr::from("new_admin");

        let env = mock_env(&alice, &[]);
        let change_admin_msg = HandleMsg::ChangeAdmin {
            admin: new_admin.clone(),
            padding: None,
        };

        let response = handle(&mut deps, env, change_admin_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let env = mock_env(&admin, &[]);
        handle(&mut deps, env, change_admin_msg).unwrap();

        let config = Config::load(&deps.storage).unwrap();
        let new_admin_canonical = deps.api.canonical_address(&new_admin).unwrap();
        assert_eq!(config.admin, new_admin_canonical);
    }

    #[test]
    fn change_status() {
        let mut deps = init_with_default();
        let admin = HumanAddr::from("admin");
        let alice = HumanAddr::from("alice");

        let env = mock_env(&alice, &[]);
        let change_admin_msg = HandleMsg::ChangeStatus {
            status: ContractStatus::Stopped,
            padding: None,
        };

        let response = handle(&mut deps, env, change_admin_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let env = mock_env(&admin, &[]);
        handle(&mut deps, env.clone(), change_admin_msg).unwrap();

        let config = Config::load(&deps.storage).unwrap();
        assert_eq!(config.status, ContractStatus::Stopped as u8);

        let change_admin_msg = HandleMsg::ChangeStatus {
            status: ContractStatus::Active,
            padding: None,
        };

        handle(&mut deps, env, change_admin_msg).unwrap();
        let config = Config::load(&deps.storage).unwrap();
        assert_eq!(config.status, ContractStatus::Active as u8);
    }

    #[test]
    fn deposit() {
        let mut deps = init_with_default();
        let alice = HumanAddr::from("alice");

        let alice_info = user_info(&deps, alice.clone());
        assert_eq!(alice_info, UserInfo::default());

        let mut env = mock_env(alice.clone(), &[]);
        env.block.time = current_time();
        env.message.sent_funds = coins(99, USCRT);

        let deposit_msg = HandleMsg::Deposit { padding: None };
        let response = handle(&mut deps, env.clone(), deposit_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You should deposit at least 100 USCRT"));

        env.message.sent_funds = coins(100, "ust");
        let response = handle(&mut deps, env.clone(), deposit_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unsopported token"));

        env.message.sent_funds = coins(100, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();

        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Deposit {
                deposit,
                tier,
                status,
            } => {
                assert_eq!(deposit.u128(), 100);
                assert_eq!(tier, 4);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        assert_eq!(response.messages.len(), 1);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Staking(StakingMsg::Delegate {
                validator: HumanAddr::from("validator"),
                amount: coin(100, USCRT)
            })
        );

        let tier_info = tier_info(&deps);
        let alice_info = user_info(&deps, alice.clone());

        assert_eq!(alice_info.deposit, 100);
        assert_eq!(alice_info.tier, 4);
        assert_eq!(alice_info.timestamp, env.block.time);
        assert_eq!(
            alice_info.withdraw_time,
            env.block.time + tier_info[0].lock_period
        );

        env.block.time += 100;
        env.message.sent_funds = coins(649, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You should deposit at least 650 USCRT"));

        env.message.sent_funds = coins(5000, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();

        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Deposit {
                deposit,
                tier,
                status,
            } => {
                assert_eq!(deposit.u128(), 5000);
                assert_eq!(tier, 2);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        assert_eq!(response.messages.len(), 2);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address.clone(),
                to_address: alice.clone(),
                amount: coins(100, USCRT),
            })
        );
        assert_eq!(
            response.messages[1],
            CosmosMsg::Staking(StakingMsg::Delegate {
                validator: HumanAddr::from("validator"),
                amount: coin(4900, USCRT)
            })
        );

        let alice_info = user_info(&deps, alice.clone());
        assert_eq!(alice_info.deposit, 5000);
        assert_eq!(alice_info.tier, 2);
        assert_eq!(alice_info.timestamp, env.block.time);
        assert_eq!(
            alice_info.withdraw_time,
            env.block.time + tier_info[2].lock_period
        );

        env.block.time += 100;
        env.message.sent_funds = coins(10000, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You should deposit at least 15000 USCRT"));

        env.message.sent_funds = coins(50000, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();

        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Deposit {
                deposit,
                tier,
                status,
            } => {
                assert_eq!(deposit.u128(), 20000);
                assert_eq!(tier, 1);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        assert_eq!(response.messages.len(), 2);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address.clone(),
                to_address: alice.clone(),
                amount: coins(35000, USCRT),
            })
        );
        assert_eq!(
            response.messages[1],
            CosmosMsg::Staking(StakingMsg::Delegate {
                validator: HumanAddr::from("validator"),
                amount: coin(15000, USCRT)
            })
        );

        let alice_info = user_info(&deps, alice);
        assert_eq!(alice_info.deposit, 20000);
        assert_eq!(alice_info.tier, 1);
        assert_eq!(alice_info.timestamp, env.block.time);
        assert_eq!(
            alice_info.withdraw_time,
            env.block.time + tier_info[3].lock_period
        );

        let response = handle(&mut deps, env, deposit_msg);
        let error = extract_error(response);
        assert!(error.contains("Reached max tear"));
    }

    #[test]
    fn withdraw() {
        let mut deps = init_with_default();
        let alice = HumanAddr::from("alice");

        let mut env = mock_env(alice.clone(), &[]);
        env.block.time = current_time();
        env.message.sent_funds = coins(750, USCRT);

        let deposit_msg = HandleMsg::Deposit { padding: None };
        let withdraw_msg = HandleMsg::Withdraw { padding: None };

        handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();

        let day = 24 * 60 * 60;
        let alice_info = user_info(&deps, alice.clone());
        assert_eq!(alice_info.tier, 3);
        assert_eq!(alice_info.deposit, 750);
        assert_eq!(alice_info.timestamp, env.block.time);
        assert_eq!(alice_info.withdraw_time, env.block.time + 5 * day);

        env.message.sent_funds = Vec::new();

        // Try to withdraw tokens without waiting for locking period
        let response = handle(&mut deps, env.clone(), withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You cannot withdraw tokens yet"));

        // Deposit some tokens. It will reset deposit time
        env.block.time += 365 * day;
        env.message.sent_funds = coins(4250, USCRT);

        handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();
        let alice_info = user_info(&deps, alice.clone());
        assert_eq!(alice_info.tier, 2);
        assert_eq!(alice_info.deposit, 5000);
        assert_eq!(alice_info.timestamp, env.block.time);
        assert_eq!(alice_info.withdraw_time, env.block.time + 14 * day);

        // Try to withdraw tokens after deposit
        env.message.sent_funds = Vec::new();
        let response = handle(&mut deps, env.clone(), withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You cannot withdraw tokens yet"));

        // Withdraw tokens successfully
        env.block.time += 14 * day;
        handle(&mut deps, env.clone(), withdraw_msg.clone()).unwrap();
        let alice_info = user_info(&deps, alice.clone());

        assert_eq!(alice_info, UserInfo::default());

        // Withdraw tokens twice
        let response = handle(&mut deps, env.clone(), withdraw_msg);
        let error = extract_error(response);
        assert!(error.contains("Not found"));

        // Deposit tokens during withdrawal
        env.message.sent_funds = coins(25000, USCRT);
        handle(&mut deps, env.clone(), deposit_msg).unwrap();

        let alice_info = user_info(&deps, alice);
        assert_eq!(alice_info.tier, 1);
        assert_eq!(alice_info.deposit, 20000);
        assert_eq!(alice_info.timestamp, env.block.time);
        assert_eq!(alice_info.withdraw_time, env.block.time + 31 * day);
    }

    #[test]
    fn claim() {
        let mut deps = init_with_default();
        let alice = HumanAddr::from("alice");
        let alice_canonical = deps.api.canonical_address(&alice).unwrap();

        let mut env = mock_env(alice.clone(), &[]);
        env.block.time = current_time();

        let deposit_msg = HandleMsg::Deposit { padding: None };
        let withdraw_msg = HandleMsg::Withdraw { padding: None };
        let claim_msg = HandleMsg::Claim {
            start: None,
            limit: None,
            recipient: None,
            padding: None,
        };

        // Deposit some tokens
        let deposit = 750;
        env.message.sent_funds = coins(deposit, USCRT);
        handle(&mut deps, env.clone(), deposit_msg).unwrap();
        env.message.sent_funds = Vec::new();

        let day = 24 * 60 * 60;
        env.block.time += 5 * day;
        handle(&mut deps, env.clone(), withdraw_msg).unwrap();

        let withdrawals = state::withdrawals_list(&alice_canonical);
        assert_eq!(withdrawals.get_len(&deps.storage), Ok(1));

        let withdrawal = withdrawals.get_at(&deps.storage, 0).unwrap();
        assert_eq!(env.block.time + 21 * day, withdrawal.claim_time);

        // Try to claim without waiting for unbond period
        let response = handle(&mut deps, env.clone(), claim_msg.clone()).unwrap();
        assert_eq!(response.messages.len(), 0);

        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Claim { amount, .. } => {
                assert_eq!(amount, Uint128(0))
            }
            _ => unreachable!(),
        }

        env.block.time += 21 * day;
        let response = handle(&mut deps, env.clone(), claim_msg).unwrap();
        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Claim { amount, .. } => {
                assert_eq!(amount, Uint128(deposit))
            }
            _ => unreachable!(),
        }

        assert_eq!(response.messages.len(), 1);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address,
                to_address: alice,
                amount: coins(deposit, USCRT)
            })
        );
    }

    #[test]
    fn claim_multiple_withdrawals() {
        let mut deps = init_with_default();
        let alice = HumanAddr::from("alice");
        let alice_canonical = deps.api.canonical_address(&alice).unwrap();

        let withdrawals = state::withdrawals_list(&alice_canonical);

        let amount = 100;
        let claim_before = 500;
        let mut rng = thread_rng();

        let mut claim_amount = 0;
        let mut total_amount = 0;
        for _ in 0..amount {
            let amount = rng.gen_range(0..1000);
            let claim_time = rng.gen_range(0..1000);

            total_amount += amount;
            if claim_time <= claim_before {
                claim_amount += amount;
            }

            let withdrawal = UserWithdrawal {
                amount,
                claim_time,
                timestamp: 0,
            };

            withdrawals
                .push_back(&mut deps.storage, &withdrawal)
                .unwrap();
        }

        let mut env = mock_env(alice, &[]);
        env.block.time = claim_before;

        let claim_msg = HandleMsg::Claim {
            start: None,
            limit: Some(amount),
            recipient: None,
            padding: None,
        };

        let response = handle(&mut deps, env.clone(), claim_msg.clone()).unwrap();
        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Claim { amount, status } => {
                assert_eq!(amount.u128(), claim_amount);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        env.block.time = 1000;
        let response = handle(&mut deps, env, claim_msg).unwrap();
        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Claim { amount, status } => {
                assert_eq!(amount.u128(), total_amount - claim_amount);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        let withdrawals = state::withdrawals_list(&alice_canonical);
        assert_eq!(withdrawals.get_len(&deps.storage), Ok(0));
    }

    #[test]
    fn redelegate() {
        let mut deps = init_with_default();
        let admin = HumanAddr::from("admin");
        let alice = HumanAddr::from("alice");
        let validator = HumanAddr::from("validator");
        let new_validator = HumanAddr::from("new_validator");

        let mut env = mock_env(alice, &[]);
        env.block.time = current_time();

        let redelegate_msg = HandleMsg::Redelegate {
            validator_address: new_validator.clone(),
            recipient: None,
            padding: None,
        };

        let redelegate_back_msg = HandleMsg::Redelegate {
            validator_address: validator.clone(),
            recipient: None,
            padding: None,
        };

        // Alice calls redelegate
        let response = handle(&mut deps, env.clone(), redelegate_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let delegated_amount = 1000000;
        let accumulated_rewards = 10000;

        // Redelegate without deposit
        env.message.sender = admin.clone();
        let response = handle(&mut deps, env.clone(), redelegate_msg.clone()).unwrap();
        assert!(response.messages.is_empty());
        let config = Config::load(&deps.storage).unwrap();
        assert_eq!(config.validator, new_validator);

        let response = handle(&mut deps, env.clone(), redelegate_back_msg.clone()).unwrap();
        assert!(response.messages.is_empty());
        let config = Config::load(&deps.storage).unwrap();
        assert_eq!(config.validator, validator);

        // Redelegate to itself
        let response = handle(&mut deps, env.clone(), redelegate_back_msg);
        let error = extract_error(response);
        assert!(error.contains("Redelegation to the same validator"));

        // Can redelegate = 0
        let mut delegation = FullDelegation {
            delegator: env.contract.address.clone(),
            validator: validator.clone(),
            amount: coin(delegated_amount, USCRT),
            accumulated_rewards: coin(accumulated_rewards, USCRT),
            can_redelegate: coin(0, USCRT),
        };

        deps.querier
            .update_staking(USCRT, &[], &[delegation.clone()]);

        let response = handle(&mut deps, env.clone(), redelegate_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Cannot redelegate full delegation amount"));

        // Can redelegate != delegated_amount
        delegation.can_redelegate = coin(500, USCRT);
        deps.querier
            .update_staking(USCRT, &[], &[delegation.clone()]);

        let response = handle(&mut deps, env.clone(), redelegate_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Cannot redelegate full delegation amount"));

        // Can redelegate full amount
        delegation.can_redelegate = coin(delegated_amount, USCRT);
        deps.querier.update_staking(USCRT, &[], &[delegation]);

        let response = handle(&mut deps, env, redelegate_msg).unwrap();
        assert_eq!(response.messages.len(), 2);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Staking(StakingMsg::Withdraw {
                validator: validator.clone(),
                recipient: Some(admin)
            })
        );
        assert_eq!(
            response.messages[1],
            CosmosMsg::Staking(StakingMsg::Redelegate {
                src_validator: validator,
                dst_validator: new_validator,
                amount: coin(delegated_amount, USCRT),
            })
        );
    }

    #[test]
    fn withdraw_rewards() {
        let mut deps = init_with_default();
        let admin = HumanAddr::from("admin");
        let alice = HumanAddr::from("alice");
        let validator = HumanAddr::from("validator");

        let mut env = mock_env(admin.clone(), &[]);
        env.block.time = current_time();

        let withdraw_rewards_msg = HandleMsg::WithdrawRewards {
            recipient: None,
            padding: None,
        };

        // Nothing to withdraw
        let response = handle(&mut deps, env.clone(), withdraw_rewards_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("There is nothing to withdraw"));

        let mut delegation = FullDelegation {
            delegator: env.contract.address.clone(),
            validator: validator.clone(),
            amount: coin(0, USCRT),
            accumulated_rewards: coin(0, USCRT),
            can_redelegate: coin(0, USCRT),
        };

        deps.querier
            .update_staking(USCRT, &[], &[delegation.clone()]);

        // Alice tries to withdraw
        env.message.sender = alice;
        let response = handle(&mut deps, env.clone(), withdraw_rewards_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        // Nothing to withdraw
        env.message.sender = admin.clone();
        let response = handle(&mut deps, env.clone(), withdraw_rewards_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("There is nothing to withdraw"));

        delegation.accumulated_rewards = coin(1, USCRT);
        deps.querier.update_staking(USCRT, &[], &[delegation]);

        let response = handle(&mut deps, env, withdraw_rewards_msg).unwrap();
        assert_eq!(response.messages.len(), 1);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Staking(StakingMsg::Withdraw {
                validator,
                recipient: Some(admin)
            })
        );
    }
}
