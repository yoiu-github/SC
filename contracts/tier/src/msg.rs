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
    Claim { recipient: Option<HumanAddr> },
    WithdrawRewards { recipient: Option<HumanAddr> },
    Redelegate { validator_address: HumanAddr },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    Deposit { status: ResponseStatus },
    Withdraw { status: ResponseStatus },
    Claim { status: ResponseStatus },
    WithdrawRewards { status: ResponseStatus },
    Redelegate { status: ResponseStatus },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    TierOf { address: HumanAddr },
    DepositOf { address: HumanAddr },
    WhenCanWithdraw { address: HumanAddr },
    WhenCanClaim { address: HumanAddr },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    TierOf { tier: u8 },
    DepositOf { deposit: Uint128 },
    CanClaim { time: Option<u64> },
    CanWithdraw { time: Option<u64> },
}
