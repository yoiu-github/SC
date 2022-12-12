use crate::{
    msg::{ContractStatus, QueryAnswer, SerializedTierInfo, SerializedWithdrawals},
    utils::normalize_tier,
};
use cosmwasm_std::{
    CanonicalAddr, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128,
};
use schemars::JsonSchema;
use secret_toolkit_storage::{AppendStore, DequeStore, Item, Keymap};
use serde::{Deserialize, Serialize};

static CONFIG_ITEM: Item<Config> = Item::new(b"config");
static WITHDRAWALS_LIST: DequeStore<UserWithdrawal> = DequeStore::new(b"withdraw");

pub fn tier_info_list() -> AppendStore<'static, TierInfo> {
    AppendStore::new(b"tier")
}

pub fn user_infos() -> Keymap<'static, CanonicalAddr, UserInfo> {
    Keymap::new(b"user_info")
}

pub fn withdrawals_list(address: &CanonicalAddr) -> DequeStore<'static, UserWithdrawal> {
    WITHDRAWALS_LIST.add_suffix(address.as_slice())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub admin: CanonicalAddr,
    pub validator: HumanAddr,
    pub status: u8,
    pub band_oracle: HumanAddr,
    pub band_code_hash: String,
}

impl Config {
    pub fn load<S: ReadonlyStorage>(storage: &S) -> StdResult<Self> {
        CONFIG_ITEM.load(storage)
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        CONFIG_ITEM.save(storage, self)
    }

    pub fn assert_contract_active(&self) -> StdResult<()> {
        let active = ContractStatus::Active as u8;
        if self.status != active {
            return Err(StdError::generic_err("Contract is not active"));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TierInfo {
    pub deposit: u128,
    pub lock_period: u64,
}

impl TierInfo {
    pub fn to_serialized(&self) -> SerializedTierInfo {
        SerializedTierInfo {
            deposit: Uint128(self.deposit),
            lock_period: self.lock_period,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserInfo {
    pub tier: u8,
    pub deposit: u128,
    pub withdraw_time: u64,
    pub timestamp: u64,
}

impl UserInfo {
    pub fn to_answer<S: ReadonlyStorage>(&self, storage: &S) -> StdResult<QueryAnswer> {
        let length = tier_info_list().get_len(storage)?;

        Ok(QueryAnswer::UserInfo {
            tier: normalize_tier(self.tier, length as u8),
            deposit: Uint128(self.deposit),
            withdraw_time: self.withdraw_time,
            timestamp: self.timestamp,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserWithdrawal {
    pub amount: u128,
    pub claim_time: u64,
    pub timestamp: u64,
}

impl UserWithdrawal {
    pub fn to_serialized(&self) -> SerializedWithdrawals {
        SerializedWithdrawals {
            amount: Uint128(self.amount),
            claim_time: self.claim_time,
            timestamp: self.timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api, HumanAddr};

    #[test]
    fn config() {
        let mut deps = mock_dependencies(20, &[]);
        let owner = HumanAddr::from("owner");
        let validator = HumanAddr::from("validator");

        let mut config = Config {
            status: ContractStatus::Stopped as u8,
            admin: deps.api.canonical_address(&owner).unwrap(),
            validator,
            band_oracle: "band_oracle".into(),
            band_code_hash: String::new(),
        };
        assert!(config.assert_contract_active().is_err());

        config.status = ContractStatus::Active as u8;
        assert!(config.assert_contract_active().is_ok());

        config.save(&mut deps.storage).unwrap();

        let loaded_config = Config::load(&deps.storage).unwrap();
        assert_eq!(config, loaded_config);
    }
}
