use cosmwasm_std::{CanonicalAddr, ReadonlyStorage, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &[u8] = b"config";
pub const USER_PREFIX: &[u8] = b"user";
pub const TIER_PREFIX: &[u8] = b"tier";
pub const TIER_LEN_PREFIX: &[u8] = b"tier_len";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub validator: CanonicalAddr,
}

impl Config {
    pub fn load<S: ReadonlyStorage>(storage: &S) -> StdResult<Self> {
        singleton_read(storage, CONFIG_KEY).load()
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        singleton(storage, CONFIG_KEY).save(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Tier {
    #[serde(skip)]
    pub index: u8,
    pub deposit: Uint128,
    pub lock_period: u32,
}

impl Tier {
    pub fn set_len<S: Storage>(storage: &mut S, length: u8) -> StdResult<()> {
        singleton(storage, TIER_LEN_PREFIX).save(&[length])
    }

    pub fn len<S: ReadonlyStorage>(storage: &S) -> StdResult<u8> {
        let bytes: Vec<u8> = singleton_read(storage, TIER_LEN_PREFIX).load()?;
        let len_bytes = bytes.as_slice().try_into().unwrap();
        Ok(u8::from_le_bytes(len_bytes))
    }

    pub fn load<S: ReadonlyStorage>(storage: &S, index: u8) -> StdResult<Self> {
        let mut tier_state: Tier = bucket_read(TIER_PREFIX, storage).load(&[index])?;
        tier_state.index = index;
        Ok(tier_state)
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        bucket(TIER_PREFIX, storage).save(&[self.index], self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserState {
    Deposit,
    Withdraw,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct User {
    #[serde(skip)]
    pub address: CanonicalAddr,
    pub state: UserState,
    pub deposit_amount: Uint128,
    pub deposit_time: u64,
    pub withdraw_time: Option<u64>,
}

impl User {
    pub fn load<S: ReadonlyStorage>(storage: &S, address: &CanonicalAddr) -> StdResult<Self> {
        let mut user: User = bucket_read(USER_PREFIX, storage).load(address.as_slice())?;
        user.address = address.clone();
        Ok(user)
    }

    pub fn may_load<S: ReadonlyStorage>(
        storage: &S,
        address: &CanonicalAddr,
    ) -> StdResult<Option<Self>> {
        let mut user_option: Option<User> =
            bucket_read(USER_PREFIX, storage).may_load(address.as_slice())?;

        if let Some(user) = user_option.as_mut() {
            user.address = address.clone();
        }

        Ok(user_option)
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        bucket(USER_PREFIX, storage).save(self.address.as_slice(), self)
    }

    pub fn remove<S: Storage>(self, storage: &mut S) {
        bucket::<S, User>(USER_PREFIX, storage).remove(self.address.as_slice());
    }
}
