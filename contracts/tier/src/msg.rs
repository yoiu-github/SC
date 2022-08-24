use crate::state::Tier;
use cosmwasm_std::{HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InitMsg {
    pub owner: Option<HumanAddr>,
    pub validator: HumanAddr,
    pub deposits: Vec<Uint128>,
    pub lock_periods: Vec<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Deposit {
        padding: Option<String>,
    },
    Withdraw {
        padding: Option<String>,
    },
    Claim {
        recipient: Option<HumanAddr>,
        padding: Option<String>,
    },
    WithdrawRewards {
        recipient: Option<HumanAddr>,
        padding: Option<String>,
    },
    Redelegate {
        validator_address: HumanAddr,
        recipient: Option<HumanAddr>,
        padding: Option<String>,
    },
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
    TierInfo {},
    TierOf { address: HumanAddr },
    DepositOf { address: HumanAddr },
    WhenCanWithdraw { address: HumanAddr },
    WhenCanClaim { address: HumanAddr },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    TierInfo {
        owner: HumanAddr,
        validator: HumanAddr,
        tier_list: Vec<Tier>,
    },
    TierOf {
        tier: u8,
    },
    DepositOf {
        deposit: Uint128,
    },
    CanClaim {
        time: Option<u64>,
    },
    CanWithdraw {
        time: Option<u64>,
    },
}
