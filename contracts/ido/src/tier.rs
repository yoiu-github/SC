use crate::{
    contract::BLOCK_SIZE,
    msg::NftToken,
    state::{self, Config},
};
use cosmwasm_std::{Api, Extern, HumanAddr, Querier, StdResult, Storage};
use secret_toolkit_snip721::{
    all_nft_info_query, private_metadata_query, Extension, Metadata, ViewerInfo,
};
use secret_toolkit_utils::Query;
use serde::{Deserialize, Serialize};
use std::cmp::max;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TierContractQuery {
    TierOf { address: HumanAddr },
}

impl Query for TierContractQuery {
    const BLOCK_SIZE: usize = 256;
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierReponse {
    TierOf { tier: u8 },
}

fn find_tier_in_metadata(metadata: Metadata) -> u8 {
    let attrubutes = match metadata.extension {
        Some(Extension {
            attributes: Some(attributes),
            ..
        }) => attributes,
        _ => return 0,
    };

    for attribute in attrubutes {
        let trait_type = attribute.trait_type.map(|t| t.to_lowercase());
        if let Some(name) = trait_type {
            if name != "tier" {
                continue;
            }

            if let Ok(tier) = attribute.value.parse() {
                return tier;
            }
        }
    }

    0
}

fn get_tier_from_nft_contract<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: &HumanAddr,
    config: &Config,
    token: NftToken,
) -> StdResult<u8> {
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
        return Ok(0);
    }

    if let Some(public_metadata) = nft_info.info {
        let tier = find_tier_in_metadata(public_metadata);
        if tier != 0 {
            return Ok(tier);
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
    let tier_of = TierContractQuery::TierOf { address };

    let TierReponse::TierOf { tier } = tier_of.query(
        &deps.querier,
        config.tier_contract_hash.clone(),
        tier_contract,
    )?;

    Ok(tier)
}

pub fn get_tier_index<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    ido_id: u32,
    token: Option<NftToken>,
) -> StdResult<u8> {
    let canonical_address = deps.api.canonical_address(&address)?;

    // If address not in whitelist, tier = 0
    let common_whitelist = state::common_whitelist();
    if !common_whitelist.contains(&deps.storage, &canonical_address) {
        let ido_whitelist = state::ido_whitelist(ido_id);
        if !ido_whitelist.contains(&deps.storage, &canonical_address) {
            return Ok(0);
        }
    }

    let config = Config::load(&deps.storage)?;
    let from_nft_contract = token
        .map(|token| get_tier_from_nft_contract(deps, &address, &config, token))
        .unwrap_or(Ok(0))?;

    let from_tier_contract = get_tier_from_tier_contract(deps, address, &config)?;
    let tier = max(from_nft_contract, from_tier_contract);

    if tier == 0 {
        Ok(0)
    } else {
        let max_tier = config.max_payments.len() as u8;
        Ok(max_tier
            .checked_sub(tier)
            .and_then(|t| t.checked_add(1))
            .unwrap())
    }
}
