use cosmwasm_std::{CanonicalAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket};
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &[u8] = b"config";
pub const IDO_LEN_KEY: &[u8] = b"idolen";

pub const PREFIX_WHITELIST: &[u8] = b"whitelist";
pub const PREFIX_ID_TO_IDO: &[u8] = b"id2ido";
pub const PREFIX_INVESTOR_MAP: &[u8] = b"inv2ids";
pub const PREFIX_ID_TO_INVESTOR_PURCHASES: &[u8] = b"id2ps";

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
            bucket(PREFIX_WHITELIST, storage).save(address.as_slice(), &true)
        }
    }

    pub fn remove<S: Storage>(storage: &mut S, address: &CanonicalAddr) -> StdResult<()> {
        if Whitelist::contains(storage, address)? {
            bucket::<S, bool>(PREFIX_WHITELIST, storage).remove(address.as_slice());
            Ok(())
        } else {
            Err(StdError::generic_err("Address not found"))
        }
    }

    pub fn contains<S: ReadonlyStorage>(storage: &S, address: &CanonicalAddr) -> StdResult<bool> {
        let result = bucket_read(PREFIX_WHITELIST, storage).may_load(address.as_slice())?;
        Ok(result == Some(true))
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct IdoWhitelist {}

impl IdoWhitelist {
    pub fn add<S: Storage>(storage: &mut S, ido_id: u64, address: &CanonicalAddr) -> StdResult<()> {
        if IdoWhitelist::contains(storage, ido_id, address)? {
            Err(StdError::generic_err("Address already in whitelist"))
        } else {
            Ido::load(storage, ido_id)?;

            let mut bucket =
                Bucket::multilevel(&[PREFIX_WHITELIST, &ido_id.to_le_bytes()], storage);

            bucket.save(address.as_slice(), &true)
        }
    }

    pub fn remove<S: Storage>(
        storage: &mut S,
        ido_id: u64,
        address: &CanonicalAddr,
    ) -> StdResult<()> {
        if IdoWhitelist::contains(storage, ido_id, address)? {
            let mut bucket =
                Bucket::<S, bool>::multilevel(&[PREFIX_WHITELIST, &ido_id.to_le_bytes()], storage);

            bucket.remove(address.as_slice());
            Ok(())
        } else {
            Err(StdError::generic_err("Address not found"))
        }
    }

    pub fn contains<S: ReadonlyStorage>(
        storage: &S,
        ido_id: u64,
        address: &CanonicalAddr,
    ) -> StdResult<bool> {
        let bucket =
            ReadonlyBucket::multilevel(&[PREFIX_WHITELIST, &ido_id.to_le_bytes()], storage);
        let result = bucket.may_load(address.as_slice())?;

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

        let bucket = ReadonlyBucket::multilevel(
            &[PREFIX_ID_TO_INVESTOR_PURCHASES, &ido_id.to_le_bytes()],
            storage,
        );

        let purchases: Option<Self> = bucket.may_load(investor.as_slice())?;
        let mut purchases = purchases.unwrap_or_default();

        purchases.investor = investor;
        purchases.ido_id = ido_id;

        Ok(purchases)
    }

    fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        let mut bucket = Bucket::multilevel(
            &[PREFIX_ID_TO_INVESTOR_PURCHASES, &self.ido_id.to_le_bytes()],
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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get<S: ReadonlyStorage>(&self, index: u64, storage: &S) -> StdResult<Purchase> {
        let bucket = ReadonlyBucket::multilevel(
            &[
                PREFIX_ID_TO_INVESTOR_PURCHASES,
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
                PREFIX_ID_TO_INVESTOR_PURCHASES,
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
                PREFIX_ID_TO_INVESTOR_PURCHASES,
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
pub struct InvestorInfo {
    #[serde(skip)]
    address: CanonicalAddr,

    ido_amount: u64,
}

impl InvestorInfo {
    pub fn load<S: ReadonlyStorage>(storage: &S, address: CanonicalAddr) -> StdResult<Self> {
        let investor_info: Option<Self> =
            bucket_read(PREFIX_INVESTOR_MAP, storage).may_load(address.as_slice())?;

        let mut investor_info = investor_info.unwrap_or_default();
        investor_info.address = address;

        Ok(investor_info)
    }

    pub fn ido_amount(&self) -> u64 {
        self.ido_amount
    }

    fn save<S: Storage>(&mut self, storage: &mut S) -> StdResult<()> {
        bucket(PREFIX_INVESTOR_MAP, storage).save(self.address.as_slice(), self)
    }

    pub fn add_ido_id<S: Storage>(&mut self, storage: &mut S, ido_id: u64) -> StdResult<()> {
        let next_index = self.ido_amount;
        let mut bucket =
            Bucket::multilevel(&[PREFIX_INVESTOR_MAP, self.address.as_slice()], storage);
        bucket.save(&next_index.to_le_bytes(), &ido_id)?;

        self.ido_amount = self.ido_amount.checked_add(1).unwrap();
        self.save(storage)
    }

    pub fn get_ido_id<S: ReadonlyStorage>(&self, storage: &S, index: u64) -> StdResult<u64> {
        let bucket =
            ReadonlyBucket::multilevel(&[PREFIX_INVESTOR_MAP, self.address.as_slice()], storage);
        bucket.load(&index.to_le_bytes())
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
        let mut ido: Ido = bucket_read(PREFIX_ID_TO_IDO, storage).load(&id.to_le_bytes())?;
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
        bucket(PREFIX_ID_TO_IDO, storage)
            .save(&id.to_le_bytes(), self)
            .map(|_| id)
    }

    fn set_len<S: Storage>(storage: &mut S, len: u64) -> StdResult<()> {
        singleton(storage, IDO_LEN_KEY).save(&len)
    }

    pub fn len<S: ReadonlyStorage>(storage: &S) -> StdResult<u64> {
        let len: Option<u64> = singleton_read(storage, IDO_LEN_KEY).may_load()?;
        Ok(len.unwrap_or(0))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api, HumanAddr};
    use rand::{thread_rng, Rng};

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
    fn ido_whitelist() {
        let deps = mock_dependencies(20, &[]);
        let mut storage = deps.storage;
        let address = HumanAddr::from("address");
        let canonical_address = deps.api.canonical_address(&address).unwrap();

        let contains = IdoWhitelist::contains(&storage, 0, &canonical_address);
        assert_eq!(contains, Ok(false));

        assert!(IdoWhitelist::add(&mut storage, 0, &canonical_address).is_err());
        let mut ido = Ido::default();
        ido.save(&mut storage).unwrap();

        IdoWhitelist::add(&mut storage, 0, &canonical_address).unwrap();
        let contains = IdoWhitelist::contains(&storage, 0, &canonical_address);
        assert_eq!(contains, Ok(true));
        assert!(IdoWhitelist::add(&mut storage, 0, &canonical_address).is_err());

        IdoWhitelist::remove(&mut storage, 0, &canonical_address).unwrap();
        let contains = IdoWhitelist::contains(&storage, 0, &canonical_address);
        assert_eq!(contains, Ok(false));

        assert!(IdoWhitelist::remove(&mut storage, 0, &canonical_address).is_err());
    }

    #[test]
    fn investor_info() {
        let deps = mock_dependencies(20, &[]);
        let mut storage = deps.storage;

        let address = HumanAddr::from("investor");
        let canonical_address = deps.api.canonical_address(&address).unwrap();
        let mut investor_info = InvestorInfo::load(&storage, canonical_address.clone()).unwrap();

        assert_eq!(investor_info.ido_amount(), 0);
        assert!(investor_info.get_ido_id(&storage, 0).is_err());

        let mut rng = thread_rng();
        let ido_id = rng.gen();
        investor_info.add_ido_id(&mut storage, ido_id).unwrap();

        assert_eq!(investor_info.ido_amount(), 1);
        assert_eq!(investor_info.get_ido_id(&storage, 0), Ok(ido_id));

        let new_ido_id = rng.gen();
        investor_info.add_ido_id(&mut storage, new_ido_id).unwrap();

        assert_eq!(investor_info.ido_amount(), 2);
        assert_eq!(investor_info.get_ido_id(&storage, 0), Ok(ido_id));
        assert_eq!(investor_info.get_ido_id(&storage, 1), Ok(new_ido_id));

        let loaded_investor_info = InvestorInfo::load(&storage, canonical_address).unwrap();
        assert_eq!(investor_info, loaded_investor_info);
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
