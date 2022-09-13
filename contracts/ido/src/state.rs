use cosmwasm_std::{CanonicalAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket};
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &[u8] = b"c";
pub const WHITELIST_PREFIX: &[u8] = b"w";

pub const IDO_PREFIX: &[u8] = b"id";
pub const IDO_LEN_PREFIX: &[u8] = b"il";

pub const PURCHASES_LIST_PREFIX: &[u8] = b"l";
pub const PURCHASE_PREFIX: &[u8] = b"p";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub lock_period: u64,
    pub tier_contract: CanonicalAddr,
    pub tier_contract_hash: String,
    pub nft_contract: CanonicalAddr,
    pub nft_contract_hash: String,
    pub token_contract: CanonicalAddr,
    pub token_contract_hash: String,
    pub max_payments: Vec<Uint128>,
}

impl Config {
    pub fn load<S: ReadonlyStorage>(storage: &S) -> StdResult<Self> {
        singleton_read(storage, CONFIG_KEY).load()
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        singleton(storage, CONFIG_KEY).save(self)
    }
}

pub struct Whitelist {}

impl Whitelist {
    pub fn add<S: Storage>(storage: &mut S, address: &CanonicalAddr) -> StdResult<()> {
        if Whitelist::contains(storage, address)? {
            Err(StdError::generic_err("Address already in whitelist"))
        } else {
            bucket(WHITELIST_PREFIX, storage).save(address.as_slice(), &true)
        }
    }

    pub fn remove<S: Storage>(storage: &mut S, address: &CanonicalAddr) -> StdResult<()> {
        if Whitelist::contains(storage, address)? {
            bucket::<S, bool>(WHITELIST_PREFIX, storage).remove(address.as_slice());
            Ok(())
        } else {
            Err(StdError::generic_err("Address not found"))
        }
    }

    pub fn contains<S: ReadonlyStorage>(storage: &S, address: &CanonicalAddr) -> StdResult<bool> {
        let result: Option<bool> =
            bucket_read(WHITELIST_PREFIX, storage).may_load(address.as_slice())?;

        Ok(result == Some(true))
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Purchase {
    #[serde(skip)]
    payment: Uint128,
    pub tokens_amount: Uint128,
    pub payment_time: u64,
}

impl Purchase {
    pub fn new(payment: u128, tokens_amount: u128, payment_time: u64) -> Self {
        Purchase {
            payment: Uint128(payment),
            tokens_amount: Uint128(tokens_amount),
            payment_time,
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Purchases {
    #[serde(skip)]
    ido_id: u64,
    #[serde(skip)]
    investor: CanonicalAddr,

    total_payment: Uint128,
    index_from: u64,
    index_to: u64,
}

impl Purchases {
    pub fn load<S: ReadonlyStorage>(
        storage: &S,
        investor: CanonicalAddr,
        ido_id: u64,
    ) -> StdResult<Self> {
        Ido::load(storage, ido_id)?;

        let bucket =
            ReadonlyBucket::multilevel(&[PURCHASES_LIST_PREFIX, &ido_id.to_le_bytes()], storage);

        let purchases: Option<Self> = bucket.may_load(investor.as_slice())?;
        let mut purchases = purchases.unwrap_or_default();

        purchases.investor = investor;
        purchases.ido_id = ido_id;

        Ok(purchases)
    }

    fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        let mut bucket = Bucket::multilevel(
            &[PURCHASES_LIST_PREFIX, &self.ido_id.to_le_bytes()],
            storage,
        );

        bucket.save(self.investor.as_slice(), self)
    }

    pub fn total_payment(&self) -> u128 {
        self.total_payment.u128()
    }

    pub fn len(&self) -> u64 {
        self.index_to.checked_sub(self.index_from).unwrap()
    }

    pub fn is_new_participant(&self) -> bool {
        self.index_to == 0
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get<S: ReadonlyStorage>(&self, index: u64, storage: &S) -> StdResult<Purchase> {
        let bucket = ReadonlyBucket::multilevel(
            &[
                PURCHASE_PREFIX,
                &self.ido_id.to_le_bytes(),
                self.investor.as_slice(),
            ],
            storage,
        );

        let index = index.checked_add(self.index_from).unwrap();
        bucket.load(&index.to_le_bytes())
    }

    pub fn add<S: Storage>(&mut self, purchase: &Purchase, storage: &mut S) -> StdResult<()> {
        let mut bucket = Bucket::multilevel(
            &[
                PURCHASE_PREFIX,
                &self.ido_id.to_le_bytes(),
                self.investor.as_slice(),
            ],
            storage,
        );

        bucket.save(&self.index_to.to_le_bytes(), purchase)?;

        let previous_payments = self.total_payment.u128();
        let new_payment = purchase.payment.u128();

        self.index_to = self.index_to.checked_add(1).unwrap();
        self.total_payment = Uint128(previous_payments.checked_add(new_payment).unwrap());

        self.save(storage)
    }

    pub fn remove<S: Storage>(&mut self, storage: &mut S, amount: u64) -> StdResult<()> {
        if amount == 0 {
            return Ok(());
        }

        let len = self.len();
        if amount > len {
            let msg = format!("You cannot remove more than {} elements", len);
            return Err(StdError::generic_err(&msg));
        }

        let mut bucket: Bucket<S, Purchase> = Bucket::multilevel(
            &[
                PURCHASE_PREFIX,
                &self.ido_id.to_le_bytes(),
                self.investor.as_slice(),
            ],
            storage,
        );

        for _ in 0..amount {
            bucket.remove(&self.index_from.to_le_bytes());
            self.index_from = self.index_from.checked_add(1).unwrap();
        }

        self.save(storage)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Ido {
    #[serde(skip)]
    id: Option<u64>,
    pub owner: CanonicalAddr,
    pub start_time: u64,
    pub end_time: u64,
    pub token_contract: CanonicalAddr,
    pub token_contract_hash: String,
    pub price: Uint128,
    pub participants: u64,
    pub sold_amount: Uint128,
    pub total_tokens_amount: Uint128,
    pub total_payment: Uint128,
    pub withdrawn: bool,
}

impl Ido {
    pub fn load<S: ReadonlyStorage>(storage: &S, id: u64) -> StdResult<Self> {
        let mut ido: Ido = bucket_read(IDO_PREFIX, storage).load(&id.to_le_bytes())?;
        ido.id = Some(id);

        Ok(ido)
    }

    pub fn is_active(&self, time: u64) -> bool {
        time >= self.start_time && time < self.end_time
    }

    pub fn remaining_amount(&self) -> u128 {
        self.total_tokens_amount
            .u128()
            .checked_sub(self.sold_amount.u128())
            .unwrap()
    }

    pub fn id(&self) -> Option<u64> {
        self.id
    }

    pub fn save<S: Storage>(&mut self, storage: &mut S) -> StdResult<u64> {
        if self.id.is_none() {
            let id = Ido::len(storage)?;
            self.id = Some(id);

            let new_len = id.checked_add(1).unwrap();
            Ido::set_len(storage, new_len)?;
        }

        let id = self.id.unwrap();
        bucket(IDO_PREFIX, storage)
            .save(&id.to_le_bytes(), self)
            .map(|_| id)
    }

    fn set_len<S: Storage>(storage: &mut S, len: u64) -> StdResult<()> {
        singleton(storage, IDO_LEN_PREFIX).save(&len.to_le_bytes())
    }

    pub fn len<S: ReadonlyStorage>(storage: &S) -> StdResult<u64> {
        let bytes: Option<Vec<u8>> = singleton_read(storage, IDO_LEN_PREFIX).may_load()?;
        if let Some(bytes) = bytes {
            let len_bytes = bytes.as_slice().try_into().unwrap();
            Ok(u64::from_le_bytes(len_bytes))
        } else {
            Ok(0)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api, HumanAddr};

    #[test]
    fn whitelist() {
        let deps = mock_dependencies(20, &[]);
        let mut storage = deps.storage;
        let address = HumanAddr::from("address");
        let canonical_address = deps.api.canonical_address(&address).unwrap();

        assert_eq!(Whitelist::contains(&storage, &canonical_address), Ok(false));

        Whitelist::add(&mut storage, &canonical_address).unwrap();
        assert_eq!(Whitelist::contains(&storage, &canonical_address), Ok(true));
        assert!(Whitelist::add(&mut storage, &canonical_address).is_err());

        Whitelist::remove(&mut storage, &canonical_address).unwrap();
        assert_eq!(Whitelist::contains(&storage, &canonical_address), Ok(false));

        assert!(Whitelist::remove(&mut storage, &canonical_address).is_err());
    }

    #[test]
    fn ido() {
        let deps = mock_dependencies(20, &[]);
        let mut storage = deps.storage;

        assert_eq!(Ido::len(&storage), Ok(0));

        let loaded_ido = Ido::load(&storage, 0);
        assert!(loaded_ido.is_err());

        let token_address = HumanAddr::from("token");
        let canonical_token_address = deps.api.canonical_address(&token_address).unwrap();

        let mut new_ido = Ido {
            start_time: 100,
            end_time: 150,
            token_contract: canonical_token_address,
            price: Uint128(100),
            total_tokens_amount: Uint128(1000),
            ..Ido::default()
        };

        assert_eq!(new_ido.id(), None);
        assert_eq!(Ido::len(&storage), Ok(0));

        new_ido.save(&mut storage).unwrap();
        assert_eq!(new_ido.id(), Some(0));
        assert_eq!(Ido::len(&storage), Ok(1));

        new_ido.save(&mut storage).unwrap();
        assert_eq!(new_ido.id(), Some(0));
        assert_eq!(Ido::len(&storage), Ok(1));

        let mut loaded_ido = Ido::load(&storage, 0).unwrap();
        assert_eq!(new_ido, loaded_ido);

        loaded_ido.save(&mut storage).unwrap();
        assert_eq!(new_ido, loaded_ido);
        assert_eq!(loaded_ido.id(), Some(0));
        assert_eq!(Ido::len(&storage), Ok(1));

        loaded_ido.id = None;
        loaded_ido.save(&mut storage).unwrap();
        assert_eq!(loaded_ido.id(), Some(1));
        assert_eq!(Ido::len(&storage), Ok(2));
    }

    #[test]
    fn purchases() {
        let deps = mock_dependencies(20, &[]);
        let mut storage = deps.storage;

        let investor = deps.api.canonical_address(&HumanAddr::from("001")).unwrap();
        assert!(Purchases::load(&storage, investor.clone(), 0).is_err());

        let mut first_iso = Ido::default();
        let mut second_ido = Ido::default();
        let first_ido_id = first_iso.save(&mut storage).unwrap();
        let second_ido_id = second_ido.save(&mut storage).unwrap();

        let mut purchases = Purchases::load(&storage, investor.clone(), first_ido_id).unwrap();

        assert!(purchases.is_empty());
        assert!(purchases.remove(&mut storage, 1).is_err());

        for i in 0..10 {
            let purchase = Purchase {
                payment: Uint128(10 * i),
                payment_time: i as u64,
                tokens_amount: Uint128(i + 100),
            };

            purchases.add(&purchase, &mut storage).unwrap();
            assert_eq!(purchases.len(), 1 + i as u64);
            assert_eq!(purchases.total_payment(), 5 * i * (i + 1));
            assert!(!purchases.is_empty());
        }

        assert!(purchases.remove(&mut storage, 0).is_ok());
        assert!(purchases.remove(&mut storage, 11).is_err());
        assert_eq!(purchases.len(), 10);

        for i in 0..10 {
            let loaded_purchase = purchases.get(i, &storage).unwrap();
            assert_eq!(loaded_purchase.tokens_amount, Uint128(100 + i as u128));
            assert_eq!(loaded_purchase.payment_time, i);
        }

        assert!(purchases.get(10, &storage).is_err());

        purchases.remove(&mut storage, 3).unwrap();
        assert!(purchases.get(7, &storage).is_err());
        assert!(purchases.get(8, &storage).is_err());
        assert!(purchases.get(9, &storage).is_err());

        let loaded_purchase = purchases.get(0, &storage).unwrap();
        assert_eq!(loaded_purchase.tokens_amount, Uint128(103));
        assert_eq!(loaded_purchase.payment_time, 3);

        assert_eq!(purchases.len(), 7);
        assert_eq!(purchases.index_from, 3);
        assert_eq!(purchases.index_to, 10);

        let loaded_purchases = Purchases::load(&storage, investor.clone(), first_ido_id).unwrap();
        assert_eq!(loaded_purchases, purchases);

        let purchases_second_ido = Purchases::load(&storage, investor, second_ido_id).unwrap();
        assert_eq!(purchases_second_ido.len(), 0);
        assert!(purchases_second_ido.is_empty());
        assert!(purchases_second_ido.get(3, &storage).is_err());
    }
}
