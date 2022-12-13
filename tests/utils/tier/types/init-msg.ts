export type Uint128 = string;
export type HumanAddr = string;

export interface InitMsg {
  deposits: Uint128[];
  lock_periods: number[];
  owner?: HumanAddr | null;
  validator: HumanAddr;
  band_oracle: HumanAddr;
  band_code_hash: String;
}
