use cosmwasm_std::{HumanAddr, StdError, StdResult, Uint128};
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
    pub lock_periods: Vec<u64>,
    pub tier_contract: HumanAddr,
    pub tier_contract_hash: String,
    pub nft_contract: HumanAddr,
    pub nft_contract_hash: String,
    pub token_contract: HumanAddr,
    pub token_contract_hash: String,
    pub whitelist: Option<Vec<HumanAddr>>,
}

impl InitMsg {
    pub fn check(&self) -> StdResult<()> {
        if self.max_payments.is_empty() {
            return Err(StdError::generic_err("Specify max payments array"));
        }

        let is_sorted = self.max_payments.as_slice().windows(2).all(|v| v[0] < v[1]);
        if !is_sorted {
            return Err(StdError::generic_err(
                "Specify max payments in increasing order",
            ));
        }

        if self.max_payments.len() != self.lock_periods.len() {
            return Err(StdError::generic_err("Arrays have different size"));
        }

        Ok(())
    }
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
        whitelist: Option<Vec<HumanAddr>>,
    },
    WhitelistAdd {
        addresses: Vec<HumanAddr>,
        ido_id: Option<u32>,
    },
    WhitelistRemove {
        addresses: Vec<HumanAddr>,
        ido_id: Option<u32>,
    },
    BuyTokens {
        ido_id: u32,
        amount: Uint128,
        token_id: Option<String>,
    },
    RecvTokens {
        ido_id: u32,
        start: Option<u32>,
        limit: Option<u32>,
        purchase_indices: Option<Vec<u32>>,
    },
    Withdraw {
        ido_id: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    StartIdo {
        ido_id: u32,
        whitelist_size: u32,
        status: ResponseStatus,
    },
    WhitelistAdd {
        whitelist_size: u32,
        status: ResponseStatus,
    },
    WhitelistRemove {
        whitelist_size: u32,
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
    Config {},
    IdoAmount {},
    IdoInfo {
        ido_id: u32,
    },
    WhitelistAmount {
        ido_id: Option<u32>,
    },
    Whitelist {
        ido_id: Option<u32>,
        start: u32,
        limit: u32,
    },
    IdoAmountOwnedBy {
        address: HumanAddr,
    },
    IdoListOwnedBy {
        address: HumanAddr,
        start: u32,
        limit: u32,
    },
    PurchasesAmount {
        address: HumanAddr,
    },
    Purchases {
        address: HumanAddr,
        start: u32,
        limit: u32,
    },
    ArchivedPurchasesAmount {
        address: HumanAddr,
    },
    ArchivedPurchases {
        address: HumanAddr,
        start: u32,
        limit: u32,
    },
    UserInfo {
        address: HumanAddr,
        ido_id: Option<u32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PurchaseAnswer {
    pub tokens_amount: Uint128,
    pub timestamp: u64,
    pub unlock_time: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    Config {
        owner: HumanAddr,
        tier_contract: HumanAddr,
        tier_contract_hash: String,
        nft_contract: HumanAddr,
        nft_contract_hash: String,
        token_contract: HumanAddr,
        token_contract_hash: String,
        max_payments: Vec<Uint128>,
        lock_periods: Vec<u64>,
    },
    IdoAmount {
        amount: u32,
    },
    IdoInfo {
        owner: HumanAddr,
        start_time: u64,
        end_time: u64,
        token_contract: HumanAddr,
        token_contract_hash: String,
        price: Uint128,
        participants: u64,
        sold_amount: Uint128,
        total_tokens_amount: Uint128,
        total_payment: Uint128,
        withdrawn: bool,
    },
    WhitelistAmount {
        amount: u32,
    },
    Whitelist {
        addresses: Vec<HumanAddr>,
    },
    IdoAmountOwnedBy {
        amount: u32,
    },
    IdoListOwnedBy {
        ido_ids: Vec<u32>,
    },
    PurchasesAmount {
        amount: u32,
    },
    Purchases {
        purchases: Vec<PurchaseAnswer>,
    },
    ArchivedPurchasesAmount {
        amount: u32,
    },
    ArchivedPurchases {
        purchases: Vec<PurchaseAnswer>,
    },
    UserInfo {
        total_payment: Uint128,
        total_tokens_bought: Uint128,
        total_tokens_received: Uint128,
    },
}
