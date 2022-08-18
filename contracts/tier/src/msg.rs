use cosmwasm_std::{HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub owner: Option<HumanAddr>,
    pub validator: HumanAddr,
    pub deposits: Vec<Uint128>,
    pub lock_periods: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Deposit,
    Withdraw,
    Claim,
    WithdrawRewards,
    Redelegate { validator_address: HumanAddr },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    Deposit { status: ResponseStatus },
    Withdraw { status: ResponseStatus },
    UpdateValidator { status: ResponseStatus },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    TierOf { address: HumanAddr },
    TierInfo { tier: u8 },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    TierOf { tier: u8 },
    TierInfo { deposit: Uint128, months: u32 },
}
