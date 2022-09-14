use cosmwasm_std::{CanonicalAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const CONFIG_KEY: &[u8] = b"config";
pub const IDO_LEN_KEY: &[u8] = b"idolen";
pub const WHITELIST_KEY: &[u8] = b"whitelist";

pub const PREFIX_WHITELIST_APPEND_STORE: &[u8] = b"app2wh";
pub const PREFIX_INVESTOR_TO_WHITELIST_INDEX: &[u8] = b"in2idx";
pub const PREFIX_ID_TO_IDO: &[u8] = b"id2ido";
pub const PREFIX_ID_TO_INVESTOR_PURCHASES: &[u8] = b"id2ps";
pub const PREFIX_INVESTOR_MAP: &[u8] = b"inv2ids";

pub type IdoSize = u64;
pub type WhitelistSize = u32;
pub type PurchasesSize = u64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub tier_contract: CanonicalAddr,
    pub tier_contract_hash: String,
    pub nft_contract: CanonicalAddr,
    pub nft_contract_hash: String,
    pub token_contract: CanonicalAddr,
    pub token_contract_hash: String,
    pub max_payments: Vec<Uint128>,
    pub lock_periods: Vec<u64>,
}

impl Config {
    pub fn load<S: ReadonlyStorage>(storage: &S) -> StdResult<Self> {
        singleton_read(storage, CONFIG_KEY).load()
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        singleton(storage, CONFIG_KEY).save(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Whitelist {
    #[serde(skip)]
    ido_id: Option<IdoSize>,

    len: WhitelistSize,
}

impl Whitelist {
    pub fn load<S: ReadonlyStorage>(storage: &S, ido_id: Option<IdoSize>) -> StdResult<Self> {
        let whitelist: Option<Self> = if let Some(ido_id) = ido_id {
            Ido::load(storage, ido_id)?;
            bucket_read(WHITELIST_KEY, storage).may_load(&ido_id.to_le_bytes())?
        } else {
            singleton_read(storage, WHITELIST_KEY).may_load()?
        };

        Ok(Whitelist {
            ido_id,
            len: whitelist.map(|w| w.len).unwrap_or(0),
        })
    }

    pub fn len(&self) -> WhitelistSize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        if let Some(ido_id) = self.ido_id {
            bucket(WHITELIST_KEY, storage).save(&ido_id.to_le_bytes(), self)
        } else {
            singleton(storage, WHITELIST_KEY).save(self)
        }
    }

    fn bucket<'s, 'storage, S, T>(&'s self, storage: &'storage mut S, prefix: &[u8]) -> Bucket<S, T>
    where
        'storage: 's,
        S: Storage,
        T: Serialize + DeserializeOwned,
    {
        if let Some(ido_id) = self.ido_id {
            Bucket::multilevel(&[prefix, &ido_id.to_le_bytes()], storage)
        } else {
            Bucket::new(prefix, storage)
        }
    }

    fn bucket_read<'s, 'storage, S, T>(
        &'s self,
        storage: &'storage S,
        prefix: &[u8],
    ) -> ReadonlyBucket<S, T>
    where
        'storage: 's,
        S: ReadonlyStorage,
        T: Serialize + DeserializeOwned,
    {
        if let Some(ido_id) = self.ido_id {
            ReadonlyBucket::multilevel(&[prefix, &ido_id.to_le_bytes()], storage)
        } else {
            ReadonlyBucket::new(prefix, storage)
        }
    }

    pub fn get<S: ReadonlyStorage>(
        &self,
        storage: &S,
        index: WhitelistSize,
    ) -> StdResult<CanonicalAddr> {
        let bucket = self.bucket_read(storage, PREFIX_WHITELIST_APPEND_STORE);
        bucket.load(&index.to_le_bytes())
    }

    pub fn add_unchecked<S: Storage>(
        &mut self,
        storage: &mut S,
        address: &CanonicalAddr,
    ) -> StdResult<()> {
        let index = self.len;

        let mut bucket = self.bucket(storage, PREFIX_WHITELIST_APPEND_STORE);
        bucket.save(&index.to_le_bytes(), address)?;

        let mut bucket = self.bucket(storage, PREFIX_INVESTOR_TO_WHITELIST_INDEX);
        bucket.save(address.as_slice(), &index)?;

        self.len = self.len.checked_add(1).unwrap();
        self.save(storage)
    }

    pub fn add<S: Storage>(&mut self, storage: &mut S, address: &CanonicalAddr) -> StdResult<()> {
        if self.contains(storage, address)? {
            Err(StdError::generic_err("Address already in whitelist"))
        } else {
            self.add_unchecked(storage, address)
        }
    }

    pub fn remove_by_index<S: Storage>(
        &mut self,
        storage: &mut S,
        index: WhitelistSize,
    ) -> StdResult<CanonicalAddr> {
        let bucket: ReadonlyBucket<S, CanonicalAddr> =
            self.bucket_read(storage, PREFIX_WHITELIST_APPEND_STORE);

        let address = bucket.load(&index.to_le_bytes())?;
        self.remove(storage, &address)?;

        Ok(address)
    }

    pub fn remove<S: Storage>(
        &mut self,
        storage: &mut S,
        address: &CanonicalAddr,
    ) -> StdResult<()> {
        let mut bucket: Bucket<S, WhitelistSize> =
            self.bucket(storage, PREFIX_INVESTOR_TO_WHITELIST_INDEX);

        let index = bucket.load(address.as_slice())?;
        bucket.remove(address.as_slice());

        let mut bucket: Bucket<S, CanonicalAddr> =
            self.bucket(storage, PREFIX_WHITELIST_APPEND_STORE);

        bucket.remove(&index.to_le_bytes());

        let last_index = self.len.checked_sub(1).unwrap();
        if index != last_index {
            let last_address = bucket.load(&last_index.to_le_bytes())?;
            bucket.remove(&last_index.to_le_bytes());
            bucket.save(&index.to_le_bytes(), &last_address)?;
        }

        self.len = last_index;
        self.save(storage)
    }

    pub fn contains<S: ReadonlyStorage>(
        &self,
        storage: &S,
        address: &CanonicalAddr,
    ) -> StdResult<bool> {
        let in_main_whitelist: Option<WhitelistSize> =
            bucket_read(PREFIX_INVESTOR_TO_WHITELIST_INDEX, storage)
                .may_load(address.as_slice())?;

        if in_main_whitelist.is_some() {
            return Ok(true);
        }

        if let Some(ido_id) = self.ido_id {
            let in_ido_whitelist: Option<WhitelistSize> = ReadonlyBucket::multilevel(
                &[PREFIX_INVESTOR_TO_WHITELIST_INDEX, &ido_id.to_le_bytes()],
                storage,
            )
            .may_load(address.as_slice())?;

            return Ok(in_ido_whitelist.is_some());
        }

        Ok(false)
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Purchase {
    #[serde(skip)]
    payment: Uint128,
    pub tokens_amount: Uint128,
    pub payment_time: u64,
    pub unlock_time: u64,
}

impl Purchase {
    pub fn new(payment: u128, tokens_amount: u128, payment_time: u64, lock_period: u64) -> Self {
        let unlock_time = payment_time.checked_add(lock_period).unwrap();

        Purchase {
            payment: Uint128(payment),
            tokens_amount: Uint128(tokens_amount),
            payment_time,
            unlock_time,
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Purchases {
    #[serde(skip)]
    ido_id: IdoSize,
    #[serde(skip)]
    investor: CanonicalAddr,

    total_payment: Uint128,
    index_from: PurchasesSize,
    index_to: PurchasesSize,
}

impl Purchases {
    pub fn load<S: ReadonlyStorage>(
        storage: &S,
        investor: CanonicalAddr,
        ido_id: IdoSize,
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

    pub fn len(&self) -> PurchasesSize {
        self.index_to.checked_sub(self.index_from).unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get<S: ReadonlyStorage>(
        &self,
        index: PurchasesSize,
        storage: &S,
    ) -> StdResult<Purchase> {
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

    pub fn remove<S: Storage>(
        &mut self,
        storage: &mut S,
        index: PurchasesSize,
    ) -> StdResult<Purchase> {
        let index = index.checked_add(self.index_from).unwrap();

        let mut bucket: Bucket<S, Purchase> = Bucket::multilevel(
            &[
                PREFIX_ID_TO_INVESTOR_PURCHASES,
                &self.ido_id.to_le_bytes(),
                self.investor.as_slice(),
            ],
            storage,
        );

        let first_purchase = bucket.load(&self.index_from.to_le_bytes())?;
        let removed_purchase = bucket.load(&index.to_le_bytes())?;

        // Copy first element to `index` and remove first
        bucket.remove(&self.index_from.to_le_bytes());
        if index != self.index_from {
            bucket.save(&index.to_le_bytes(), &first_purchase)?;
        }

        self.index_from = self.index_from.checked_add(1).unwrap();
        self.save(storage)?;

        Ok(removed_purchase)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct InvestorInfo {
    #[serde(skip)]
    address: CanonicalAddr,

    ido_amount: IdoSize,
}

impl InvestorInfo {
    pub fn load<S: ReadonlyStorage>(storage: &S, address: CanonicalAddr) -> StdResult<Self> {
        let investor_info: Option<Self> =
            bucket_read(PREFIX_INVESTOR_MAP, storage).may_load(address.as_slice())?;

        let mut investor_info = investor_info.unwrap_or_default();
        investor_info.address = address;

        Ok(investor_info)
    }

    pub fn ido_amount(&self) -> IdoSize {
        self.ido_amount
    }

    fn save<S: Storage>(&mut self, storage: &mut S) -> StdResult<()> {
        bucket(PREFIX_INVESTOR_MAP, storage).save(self.address.as_slice(), self)
    }

    pub fn add_ido_id<S: Storage>(&mut self, storage: &mut S, ido_id: IdoSize) -> StdResult<()> {
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
    id: Option<IdoSize>,
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
    pub fn load<S: ReadonlyStorage>(storage: &S, id: IdoSize) -> StdResult<Self> {
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

    pub fn id(&self) -> Option<IdoSize> {
        self.id
    }

    pub fn save<S: Storage>(&mut self, storage: &mut S) -> StdResult<IdoSize> {
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

    fn set_len<S: Storage>(storage: &mut S, len: IdoSize) -> StdResult<()> {
        singleton(storage, IDO_LEN_KEY).save(&len)
    }

    pub fn len<S: ReadonlyStorage>(storage: &S) -> StdResult<IdoSize> {
        let len: Option<IdoSize> = singleton_read(storage, IDO_LEN_KEY).may_load()?;
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
        let mut deps = mock_dependencies(20, &[]);
        let storage = &mut deps.storage;
        let human_address = HumanAddr::from("address");
        let address = deps.api.canonical_address(&human_address).unwrap();

        let mut whitelist = Whitelist::load(storage, None).unwrap();
        assert!(whitelist.is_empty());
        assert_eq!(whitelist.len(), 0);
        assert_eq!(whitelist.contains(storage, &address), Ok(false));

        whitelist.add(storage, &address).unwrap();
        assert_eq!(whitelist.len(), 1);
        assert!(!whitelist.is_empty());
        assert_eq!(whitelist.contains(storage, &address), Ok(true));
        assert!(whitelist.add(storage, &address).is_err());

        whitelist.remove(storage, &address).unwrap();
        assert_eq!(whitelist.contains(storage, &address), Ok(false));
        assert!(whitelist.is_empty());
        assert_eq!(whitelist.len(), 0);

        assert!(whitelist.remove(storage, &address).is_err());
    }

    #[test]
    fn whitelist_multiple_addresses() {
        let mut deps = mock_dependencies(20, &[]);
        let storage = &mut deps.storage;

        let mut common_addresses = (0..100)
            .map(|i| {
                let human_address = format!("whitelist_{}", i);
                deps.api
                    .canonical_address(&HumanAddr(human_address))
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let mut whitelist = Whitelist::load(storage, None).unwrap();
        for (i, address) in common_addresses.iter().enumerate() {
            assert_eq!(whitelist.len(), i as WhitelistSize);
            assert_eq!(whitelist.contains(storage, address), Ok(false));

            whitelist.add(storage, address).unwrap();
            assert_eq!(whitelist.contains(storage, address), Ok(true));
        }

        for (i, expected) in common_addresses.iter().enumerate() {
            let canonical_address = whitelist.get(storage, i as WhitelistSize).unwrap();
            assert_eq!(canonical_address, *expected);
        }

        assert!(whitelist.get(storage, whitelist.len()).is_err());

        let loaded_whitelist = Whitelist::load(storage, None).unwrap();
        assert_eq!(whitelist, loaded_whitelist);

        assert!(Whitelist::load(storage, Some(0)).is_err());
        let ido_id = Ido::default().save(storage).unwrap();

        let mut ido_whitelist = Whitelist::load(storage, Some(ido_id)).unwrap();
        assert!(ido_whitelist.is_empty());
        assert_eq!(ido_whitelist.len(), 0);

        let ido_addresses = (0..100)
            .map(|i| {
                let human_address = format!("ido_whitelist_{}", i);
                deps.api
                    .canonical_address(&HumanAddr(human_address))
                    .unwrap()
            })
            .collect::<Vec<_>>();

        for (i, address) in ido_addresses.iter().enumerate() {
            assert_eq!(ido_whitelist.contains(storage, address), Ok(false));
            assert_eq!(ido_whitelist.len(), i as WhitelistSize);

            ido_whitelist.add(storage, address).unwrap();
            assert_eq!(ido_whitelist.contains(storage, address), Ok(true));
        }

        for (i, expected) in ido_addresses.iter().enumerate() {
            let address = ido_whitelist.get(storage, i as WhitelistSize).unwrap();
            assert_eq!(address, *expected);
        }

        assert!(ido_whitelist.get(storage, ido_whitelist.len()).is_err());

        let ido_id = Ido::default().save(storage).unwrap();
        let another_ido_whitelist = Whitelist::load(storage, Some(ido_id)).unwrap();
        assert!(another_ido_whitelist.is_empty());
        assert!(another_ido_whitelist.get(storage, 0).is_err());
        assert_eq!(another_ido_whitelist.len(), 0);

        for address in common_addresses.iter() {
            assert_eq!(whitelist.contains(storage, address), Ok(true));
            assert_eq!(ido_whitelist.contains(storage, address), Ok(true));
            assert_eq!(another_ido_whitelist.contains(storage, address), Ok(true));
        }

        for address in ido_addresses.iter() {
            assert_eq!(whitelist.contains(storage, address), Ok(false));
            assert_eq!(ido_whitelist.contains(storage, address), Ok(true));
            assert_eq!(another_ido_whitelist.contains(storage, address), Ok(false));
        }

        let address = whitelist.remove_by_index(storage, 99).unwrap();
        assert_eq!(address, common_addresses[99]);
        assert_eq!(whitelist.len(), 99);
        assert_eq!(whitelist.contains(storage, &address), Ok(false));
        assert_eq!(ido_whitelist.contains(storage, &address), Ok(false));
        assert_eq!(another_ido_whitelist.contains(storage, &address), Ok(false));

        let address = whitelist.remove_by_index(storage, 50).unwrap();
        assert_eq!(address, common_addresses[50]);
        assert_eq!(whitelist.len(), 98);
        assert_eq!(whitelist.contains(storage, &address), Ok(false));
        assert_eq!(ido_whitelist.contains(storage, &address), Ok(false));
        assert_eq!(another_ido_whitelist.contains(storage, &address), Ok(false));

        let address = whitelist.remove_by_index(storage, 0).unwrap();
        assert_eq!(address, common_addresses[0]);
        assert_eq!(whitelist.len(), 97);
        assert_eq!(whitelist.contains(storage, &address), Ok(false));
        assert_eq!(ido_whitelist.contains(storage, &address), Ok(false));
        assert_eq!(another_ido_whitelist.contains(storage, &address), Ok(false));

        common_addresses.remove(99);
        common_addresses[50] = common_addresses.remove(98);
        common_addresses[0] = common_addresses.remove(97);

        for address in common_addresses.iter() {
            assert_eq!(whitelist.contains(storage, address), Ok(true));
            assert_eq!(ido_whitelist.contains(storage, address), Ok(true));
            assert_eq!(another_ido_whitelist.contains(storage, address), Ok(true));
        }

        for address in ido_addresses.iter() {
            assert_eq!(whitelist.contains(storage, address), Ok(false));
            assert_eq!(ido_whitelist.contains(storage, address), Ok(true));
            assert_eq!(another_ido_whitelist.contains(storage, address), Ok(false));
        }

        let loaded_whitelist = Whitelist::load(storage, None).unwrap();
        assert_eq!(whitelist, loaded_whitelist);

        let loaded_ido_whitelist = Whitelist::load(storage, Some(0)).unwrap();
        assert_eq!(ido_whitelist, loaded_ido_whitelist);
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

        let ido_id = Ido::default().save(&mut storage).unwrap();
        let mut purchases = Purchases::load(&storage, investor.clone(), ido_id).unwrap();

        assert!(purchases.is_empty());
        assert!(purchases.remove(&mut storage, 0).is_err());

        let mut purchases_vector = (0..10)
            .map(|i| Purchase {
                payment: Uint128(10 * i),
                payment_time: i as u64,
                unlock_time: 10 + i as u64,
                tokens_amount: Uint128(i + 100),
            })
            .collect::<Vec<_>>();

        for (i, purchase) in purchases_vector.iter().enumerate() {
            purchases.add(purchase, &mut storage).unwrap();
            assert_eq!(purchases.len(), 1 + i as PurchasesSize);
            assert_eq!(purchases.total_payment() as usize, 5 * i * (i + 1));
            assert!(!purchases.is_empty());
        }

        for purchase in purchases_vector.iter_mut() {
            purchase.payment = Uint128(0);
        }

        assert!(purchases.remove(&mut storage, 11).is_err());
        assert_eq!(purchases.len(), 10);

        for i in 0..10 {
            let loaded_purchase = purchases.get(i, &storage).unwrap();
            assert_eq!(loaded_purchase.tokens_amount, Uint128(100 + i as u128));
            assert_eq!(loaded_purchase.payment_time, i);
            assert_eq!(loaded_purchase.unlock_time, 10 + i);
        }

        assert!(purchases.get(10, &storage).is_err());

        let removed_purchase = purchases.remove(&mut storage, 0).unwrap();
        assert_eq!(removed_purchase, purchases_vector[0]);
        assert_eq!(purchases.len(), 9);
        assert!(purchases.get(9, &storage).is_err());

        purchases_vector.remove(0);

        for (i, purchase) in purchases_vector.iter().enumerate() {
            let loaded_purchase = purchases.get(i as PurchasesSize, &storage).unwrap();
            assert_eq!(loaded_purchase, *purchase);
        }

        let removed_purchase = purchases.remove(&mut storage, 4).unwrap();
        assert_eq!(removed_purchase, purchases_vector[4]);
        assert_eq!(purchases.len(), 8);
        assert!(purchases.get(8, &storage).is_err());

        purchases_vector[4] = purchases_vector[0].clone();
        purchases_vector.remove(0);

        for (i, purchase) in purchases_vector.iter().enumerate() {
            let loaded_purchase = purchases.get(i as PurchasesSize, &storage).unwrap();
            assert_eq!(loaded_purchase, *purchase);
        }

        let loaded_purchases = Purchases::load(&storage, investor, ido_id).unwrap();
        assert_eq!(loaded_purchases, purchases);
    }
}
