use crate::state::{Config, Ido};
use cosmwasm_std::{Api, Env, StdError, StdResult};

pub fn assert_owner<A: Api>(api: &A, env: &Env, config: &Config) -> StdResult<()> {
    let sender = api.canonical_address(&env.message.sender)?;
    if sender != config.owner {
        return Err(StdError::unauthorized());
    }

    Ok(())
}

pub fn assert_ido_owner<A: Api>(api: &A, env: &Env, ido: &Ido) -> StdResult<()> {
    let sender = api.canonical_address(&env.message.sender)?;
    if sender != ido.owner {
        return Err(StdError::unauthorized());
    }

    Ok(())
}
