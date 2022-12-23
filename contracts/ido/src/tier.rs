#[cfg(not(test))]
mod query {
    use crate::{
        contract::BLOCK_SIZE,
        msg::{ContractStatus, NftToken},
        state::Config,
    };
    use cosmwasm_std::{Api, Extern, HumanAddr, Querier, StdError, StdResult, Storage, Uint128};
    use secret_toolkit_snip721::{
        all_nft_info_query, private_metadata_query, Extension, Metadata, ViewerInfo,
    };
    use secret_toolkit_utils::Query;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum TierContractQuery {
        Config {},
        UserInfo { address: HumanAddr },
    }

    impl Query for TierContractQuery {
        const BLOCK_SIZE: usize = 256;
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum TierResponse {
        UserInfo {
            tier: u8,
        },
        Config {
            admin: HumanAddr,
            validator: HumanAddr,
            status: ContractStatus,
            band_oracle: HumanAddr,
            band_code_hash: String,
            usd_deposits: Vec<Uint128>,
            min_tier: u8,
        },
    }

    fn find_tier_in_metadata(metadata: Metadata) -> Option<u8> {
        let attrubutes = match metadata.extension {
            Some(Extension {
                attributes: Some(attributes),
                ..
            }) => attributes,
            _ => return None,
        };

        for attribute in attrubutes {
            let trait_type = attribute.trait_type.map(|t| t.to_lowercase());
            if let Some(name) = trait_type {
                if name != "tier" {
                    continue;
                }

                if let Ok(tier) = attribute.value.parse() {
                    return Some(tier);
                }
            }
        }

        None
    }

    fn get_tier_from_nft_contract<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
        address: &HumanAddr,
        config: &Config,
        token: NftToken,
    ) -> StdResult<Option<u8>> {
        let nft_contract = deps.api.human_address(&config.nft_contract)?;
        let token_viewer = ViewerInfo {
            address: address.clone(),
            viewing_key: token.viewing_key,
        };
        let nft_info = all_nft_info_query(
            &deps.querier,
            token.token_id.clone(),
            Some(token_viewer.clone()),
            Some(false),
            BLOCK_SIZE,
            config.nft_contract_hash.clone(),
            nft_contract.clone(),
        )?;

        if nft_info.access.owner.as_ref() != Some(address) {
            return Ok(None);
        }

        if let Some(public_metadata) = nft_info.info {
            let tier = find_tier_in_metadata(public_metadata);
            if let Some(tier) = tier {
                return Ok(Some(tier));
            }
        };

        let private_metadata = private_metadata_query(
            &deps.querier,
            token.token_id,
            Some(token_viewer),
            BLOCK_SIZE,
            config.nft_contract_hash.clone(),
            nft_contract,
        )?;

        Ok(find_tier_in_metadata(private_metadata))
    }

    fn get_tier_from_tier_contract<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
        address: HumanAddr,
        config: &Config,
    ) -> StdResult<u8> {
        let tier_contract = deps.api.human_address(&config.tier_contract)?;
        let user_info = TierContractQuery::UserInfo { address };

        if let TierResponse::UserInfo { tier } = user_info.query(
            &deps.querier,
            config.tier_contract_hash.clone(),
            tier_contract,
        )? {
            Ok(tier)
        } else {
            Err(StdError::generic_err("Cannot get tier"))
        }
    }

    pub fn get_tier<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
        address: HumanAddr,
        token: Option<NftToken>,
    ) -> StdResult<u8> {
        let config = Config::load(&deps.storage)?;
        let from_nft_contract = token
            .map(|token| get_tier_from_nft_contract(deps, &address, &config, token))
            .unwrap_or(Ok(None))?;

        let mut tier = get_tier_from_tier_contract(deps, address, &config)?;
        if let Some(nft_tier) = from_nft_contract {
            if nft_tier < tier {
                tier = nft_tier
            }
        }

        Ok(tier)
    }

    pub fn get_min_tier<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
        config: &Config,
    ) -> StdResult<u8> {
        let tier_contract = deps.api.human_address(&config.tier_contract)?;
        let user_info = TierContractQuery::Config {};

        if let TierResponse::Config { min_tier, .. } = user_info.query(
            &deps.querier,
            config.tier_contract_hash.clone(),
            tier_contract,
        )? {
            Ok(min_tier)
        } else {
            Err(StdError::generic_err("Cannot get min tier"))
        }
    }
}

#[cfg(test)]
pub mod manual {
    use crate::{msg::NftToken, state::Config};
    use cosmwasm_std::{Api, Extern, HumanAddr, Querier, StdResult, Storage};
    use std::sync::Mutex;

    static TIER: Mutex<u8> = Mutex::new(0);
    static MIN_TIER: Mutex<u8> = Mutex::new(4);

    pub fn set_tier(tier: u8) {
        let mut tier_lock = TIER.lock().unwrap();
        *tier_lock = tier;
    }

    pub fn set_min_tier(tier: u8) {
        let mut tier_lock = MIN_TIER.lock().unwrap();
        *tier_lock = tier;
    }

    pub fn get_tier<S: Storage, A: Api, Q: Querier>(
        _deps: &Extern<S, A, Q>,
        _address: HumanAddr,
        _token: Option<NftToken>,
    ) -> StdResult<u8> {
        let tier_lock = TIER.lock().unwrap();
        Ok(*tier_lock)
    }

    pub fn get_min_tier<S: Storage, A: Api, Q: Querier>(
        _deps: &Extern<S, A, Q>,
        _config: &Config,
    ) -> StdResult<u8> {
        let tier_lock = MIN_TIER.lock().unwrap();
        Ok(*tier_lock)
    }
}

#[cfg(not(test))]
pub use query::get_tier;

#[cfg(not(test))]
pub use query::get_min_tier;

#[cfg(test)]
pub use manual::get_tier;

#[cfg(test)]
pub use manual::get_min_tier;

#[cfg(test)]
mod tests {
    use super::manual::{get_tier, set_tier};
    use cosmwasm_std::{testing::mock_dependencies, HumanAddr};

    #[test]
    fn manual_tier() {
        let deps = mock_dependencies(20, &[]);
        let address = HumanAddr::from("address");

        for i in 1..100 {
            set_tier(i);
            assert_eq!(get_tier(&deps, address.clone(), None), Ok(i));
        }
    }
}
