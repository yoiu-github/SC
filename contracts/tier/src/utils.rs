use crate::state::Config;
use chrono::{TimeZone, Utc};
use chronoutil::shift_months;
use cosmwasm_std::{Api, Env, FullDelegation, HumanAddr, Querier, StdError, StdResult};

pub const UNBOUND_LATENCY: u64 = 21 * 24 * 60 * 60;

pub fn assert_owner<A: Api>(api: &A, env: &Env, config: &Config) -> StdResult<()> {
    let owner = api.human_address(&config.owner)?;
    if env.message.sender != owner {
        return Err(StdError::unauthorized());
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

pub fn withdraw_time(deposit_time: u64, months: u32) -> u64 {
    let months = months.try_into().unwrap();
    let start_datetime = Utc.timestamp(deposit_time as i64, 0);
    let end_datetime = shift_months(start_datetime, months);
    end_datetime.timestamp() as u64
}

pub fn claim_time(withdraw_time: u64) -> u64 {
    withdraw_time.checked_add(UNBOUND_LATENCY).unwrap()
}
