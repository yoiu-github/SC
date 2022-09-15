use cosmwasm_std::{CanonicalAddr, ReadonlyStorage, StdResult, Storage};
use secret_toolkit_storage::{AppendStore, DequeStore, Item, Keymap};
use serde::{Deserialize, Serialize};

static CONFIG_KEY: Item<Config> = Item::new(b"config");
static INVESTOR_IDO_INFO: Keymap<u32, InvestorIdoInfo> = Keymap::new(b"inv2idoinfo");
static INVESTOR_IDO_PURCHASES: DequeStore<Purchase> = DequeStore::new(b"inv2purchases");
static STARTUP_IDO_LIST: AppendStore<u32> = AppendStore::new(b"startup_idos");

pub fn common_whitelist() -> Keymap<'static, CanonicalAddr, bool> {
    Keymap::new(b"whitelist")
}

pub fn ido_whitelist(ido_id: u32) -> Keymap<'static, CanonicalAddr, bool> {
    let whitelist = common_whitelist();
    whitelist.add_suffix(&ido_id.to_le_bytes())
}

pub fn investor_ido_info(investor: &CanonicalAddr) -> Keymap<'static, u32, InvestorIdoInfo> {
    INVESTOR_IDO_INFO.add_suffix(investor.as_slice())
}

pub fn investor_ido_purchases(investor: &CanonicalAddr, ido_id: u32) -> DequeStore<Purchase> {
    INVESTOR_IDO_PURCHASES
        .add_suffix(investor.as_slice())
        .add_suffix(&ido_id.to_le_bytes())
}

pub fn startup_ido_list(owner: &CanonicalAddr) -> AppendStore<u32> {
    STARTUP_IDO_LIST.add_suffix(owner.as_slice())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub tier_contract: CanonicalAddr,
    pub tier_contract_hash: String,
    pub nft_contract: CanonicalAddr,
    pub nft_contract_hash: String,
    pub token_contract: CanonicalAddr,
    pub token_contract_hash: String,
    pub max_payments: Vec<u128>,
    pub lock_periods: Vec<u64>,
}

impl Config {
    pub fn load<S: ReadonlyStorage>(storage: &S) -> StdResult<Self> {
        CONFIG_KEY.load(storage)
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        CONFIG_KEY.save(storage, self)
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Purchase {
    pub tokens_amount: u128,
    pub unlock_time: u64,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvestorIdoInfo {
    pub total_payment: u128,
    pub total_tokens_bought: u128,
    pub total_tokens_received: u128,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Ido {
    #[serde(skip)]
    id: Option<u32>,
    pub owner: CanonicalAddr,
    pub start_time: u64,
    pub end_time: u64,
    pub token_contract: CanonicalAddr,
    pub token_contract_hash: String,
    pub price: u128,
    pub participants: u64,
    pub sold_amount: u128,
    pub total_tokens_amount: u128,
    pub total_payment: u128,
    pub withdrawn: bool,
}

impl Ido {
    fn list() -> AppendStore<'static, Self> {
        AppendStore::new(b"ido")
    }

    pub fn load<S: ReadonlyStorage>(storage: &S, id: u32) -> StdResult<Self> {
        let list = Ido::list();
        let mut ido = list.get_at(storage, id)?;
        ido.id = Some(id);
        Ok(ido)
    }

    pub fn len<S: Storage>(storage: &S) -> StdResult<u32> {
        let list = Ido::list();
        list.get_len(storage)
    }

    pub fn save<S: Storage>(&mut self, storage: &mut S) -> StdResult<u32> {
        let list = Ido::list();

        let id = if let Some(id) = self.id {
            list.set_at(storage, id, self)?;
            id
        } else {
            let id = list.get_len(storage)?;
            self.id = Some(id);
            list.push(storage, self)?;
            id
        };

        Ok(id)
    }

    pub fn id(&self) -> Option<u32> {
        self.id
    }

    pub fn is_active(&self, current_time: u64) -> bool {
        current_time >= self.start_time && current_time < self.end_time
    }

    pub fn remaining_amount(&self) -> u128 {
        self.total_tokens_amount
            .checked_sub(self.sold_amount)
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api, HumanAddr};

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
            price: 100,
            total_tokens_amount: 1000,
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
}
