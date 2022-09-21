import { SecretNetworkClient } from "secretjs";
import { ContractDeployInfo, deployContractIfNeeded } from "./utils";

export type IdoInitMsg = {
  max_payments: string[];
  lock_periods: number[];
  tier_contract: string;
  tier_contract_hash: string;
  nft_contract: string;
  nft_contract_hash: string;
  token_contract: string;
  token_contract_hash: string;
  whitelist?: string[];
};

export type IdoExecuteMsg =
  | {
    start_ido: {
      end_time: number;
      padding?: string | null;
      price: Uint128;
      start_time: number;
      token_contract: HumanAddr;
      token_contract_hash: string;
      tokens_per_tier?: Uint128[] | null;
      total_amount: Uint128;
      whitelist?: HumanAddr[] | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    whitelist_add: {
      addresses: HumanAddr[];
      ido_id?: number | null;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    whitelist_remove: {
      addresses: HumanAddr[];
      ido_id?: number | null;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    buy_tokens: {
      amount: Uint128;
      ido_id: number;
      padding?: string | null;
      token_id?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    recv_tokens: {
      ido_id: number;
      limit?: number | null;
      padding?: string | null;
      purchase_indices?: number[] | null;
      start?: number | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    withdraw: {
      ido_id: number;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };

export type Uint128 = string;
export type HumanAddr = string;

export async function deployIdo(
  client: SecretNetworkClient,
  initMsg: IdoInitMsg,
  label = "ido",
): Promise<ContractDeployInfo> {
  return await deployContractIfNeeded(
    client,
    "./build/ido.wasm",
    initMsg,
    label,
  );
}
