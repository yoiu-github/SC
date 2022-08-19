use cosmwasm_std::{CanonicalAddr, Storage, Uint128};
use cosmwasm_storage::{
    bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton,
    Singleton,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &[u8] = b"config";
pub const USER_PREFIX: &[u8] = b"user";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub validator: CanonicalAddr,
    pub deposits: Vec<Uint128>,
    pub lock_periods: Vec<u32>,
}

pub fn config<S: Storage>(storage: &mut S) -> Singleton<S, Config> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, Config> {
    singleton_read(storage, CONFIG_KEY)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserState {
    Deposit,
    Withdraw,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct User {
    pub state: UserState,
    pub deposit_amount: Uint128,
    pub deposit_time: u64,
    pub withdraw_time: Option<u64>,
}

pub fn users<S: Storage>(storage: &mut S) -> Bucket<S, User> {
    bucket(USER_PREFIX, storage)
}

pub fn users_read<S: Storage>(storage: &S) -> ReadonlyBucket<S, User> {
    bucket_read(USER_PREFIX, storage)
}
