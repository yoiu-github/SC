use cosmwasm_std::{HumanAddr, Uint128};
use schemars::JsonSchema;
use secret_toolkit_utils::Query;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TierTokenQuery {
    TierOf { token_id: String },
}

impl Query for TierTokenQuery {
    const BLOCK_SIZE: usize = 256;
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TierContractQuery {
    TierOf { address: HumanAddr },
}

impl Query for TierContractQuery {
    const BLOCK_SIZE: usize = 256;
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierReponse {
    TierOf { tier: u8 },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InitMsg {
    pub max_payments: Vec<Uint128>,
    pub tier_contract: HumanAddr,
    pub tier_contract_hash: String,
    pub nft_contract: HumanAddr,
    pub nft_contract_hash: String,
    pub token_contract: HumanAddr,
    pub token_contract_hash: String,
    pub lock_period: u64,
    pub whitelist: Option<Vec<HumanAddr>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    StartIdo {
        start_time: u64,
        end_time: u64,
        token_contract: HumanAddr,
        token_contract_hash: String,
        price: Uint128,
        total_amount: Uint128,
    },
    WhitelistAdd {
        addresses: Vec<HumanAddr>,
    },
    WhitelistRemove {
        addresses: Vec<HumanAddr>,
    },
    BuyTokens {
        ido_id: u64,
        amount: Uint128,
        token_id: Option<String>,
    },
    RecvTokens {
        ido_id: u64,
    },
    Withdraw {
        ido_id: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    StartIdo {
        ido_id: u64,
        status: ResponseStatus,
    },
    WhitelistAdd {
        status: ResponseStatus,
    },
    WhitelistRemove {
        status: ResponseStatus,
    },
    BuyTokens {
        amount: Uint128,
        status: ResponseStatus,
    },
    RecvTokens {
        amount: Uint128,
        status: ResponseStatus,
    },
    Withdraw {
        amount: Uint128,
        status: ResponseStatus,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    IdoInfo { ido_id: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    IdoInfo {
        ido_id: u64,
        start_time: u64,
        end_time: u64,
        token: HumanAddr,
        price: Uint128,
        total_amount: Uint128,
        participants: u64,
    },
}
