use crate::{contract::BLOCK_SIZE, msg::NftToken, state::Config};
use cosmwasm_std::{Api, Extern, HumanAddr, Querier, StdResult, Storage};
use secret_toolkit_snip721::{
    all_nft_info_query, private_metadata_query, Extension, Metadata, ViewerInfo,
};
use secret_toolkit_utils::Query;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TierContractQuery {
    UserInfo { address: HumanAddr },
}

impl Query for TierContractQuery {
    const BLOCK_SIZE: usize = 256;
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierReponse {
    UserInfo { tier: u8 },
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

    let TierReponse::UserInfo { tier } = user_info.query(
        &deps.querier,
        config.tier_contract_hash.clone(),
        tier_contract,
    )?;

    Ok(tier)
}

pub fn get_tier_index<S: Storage, A: Api, Q: Querier>(
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

    Ok(tier.checked_sub(1).unwrap())
}
