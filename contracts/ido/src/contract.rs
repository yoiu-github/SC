use crate::{
    msg::{
        ContractStatus, HandleAnswer, HandleMsg, InitMsg, NftToken, PaymentMethod, QueryAnswer,
        QueryMsg, ResponseStatus, Whitelist,
    },
    state::{self, Config, Ido, Purchase},
    tier::{get_min_tier, get_tier},
    utils::{self, assert_admin, assert_contract_active, assert_ido_admin},
};
use cosmwasm_std::{
    coins, to_binary, Api, BankMsg, CosmosMsg, Env, Extern, HandleResponse, HandleResult,
    HumanAddr, InitResponse, InitResult, Querier, QueryResult, StdError, Storage, Uint128,
};
use secret_toolkit_snip20::{transfer_from_msg, transfer_msg};
use secret_toolkit_utils::{pad_handle_result, pad_query_result};

pub const BLOCK_SIZE: usize = 256;
pub const USCRT: &str = "uscrt";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    let admin = msg.admin.unwrap_or(env.message.sender);
    let canonical_admin = deps.api.canonical_address(&admin)?;
    let tier_contract = deps.api.canonical_address(&msg.tier_contract)?;
    let nft_contract = deps.api.canonical_address(&msg.nft_contract)?;
    let lock_periods_len = msg.lock_periods.len();

    let mut config = Config {
        admin: canonical_admin,
        status: ContractStatus::Active as u8,
        tier_contract,
        nft_contract,
        tier_contract_hash: msg.tier_contract_hash,
        nft_contract_hash: msg.nft_contract_hash,
        lock_periods: msg.lock_periods,
        min_tier: 0,
    };

    let min_tier = get_min_tier(deps, &config)?;
    config.min_tier = min_tier;

    if lock_periods_len != min_tier as usize {
        return Err(StdError::generic_err(&format!(
            "Lock periods array must have {} items",
            min_tier
        )));
    }

    config.save(&mut deps.storage)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    let response = match msg {
        HandleMsg::ChangeAdmin { admin, .. } => change_admin(deps, env, admin),
        HandleMsg::ChangeStatus { status, .. } => change_status(deps, env, status),
        HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract,
            token_contract_hash,
            price,
            total_amount,
            tokens_per_tier,
            whitelist,
            payment,
            ..
        } => {
            let mut ido = Ido::default();
            let admin = deps.api.canonical_address(&env.message.sender)?;
            let token_contract = deps.api.canonical_address(&token_contract)?;

            ido.admin = admin;
            ido.start_time = start_time;
            ido.end_time = end_time;
            ido.token_contract = token_contract;
            ido.token_contract_hash = token_contract_hash;
            ido.price = price.u128();
            ido.total_tokens_amount = total_amount.u128();
            ido.remaining_tokens_per_tier = tokens_per_tier.into_iter().map(|v| v.u128()).collect();

            if let PaymentMethod::Token {
                contract,
                code_hash,
            } = payment
            {
                let payment_token_contract = deps.api.canonical_address(&contract)?;
                ido.payment_token_contract = Some(payment_token_contract);
                ido.payment_token_hash = Some(code_hash);
            }

            start_ido(deps, env, ido, whitelist)
        }
        HandleMsg::BuyTokens {
            amount,
            ido_id,
            token,
            ..
        } => buy_tokens(deps, env, ido_id, amount.u128(), token),
        HandleMsg::WhitelistAdd {
            addresses, ido_id, ..
        } => whitelist_add(deps, env, addresses, ido_id),
        HandleMsg::WhitelistRemove {
            addresses, ido_id, ..
        } => whitelist_remove(deps, env, addresses, ido_id),
        HandleMsg::RecvTokens {
            ido_id,
            start,
            limit,
            purchase_indices,
            ..
        } => recv_tokens(deps, env, ido_id, start, limit, purchase_indices),
        HandleMsg::Withdraw { ido_id, .. } => withdraw(deps, env, ido_id),
    };

    pad_handle_result(response, BLOCK_SIZE)
}

fn change_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    admin: HumanAddr,
) -> HandleResult {
    assert_admin(deps, &env.message.sender)?;

    let mut config = Config::load(&deps.storage)?;
    let new_admin = deps.api.canonical_address(&admin)?;
    config.admin = new_admin;

    config.save(&mut deps.storage)?;

    let answer = to_binary(&HandleAnswer::ChangeAdmin {
        status: ResponseStatus::Success,
    })?;

    Ok(HandleResponse {
        data: Some(answer),
        ..Default::default()
    })
}

fn change_status<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    status: ContractStatus,
) -> HandleResult {
    assert_admin(deps, &env.message.sender)?;

    let mut config = Config::load(&deps.storage)?;
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

fn start_ido<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    mut ido: Ido,
    whitelist: Whitelist,
) -> HandleResult {
    assert_contract_active(&deps.storage)?;

    let config = Config::load(&deps.storage)?;
    if ido.remaining_tokens_per_tier.len() != config.min_tier as usize {
        return Err(StdError::generic_err("`tokens_per_tier` has wrong size"));
    }

    let sum = ido.remaining_tokens_per_tier.iter().sum::<u128>();
    if sum < ido.total_tokens_amount {
        return Err(StdError::generic_err(
            "Sum of `tokens_per_tier` can't be less than total tokens amount",
        ));
    }

    if ido.start_time >= ido.end_time {
        return Err(StdError::generic_err(
            "End time must be greater than start time",
        ));
    }

    if env.block.time >= ido.end_time {
        return Err(StdError::generic_err("Ido ends in the past"));
    }

    ido.shared_whitelist = match whitelist {
        Whitelist::Shared { .. } => true,
        Whitelist::Empty { .. } => false,
    };

    let ido_id = ido.save(&mut deps.storage)?;
    let ido_whitelist = state::ido_whitelist(ido_id);

    match whitelist {
        Whitelist::Empty { with } => {
            for address in with.unwrap_or_default() {
                let canonical_address = deps.api.canonical_address(&address)?;
                ido_whitelist.insert(&mut deps.storage, &canonical_address, &true)?;
            }
        }
        Whitelist::Shared { with_blocked } => {
            for address in with_blocked.unwrap_or_default() {
                let canonical_address = deps.api.canonical_address(&address)?;
                ido_whitelist.insert(&mut deps.storage, &canonical_address, &false)?;
            }
        }
    }

    ido.save(&mut deps.storage)?;

    let canonical_sender = deps.api.canonical_address(&env.message.sender)?;
    let startup_ido_list = state::ido_list_owned_by(&canonical_sender);
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

fn buy_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    ido_id: u32,
    mut amount: u128,
    token: Option<NftToken>,
) -> HandleResult {
    assert_contract_active(&deps.storage)?;

    let sender = env.message.sender;
    let canonical_sender = deps.api.canonical_address(&sender)?;

    let mut ido = Ido::load(&deps.storage, ido_id)?;
    if !ido.is_active(env.block.time) {
        return Err(StdError::generic_err("IDO is not active"));
    }

    if ido.is_native_payment() {
        amount = utils::sent_funds(&env.message.sent_funds)?;
    }

    if amount == 0 {
        return Err(StdError::generic_err("Zero amount"));
    }

    let config = Config::load(&deps.storage)?;
    let tier = if utils::in_whitelist(deps, &sender, ido_id)? {
        get_tier(deps, sender.clone(), token)?
    } else {
        config.min_tier
    };

    let remaining_amount = ido.remaining_tokens_per_tier(tier);
    if remaining_amount == 0 {
        if ido.total_tokens_amount == ido.sold_amount {
            return Err(StdError::generic_err("All tokens are sold"));
        } else {
            return Err(StdError::generic_err("All tokens are sold for your tier"));
        }
    }

    if amount > remaining_amount {
        let msg = format!("You cannot buy more than {} tokens", remaining_amount);
        return Err(StdError::generic_err(&msg));
    }

    let payment = amount.checked_mul(ido.price).unwrap();
    let lock_period = config.lock_period(tier);

    let unlock_time = ido.end_time.checked_add(lock_period).unwrap();
    let tokens_amount = Uint128(amount);
    let purchase = Purchase {
        timestamp: env.block.time,
        tokens_amount: tokens_amount.u128(),
        unlock_time,
    };

    let purchases = state::purchases(&canonical_sender, ido_id);
    purchases.push_back(&mut deps.storage, &purchase)?;

    let all_user_infos_in_ido = state::user_info_in_ido(&canonical_sender);
    let mut user_ido_info = all_user_infos_in_ido
        .get(&deps.storage, &ido_id)
        .unwrap_or_default();

    if user_ido_info.total_payment == 0 {
        ido.participants = ido.participants.checked_add(1).unwrap();
    }

    user_ido_info.total_payment = user_ido_info.total_payment.checked_add(payment).unwrap();
    user_ido_info.total_tokens_bought = user_ido_info
        .total_tokens_bought
        .checked_add(amount)
        .unwrap();

    let all_user_infos = state::user_info();
    let mut user_info = all_user_infos
        .get(&deps.storage, &canonical_sender)
        .unwrap_or_default();

    user_info.total_payment = user_info.total_payment.checked_add(payment).unwrap();
    user_info.total_tokens_bought = user_info.total_tokens_bought.checked_add(amount).unwrap();

    all_user_infos.insert(&mut deps.storage, &canonical_sender, &user_info)?;
    all_user_infos_in_ido.insert(&mut deps.storage, &ido_id, &user_ido_info)?;

    let active_ido_list = state::active_ido_list(&canonical_sender);
    active_ido_list.insert(&mut deps.storage, &ido_id, &true)?;

    ido.sold_amount = ido.sold_amount.checked_add(amount).unwrap();
    ido.total_payment = ido.total_payment.checked_add(payment).unwrap();

    let tier_index = tier.checked_sub(1).unwrap() as usize;
    ido.remaining_tokens_per_tier[tier_index] = ido.remaining_tokens_per_tier[tier_index]
        .checked_sub(amount)
        .unwrap();

    ido.save(&mut deps.storage)?;

    let ido_admin = deps.api.human_address(&ido.admin)?;

    let transfer_msg = if !ido.is_native_payment() {
        let token_contract_canonical = ido.payment_token_contract.unwrap();
        let token_contract_hash = ido.payment_token_hash.unwrap();
        let token_contract = deps.api.human_address(&token_contract_canonical)?;

        transfer_from_msg(
            sender,
            ido_admin,
            Uint128(payment),
            None,
            None,
            BLOCK_SIZE,
            token_contract_hash,
            token_contract,
        )?
    } else {
        CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address,
            to_address: ido_admin,
            amount: coins(amount, USCRT),
        })
    };

    let answer = to_binary(&HandleAnswer::BuyTokens {
        unlock_time,
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
    start: Option<u32>,
    limit: Option<u32>,
    purchase_indices: Option<Vec<u32>>,
) -> HandleResult {
    assert_contract_active(&deps.storage)?;

    let canonical_sender = deps.api.canonical_address(&env.message.sender)?;
    let current_time = env.block.time;

    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(300);
    let purchases = state::purchases(&canonical_sender, ido_id);
    let purchases_iter = purchases
        .iter(&deps.storage)?
        .skip(start as usize)
        .take(limit as usize);

    let mut indices = Vec::new();
    for (i, purchase) in purchases_iter.enumerate() {
        let purchase = purchase?;

        if current_time >= purchase.unlock_time {
            let index = i.checked_add(start as usize).unwrap();
            indices.push(index);
        }
    }

    if let Some(purchase_indices) = purchase_indices {
        let end = start.checked_add(limit).unwrap();
        for index in purchase_indices {
            if index >= start && index < end {
                continue;
            }

            let purchase = purchases.get_at(&deps.storage, index)?;
            if current_time >= purchase.unlock_time {
                indices.push(index as usize);
            }
        }
    }

    indices.sort();
    indices.dedup();

    let mut recv_amount: u128 = 0;
    let archived_purchases = state::archived_purchases(&canonical_sender, ido_id);

    for (shift, index) in indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        let purchase = purchases.remove(&mut deps.storage, position as u32)?;

        recv_amount = recv_amount.checked_add(purchase.tokens_amount).unwrap();
        archived_purchases.push(&mut deps.storage, &purchase)?;
    }

    if recv_amount == 0 {
        return Err(StdError::generic_err("Nothing to receive"));
    }

    let answer = to_binary(&HandleAnswer::RecvTokens {
        amount: Uint128(recv_amount),
        status: ResponseStatus::Success,
    })?;

    let all_user_infos = state::user_info();
    let all_user_infos_in_ido = state::user_info_in_ido(&canonical_sender);

    let mut user_info = all_user_infos
        .get(&deps.storage, &canonical_sender)
        .unwrap();

    let mut user_ido_info = all_user_infos_in_ido.get(&deps.storage, &ido_id).unwrap();

    user_info.total_tokens_received = user_info
        .total_tokens_received
        .checked_add(recv_amount)
        .unwrap();

    user_ido_info.total_tokens_received = user_ido_info
        .total_tokens_received
        .checked_add(recv_amount)
        .unwrap();

    all_user_infos.insert(&mut deps.storage, &canonical_sender, &user_info)?;
    all_user_infos_in_ido.insert(&mut deps.storage, &ido_id, &user_ido_info)?;

    if user_ido_info.total_tokens_bought == user_ido_info.total_tokens_received {
        let active_ido_list = state::active_ido_list(&canonical_sender);
        active_ido_list.remove(&mut deps.storage, &ido_id)?;
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
    let ido_admin = env.message.sender;
    assert_ido_admin(deps, &ido_admin, ido_id)?;
    assert_contract_active(&deps.storage)?;

    let mut ido = Ido::load(&deps.storage, ido_id)?;
    if ido.withdrawn {
        return Err(StdError::generic_err("Already withdrawn"));
    }

    if env.block.time < ido.end_time {
        return Err(StdError::generic_err("IDO is not finished yet"));
    }

    ido.withdrawn = true;
    ido.save(&mut deps.storage)?;

    let remaining_tokens = Uint128::from(ido.remaining_tokens());
    if remaining_tokens.is_zero() {
        return Err(StdError::generic_err("Nothing to withdraw"));
    }

    let ido_token_contract = deps.api.human_address(&ido.token_contract)?;
    let transfer_tokens = transfer_msg(
        ido_admin,
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
    env: Env,
    addresses: Vec<HumanAddr>,
    ido_id: u32,
) -> HandleResult {
    assert_contract_active(&deps.storage)?;
    assert_ido_admin(deps, &env.message.sender, ido_id)?;

    let whitelist = state::ido_whitelist(ido_id);
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
    ido_id: u32,
) -> HandleResult {
    assert_contract_active(&deps.storage)?;
    assert_ido_admin(deps, &env.message.sender, ido_id)?;

    let whitelist = state::ido_whitelist(ido_id);

    for address in addresses {
        let canonical_address = deps.api.canonical_address(&address)?;
        whitelist.insert(&mut deps.storage, &canonical_address, &false)?;
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

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    let response = do_query(deps, msg);
    pad_query_result(response, BLOCK_SIZE)
}

fn do_query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    let response = match msg {
        QueryMsg::Config {} => {
            let config = Config::load(&deps.storage)?;
            config.to_answer(&deps.api)?
        }
        QueryMsg::IdoAmount {} => {
            let amount = Ido::len(&deps.storage)?;
            QueryAnswer::IdoAmount { amount }
        }
        QueryMsg::IdoInfo { ido_id } => {
            let ido = Ido::load(&deps.storage, ido_id)?;
            ido.to_answer(&deps.api)?
        }
        QueryMsg::InWhitelist { address, ido_id } => {
            let in_whitelist = utils::in_whitelist(deps, &address, ido_id)?;
            QueryAnswer::InWhitelist { in_whitelist }
        }
        QueryMsg::Whitelist {
            ido_id,
            start,
            limit,
        } => {
            let whitelist = state::ido_whitelist(ido_id);
            let canonical_addresses = whitelist.paging_keys(&deps.storage, start, limit)?;
            let mut addresses = Vec::with_capacity(canonical_addresses.len());

            for canonical_address in canonical_addresses {
                let address = deps.api.human_address(&canonical_address)?;
                addresses.push(address);
            }

            let amount = whitelist.get_len(&deps.storage)?;

            QueryAnswer::Whitelist { addresses, amount }
        }
        QueryMsg::IdoListOwnedBy {
            address,
            start,
            limit,
        } => {
            let canonical_address = deps.api.canonical_address(&address)?;
            let ido_list = state::ido_list_owned_by(&canonical_address);
            let amount = ido_list.get_len(&deps.storage)?;
            let ido_ids = ido_list.paging(&deps.storage, start, limit)?;

            QueryAnswer::IdoListOwnedBy { ido_ids, amount }
        }
        QueryMsg::Purchases {
            ido_id,
            address,
            start,
            limit,
        } => {
            let canonical_address = deps.api.canonical_address(&address)?;
            let purchases = state::purchases(&canonical_address, ido_id);
            let amount = purchases.get_len(&deps.storage)?;

            let raw_purchases = purchases.paging(&deps.storage, start, limit)?;
            let purchases = raw_purchases.into_iter().map(|p| p.to_answer()).collect();

            QueryAnswer::Purchases { purchases, amount }
        }
        QueryMsg::ArchivedPurchases {
            ido_id,
            address,
            start,
            limit,
        } => {
            let canonical_address = deps.api.canonical_address(&address)?;
            let purchases = state::archived_purchases(&canonical_address, ido_id);
            let amount = purchases.get_len(&deps.storage)?;

            let raw_purchases = purchases.paging(&deps.storage, start, limit)?;
            let purchases = raw_purchases.into_iter().map(|p| p.to_answer()).collect();

            QueryAnswer::ArchivedPurchases { purchases, amount }
        }
        QueryMsg::UserInfo { address, ido_id } => {
            let canonical_address = deps.api.canonical_address(&address)?;
            let user_info = if let Some(ido_id) = ido_id {
                let all_user_infos_in_ido = state::user_info_in_ido(&canonical_address);
                all_user_infos_in_ido.get(&deps.storage, &ido_id)
            } else {
                let all_user_infos = state::user_info();
                all_user_infos.get(&deps.storage, &canonical_address)
            }
            .unwrap_or_default();

            user_info.to_answer()
        }
    };

    to_binary(&response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{state::UserInfo, tier::manual};
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage},
        StdResult,
    };
    use rand::{thread_rng, Rng};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn get_init_msg() -> InitMsg {
        InitMsg {
            admin: None,
            tier_contract: HumanAddr::from("tier"),
            tier_contract_hash: String::from("tier_hash"),
            nft_contract: HumanAddr::from("nft"),
            nft_contract_hash: String::from("nft_hash"),
            lock_periods: vec![250, 200, 150, 100],
        }
    }

    fn initialize_with(msg: InitMsg) -> StdResult<Extern<MockStorage, MockApi, MockQuerier>> {
        let mut deps = mock_dependencies(20, &[]);
        let admin = HumanAddr::from("admin");
        let env = mock_env(admin, &[]);

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

        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let end_time = start_time + rng.gen::<u64>();

        let price = rng.gen();
        let total_amount = rng.gen();

        let mut whitelist = Vec::new();
        for i in 0..rng.gen_range(20..100) {
            let address = format!("address_{}", i);
            whitelist.push(HumanAddr(address));
        }

        let mut tokens_per_tier = Vec::new();
        let mut remaining_tokens = total_amount;
        for _ in 0..3 {
            let tokens_amount = rng.gen_range(0..=remaining_tokens);
            tokens_per_tier.push(Uint128(tokens_amount));
            remaining_tokens -= tokens_amount;
        }
        tokens_per_tier.push(Uint128(remaining_tokens));

        HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract: HumanAddr(token_contract),
            token_contract_hash,
            payment: PaymentMethod::Token {
                contract: HumanAddr::from("token"),
                code_hash: String::from("token_hash"),
            },
            price: Uint128(price),
            total_amount: Uint128(total_amount),
            whitelist: Whitelist::Empty {
                with: Some(whitelist),
            },
            tokens_per_tier,
            padding: None,
        }
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

    #[test]
    fn initialize() {
        let msg = get_init_msg();
        let deps = initialize_with(msg.clone()).unwrap();

        let config = Config::load(&deps.storage).unwrap();
        let admin = deps
            .api
            .canonical_address(&HumanAddr::from("admin"))
            .unwrap();

        let tier_contract = deps.api.canonical_address(&msg.tier_contract).unwrap();
        let nft_contract = deps.api.canonical_address(&msg.nft_contract).unwrap();
        let min_tier = manual::get_min_tier(&deps, &config).unwrap();

        assert_eq!(config.admin, admin);
        assert_eq!(config.lock_periods, msg.lock_periods);
        assert_eq!(config.tier_contract, tier_contract);
        assert_eq!(config.tier_contract_hash, msg.tier_contract_hash);
        assert_eq!(config.nft_contract, nft_contract);
        assert_eq!(config.nft_contract_hash, msg.nft_contract_hash);
        assert_eq!(config.min_tier, min_tier);
    }

    #[test]
    fn initialize_with_wrong_lock_periods() {
        let mut msg = get_init_msg();
        msg.lock_periods = vec![1, 2, 3];

        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("admin", &[]);

        let response = init(&mut deps, env, msg);
        let error = extract_error(response);

        assert!(error.contains("Lock periods array must have 4 items"));
    }

    #[test]
    fn change_admin() {
        let mut deps = initialize_with_default();
        let admin = HumanAddr::from("admin");
        let user = HumanAddr::from("user");
        let new_admin = HumanAddr::from("new_admin");

        let change_admin_msg = HandleMsg::ChangeAdmin {
            admin: new_admin.clone(),
            padding: None,
        };

        let env = mock_env(&user, &[]);
        let response = handle(&mut deps, env, change_admin_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let env = mock_env(&new_admin, &[]);
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
        let mut deps = initialize_with_default();
        let admin = HumanAddr::from("admin");
        let user = HumanAddr::from("user");

        let env = mock_env(&user, &[]);
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
    fn start_ido_wrong_date() {
        let mut deps = initialize_with_default();
        let ido_admin = HumanAddr::from("ido_admin");
        let env = mock_env(&ido_admin, &[]);

        for start_time in 1..10 {
            let start_ido_msg = HandleMsg::StartIdo {
                start_time,
                end_time: 1,
                token_contract: HumanAddr::from("token_contract"),
                token_contract_hash: String::new(),
                price: Uint128::from(1u128),
                payment: PaymentMethod::Native,
                total_amount: Uint128::from(100u128),
                padding: None,
                whitelist: Whitelist::Empty { with: None },
                tokens_per_tier: vec![100u128, 100, 100, 100]
                    .into_iter()
                    .map(Uint128::from)
                    .collect(),
            };

            let response = handle(&mut deps, env.clone(), start_ido_msg);
            let error = extract_error(response);
            assert!(error.contains("End time must be greater than start time"));
        }
    }

    #[test]
    fn start_ido_end_in_the_past() {
        let mut deps = initialize_with_default();
        let ido_admin = HumanAddr::from("ido_admin");

        let mut env = mock_env(&ido_admin, &[]);
        env.block.time = 10;

        for end_time in 1..10 {
            let start_ido_msg = HandleMsg::StartIdo {
                start_time: 0,
                end_time,
                token_contract: HumanAddr::from("token_contract"),
                token_contract_hash: String::new(),
                price: Uint128::from(1u128),
                payment: PaymentMethod::Native,
                total_amount: Uint128::from(100u128),
                padding: None,
                whitelist: Whitelist::Empty { with: None },
                tokens_per_tier: vec![100u128, 100, 100, 100]
                    .into_iter()
                    .map(Uint128::from)
                    .collect(),
            };

            let response = handle(&mut deps, env.clone(), start_ido_msg);
            let error = extract_error(response);
            assert!(error.contains("Ido ends in the past"));
        }
    }

    #[test]
    fn start_ido_with_empty_whitelist() {
        let mut deps = initialize_with_default();
        let ido_admin = HumanAddr::from("ido_admin");

        let mut env = mock_env(&ido_admin, &[]);
        env.block.time = 0;

        let allowed_addresses = (0..100)
            .map(|n| HumanAddr::from(format!("address_{}", n)))
            .collect::<Vec<_>>();

        let start_ido_msg = HandleMsg::StartIdo {
            start_time: 0,
            end_time: 1,
            token_contract: HumanAddr::from("token_contract"),
            token_contract_hash: String::new(),
            price: Uint128::from(1u128),
            payment: PaymentMethod::Native,
            total_amount: Uint128::from(100u128),
            padding: None,
            whitelist: Whitelist::Empty {
                with: Some(allowed_addresses.clone()),
            },
            tokens_per_tier: vec![100u128, 100, 100, 100]
                .into_iter()
                .map(Uint128::from)
                .collect(),
        };

        let response = handle(&mut deps, env, start_ido_msg).unwrap();
        let data = response.data.unwrap();
        let ido_id = match from_binary(&data).unwrap() {
            HandleAnswer::StartIdo { ido_id, .. } => ido_id,
            _ => unreachable!(),
        };

        assert_eq!(ido_id, 0);

        let ido = Ido::load(&deps.storage, ido_id).unwrap();
        assert!(!ido.shared_whitelist);

        for address in allowed_addresses {
            let in_whitelist = utils::in_whitelist(&deps, &address, ido_id).unwrap();
            assert!(in_whitelist);
        }

        let random_address = HumanAddr::from("random_address");
        let in_whitelist = utils::in_whitelist(&deps, &random_address, ido_id).unwrap();
        assert!(!in_whitelist);
    }

    #[test]
    fn start_ido_with_shared_whitelist() {
        let mut deps = initialize_with_default();
        let ido_admin = HumanAddr::from("ido_admin");

        let mut env = mock_env(&ido_admin, &[]);
        env.block.time = 0;

        let blocked_addresses = (0..100)
            .map(|n| HumanAddr::from(format!("address_{}", n)))
            .collect::<Vec<_>>();

        let start_ido_msg = HandleMsg::StartIdo {
            start_time: 0,
            end_time: 1,
            token_contract: HumanAddr::from("token_contract"),
            token_contract_hash: String::new(),
            price: Uint128::from(1u128),
            payment: PaymentMethod::Native,
            total_amount: Uint128::from(100u128),
            padding: None,
            whitelist: Whitelist::Shared {
                with_blocked: Some(blocked_addresses.clone()),
            },
            tokens_per_tier: vec![100u128, 100, 100, 100]
                .into_iter()
                .map(Uint128::from)
                .collect(),
        };

        let response = handle(&mut deps, env, start_ido_msg).unwrap();
        let data = response.data.unwrap();
        let ido_id = match from_binary(&data).unwrap() {
            HandleAnswer::StartIdo { ido_id, .. } => ido_id,
            _ => unreachable!(),
        };

        assert_eq!(ido_id, 0);

        let ido = Ido::load(&deps.storage, ido_id).unwrap();
        assert!(ido.shared_whitelist);

        for address in blocked_addresses {
            let in_whitelist = utils::in_whitelist(&deps, &address, ido_id).unwrap();
            assert!(!in_whitelist);
        }

        let random_address = HumanAddr::from("random_address");
        let in_whitelist = utils::in_whitelist(&deps, &random_address, ido_id).unwrap();
        assert!(in_whitelist);
    }

    #[test]
    fn start_ido() {
        let mut deps = initialize_with_default();

        let ido_admin = HumanAddr::from("ido_admin");
        let canonical_ido_admin = deps.api.canonical_address(&ido_admin).unwrap();
        let env = mock_env(&ido_admin, &[]);
        let msg = start_ido_msg();

        let startup_ido_list = state::ido_list_owned_by(&canonical_ido_admin);
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

        let startup_ido_list = state::ido_list_owned_by(&canonical_ido_admin);
        assert_eq!(startup_ido_list.get_len(&deps.storage), Ok(1));

        if let HandleMsg::StartIdo {
            start_time,
            end_time,
            token_contract,
            token_contract_hash,
            price,
            total_amount,
            whitelist,
            payment,
            ..
        } = msg
        {
            let sender = deps.api.canonical_address(&env.message.sender).unwrap();
            let token_contract_canonical = deps.api.canonical_address(&token_contract).unwrap();

            let payment_token_contract_canonical = match payment {
                PaymentMethod::Native => None,
                PaymentMethod::Token { contract, .. } => deps.api.canonical_address(&contract).ok(),
            };

            assert_eq!(ido.admin, sender);
            assert_eq!(ido.start_time, start_time);
            assert_eq!(ido.end_time, end_time);
            assert_eq!(ido.token_contract, token_contract_canonical);
            assert_eq!(ido.token_contract_hash, token_contract_hash);
            assert_eq!(ido.price, price.u128());
            assert_eq!(ido.participants, 0);
            assert_eq!(ido.sold_amount, 0);
            assert_eq!(ido.total_tokens_amount, total_amount.u128());
            assert_eq!(ido.payment_token_contract, payment_token_contract_canonical);
            assert_eq!(ido.payment_token_hash, Some(String::from("token_hash")));

            let whitelist_len = match whitelist {
                Whitelist::Empty { with } => with.map(|w| w.len()).unwrap_or(0),
                Whitelist::Shared { with_blocked } => with_blocked.map(|w| w.len()).unwrap_or(0),
            } as u32;

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

    fn change_tokens_per_tier(
        msg: &mut HandleMsg,
        mut tokens_per_tier: Option<Vec<u128>>,
    ) -> Vec<u128> {
        if tokens_per_tier.is_none() {
            let mut total_amount = match msg {
                HandleMsg::StartIdo { total_amount, .. } => total_amount.u128(),
                _ => unreachable!(),
            };

            let mut rng = thread_rng();
            let mut random_tokens_per_tier = Vec::with_capacity(5);

            for _ in 0..3 {
                let amount = if total_amount == 0 {
                    rng.gen_range(0..total_amount)
                } else {
                    0
                };

                random_tokens_per_tier.push(amount);
                total_amount -= amount;
            }

            random_tokens_per_tier.push(total_amount);
            tokens_per_tier.replace(random_tokens_per_tier);
        }

        let new_tokens_per_tier = tokens_per_tier.unwrap();

        if let HandleMsg::StartIdo {
            ref mut tokens_per_tier,
            ..
        } = msg
        {
            let new_tokens_per_tier = new_tokens_per_tier
                .clone()
                .into_iter()
                .map(Uint128)
                .collect::<Vec<_>>();

            tokens_per_tier.clear();
            tokens_per_tier.extend(&new_tokens_per_tier);
        }

        new_tokens_per_tier
    }

    #[test]
    fn start_ido_with_tokens_per_tier() {
        let mut deps = initialize_with_default();
        let mut msg = start_ido_msg();

        change_tokens_per_tier(&mut msg, Some(Vec::new()));
        let ido_admin = HumanAddr::from("ido_admin");
        let env = mock_env(&ido_admin, &[]);

        let response = handle(&mut deps, env.clone(), msg.clone());
        let error = extract_error(response);
        assert!(error.contains("`tokens_per_tier` has wrong size"));

        change_tokens_per_tier(&mut msg, Some(vec![1, 2, 3, 4]));
        let response = handle(&mut deps, env.clone(), msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Sum of `tokens_per_tier` can't be less than total tokens amount"));

        let tokens_per_tier = change_tokens_per_tier(&mut msg, None);
        let response = handle(&mut deps, env, msg).unwrap();
        let data = response.data.unwrap();
        let ido_id = match from_binary(&data).unwrap() {
            HandleAnswer::StartIdo { ido_id, .. } => ido_id,
            _ => unreachable!(),
        };

        let ido = Ido::load(&deps.storage, ido_id).unwrap();
        assert_eq!(ido.remaining_tokens_per_tier, tokens_per_tier);
    }

    #[test]
    fn buy_tokens_contract_not_active() {
        let mut deps = initialize_with_default();

        let mut config = Config::load(&deps.storage).unwrap();
        config.status = ContractStatus::Stopped as u8;
        config.save(&mut deps.storage).unwrap();

        let user = HumanAddr::from("user");
        let env = mock_env(&user, &[]);

        let mut ido = Ido::default();
        let ido_id = ido.save(&mut deps.storage).unwrap();

        let buy_tokens_msg = HandleMsg::BuyTokens {
            ido_id,
            amount: Uint128::from(100u128),
            token: None,
            padding: None,
        };

        let response = handle(&mut deps, env, buy_tokens_msg);
        let error = extract_error(response);
        assert!(error.contains("Contract is not active"));
    }

    #[test]
    fn buy_tokens_ido_not_active() {
        let mut deps = initialize_with_default();

        let user = HumanAddr::from("user");
        let mut env = mock_env(&user, &[]);
        env.block.time = 5;

        let token_contract = HumanAddr::from("token_contract");
        let canonical_token_contract = deps.api.canonical_address(&token_contract).unwrap();

        let mut ido = Ido::default();
        ido.start_time = 6;
        ido.end_time = 10;
        ido.payment_token_hash = Some(String::new());
        ido.payment_token_contract = Some(canonical_token_contract);
        let ido_id = ido.save(&mut deps.storage).unwrap();

        let buy_tokens_msg = HandleMsg::BuyTokens {
            ido_id,
            amount: Uint128::from(100u128),
            token: None,
            padding: None,
        };

        assert!(!ido.is_active(env.block.time));

        let response = handle(&mut deps, env, buy_tokens_msg);
        let error = extract_error(response);
        assert!(error.contains("IDO is not active"));
    }

    #[test]
    fn buy_tokens_all_sold() {
        let mut deps = initialize_with_default();

        let user = HumanAddr::from("user");
        let mut env = mock_env(&user, &coins(1, USCRT));
        env.block.time = 5;

        let mut ido = Ido::default();
        ido.start_time = 0;
        ido.end_time = 10;
        ido.total_tokens_amount = 100;
        ido.sold_amount = 100;
        ido.remaining_tokens_per_tier = vec![40, 30, 20, 10];
        let ido_id = ido.save(&mut deps.storage).unwrap();

        let buy_tokens_msg = HandleMsg::BuyTokens {
            ido_id,
            amount: Uint128::from(1u128),
            token: None,
            padding: None,
        };

        let response = handle(&mut deps, env.clone(), buy_tokens_msg.clone());
        let error = extract_error(response);

        assert!(error.contains("All tokens are sold"));
        assert_eq!(ido.remaining_tokens(), 0);

        ido.sold_amount = 0;
        ido.remaining_tokens_per_tier = vec![0, 0, 0, 0];
        ido.save(&mut deps.storage).unwrap();

        let response = handle(&mut deps, env, buy_tokens_msg);
        let error = extract_error(response);

        assert!(error.contains("All tokens are sold for your tier"));
        assert_eq!(ido.remaining_tokens(), 100);
        assert_eq!(ido.remaining_tokens_per_tier(1), 0);
    }

    #[test]
    fn buy_tokens_zero_amount() {
        let mut deps = initialize_with_default();

        let user = HumanAddr::from("user");
        let mut env = mock_env(&user, &[]);
        env.block.time = 5;

        let mut ido = Ido::default();
        ido.start_time = 0;
        ido.end_time = 10;
        ido.payment_token_hash = None;
        ido.payment_token_contract = None;
        let ido_id = ido.save(&mut deps.storage).unwrap();

        assert!(ido.is_native_payment());

        let buy_tokens_msg = HandleMsg::BuyTokens {
            ido_id,
            amount: Uint128::from(100u128),
            token: None,
            padding: None,
        };

        let response = handle(&mut deps, env.clone(), buy_tokens_msg);
        let error = extract_error(response);
        assert!(error.contains("Zero amount"));

        let buy_tokens_msg = HandleMsg::BuyTokens {
            ido_id,
            amount: Uint128::from(0u128),
            token: None,
            padding: None,
        };

        let token_contract = HumanAddr::from("token_contract");
        let canonical_token_contract = deps.api.canonical_address(&token_contract).unwrap();

        ido.payment_token_hash = Some(String::new());
        ido.payment_token_contract = Some(canonical_token_contract);
        ido.save(&mut deps.storage).unwrap();

        assert!(!ido.is_native_payment());

        let response = handle(&mut deps, env, buy_tokens_msg);
        let error = extract_error(response);
        assert!(error.contains("Zero amount"));
    }

    fn buy_tokens_blacklisted() {
        let mut deps = initialize_with_default();
        let user = HumanAddr::from("user");
        let canonical_user = deps.api.canonical_address(&user).unwrap();

        let mut env = mock_env(&user, &[]);
        env.block.time = 5;

        let token_contract = HumanAddr::from("token_contract");
        let canonical_token_contract = deps.api.canonical_address(&token_contract).unwrap();

        manual::set_tier(1);

        let mut ido = Ido::default();
        ido.shared_whitelist = false;
        ido.start_time = 0;
        ido.end_time = 10;
        ido.payment_token_hash = Some(String::new());
        ido.payment_token_contract = Some(canonical_token_contract);
        ido.total_tokens_amount = 90;
        ido.remaining_tokens_per_tier = vec![30, 30, 30, 0];

        let ido_id = ido.save(&mut deps.storage).unwrap();
        let buy_tokens_msg = HandleMsg::BuyTokens {
            ido_id,
            amount: Uint128::from(1u128),
            token: None,
            padding: None,
        };

        assert!(!utils::in_whitelist(&deps, &user, ido_id).unwrap());

        let response = handle(&mut deps, env.clone(), buy_tokens_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("All tokens are sold for your tier"));

        ido.shared_whitelist = true;
        ido.save(&mut deps.storage).unwrap();

        let whitelist = state::ido_whitelist(ido_id);
        whitelist
            .insert(&mut deps.storage, &canonical_user, &false)
            .unwrap();

        assert!(!utils::in_whitelist(&deps, &user, ido_id).unwrap());

        let response = handle(&mut deps, env, buy_tokens_msg);
        let error = extract_error(response);
        assert!(error.contains("All tokens are sold for your tier"));
    }

    fn buy_tokens_with_native_payment() {
        let init_msg = get_init_msg();
        let mut deps = initialize_with(init_msg).unwrap();

        let ido_admin = HumanAddr::from("ido_admin");
        let canonical_ido_admin = deps.api.canonical_address(&ido_admin).unwrap();

        let user = HumanAddr::from("user");

        let token_contract = HumanAddr::from("token_contract");
        let canonical_token_contract = deps.api.canonical_address(&token_contract).unwrap();

        let tokens_per_tier = vec![4, 3, 2, 1];

        let mut ido = Ido::default();
        ido.admin = canonical_ido_admin;
        ido.shared_whitelist = true;
        ido.start_time = 0;
        ido.end_time = 10;
        ido.payment_token_contract = None;
        ido.payment_token_hash = None;
        ido.total_tokens_amount = 100;
        ido.price = 2;
        ido.token_contract = canonical_token_contract;
        ido.remaining_tokens_per_tier = tokens_per_tier.clone();

        let ido_id = ido.save(&mut deps.storage).unwrap();

        for tier in (1..=4).rev() {
            manual::set_tier(tier);
            let tier_index = (tier - 1) as usize;
            let tokens_amount = tokens_per_tier.get(tier_index).unwrap();

            let mut env = mock_env(&user, &coins(*tokens_amount + 1, USCRT));
            env.block.time = 5;

            let buy_tokens_msg = HandleMsg::BuyTokens {
                ido_id,
                amount: Uint128::zero(),
                token: None,
                padding: None,
            };

            let response = handle(&mut deps, env.clone(), buy_tokens_msg.clone());
            let error = extract_error(response);
            assert!(error.contains(&format!(
                "You cannot buy more than {} tokens",
                tokens_amount
            )));

            let mut env = mock_env(&user, &coins(*tokens_amount, USCRT));
            env.block.time = 5;

            handle(&mut deps, env.clone(), buy_tokens_msg.clone()).unwrap();

            let mut env = mock_env(&user, &coins(1, USCRT));
            env.block.time = 5;

            let response = handle(&mut deps, env.clone(), buy_tokens_msg.clone());
            let error = extract_error(response);
            assert!(error.contains("All tokens are sold for your tier"));
        }
    }

    fn buy_tokens_state_check() {
        let init_msg = get_init_msg();
        let mut deps = initialize_with(init_msg.clone()).unwrap();

        let lock_periods = init_msg.lock_periods;

        let ido_admin = HumanAddr::from("ido_admin");
        let canonical_ido_admin = deps.api.canonical_address(&ido_admin).unwrap();

        let user = HumanAddr::from("user");
        let canonical_user = deps.api.canonical_address(&user).unwrap();

        let mut env = mock_env(&user, &[]);
        env.block.time = 5;

        let payment_token_hash = String::from("payment_token_hash");
        let payment_token_contract = HumanAddr::from("payment_contract");
        let canonical_payment_token_contract =
            deps.api.canonical_address(&payment_token_contract).unwrap();

        let token_contract = HumanAddr::from("token_contract");
        let canonical_token_contract = deps.api.canonical_address(&token_contract).unwrap();

        let mut ido = Ido::default();
        ido.admin = canonical_ido_admin;
        ido.shared_whitelist = true;
        ido.start_time = 0;
        ido.end_time = 10;
        ido.payment_token_contract = Some(canonical_payment_token_contract);
        ido.payment_token_hash = Some(payment_token_hash.clone());
        ido.total_tokens_amount = 100_000;
        ido.price = 2;
        ido.token_contract = canonical_token_contract;
        ido.remaining_tokens_per_tier = vec![40_000, 30_000, 20_000, 10_000];

        let ido_id = ido.save(&mut deps.storage).unwrap();

        let user_info_list = state::user_info();
        let user_info_in_ido_list = state::user_info_in_ido(&canonical_user);
        let config = Config::load(&deps.storage).unwrap();
        let min_tier = config.min_tier;

        for tier in (1..=4).rev() {
            manual::set_tier(tier);

            let tier_index = tier.checked_sub(1).unwrap() as usize;
            let max_tokens_amount = ido.remaining_tokens_per_tier[tier_index];

            let buy_too_much_tokens = HandleMsg::BuyTokens {
                ido_id,
                amount: Uint128::from(max_tokens_amount + 1),
                token: None,
                padding: None,
            };

            let response = handle(&mut deps, env.clone(), buy_too_much_tokens.clone());
            let error = extract_error(response);

            assert!(error.contains(&format!(
                "You cannot buy more than {} tokens",
                max_tokens_amount
            )));

            let buy_tokens_msg = HandleMsg::BuyTokens {
                ido_id,
                amount: Uint128::from(max_tokens_amount),
                token: None,
                padding: None,
            };

            let user_info = user_info_list
                .get(&deps.storage, &canonical_user)
                .unwrap_or_default();

            let initial_total_payment = user_info.total_payment;
            let initial_tokens_bought = user_info.total_tokens_bought;

            let response = handle(&mut deps, env.clone(), buy_tokens_msg.clone()).unwrap();
            let data = response.data.unwrap();

            match from_binary(&data).unwrap() {
                HandleAnswer::BuyTokens {
                    amount,
                    unlock_time,
                    status,
                } => {
                    assert_eq!(amount.u128(), max_tokens_amount);
                    assert_eq!(unlock_time, lock_periods[tier_index] + ido.end_time);
                    assert_eq!(status, ResponseStatus::Success);
                }
                _ => unreachable!(),
            }

            let messages = response.messages;
            assert_eq!(messages.len(), 1);
            assert_eq!(
                messages[0],
                transfer_from_msg(
                    user.clone(),
                    ido_admin.clone(),
                    Uint128::from(max_tokens_amount * ido.price),
                    None,
                    None,
                    BLOCK_SIZE,
                    payment_token_hash.clone(),
                    payment_token_contract.clone(),
                )
                .unwrap()
            );

            let user_info = user_info_list.get(&deps.storage, &canonical_user).unwrap();
            assert_eq!(user_info.total_tokens_received, 0);
            assert_eq!(
                user_info.total_tokens_bought,
                max_tokens_amount + initial_tokens_bought
            );
            assert_eq!(
                user_info.total_payment,
                max_tokens_amount * ido.price + initial_total_payment
            );

            let user_ido_info = user_info_in_ido_list.get(&deps.storage, &ido_id).unwrap();
            assert_eq!(user_info, user_ido_info);

            let purchases = state::purchases(&canonical_user, ido_id);
            let purchases_len = purchases.get_len(&deps.storage).unwrap();
            assert_eq!(purchases_len, (min_tier - tier + 1) as u32);

            let buy_another_token = HandleMsg::BuyTokens {
                ido_id,
                amount: Uint128::from(1u128),
                token: None,
                padding: None,
            };

            let response = handle(&mut deps, env.clone(), buy_another_token.clone());
            let error = extract_error(response);

            if tier == 1 {
                assert!(error.contains("All tokens are sold"));
            } else {
                assert!(error.contains("All tokens are sold for your tier"));
            }
        }
    }

    #[test]
    fn buy_tokens() {
        buy_tokens_blacklisted();
        buy_tokens_with_native_payment();
        buy_tokens_state_check();
    }

    #[test]
    fn whitelist_add() {
        let msg = get_init_msg();
        let mut deps = initialize_with(msg).unwrap();

        let unauthorized_user = HumanAddr::from("unauthorized");
        let env = mock_env(unauthorized_user, &[]);

        let address = HumanAddr::from("address");
        let canonical_address = deps.api.canonical_address(&address).unwrap();

        let add_ido_whitelist_msg = HandleMsg::WhitelistAdd {
            addresses: vec![address],
            ido_id: 0,
            padding: None,
        };

        let response = handle(&mut deps, env.clone(), add_ido_whitelist_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Not found"));

        let ido_admin = HumanAddr::from("ido_admin");
        let ido_admin_canonical = deps.api.canonical_address(&ido_admin).unwrap();

        let mut ido = Ido::default();
        ido.admin = ido_admin_canonical;

        ido.save(&mut deps.storage).unwrap();
        let response = handle(&mut deps, env, add_ido_whitelist_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let env = mock_env(ido_admin, &[]);
        handle(&mut deps, env, add_ido_whitelist_msg).unwrap();

        let ido_whitelist = state::ido_whitelist(0);
        assert_eq!(ido_whitelist.get_len(&deps.storage), Ok(1));
        assert_eq!(
            ido_whitelist.get(&deps.storage, &canonical_address),
            Some(true)
        );

        let whitelist_addresses = ido_whitelist.paging_keys(&deps.storage, 0, 100).unwrap();
        assert_eq!(whitelist_addresses, vec![canonical_address]);
    }

    #[test]
    fn whitelist_remove() {
        let mut deps = initialize_with_default();
        let start_ido_msg = start_ido_msg();

        let whitelist = match start_ido_msg {
            HandleMsg::StartIdo {
                whitelist: Whitelist::Empty { ref with },
                ..
            } => with.as_ref().unwrap().clone(),
            _ => unreachable!(),
        };

        let whitelist_len = whitelist.len() as u32;

        let ido_admin = HumanAddr::from("ido_admin");

        let env = mock_env(ido_admin.clone(), &[]);
        handle(&mut deps, env, start_ido_msg).unwrap();

        let remove_whitelist_msg = HandleMsg::WhitelistRemove {
            addresses: whitelist[10..20].to_vec(),
            ido_id: 0,
            padding: None,
        };

        let unauthorized_user = HumanAddr::from("unauthorized");

        let env = mock_env(unauthorized_user, &[]);
        let response = handle(&mut deps, env, remove_whitelist_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let env = mock_env(ido_admin, &[]);

        let response = handle(&mut deps, env, remove_whitelist_msg).unwrap();

        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::WhitelistRemove {
                whitelist_size,
                status,
            } => {
                assert_eq!(whitelist_size, whitelist_len);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        let ido_whitelist = state::ido_whitelist(0);
        for (index, address) in whitelist.iter().enumerate() {
            let canonical_address = deps.api.canonical_address(address).unwrap();
            let in_whitelist = !(10..20).contains(&index);

            assert_eq!(
                ido_whitelist.get(&deps.storage, &canonical_address),
                Some(in_whitelist)
            );
        }
    }

    fn generate_purchases(amount: usize) -> Vec<Purchase> {
        let mut rng = thread_rng();
        let mut purchases = Vec::with_capacity(amount);

        let purchase = Purchase {
            timestamp: 0,
            tokens_amount: rng.gen_range(0..10),
            unlock_time: rng.gen_range(1..500),
        };

        purchases.push(purchase);

        for _ in 1..purchases.capacity() {
            let purchase = Purchase {
                timestamp: 0,
                tokens_amount: rng.gen_range(0..10),
                unlock_time: rng.gen_range(1..1000),
            };

            purchases.push(purchase);
        }

        purchases
    }

    fn prepare_for_receive_tokens(
        purchases: &[Purchase],
    ) -> Extern<MockStorage, MockApi, MockQuerier> {
        let msg = get_init_msg();
        let mut deps = initialize_with(msg).unwrap();

        let token_contract = HumanAddr::from("token_contract");
        let canonical_token_contract = deps.api.canonical_address(&token_contract).unwrap();

        let mut ido = Ido::default();
        ido.token_contract = canonical_token_contract;
        let ido_id = ido.save(&mut deps.storage).unwrap();

        let user = HumanAddr::from("user");
        let canonical_user = deps.api.canonical_address(&user).unwrap();
        let user_purchases = state::purchases(&canonical_user, ido_id);
        for purchase in purchases.iter() {
            user_purchases
                .push_back(&mut deps.storage, purchase)
                .unwrap();
        }

        let total_tokens_amount = purchases.iter().map(|p| p.tokens_amount).sum();

        let info = UserInfo {
            total_payment: 0,
            total_tokens_bought: total_tokens_amount + 100,
            total_tokens_received: 100,
        };

        let ido_info = UserInfo {
            total_payment: 0,
            total_tokens_bought: total_tokens_amount,
            total_tokens_received: 0,
        };

        let user_info = state::user_info();
        let user_ido_info = state::user_info_in_ido(&canonical_user);

        user_info
            .insert(&mut deps.storage, &canonical_user, &info)
            .unwrap();

        user_ido_info
            .insert(&mut deps.storage, &ido_id, &ido_info)
            .unwrap();

        let active_ido_list = state::active_ido_list(&canonical_user);
        active_ido_list
            .insert(&mut deps.storage, &ido_id, &true)
            .unwrap();

        deps
    }

    #[test]
    fn recv_tokens() {
        let amount = 500;
        let purchases = generate_purchases(amount);
        let mut deps = prepare_for_receive_tokens(&purchases);

        let user = HumanAddr::from("user");
        let canonical_user = deps.api.canonical_address(&user).unwrap();

        let user_info = state::user_info_in_ido(&canonical_user);
        let info = user_info.get(&deps.storage, &0).unwrap();

        let total_tokens_amount = info.total_tokens_bought;
        let mut env = mock_env(user.clone(), &[]);
        env.block.time = 0;

        let recv_tokens_msg = HandleMsg::RecvTokens {
            ido_id: 0,
            start: None,
            limit: Some(amount as u32),
            purchase_indices: None,
            padding: None,
        };

        let active_ido_list = state::active_ido_list(&canonical_user);
        assert!(active_ido_list.contains(&deps.storage, &0));

        let response = handle(&mut deps, env.clone(), recv_tokens_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Nothing to receive"));

        let time = 500;
        env.block.time = time;

        let recv_amount = purchases
            .iter()
            .filter(|p| p.unlock_time <= time)
            .map(|p| p.tokens_amount)
            .sum();

        let response = handle(&mut deps, env.clone(), recv_tokens_msg.clone()).unwrap();
        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::RecvTokens { amount, status } => {
                assert_eq!(amount, Uint128(recv_amount));
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        let ido = Ido::load(&deps.storage, 0).unwrap();
        let token_contract = deps.api.human_address(&ido.token_contract).unwrap();
        let expected_message = transfer_msg(
            user.clone(),
            Uint128(recv_amount),
            None,
            None,
            BLOCK_SIZE,
            ido.token_contract_hash.clone(),
            token_contract.clone(),
        )
        .unwrap();

        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0], expected_message);

        let user_info = state::user_info_in_ido(&canonical_user);
        let info = user_info.get(&deps.storage, &0).unwrap();

        assert_eq!(info.total_tokens_bought, total_tokens_amount);
        assert_eq!(info.total_tokens_received, recv_amount);

        let active_ido_list = state::active_ido_list(&canonical_user);
        assert!(active_ido_list.contains(&deps.storage, &0));

        let user_purchases = state::purchases(&canonical_user, 0);
        let user_purchases_len = user_purchases.get_len(&deps.storage).unwrap();
        let user_purchases_iter = user_purchases.iter(&deps.storage).unwrap();

        for purchase in user_purchases_iter {
            assert!(time < purchase.unwrap().unlock_time);
        }

        let archived_purchases = state::archived_purchases(&canonical_user, 0);
        let archived_purchases_len = archived_purchases.get_len(&deps.storage).unwrap();
        let archived_purchases_iter = archived_purchases.iter(&deps.storage).unwrap();

        for purchase in archived_purchases_iter {
            assert!(time >= purchase.unwrap().unlock_time);
        }

        assert_eq!(
            user_purchases_len + archived_purchases_len,
            purchases.len() as u32
        );

        env.block.time = 1000;

        let response = handle(&mut deps, env, recv_tokens_msg).unwrap();
        let recv_amount = total_tokens_amount - recv_amount;

        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::RecvTokens { amount, status } => {
                assert_eq!(amount, Uint128(recv_amount));
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        let expected_message = transfer_msg(
            user,
            Uint128(recv_amount),
            None,
            None,
            BLOCK_SIZE,
            ido.token_contract_hash,
            token_contract,
        )
        .unwrap();

        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0], expected_message);

        let all_user_infos_in_ido = state::user_info_in_ido(&canonical_user);
        let user_ido_info = all_user_infos_in_ido.get(&deps.storage, &0).unwrap();

        assert_eq!(user_ido_info.total_tokens_bought, total_tokens_amount);
        assert_eq!(user_ido_info.total_tokens_received, total_tokens_amount);

        let all_user_infos = state::user_info();
        let user_info = all_user_infos.get(&deps.storage, &canonical_user).unwrap();

        // initially user had 100 bought and received tokens
        assert_eq!(user_info.total_tokens_bought, total_tokens_amount + 100);
        assert_eq!(user_info.total_tokens_received, total_tokens_amount + 100);

        let user_purchases = state::purchases(&canonical_user, 0);
        let archived_purchases = state::archived_purchases(&canonical_user, 0);
        assert_eq!(user_purchases.get_len(&deps.storage), Ok(0));
        assert_eq!(
            archived_purchases.get_len(&deps.storage),
            Ok(purchases.len() as u32)
        );

        let active_ido_list = state::active_ido_list(&canonical_user);
        assert!(!active_ido_list.contains(&deps.storage, &0));
    }

    #[test]
    fn recv_tokens_by_indices() {
        let amount = 20;
        let purchases = generate_purchases(amount);
        let mut deps = prepare_for_receive_tokens(&purchases);

        let user = HumanAddr::from("user");
        let canonical_user = deps.api.canonical_address(&user).unwrap();

        let mut env = mock_env(user.clone(), &[]);
        env.block.time = 1000;

        let mut purchase_indices = (0..10).into_iter().collect::<Vec<_>>();
        purchase_indices.extend(&[17, 18, 19]);

        let recv_tokens_msg = HandleMsg::RecvTokens {
            ido_id: 0,
            start: Some(4),
            limit: Some(10),
            purchase_indices: Some(purchase_indices),
            padding: None,
        };

        let recv_amount = purchases[0..14]
            .iter()
            .chain(purchases[17..].iter())
            .map(|p| p.tokens_amount)
            .sum();

        let response = handle(&mut deps, env, recv_tokens_msg).unwrap();
        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::RecvTokens { amount, status } => {
                assert_eq!(amount, Uint128(recv_amount));
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        let ido = Ido::load(&deps.storage, 0).unwrap();
        let token_contract = deps.api.human_address(&ido.token_contract).unwrap();
        let expected_message = transfer_msg(
            user,
            Uint128(recv_amount),
            None,
            None,
            BLOCK_SIZE,
            ido.token_contract_hash,
            token_contract,
        )
        .unwrap();

        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0], expected_message);

        let user_purchases = state::purchases(&canonical_user, 0);
        assert_eq!(user_purchases.get_len(&deps.storage), Ok(3));

        for (i, purchase) in purchases[14..17].iter().enumerate() {
            assert_eq!(
                user_purchases.get_at(&deps.storage, i as u32).unwrap(),
                *purchase
            );
        }
    }

    #[test]
    fn withdraw() {
        let msg = get_init_msg();
        let mut deps = initialize_with(msg).unwrap();

        let unauthorized_user = HumanAddr::from("unauthorized");
        let admin = HumanAddr::from("admin");
        let ido_admin = HumanAddr::from("ido_admin");
        let canonical_ido_admin = deps.api.canonical_address(&ido_admin).unwrap();

        let token_contract = HumanAddr::from("token_contract");
        let canonical_token_contract = deps.api.canonical_address(&token_contract).unwrap();

        let mut ido = Ido::default();
        ido.start_time = 100;
        ido.end_time = 1000;
        ido.admin = canonical_ido_admin;
        ido.total_tokens_amount = 100;
        ido.sold_amount = 30;
        ido.token_contract = canonical_token_contract;

        let withdraw_amount = ido.total_tokens_amount - ido.sold_amount;

        let ido_id = ido.save(&mut deps.storage).unwrap();
        let withdraw_msg = HandleMsg::Withdraw {
            ido_id,
            padding: None,
        };

        let env = mock_env(unauthorized_user, &[]);
        let response = handle(&mut deps, env, withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let env = mock_env(admin, &[]);
        let response = handle(&mut deps, env, withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("Unauthorized"));

        let mut env = mock_env(ido_admin.clone(), &[]);

        env.block.time = 0;
        let response = handle(&mut deps, env.clone(), withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("IDO is not finished yet"));

        env.block.time = 500;
        let response = handle(&mut deps, env.clone(), withdraw_msg.clone());
        let error = extract_error(response);
        assert!(error.contains("IDO is not finished yet"));

        env.block.time = 1000;
        let response = handle(&mut deps, env.clone(), withdraw_msg.clone()).unwrap();
        match from_binary(&response.data.unwrap()).unwrap() {
            HandleAnswer::Withdraw { amount, status } => {
                assert_eq!(amount, Uint128(withdraw_amount));
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        let expected_message = transfer_msg(
            ido_admin,
            Uint128(withdraw_amount),
            None,
            None,
            BLOCK_SIZE,
            ido.token_contract_hash,
            token_contract,
        )
        .unwrap();

        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0], expected_message);

        let response = handle(&mut deps, env, withdraw_msg);
        let error = extract_error(response);
        assert!(error.contains("Already withdrawn"));
    }

    #[test]
    fn withdraw_zero_tokens() {
        let msg = get_init_msg();
        let mut deps = initialize_with(msg).unwrap();

        let ido_admin = HumanAddr::from("ido_admin");
        let canonical_ido_admin = deps.api.canonical_address(&ido_admin).unwrap();

        let mut ido = Ido::default();
        ido.start_time = 100;
        ido.end_time = 1000;
        ido.admin = canonical_ido_admin;
        ido.total_tokens_amount = 100;
        ido.sold_amount = 100;

        let ido_id = ido.save(&mut deps.storage).unwrap();
        let withdraw_msg = HandleMsg::Withdraw {
            ido_id,
            padding: None,
        };

        let mut env = mock_env(ido_admin, &[]);
        env.block.time = 1000;

        let response = handle(&mut deps, env, withdraw_msg);
        let error = extract_error(response);
        assert!(error.contains("Nothing to withdraw"));
    }
}
