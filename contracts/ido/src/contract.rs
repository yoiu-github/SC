use crate::{
    msg::{
        HandleAnswer, HandleMsg, InitMsg, QueryMsg, ResponseStatus, TierContractQuery, TierReponse,
        TierTokenQuery,
    },
    state::{self, Config, Ido, Purchase},
    utils::assert_ido_owner,
};
use cosmwasm_std::{
    to_binary, Api, Env, Extern, HandleResponse, HandleResult, HumanAddr, InitResponse, InitResult,
    Querier, QueryResponse, QueryResult, StdError, StdResult, Storage, Uint128,
};
use secret_toolkit_snip20::{transfer_from_msg, transfer_msg};
use secret_toolkit_utils::Query;
use std::cmp::{max, min};

const BLOCK_SIZE: usize = 256;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    msg.check()?;

    let canonical_owner = deps.api.canonical_address(&env.message.sender)?;
    let tier_contract = deps.api.canonical_address(&msg.tier_contract)?;
    let nft_contract = deps.api.canonical_address(&msg.nft_contract)?;
    let token_contract = deps.api.canonical_address(&msg.token_contract)?;

    if let Some(addresses) = msg.whitelist {
        for address in addresses {
            let canonical_address = deps.api.canonical_address(&address)?;
            let whitelist = state::common_whitelist();
            whitelist.insert(&mut deps.storage, &canonical_address, &true)?;
        }
    }

    let max_payments = msg.max_payments.into_iter().map(|p| p.u128()).collect();
    let config = Config {
        owner: canonical_owner,
        tier_contract,
        nft_contract,
        token_contract,
        tier_contract_hash: msg.tier_contract_hash,
        nft_contract_hash: msg.nft_contract_hash,
        token_contract_hash: msg.token_contract_hash,
        lock_periods: msg.lock_periods,
        max_payments,
    };

    config.save(&mut deps.storage)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    match msg {
        HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract,
            token_contract_hash: token_hash,
            price,
            total_amount,
            whitelist,
        } => {
            let mut ido = Ido::default();
            let owner = deps.api.canonical_address(&env.message.sender)?;
            let token_contract = deps.api.canonical_address(&token_contract)?;

            ido.owner = owner;
            ido.start_time = start_time;
            ido.end_time = end_time;
            ido.token_contract = token_contract;
            ido.token_contract_hash = token_hash;
            ido.price = price.u128();
            ido.total_tokens_amount = total_amount.u128();

            start_ido(deps, env, ido, whitelist)
        }
        HandleMsg::BuyTokens {
            amount,
            ido_id,
            token_id,
        } => buy_tokens(deps, env, ido_id, amount.u128(), token_id),
        HandleMsg::WhitelistAdd { addresses, ido_id } => {
            whitelist_add(deps, env, addresses, ido_id)
        }
        HandleMsg::WhitelistRemove { addresses, ido_id } => {
            whitelist_remove(deps, env, addresses, ido_id)
        }

        HandleMsg::RecvTokens { ido_id } => recv_tokens(deps, env, ido_id),
        HandleMsg::Withdraw { ido_id } => withdraw(deps, env, ido_id),
    }
}

fn start_ido<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    mut ido: Ido,
    whitelist_addresses: Option<Vec<HumanAddr>>,
) -> HandleResult {
    if ido.start_time >= ido.end_time {
        return Err(StdError::generic_err(
            "End time must be greater than start time",
        ));
    }

    let ido_id = ido.save(&mut deps.storage)?;
    let ido_whitelist = state::ido_whitelist(ido_id);

    if let Some(whitelist_addresses) = whitelist_addresses {
        for address in whitelist_addresses {
            let canonical_address = deps.api.canonical_address(&address)?;
            ido_whitelist.insert(&mut deps.storage, &canonical_address, &true)?;
        }
    }

    let canonical_sender = deps.api.canonical_address(&env.message.sender)?;
    let startup_ido_list = state::startup_ido_list(&canonical_sender);
    startup_ido_list.push(&mut deps.storage, &ido_id)?;

    let token_address = deps.api.human_address(&ido.token_contract)?;
    let transfer_msg = transfer_from_msg(
        env.message.sender,
        env.contract.address,
        Uint128(ido.total_tokens_amount),
        None,
        None,
        BLOCK_SIZE,
        ido.token_contract_hash,
        token_address,
    )?;

    let answer = to_binary(&HandleAnswer::StartIdo {
        ido_id,
        status: ResponseStatus::Success,
        whitelist_size: ido_whitelist.get_len(&deps.storage)?,
    })?;

    Ok(HandleResponse {
        messages: vec![transfer_msg],
        data: Some(answer),
        ..Default::default()
    })
}

fn get_tier<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    ido_id: u32,
    token_id: Option<String>,
) -> StdResult<u8> {
    let config = Config::load(&deps.storage)?;
    let canonical_address = deps.api.canonical_address(&address)?;

    // If address not in whitelist, tier = 0
    let common_whitelist = state::common_whitelist();
    if !common_whitelist.contains(&deps.storage, &canonical_address) {
        let ido_whitelist = state::ido_whitelist(ido_id);
        if !ido_whitelist.contains(&deps.storage, &canonical_address) {
            return Ok(0);
        }
    }

    let mut nft_tier = 0;
    if let Some(token_id) = token_id {
        let tier_of = TierTokenQuery::TierOf { token_id };
        let nft_contract = deps.api.human_address(&config.nft_contract)?;
        let TierReponse::TierOf { tier } =
            tier_of.query(&deps.querier, config.nft_contract_hash, nft_contract)?;

        nft_tier = tier;
    }

    let tier_of = TierContractQuery::TierOf { address };
    let tier_contract = deps.api.human_address(&config.tier_contract)?;
    let TierReponse::TierOf { tier } =
        tier_of.query(&deps.querier, config.tier_contract_hash, tier_contract)?;

    Ok(max(tier, nft_tier))
}

fn buy_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    ido_id: u32,
    amount: u128,
    token_id: Option<String>,
) -> HandleResult {
    let mut ido = Ido::load(&deps.storage, ido_id)?;
    let remaining_amount = ido.remaining_amount();

    if !ido.is_active(env.block.time) {
        return Err(StdError::generic_err("IDO is not active"));
    }

    if remaining_amount == 0 {
        return Err(StdError::generic_err("All tokens are sold"));
    }

    let sender = env.message.sender;
    let canonical_sender = deps.api.canonical_address(&sender)?;

    let investor_info = state::investor_ido_info(&canonical_sender);
    let mut investor_ido_info = investor_info
        .get(&deps.storage, &ido_id)
        .unwrap_or_default();

    let config = Config::load(&deps.storage)?;
    let tier = get_tier(deps, sender.clone(), ido_id, token_id)?;
    let max_tier_payment = config.max_payments[tier as usize];

    let current_payment = investor_ido_info.total_payment;
    let available_payment = max_tier_payment.checked_sub(current_payment).unwrap();
    let max_tokens_amount = available_payment.checked_div(ido.price).unwrap();
    let can_buy_tokens = min(max_tokens_amount, remaining_amount);

    if can_buy_tokens == 0 {
        return Err(StdError::generic_err(
            "You cannot buy more tokens with current tier",
        ));
    }

    if amount > can_buy_tokens {
        let msg = format!("You cannot buy more than {} tokens", can_buy_tokens);
        return Err(StdError::generic_err(&msg));
    }

    let payment = amount.checked_mul(ido.price).unwrap();
    let lock_period = config.lock_periods[tier as usize];

    let unlock_time = env.block.time.checked_add(lock_period).unwrap();
    let tokens_amount = Uint128(amount);
    let purchase = Purchase {
        tokens_amount: tokens_amount.u128(),
        unlock_time,
    };

    let investor_purchases = state::investor_ido_purchases(&canonical_sender, ido_id);
    investor_purchases.push_back(&mut deps.storage, &purchase)?;

    investor_ido_info.total_payment = investor_ido_info
        .total_payment
        .checked_add(payment)
        .unwrap();

    investor_ido_info.total_tokens_bought = investor_ido_info
        .total_tokens_bought
        .checked_add(amount)
        .unwrap();

    investor_info.insert(&mut deps.storage, &ido_id, &investor_ido_info)?;

    ido.sold_amount = ido.sold_amount.checked_add(amount).unwrap();
    ido.total_payment = ido.total_payment.checked_add(payment).unwrap();

    if current_payment == 0 {
        ido.participants = ido.participants.checked_add(1).unwrap();
    }

    ido.save(&mut deps.storage)?;

    let token_address = deps.api.human_address(&ido.token_contract)?;
    let ido_owner = deps.api.human_address(&ido.owner)?;

    let transfer_msg = transfer_from_msg(
        sender,
        ido_owner,
        Uint128(payment),
        None,
        None,
        BLOCK_SIZE,
        ido.token_contract_hash,
        token_address,
    )?;

    let answer = to_binary(&HandleAnswer::BuyTokens {
        amount: Uint128(amount),
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![transfer_msg],
        data: Some(answer),
        ..Default::default()
    })
}

fn recv_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    ido_id: u32,
) -> HandleResult {
    let canonical_sender = deps.api.canonical_address(&env.message.sender)?;
    let current_time = env.block.time;

    let investor_info = state::investor_ido_info(&canonical_sender);
    let mut investor_ido_info = investor_info
        .get(&deps.storage, &ido_id)
        .unwrap_or_default();

    let purchases = state::investor_ido_purchases(&canonical_sender, ido_id);
    let purchases_iter = purchases.iter(&deps.storage)?;

    let mut indices = Vec::new();
    let mut recv_amount: u128 = 0;

    for (i, purchase) in purchases_iter.enumerate() {
        let purchase = purchase?;

        if purchase.unlock_time >= current_time {
            recv_amount = recv_amount.checked_add(purchase.tokens_amount).unwrap();
            indices.push(i);
        }
    }

    let answer = to_binary(&HandleAnswer::RecvTokens {
        amount: Uint128(recv_amount),
        status: ResponseStatus::Success,
    })?;

    if recv_amount == 0 {
        return Ok(HandleResponse {
            data: Some(answer),
            ..Default::default()
        });
    }

    investor_ido_info.total_tokens_received = investor_ido_info
        .total_tokens_received
        .checked_add(recv_amount)
        .unwrap();

    for (shift, index) in indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        purchases.remove(&mut deps.storage, position as u32)?;
    }

    let ido = Ido::load(&deps.storage, ido_id)?;
    let token_contract = deps.api.human_address(&ido.token_contract)?;

    let transfer_msg = transfer_msg(
        env.message.sender,
        Uint128(recv_amount),
        None,
        None,
        BLOCK_SIZE,
        ido.token_contract_hash,
        token_contract,
    )?;

    Ok(HandleResponse {
        messages: vec![transfer_msg],
        data: Some(answer),
        ..Default::default()
    })
}

fn withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    ido_id: u32,
) -> HandleResult {
    let mut ido = Ido::load(&deps.storage, ido_id)?;
    assert_ido_owner(&deps.api, &env, &ido)?;

    if ido.withdrawn {
        return Err(StdError::generic_err("Already withdrawn"));
    }

    if env.block.time < ido.end_time {
        return Err(StdError::generic_err("IDO is not finished yet"));
    }

    ido.withdrawn = true;
    ido.save(&mut deps.storage)?;

    let remaining_tokens = Uint128(ido.remaining_amount());
    let ido_token_contract = deps.api.human_address(&ido.token_contract)?;
    let ido_owner = deps.api.human_address(&ido.owner)?;

    let transfer_tokens = transfer_msg(
        ido_owner,
        remaining_tokens,
        None,
        None,
        BLOCK_SIZE,
        ido.token_contract_hash,
        ido_token_contract,
    )?;

    let answer = to_binary(&HandleAnswer::Withdraw {
        amount: remaining_tokens,
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![transfer_tokens],
        data: Some(answer),
        ..Default::default()
    })
}

fn check_whitelist_authority<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    sender: &HumanAddr,
    ido_id: Option<u32>,
) -> StdResult<()> {
    let sender = deps.api.canonical_address(sender)?;

    if let Some(ido_id) = ido_id {
        let ido = Ido::load(&deps.storage, ido_id)?;
        if sender != ido.owner {
            return Err(StdError::unauthorized());
        }
    } else {
        let config = Config::load(&deps.storage)?;
        if sender != config.owner {
            return Err(StdError::unauthorized());
        }
    }

    Ok(())
}

fn whitelist_add<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    addresses: Vec<HumanAddr>,
    ido_id: Option<u32>,
) -> HandleResult {
    check_whitelist_authority(deps, &env.message.sender, ido_id)?;

    let whitelist = ido_id
        .map(state::ido_whitelist)
        .unwrap_or_else(state::common_whitelist);

    for address in addresses {
        let canonical_address = deps.api.canonical_address(&address)?;
        whitelist.insert(&mut deps.storage, &canonical_address, &true)?;
    }

    let answer = to_binary(&HandleAnswer::WhitelistAdd {
        status: ResponseStatus::Success,
        whitelist_size: whitelist.get_len(&deps.storage)?,
    })?;

    Ok(HandleResponse {
        data: Some(answer),
        ..Default::default()
    })
}

fn whitelist_remove<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    addresses: Vec<HumanAddr>,
    ido_id: Option<u32>,
) -> HandleResult {
    check_whitelist_authority(deps, &env.message.sender, ido_id)?;

    let whitelist = ido_id
        .map(state::ido_whitelist)
        .unwrap_or_else(state::common_whitelist);

    for address in addresses {
        let canonical_address = deps.api.canonical_address(&address)?;
        whitelist.remove(&mut deps.storage, &canonical_address)?;
    }

    let answer = to_binary(&HandleAnswer::WhitelistRemove {
        status: ResponseStatus::Success,
        whitelist_size: whitelist.get_len(&deps.storage)?,
    })?;

    Ok(HandleResponse {
        data: Some(answer),
        ..Default::default()
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    _msg: QueryMsg,
) -> QueryResult {
    Ok(QueryResponse::default())
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage},
    };
    use rand::{thread_rng, Rng};

    fn get_init_msg() -> InitMsg {
        InitMsg {
            max_payments: [100, 500, 1000, 2000].into_iter().map(Uint128).collect(),
            tier_contract: HumanAddr::from("tier"),
            tier_contract_hash: String::from("tier_hash"),
            nft_contract: HumanAddr::from("nft"),
            nft_contract_hash: String::from("nft_hash"),
            token_contract: HumanAddr::from("token"),
            token_contract_hash: String::from("token_hash"),
            lock_periods: vec![100, 150, 200, 250],
            whitelist: None,
        }
    }

    fn initialize_with(msg: InitMsg) -> StdResult<Extern<MockStorage, MockApi, MockQuerier>> {
        let mut deps = mock_dependencies(20, &[]);
        let owner = HumanAddr::from("owner");
        let env = mock_env(owner, &[]);

        init(&mut deps, env, msg)?;
        Ok(deps)
    }

    fn initialize_with_default() -> Extern<MockStorage, MockApi, MockQuerier> {
        let msg = get_init_msg();
        initialize_with(msg).unwrap()
    }

    fn start_ido_msg() -> HandleMsg {
        let mut rng = thread_rng();
        let token_contract = format!("token_{}", rng.gen_range(0..1000));
        let token_contract_hash = format!("{}_hash", token_contract);

        let mut start_time = rng.gen();
        let mut end_time = rng.gen();

        if start_time > end_time {
            [start_time, end_time] = [end_time, start_time];
        }

        let price = rng.gen();
        let total_amount = rng.gen();

        let mut whitelist = Vec::new();
        for i in 0..rng.gen_range(0..100) {
            let address = format!("address_{}", i);
            whitelist.push(HumanAddr(address));
        }

        HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract: HumanAddr(token_contract),
            token_contract_hash,
            price: Uint128(price),
            total_amount: Uint128(total_amount),
            whitelist: Some(whitelist),
        }
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
    fn initialize() {
        let mut msg = get_init_msg();

        msg.max_payments = Vec::new();
        let error = extract_error(initialize_with(msg.clone()));
        assert!(error.contains("Specify max payments array"));

        msg.max_payments = [1, 2, 4, 3].into_iter().map(Uint128).collect();
        let error = extract_error(initialize_with(msg.clone()));
        assert!(error.contains("Specify max payments in increasing order"));

        msg.max_payments = [1, 2, 3, 4].into_iter().map(Uint128).collect();
        let deps = initialize_with(msg.clone()).unwrap();

        let config = Config::load(&deps.storage).unwrap();
        let owner = deps
            .api
            .canonical_address(&HumanAddr::from("owner"))
            .unwrap();
        let tier_contract = deps.api.canonical_address(&msg.tier_contract).unwrap();
        let nft_contract = deps.api.canonical_address(&msg.nft_contract).unwrap();
        let token_contract = deps.api.canonical_address(&msg.token_contract).unwrap();

        assert_eq!(config.owner, owner);
        assert_eq!(config.lock_periods, msg.lock_periods);
        assert_eq!(config.tier_contract, tier_contract);
        assert_eq!(config.tier_contract_hash, msg.tier_contract_hash);
        assert_eq!(config.nft_contract, nft_contract);
        assert_eq!(config.nft_contract_hash, msg.nft_contract_hash);
        assert_eq!(config.token_contract, token_contract);
        assert_eq!(config.token_contract_hash, msg.token_contract_hash);
        assert_eq!(
            config.max_payments,
            msg.max_payments
                .into_iter()
                .map(|p| p.u128())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn initialize_with_whitelist() {
        let mut msg = get_init_msg();

        let whitelist_len = 30;
        let mut whitelist_addresses = Vec::with_capacity(whitelist_len);

        for i in 0..whitelist_len {
            let address_str = format!("{:03}", i);
            let address = HumanAddr(address_str);
            whitelist_addresses.push(address);
        }

        msg.whitelist = Some(whitelist_addresses.clone());
        let deps = initialize_with(msg).unwrap();

        let whitelist = state::common_whitelist();
        for address in whitelist_addresses {
            let canonical_address = deps.api.canonical_address(&address).unwrap();
            assert!(whitelist.contains(&deps.storage, &canonical_address));
        }
    }

    #[test]
    fn start_ido() {
        let mut deps = initialize_with_default();

        let ido_owner = HumanAddr::from("ido_owner");
        let canonical_ido_owner = deps.api.canonical_address(&ido_owner).unwrap();
        let env = mock_env(&ido_owner, &[]);
        let msg = start_ido_msg();

        let startup_ido_list = state::startup_ido_list(&canonical_ido_owner);
        assert_eq!(startup_ido_list.get_len(&deps.storage), Ok(0));
        assert_eq!(Ido::len(&deps.storage), Ok(0));

        let HandleResponse { messages, data, .. } =
            handle(&mut deps, env.clone(), msg.clone()).unwrap();

        match from_binary(&data.unwrap()).unwrap() {
            HandleAnswer::StartIdo { ido_id, status, .. } => {
                assert_eq!(ido_id, 0);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        assert_eq!(Ido::len(&deps.storage), Ok(1));
        let ido = Ido::load(&deps.storage, 0).unwrap();

        let startup_ido_list = state::startup_ido_list(&canonical_ido_owner);
        assert_eq!(startup_ido_list.get_len(&deps.storage), Ok(1));

        if let HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract,
            token_contract_hash,
            price,
            total_amount,
            whitelist,
        } = msg
        {
            let sender = deps.api.canonical_address(&env.message.sender).unwrap();
            let token_contract_canonical = deps.api.canonical_address(&token_contract).unwrap();

            assert_eq!(ido.owner, sender);
            assert_eq!(ido.start_time, start_time);
            assert_eq!(ido.end_time, end_time);
            assert_eq!(ido.token_contract, token_contract_canonical);
            assert_eq!(ido.token_contract_hash, token_contract_hash);
            assert_eq!(ido.price, price.u128());
            assert_eq!(ido.participants, 0);
            assert_eq!(ido.sold_amount, 0);
            assert_eq!(ido.total_tokens_amount, total_amount.u128());

            let whitelist_len = whitelist.unwrap().len() as u32;
            let ido_whitelist = state::ido_whitelist(0);
            assert_eq!(ido_whitelist.get_len(&deps.storage), Ok(whitelist_len));

            let expected_message = transfer_from_msg(
                env.message.sender,
                env.contract.address,
                total_amount,
                None,
                None,
                BLOCK_SIZE,
                token_contract_hash,
                token_contract,
            )
            .unwrap();

            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0], expected_message);
        } else {
            unreachable!();
        }
    }
}
