use crate::{
    msg::{HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg, ResponseStatus},
    state::{Config, Tier, User, UserState},
    utils,
};
use cosmwasm_std::{
    coins, to_binary, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, HandleResult,
    HumanAddr, InitResponse, InitResult, Querier, QueryResult, StakingMsg, StdError, Storage,
    Uint128,
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
        validator: msg.validator,
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
        HandleMsg::Deposit { .. } => try_deposit(deps, env),
        HandleMsg::Withdraw { .. } => try_withdraw(deps, env),
        HandleMsg::Claim { recipient, .. } => try_claim(deps, env, recipient),
        HandleMsg::WithdrawRewards { recipient, .. } => try_withdraw_rewards(deps, env, recipient),
        HandleMsg::Redelegate {
            validator_address,
            recipient,
            ..
        } => try_redelegate(deps, env, validator_address, recipient),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::TierInfo {} => query_tier_info(deps),
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

    let mut current_tier = 0;
    if let Some(ref user) = user_option {
        if user.state != UserState::Deposit {
            return Err(StdError::generic_err("Claim your tokens first"));
        }

        current_tier = user.tier(&deps.storage)?;
    }

    let mut user_state = user_option.unwrap_or(User {
        state: UserState::Deposit,
        deposit_amount: Uint128(0),
        deposit_time: env.block.time,
        withdraw_time: None,
        address: sender,
    });

    let user_old_deposit = user_state.deposit_amount.u128();
    user_state.deposit_amount = Uint128(user_old_deposit.checked_add(deposit).unwrap());

    let new_tier = user_state.tier(&deps.storage)?;
    if current_tier == new_tier {
        let max_tier = Tier::len(&deps.storage)?;
        if current_tier == max_tier {
            return Err(StdError::generic_err("Reached max tear"));
        }

        let next_tier_index = current_tier;
        let next_tier_state = Tier::load(&deps.storage, next_tier_index)?;

        let min_deposit = next_tier_state.deposit.u128();
        let expected_deposit = min_deposit.checked_sub(user_old_deposit).unwrap();

        let err_msg = format!("You should deposit at least {} USCRT", expected_deposit);
        return Err(StdError::generic_err(&err_msg));
    }

    let mut messages = Vec::with_capacity(2);
    let new_tier_index = new_tier.checked_sub(1).unwrap();
    let new_tier_state = Tier::load(&deps.storage, new_tier_index)?;

    let tier_deposit = new_tier_state.deposit.u128();
    let user_deposit = user_state.deposit_amount.u128();

    let refund = user_deposit.checked_sub(tier_deposit).unwrap();
    if refund != 0 {
        let amount = coins(refund, USCRT);
        let send_msg = BankMsg::Send {
            from_address: env.contract.address,
            to_address: env.message.sender,
            amount,
        };

        let msg = CosmosMsg::Bank(send_msg);
        messages.push(msg);
    }

    user_state.deposit_amount = new_tier_state.deposit;
    user_state.deposit_time = env.block.time;
    user_state.save(&mut deps.storage)?;

    let config_state = Config::load(&deps.storage)?;
    let validator = config_state.validator;

    let deposited = deposit.checked_sub(refund).unwrap();
    let amount = Coin::new(deposited, USCRT);

    let delegate_msg = StakingMsg::Delegate { validator, amount };
    let msg = CosmosMsg::Staking(delegate_msg);
    messages.push(msg);

    let status = to_binary(&HandleAnswer::Deposit {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages,
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

    let when_can_withdraw = user.can_withdraw_at(&deps.storage)?;
    let current_time = env.block.time;
    if current_time < when_can_withdraw {
        return Err(StdError::generic_err("You cannot withdraw tokens yet"));
    }

    user.state = UserState::Withdraw;
    user.withdraw_time = Some(current_time);
    user.save(&mut deps.storage)?;

    let config_state = Config::load(&deps.storage)?;
    let validator = config_state.validator;
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

    let claim_time = user.can_claim_at().unwrap();
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

    let validator = config_state.validator;
    let delegation = utils::query_delegation(&deps.querier, &env, &validator)?;

    let can_withdraw = delegation
        .map(|d| d.accumulated_rewards.amount.u128())
        .unwrap_or(0);

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
    recipient: Option<HumanAddr>,
) -> HandleResult {
    let mut config_state = Config::load(&deps.storage)?;
    utils::assert_owner(&deps.api, &env, &config_state)?;

    let old_validator = config_state.validator;
    let delegation = utils::query_delegation(&deps.querier, &env, &old_validator)?;

    if old_validator == validator_address {
        return Err(StdError::generic_err("Redelegation to the same validator"));
    }

    if delegation.is_none() {
        config_state.validator = validator_address;
        config_state.save(&mut deps.storage)?;

        let status = to_binary(&HandleAnswer::Redelegate {
            status: ResponseStatus::Success,
        })?;

        return Ok(HandleResponse {
            data: Some(status),
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

    config_state.validator = validator_address.clone();
    config_state.save(&mut deps.storage)?;

    let mut messages = Vec::with_capacity(2);
    if can_withdraw != 0 {
        let owner = deps.api.human_address(&config_state.owner)?;
        let recipient = recipient.unwrap_or(owner);
        let withdraw_msg = StakingMsg::Withdraw {
            validator: old_validator.clone(),
            recipient: Some(recipient),
        };

        messages.push(CosmosMsg::Staking(withdraw_msg));
    }

    let coin = Coin::new(can_redelegate, USCRT);
    let redelegate_msg = StakingMsg::Redelegate {
        src_validator: old_validator,
        dst_validator: validator_address,
        amount: coin,
    };

    messages.push(CosmosMsg::Staking(redelegate_msg));
    let status = to_binary(&HandleAnswer::Redelegate {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages,
        data: Some(status),
        ..Default::default()
    })
}

pub fn query_tier_info<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> QueryResult {
    let config = Config::load(&deps.storage)?;
    let owner = deps.api.human_address(&config.owner)?;
    let validator = config.validator;

    let tier_len = Tier::len(&deps.storage)?;
    let mut tier_list = Vec::with_capacity(tier_len.into());

    for index in 0..tier_len {
        let tier_state = Tier::load(&deps.storage, index)?;
        tier_list.push(tier_state);
    }

    let answer = QueryAnswer::TierInfo {
        owner,
        validator,
        tier_list,
    };

    to_binary(&answer)
}

pub fn query_tier_of<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = User::may_load(&deps.storage, &canonical_address)?;

    let tier = user.and_then(|u| u.tier(&deps.storage).ok()).unwrap_or(0);
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
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = User::may_load(&deps.storage, &canonical_address)?;
    let withdraw_time = user.and_then(|u| u.can_withdraw_at(&deps.storage).ok());

    let answer = QueryAnswer::CanWithdraw {
        time: withdraw_time,
    };

    to_binary(&answer)
}

pub fn query_when_can_claim<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> QueryResult {
    let canonical_address = deps.api.canonical_address(&address)?;
    let user = User::may_load(&deps.storage, &canonical_address)?;
    let claim_time = user.and_then(|u| u.can_claim_at());

    let answer = QueryAnswer::CanClaim { time: claim_time };
    to_binary(&answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        coins, from_binary,
        testing::{mock_dependencies, mock_env, MockApi, MockQuerier},
        FullDelegation, MemoryStorage, StdResult,
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
        env.block.time = current_time();

        init(&mut deps, env, init_msg).map(|_| deps)
    }

    fn tier_of(deps: &Extern<MemoryStorage, MockApi, MockQuerier>, address: &HumanAddr) -> u8 {
        let msg = QueryMsg::TierOf {
            address: address.clone(),
        };

        let answer = query(deps, msg).unwrap();
        match from_binary(&answer).unwrap() {
            QueryAnswer::TierOf { tier } => tier,
            _ => panic!("Wrong query"),
        }
    }

    fn deposit_of(deps: &Extern<MemoryStorage, MockApi, MockQuerier>, address: &HumanAddr) -> u128 {
        let msg = QueryMsg::DepositOf {
            address: address.clone(),
        };

        let answer = query(deps, msg).unwrap();
        match from_binary(&answer).unwrap() {
            QueryAnswer::DepositOf { deposit } => deposit.u128(),
            _ => panic!("Wrong query"),
        }
    }

    fn can_withdraw_at(
        deps: &Extern<MemoryStorage, MockApi, MockQuerier>,
        address: &HumanAddr,
    ) -> Option<u64> {
        let msg = QueryMsg::WhenCanWithdraw {
            address: address.clone(),
        };

        let answer = query(deps, msg).unwrap();
        match from_binary(&answer).unwrap() {
            QueryAnswer::CanWithdraw { time } => time,
            _ => panic!("Wrong query"),
        }
    }

    fn can_claim_at(
        deps: &Extern<MemoryStorage, MockApi, MockQuerier>,
        address: &HumanAddr,
    ) -> Option<u64> {
        let msg = QueryMsg::WhenCanClaim {
            address: address.clone(),
        };

        let answer = query(deps, msg).unwrap();
        match from_binary(&answer).unwrap() {
            QueryAnswer::CanClaim { time } => time,
            _ => panic!("Wrong query"),
        }
    }

    fn init_with_default() -> Extern<MemoryStorage, MockApi, MockQuerier> {
        let owner = HumanAddr::from("admin");
        let validator = HumanAddr::from("validator");
        let deposits = vec![100u128, 750, 5000, 20000]
            .into_iter()
            .map(Into::into)
            .collect();

        let day = 24 * 60 * 60;
        let lock_periods = vec![2 * day, 5 * day, 14 * day, 31 * day];
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
                StdError::Unauthorized { .. } => "Unauthorized".into(),
                _ => panic!("Unexpected error"),
            },
        }
    }

    #[test]
    fn initialization() {
        let owner = HumanAddr::from("admin");
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
            owner: Some(owner.clone()),
            validator: validator.clone(),
            deposits: wrong_deposits,
            lock_periods: lock_periods.clone(),
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
        let length = Tier::len(&deps.storage).unwrap();

        assert_eq!(config.owner, canonical_owner);
        assert_eq!(config.validator, validator);
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

        assert_eq!(config.owner, canonical_alice);
        assert_eq!(config.validator, validator);
    }

    #[test]
    fn deposit() {
        let mut deps = init_with_default();
        let alice = HumanAddr::from("alice");
        let alice_canonical = deps.api.canonical_address(&alice).unwrap();

        let tier = tier_of(&deps, &alice);
        assert_eq!(tier, 0);

        let deposit = deposit_of(&deps, &alice);
        assert_eq!(deposit, 0);

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
        assert_eq!(response.messages.len(), 1);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Staking(StakingMsg::Delegate {
                validator: HumanAddr::from("validator"),
                amount: Coin::new(100, USCRT)
            })
        );

        let user = User::load(&deps.storage, &alice_canonical).unwrap();
        assert_eq!(user.state, UserState::Deposit);
        assert_eq!(user.deposit_amount.u128(), 100);
        assert_eq!(user.deposit_time, env.block.time);
        assert_eq!(user.withdraw_time, None);

        let deposit = deposit_of(&deps, &alice);
        assert_eq!(deposit, 100);

        let tier = tier_of(&deps, &alice);
        assert_eq!(tier, 1);

        env.block.time += 100;
        env.message.sent_funds = coins(649, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You should deposit at least 650 USCRT"));

        env.message.sent_funds = coins(5000, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();
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
                amount: Coin::new(4900, USCRT)
            })
        );

        let user = User::load(&deps.storage, &alice_canonical).unwrap();
        assert_eq!(user.state, UserState::Deposit);
        assert_eq!(user.deposit_amount.u128(), 5000);
        assert_eq!(user.deposit_time, env.block.time);
        assert_eq!(user.withdraw_time, None);

        let deposit = deposit_of(&deps, &alice);
        assert_eq!(deposit, 5000);

        let tier = tier_of(&deps, &alice);
        assert_eq!(tier, 3);

        env.block.time += 100;
        env.message.sent_funds = coins(10000, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You should deposit at least 15000 USCRT"));

        env.message.sent_funds = coins(50000, USCRT);
        let response = handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();
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
                amount: Coin::new(15000, USCRT)
            })
        );

        let deposit = deposit_of(&deps, &alice);
        assert_eq!(deposit, 20000);

        let user = User::load(&deps.storage, &alice_canonical).unwrap();
        assert_eq!(user.state, UserState::Deposit);
        assert_eq!(user.deposit_amount.u128(), 20000);
        assert_eq!(user.deposit_time, env.block.time);
        assert_eq!(user.withdraw_time, None);

        let tier = tier_of(&deps, &alice);
        assert_eq!(tier, 4);

        let response = handle(&mut deps, env, deposit_msg);
        let error = extract_error(response);
        assert!(error.contains("Reached max tear"));
    }

    #[test]
    fn withdraw() {
        let mut deps = init_with_default();
        let alice = HumanAddr::from("alice");
        let alice_canonical = deps.api.canonical_address(&alice).unwrap();

        let withdraw_time = can_withdraw_at(&deps, &alice);
        assert!(withdraw_time.is_none());

        let mut env = mock_env(alice.clone(), &[]);
        env.block.time = current_time();
        env.message.sent_funds = coins(750, USCRT);

        let deposit_msg = HandleMsg::Deposit { padding: None };
        let withdraw_msg = HandleMsg::Withdraw { padding: None };

        // Deposit some tokens. It will set deposit_time
        handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();
        assert_eq!(tier_of(&deps, &alice), 2);
        assert_eq!(deposit_of(&deps, &alice), 750);

        let day = 24 * 60 * 60;
        env.message.sent_funds = Vec::new();
        let withdraw_time = can_withdraw_at(&deps, &alice).unwrap();
        assert_eq!(withdraw_time, env.block.time + 5 * day);

        // Try to withdraw tokens without waiting for locking period
        let response = handle(&mut deps, env.clone(), withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You cannot withdraw tokens yet"));

        // Deposit some tokens. It will reset deposit_time
        env.block.time += 365 * day;
        env.message.sent_funds = coins(4250, USCRT);

        handle(&mut deps, env.clone(), deposit_msg.clone()).unwrap();
        assert_eq!(tier_of(&deps, &alice), 3);
        assert_eq!(deposit_of(&deps, &alice), 5000);

        let withdraw_time = can_withdraw_at(&deps, &alice).unwrap();
        assert_eq!(withdraw_time, env.block.time + 14 * day);

        // Try to withdraw tokens after deposit
        env.message.sent_funds = Vec::new();
        let response = handle(&mut deps, env.clone(), withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("You cannot withdraw tokens yet"));

        // Withdraw tokens successfully
        env.block.time += 14 * day;
        assert_eq!(withdraw_time, env.block.time);
        handle(&mut deps, env.clone(), withdraw_msg.clone()).unwrap();

        let user = User::load(&deps.storage, &alice_canonical).unwrap();
        assert_eq!(user.state, UserState::Withdraw);
        assert_eq!(user.withdraw_time, Some(env.block.time));
        assert_eq!(tier_of(&deps, &alice), 0);

        // Withdraw tokens twive
        let response = handle(&mut deps, env.clone(), withdraw_msg);
        let error = extract_error(response);
        assert!(error.contains("You have already withdrawn tokens"));

        // Deposit tokens during withdrawal
        env.message.sent_funds = coins(20000, USCRT);
        let response = handle(&mut deps, env, deposit_msg);
        let error = extract_error(response);
        assert!(error.contains("Claim your tokens first"));
    }

    #[test]
    fn claim() {
        let mut deps = init_with_default();
        let alice = HumanAddr::from("alice");
        let alice_canonical = deps.api.canonical_address(&alice).unwrap();

        let claim_time = can_claim_at(&deps, &alice);
        assert!(claim_time.is_none());

        let mut env = mock_env(alice.clone(), &[]);
        env.block.time = current_time();

        let deposit_msg = HandleMsg::Deposit { padding: None };
        let withdraw_msg = HandleMsg::Withdraw { padding: None };
        let claim_msg = HandleMsg::Claim {
            recipient: None,
            padding: None,
        };

        // Deposit some tokens
        env.message.sent_funds = coins(750, USCRT);
        handle(&mut deps, env.clone(), deposit_msg).unwrap();
        env.message.sent_funds = Vec::new();

        let day = 24 * 60 * 60;
        env.block.time += 5 * day;
        handle(&mut deps, env.clone(), withdraw_msg).unwrap();

        let claim_time = can_claim_at(&deps, &alice).unwrap();
        assert_eq!(env.block.time + 21 * day, claim_time);

        // Try to claim without waiting for unbond period
        let response = handle(&mut deps, env.clone(), claim_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Wait for tokens undelegation"));

        env.block.time += 21 * day;
        let response = handle(&mut deps, env.clone(), claim_msg).unwrap();
        assert_eq!(response.messages.len(), 1);
        assert_eq!(
            response.messages[0],
            CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address,
                to_address: alice,
                amount: coins(750, USCRT)
            })
        );

        let user = User::may_load(&deps.storage, &alice_canonical).unwrap();
        assert!(user.is_none());
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
            amount: Coin::new(delegated_amount, USCRT),
            accumulated_rewards: Coin::new(accumulated_rewards, USCRT),
            can_redelegate: Coin::new(0, USCRT),
        };

        deps.querier
            .update_staking(USCRT, &[], &[delegation.clone()]);

        let response = handle(&mut deps, env.clone(), redelegate_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Cannot redelegate full delegation amount"));

        // Can redelegate != delegated_amount
        delegation.can_redelegate = Coin::new(500, USCRT);
        deps.querier
            .update_staking(USCRT, &[], &[delegation.clone()]);

        let response = handle(&mut deps, env.clone(), redelegate_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Cannot redelegate full delegation amount"));

        // Can redelegate full amount
        delegation.can_redelegate = Coin::new(delegated_amount, USCRT);
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
                amount: Coin::new(delegated_amount, USCRT),
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
            amount: Coin::new(0, USCRT),
            accumulated_rewards: Coin::new(0, USCRT),
            can_redelegate: Coin::new(0, USCRT),
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

        delegation.accumulated_rewards = Coin::new(1, USCRT);
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
