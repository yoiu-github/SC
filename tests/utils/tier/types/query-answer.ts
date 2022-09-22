export type TierInfo = {
  tier_info: {
    owner: HumanAddr;
    tier_list: Tier[];
    validator: HumanAddr;
  };
};

export type TierOf = {
  tier_of: {
    tier: number;
  };
};

export type DepositOf = {
  deposit_of: {
    deposit: Uint128;
  };
};

export type CanClaim = {
  can_claim: {
    time?: number | null;
  };
};

export type CanWithdraw = {
  can_withdraw: {
    time?: number | null;
  };
};

export type HumanAddr = string;
export type Uint128 = string;

export interface Tier {
  deposit: Uint128;
  lock_period: number;
}
