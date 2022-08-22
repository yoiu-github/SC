use crate::state::Config;
use cosmwasm_std::{Api, Env, FullDelegation, HumanAddr, Querier, StdError, StdResult};

pub fn assert_owner<A: Api>(api: &A, env: &Env, config: &Config) -> StdResult<()> {
    let owner = api.human_address(&config.owner)?;
    if env.message.sender != owner {
        return Err(StdError::unauthorized());
    }

    Ok(())
}

pub fn check_validator<Q: Querier>(querier: &Q, validator: &HumanAddr) -> StdResult<()> {
    let validators = querier.query_validators()?;
    let has_validator = validators.iter().any(|v| v.address == *validator);
    if !has_validator {
        return Err(StdError::generic_err(&format!(
            "Validator {} not found",
            validator
        )));
    }

    Ok(())
}

pub fn get_deposit(env: &Env) -> StdResult<u128> {
    let mut funds: u128 = 0;
    for coin in &env.message.sent_funds {
        if coin.denom != "uscrt" {
            return Err(StdError::generic_err("Unsopported token"));
        }

        funds = funds.checked_add(coin.amount.u128()).unwrap();
    }

    Ok(funds)
}

pub fn query_delegation<Q: Querier>(
    querier: &Q,
    env: &Env,
    validator: &HumanAddr,
) -> StdResult<FullDelegation> {
    match querier.query_delegation(&env.contract.address, validator)? {
        Some(delegation) => Ok(delegation),
        None => Err(StdError::generic_err("Cannot query delegation")),
    }
}
