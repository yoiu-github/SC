export type Uint128 = string;
export type HumanAddr = string;

export interface InitMsg {
  lock_periods: number[];
  max_payments: Uint128[];
  nft_contract: HumanAddr;
  nft_contract_hash: string;
  tier_contract: HumanAddr;
  tier_contract_hash: string;
  whitelist?: HumanAddr[] | null;
}
