use cosmwasm_std::{CanonicalAddr, HumanAddr, ReadonlyStorage, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &[u8] = b"config";
pub const USER_PREFIX: &[u8] = b"user";
pub const TIER_PREFIX: &[u8] = b"tier";
pub const TIER_LEN_PREFIX: &[u8] = b"tier_len";
pub const UNBOUND_LATENCY: u64 = 21 * 24 * 60 * 60;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub validator: HumanAddr,
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
    pub lock_period: u64,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserState {
    Deposit,
    Withdraw,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

    pub fn tier<S: ReadonlyStorage>(&self, storage: &S) -> StdResult<u8> {
        let length = Tier::len(storage)?;
        let mut tier = 0;

        if self.state == UserState::Withdraw {
            return Ok(0);
        }

        for index in 0..length {
            let tier_state = Tier::load(storage, index)?;
            let deposit = self.deposit_amount.u128();
            if deposit < tier_state.deposit.u128() {
                break;
            } else {
                tier = index.checked_add(1).unwrap();
            }
        }

        Ok(tier)
    }

    pub fn can_withdraw_at<S: ReadonlyStorage>(&self, storage: &S) -> StdResult<u64> {
        let tier = self.tier(storage)?;
        if tier == 0 {
            return Ok(self.deposit_time);
        }

        let tier_index = tier.checked_sub(1).unwrap();
        let tier_state = Tier::load(storage, tier_index)?;
        Ok(self
            .deposit_time
            .checked_add(tier_state.lock_period)
            .unwrap())
    }

    pub fn can_claim_at(&self) -> Option<u64> {
        self.withdraw_time
            .map(|w| w.checked_add(UNBOUND_LATENCY).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use cosmwasm_std::{testing::mock_dependencies, Api, HumanAddr};

    #[test]
    fn config() {
        let mut deps = mock_dependencies(20, &[]);
        let owner = HumanAddr::from("owner");
        let validator = HumanAddr::from("validator");

        let config = Config {
            owner: deps.api.canonical_address(&owner).unwrap(),
            validator,
        };

        config.save(&mut deps.storage).unwrap();

        let loaded_config = Config::load(&deps.storage).unwrap();
        assert_eq!(config, loaded_config);
    }

    #[test]
    fn tier() {
        let mut deps = mock_dependencies(20, &[]);
        let index = 3;
        let tier = Tier {
            index,
            deposit: Uint128(100),
            lock_period: 2,
        };

        tier.save(&mut deps.storage).unwrap();

        let loaded_tier = Tier::load(&deps.storage, index).unwrap();
        assert_eq!(tier, loaded_tier);

        for i in 0..index {
            assert!(Tier::load(&deps.storage, i).is_err());
        }
    }

    #[test]
    fn tier_len() {
        let mut deps = mock_dependencies(20, &[]);
        assert!(Tier::len(&deps.storage).is_err());

        for index in 0..5 {
            Tier::set_len(&mut deps.storage, index).unwrap();
            assert_eq!(Tier::len(&deps.storage).unwrap(), index);
        }
    }

    #[test]
    fn user() {
        let mut deps = mock_dependencies(20, &[]);
        let address = HumanAddr::from("user");
        let canonical_address = deps.api.canonical_address(&address).unwrap();
        let deposit_amount = Uint128(1234);
        let deposit_time = 1_534_342_300;
        let withdraw_time = None;

        assert!(User::may_load(&deps.storage, &canonical_address)
            .unwrap()
            .is_none());

        let user = User {
            address: canonical_address.clone(),
            state: UserState::Deposit,
            deposit_amount,
            deposit_time,
            withdraw_time,
        };

        user.save(&mut deps.storage).unwrap();

        let loaded_user = User::load(&deps.storage, &canonical_address).unwrap();
        assert_eq!(user, loaded_user);

        let loaded_user = User::may_load(&deps.storage, &canonical_address).unwrap();
        assert_eq!(user, loaded_user.unwrap());
    }

    #[test]
    fn user_tier() {
        let mut deps = mock_dependencies(20, &[]);
        let length = 3;
        Tier::set_len(&mut deps.storage, length).unwrap();

        let day = 24u64 * 60 * 60;
        for index in 0..length {
            let tier = Tier {
                index,
                deposit: Uint128(10 * (index + 1) as u128),
                lock_period: day * (index as u64 + 1),
            };

            tier.save(&mut deps.storage).unwrap();
        }
        let address = HumanAddr::from("user");
        let canonical_address = deps.api.canonical_address(&address).unwrap();

        let mut user = User {
            address: canonical_address,
            state: UserState::Deposit,
            deposit_amount: Uint128(0),
            deposit_time: 0,
            withdraw_time: None,
        };

        let mut expected_time = Utc.ymd(1970, 1, 1).and_hms(0, 0, 0).timestamp();
        for i in 0..10 {
            user.deposit_amount = Uint128(i);
            let can_withdraw_at = user.can_withdraw_at(&deps.storage).unwrap();

            assert_eq!(user.tier(&deps.storage), Ok(0));
            assert_eq!(can_withdraw_at, user.deposit_time);
            assert_eq!(can_withdraw_at, expected_time as u64);
        }

        expected_time = Utc.ymd(1970, 1, 2).and_hms(0, 0, 0).timestamp();
        for i in 10..20 {
            user.deposit_amount = Uint128(i);
            let can_withdraw_at = user.can_withdraw_at(&deps.storage).unwrap();

            assert_eq!(user.tier(&deps.storage), Ok(1));
            assert_eq!(can_withdraw_at, expected_time as u64);
        }

        expected_time = Utc.ymd(1970, 1, 3).and_hms(0, 0, 0).timestamp();
        for i in 20..30 {
            user.deposit_amount = Uint128(i);
            let can_withdraw_at = user.can_withdraw_at(&deps.storage).unwrap();

            assert_eq!(user.tier(&deps.storage), Ok(2));
            assert_eq!(can_withdraw_at, expected_time as u64);
        }

        expected_time = Utc.ymd(1970, 1, 4).and_hms(0, 0, 0).timestamp();
        for i in 30..100 {
            user.deposit_amount = Uint128(i);
            let can_withdraw_at = user.can_withdraw_at(&deps.storage).unwrap();

            assert_eq!(user.tier(&deps.storage), Ok(3));
            assert_eq!(can_withdraw_at, expected_time as u64);
        }
    }

    #[test]
    fn user_claim_time() {
        let deps = mock_dependencies(20, &[]);
        let address = HumanAddr::from("user");
        let canonical_address = deps.api.canonical_address(&address).unwrap();

        // 21 days difference
        let withdraw_time = Utc.ymd(1985, 5, 3).and_hms(18, 30, 4).timestamp();
        let expected_claim_time = Utc.ymd(1985, 5, 24).and_hms(18, 30, 4).timestamp();

        let user = User {
            address: canonical_address,
            state: UserState::Deposit,
            deposit_amount: Uint128(0),
            deposit_time: 0,
            withdraw_time: Some(withdraw_time as u64),
        };

        assert_eq!(user.can_claim_at().unwrap(), expected_claim_time as u64);
    }
}
