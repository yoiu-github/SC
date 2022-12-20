use crate::{
    msg::ContractStatus,
    state::{self, Config, Ido},
};
use cosmwasm_std::{
    Api, Coin, Extern, HumanAddr, Querier, ReadonlyStorage, StdError, StdResult, Storage,
};

pub fn assert_contract_active<S: ReadonlyStorage>(storage: &S) -> StdResult<()> {
    let config = Config::load(storage)?;
    let active_status = ContractStatus::Active as u8;

    if config.status != active_status {
        return Err(StdError::generic_err("Contract is not active"));
    }

    Ok(())
}

pub fn assert_admin<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: &HumanAddr,
) -> StdResult<()> {
    let canonical_admin = deps.api.canonical_address(address)?;
    let config = Config::load(&deps.storage)?;

    if config.admin != canonical_admin {
        return Err(StdError::unauthorized());
    }

    Ok(())
}

pub fn assert_ido_admin<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: &HumanAddr,
    ido_id: u32,
) -> StdResult<()> {
    let canonical_admin = deps.api.canonical_address(address)?;
    let ido = Ido::load(&deps.storage, ido_id)?;

    if ido.admin != canonical_admin {
        return Err(StdError::unauthorized());
    }

    Ok(())
}

pub fn in_whitelist<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: &HumanAddr,
    ido_id: u32,
) -> StdResult<bool> {
    let canonical_address = deps.api.canonical_address(address)?;

    let ido_whitelist = state::ido_whitelist(ido_id);
    let whitelist_status = ido_whitelist.get(&deps.storage, &canonical_address);

    match whitelist_status {
        Some(value) => Ok(value),
        None => {
            let ido = Ido::load(&deps.storage, ido_id)?;
            Ok(ido.shared_whitelist)
        }
    }
}

pub fn sent_funds(coins: &[Coin]) -> StdResult<u128> {
    let mut amount: u128 = 0;

    for coin in coins {
        if coin.denom != "uscrt" {
            return Err(StdError::generic_err("Unsopported token"));
        }

        amount = amount.checked_add(coin.amount.u128()).unwrap();
    }

    Ok(amount)
}
