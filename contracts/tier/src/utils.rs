use crate::state::Config;
use cosmwasm_std::{Api, Env, StdError, StdResult};

pub fn assert_owner<A: Api>(api: &A, env: &Env, config: &Config) -> StdResult<()> {
    let owner = api.human_address(&config.owner)?;
    if env.message.sender != owner {
        return Err(StdError::generic_err("Authorization error"));
    }

    Ok(())
}

pub fn get_deposit(env: &Env) -> StdResult<u128> {
    let mut funds = 0;
    for coin in &env.message.sent_funds {
        if coin.denom != "uscrt" {
            return Err(StdError::generic_err("Unsopported token as a payment"));
        }

        funds = funds.checked_add(coin.amount.0).unwrap();
    }

    Ok(funds)
}
