use crate::{
    contract::USCRT,
    state::{self, Config},
};
use cosmwasm_std::{
    Api, Coin, Empty, Env, FullDelegation, HumanAddr, Querier, QueryRequest, ReadonlyStorage,
    StakingQuery, StdError, StdResult,
};
use serde::Deserialize;

pub fn assert_admin<A: Api>(api: &A, env: &Env, config: &Config) -> StdResult<()> {
    let owner = api.human_address(&config.admin)?;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FixedDelegationResponse {
    pub delegation: Option<FixedFullDelegation>,
}

#[derive(Debug, Deserialize)]
pub struct FixedFullDelegation {
    pub delegator: HumanAddr,
    pub validator: HumanAddr,
    pub amount: Coin,
    pub can_redelegate: Coin,
    pub accumulated_rewards: Vec<Coin>,
}

impl From<FixedFullDelegation> for FullDelegation {
    fn from(val: FixedFullDelegation) -> Self {
        let found_rewards = val
            .accumulated_rewards
            .into_iter()
            .find(|r| r.denom == USCRT);

        let accumulated_rewards = found_rewards.unwrap_or_else(|| Coin::new(0, USCRT));
        FullDelegation {
            delegator: val.delegator,
            validator: val.validator,
            amount: val.amount,
            can_redelegate: val.can_redelegate,
            accumulated_rewards,
        }
    }
}

pub fn query_delegation<Q: Querier>(
    querier: &Q,
    env: &Env,
    validator: &HumanAddr,
) -> StdResult<Option<FullDelegation>> {
    let delegation_request = StakingQuery::Delegation {
        delegator: env.contract.address.clone(),
        validator: validator.clone(),
    };

    let request: QueryRequest<Empty> = QueryRequest::Staking(delegation_request);
    let response: StdResult<FixedDelegationResponse> = querier.custom_query(&request);

    let delegation = match response {
        Ok(response) => response.delegation.map(Into::into),
        _ => querier.query_delegation(&env.contract.address, validator)?,
    };

    Ok(delegation)
}

pub fn get_tier_by_deposit<S: ReadonlyStorage>(storage: &S, deposit: u128) -> StdResult<u8> {
    let tier_list = state::tier_info_list();
    let length = tier_list.get_len(storage)? as u8;

    let mut tier = 0;
    for index in 0..length {
        let tier_state = tier_list.get_at(storage, index.into())?;
        if deposit < tier_state.deposit {
            break;
        } else {
            tier = index.checked_add(1).unwrap();
        }
    }

    Ok(tier)
}

// Input:  0 1 2 3 4
// Output: 0 4 3 2 1
pub fn normalize_tier(tier: u8, length: u8) -> u8 {
    if tier == 0 {
        return 0;
    }

    length
        .checked_add(1)
        .and_then(|v| v.checked_sub(tier))
        .unwrap()
}
