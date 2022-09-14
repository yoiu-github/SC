use crate::{
    msg::{
        HandleAnswer, HandleMsg, InitMsg, QueryMsg, ResponseStatus, TierContractQuery, TierReponse,
        TierTokenQuery,
    },
    state::{Config, Ido, Purchase, Purchases, Whitelist},
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
        let mut whitelist = Whitelist::load(&deps.storage, None)?;
        for address in addresses {
            let canonical_address = deps.api.canonical_address(&address)?;
            whitelist.add(&mut deps.storage, &canonical_address)?;
        }
    }

    let config = Config {
        owner: canonical_owner,
        tier_contract,
        nft_contract,
        token_contract,
        tier_contract_hash: msg.tier_contract_hash,
        nft_contract_hash: msg.nft_contract_hash,
        token_contract_hash: msg.token_contract_hash,
        lock_periods: msg.lock_periods,
        max_payments: msg.max_payments,
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
        } => {
            let mut ido = Ido::default();
            let token_contract = deps.api.canonical_address(&token_contract)?;

            ido.owner = deps.api.canonical_address(&env.message.sender)?;
            ido.start_time = start_time;
            ido.end_time = end_time;
            ido.token_contract = token_contract;
            ido.token_contract_hash = token_hash;
            ido.price = price;
            ido.total_tokens_amount = total_amount;

            start_ido(deps, env, ido)
        }
        HandleMsg::BuyTokens {
            amount,
            ido_id,
            token_id,
        } => buy_tokens(deps, env, ido_id, amount.u128(), token_id),
        HandleMsg::WhitelistAdd { addresses } => whitelist_add(deps, addresses),
        HandleMsg::WhitelistRemove { addresses } => whitelist_remove(deps, addresses),
        HandleMsg::RecvTokens { ido_id } => recv_tokens(deps, env, ido_id),
        HandleMsg::Withdraw { ido_id } => withdraw(deps, env, ido_id),
    }
}

fn start_ido<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    mut ido: Ido,
) -> HandleResult {
    if ido.start_time >= ido.end_time {
        return Err(StdError::generic_err(
            "End time must be greater than start time",
        ));
    }

    let ido_id = ido.save(&mut deps.storage)?;
    let token_address = deps.api.human_address(&ido.token_contract)?;

    let transfer_msg = transfer_from_msg(
        env.message.sender,
        env.contract.address,
        ido.total_tokens_amount,
        None,
        None,
        BLOCK_SIZE,
        ido.token_contract_hash,
        token_address,
    )?;

    let answer = to_binary(&HandleAnswer::StartIdo {
        ido_id,
        status: ResponseStatus::Success,
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
    token_id: Option<String>,
) -> StdResult<u8> {
    let config = Config::load(&deps.storage)?;
    let canonical_address = deps.api.canonical_address(&address)?;

    // If address not in whitelist, tier = 0

    let whitelist = Whitelist::load(&deps.storage, None)?;
    if !whitelist.contains(&deps.storage, &canonical_address)? {
        return Ok(0);
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

fn investor_can_buy<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    config: &Config,
    investor: &HumanAddr,
    ido_id: u64,
    tier: u8,
) -> StdResult<u128> {
    let ido = Ido::load(&deps.storage, ido_id)?;
    let remaning_amount = ido.remaining_amount();

    if remaning_amount == 0 {
        return Ok(0);
    }

    let canonical_investor = deps.api.canonical_address(investor)?;
    let purchases = Purchases::load(&deps.storage, canonical_investor, ido_id)?;
    let investor_payment = purchases.total_payment();

    let max_payment = config.max_payments[tier as usize].u128();
    let available_payment = max_payment.checked_sub(investor_payment).unwrap();
    let max_tokens_allowed = available_payment.checked_div(ido.price.u128()).unwrap();

    Ok(min(remaning_amount, max_tokens_allowed))
}

fn buy_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    ido_id: u64,
    amount: u128,
    token_id: Option<String>,
) -> HandleResult {
    let mut ido = Ido::load(&deps.storage, ido_id)?;
    let time = env.block.time;

    if !ido.is_active(time) {
        return Err(StdError::generic_err("IDO is not active"));
    }

    let investor = env.message.sender;
    let config = Config::load(&deps.storage)?;
    let tier = get_tier(deps, investor.clone(), token_id)?;
    let can_buy = investor_can_buy(deps, &config, &investor, ido_id, tier)?;

    if can_buy == 0 {
        let msg = if ido.remaining_amount() == 0 {
            "All tokens are sold"
        } else {
            "You cannot buy more tokens with current tier"
        };

        return Err(StdError::generic_err(msg));
    }

    if amount > can_buy {
        let msg = format!("You cannot buy more than {} tokens", can_buy);
        return Err(StdError::generic_err(&msg));
    }

    let payment = amount.checked_mul(ido.price.u128()).unwrap();

    ido.sold_amount = ido
        .sold_amount
        .u128()
        .checked_add(amount)
        .map(Uint128)
        .unwrap();

    ido.total_payment = ido
        .total_payment
        .u128()
        .checked_add(payment)
        .map(Uint128)
        .unwrap();

    let canonical_investor = deps.api.canonical_address(&investor)?;
    let mut purchases = Purchases::load(&deps.storage, canonical_investor, ido_id)?;

    if purchases.total_payment() == 0 {
        ido.participants = ido.participants.checked_add(1).unwrap();
    }

    ido.save(&mut deps.storage)?;

    let lock_period = config.lock_periods[tier as usize];
    let purchase = Purchase::new(payment, amount, env.block.time, lock_period);
    purchases.add(&purchase, &mut deps.storage)?;

    let token_address = deps.api.human_address(&ido.token_contract)?;
    let ido_owner = deps.api.human_address(&ido.owner)?;

    let transfer_msg = transfer_from_msg(
        investor,
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
    ido_id: u64,
) -> HandleResult {
    let investor = deps.api.canonical_address(&env.message.sender)?;
    let current_time = env.block.time;

    let mut purchases = Purchases::load(&deps.storage, investor, ido_id)?;
    let mut recv_amount: u128 = 0;
    let mut index = 0;

    let len = purchases.len();
    while index < len {
        let purchase = purchases.get(index, &deps.storage)?;
        index = index.checked_add(1).unwrap();

        if purchase.unlock_time >= current_time {
            break;
        }

        recv_amount = recv_amount
            .checked_add(purchase.tokens_amount.u128())
            .unwrap();
    }

    purchases.remove(&mut deps.storage, index)?;

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

    let answer = to_binary(&HandleAnswer::RecvTokens {
        amount: Uint128(recv_amount),
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        messages: vec![transfer_msg],
        data: Some(answer),
        ..Default::default()
    })
}

fn withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    ido_id: u64,
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

fn whitelist_add<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    addresses: Vec<HumanAddr>,
) -> HandleResult {
    let mut whitelist = Whitelist::load(&deps.storage, None)?;
    for address in addresses {
        let canonical_address = deps.api.canonical_address(&address)?;
        whitelist.add(&mut deps.storage, &canonical_address)?;
    }

    let answer = to_binary(&HandleAnswer::WhitelistAdd {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        data: Some(answer),
        ..Default::default()
    })
}

fn whitelist_remove<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    addresses: Vec<HumanAddr>,
) -> HandleResult {
    let mut whitelist = Whitelist::load(&deps.storage, None)?;
    for address in addresses {
        let canonical_address = deps.api.canonical_address(&address)?;
        whitelist.remove(&mut deps.storage, &canonical_address)?;
    }

    let answer = to_binary(&HandleAnswer::WhitelistRemove {
        status: ResponseStatus::Success,
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

        HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract: HumanAddr(token_contract),
            token_contract_hash,
            price: Uint128(price),
            total_amount: Uint128(total_amount),
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
        assert_eq!(config.max_payments, msg.max_payments);
        assert_eq!(config.lock_periods, msg.lock_periods);
        assert_eq!(config.tier_contract, tier_contract);
        assert_eq!(config.tier_contract_hash, msg.tier_contract_hash);
        assert_eq!(config.nft_contract, nft_contract);
        assert_eq!(config.nft_contract_hash, msg.nft_contract_hash);
        assert_eq!(config.token_contract, token_contract);
        assert_eq!(config.token_contract_hash, msg.token_contract_hash);
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

        let whitelist = Whitelist::load(&deps.storage, None).unwrap();
        for address in whitelist_addresses {
            let canonical_address = deps.api.canonical_address(&address).unwrap();
            assert!(whitelist
                .contains(&deps.storage, &canonical_address)
                .unwrap());
        }
    }

    #[test]
    fn start_ido() {
        let mut deps = initialize_with_default();

        let ido_owner = HumanAddr::from("ido_owner");
        let env = mock_env(&ido_owner, &[]);
        let msg = start_ido_msg();

        assert_eq!(Ido::len(&deps.storage), Ok(0));

        let HandleResponse { messages, data, .. } =
            handle(&mut deps, env.clone(), msg.clone()).unwrap();

        match from_binary(&data.unwrap()).unwrap() {
            HandleAnswer::StartIdo { ido_id, status } => {
                assert_eq!(ido_id, 0);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        assert_eq!(Ido::len(&deps.storage), Ok(1));
        let ido = Ido::load(&deps.storage, 0).unwrap();

        if let HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract,
            token_contract_hash,
            price,
            total_amount,
        } = msg
        {
            let sender = deps.api.canonical_address(&env.message.sender).unwrap();
            let token_contract_canonical = deps.api.canonical_address(&token_contract).unwrap();

            assert_eq!(ido.owner, sender);
            assert_eq!(ido.start_time, start_time);
            assert_eq!(ido.end_time, end_time);
            assert_eq!(ido.token_contract, token_contract_canonical);
            assert_eq!(ido.token_contract_hash, token_contract_hash);
            assert_eq!(ido.price, price);
            assert_eq!(ido.participants, 0);
            assert_eq!(ido.sold_amount, Uint128(0));
            assert_eq!(ido.total_tokens_amount, total_amount);

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
