use crate::{
    msg::ContractStatus,
    state::{Config, Ido},
};
use cosmwasm_std::{
    Api, Extern, HumanAddr, Querier, ReadonlyStorage, StdError, StdResult, Storage,
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
