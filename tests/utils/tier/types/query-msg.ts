export type TierInfo = {
  tier_info: Record<string, never>;
};

export type TierOf = {
  tier_of: {
    address: HumanAddr;
  };
};

export type DepositOf = {
  deposit_of: {
    address: HumanAddr;
  };
};

export type WhenCanWithdraw = {
  when_can_withdraw: {
    address: HumanAddr;
  };
};

export type WhenCanClaim = {
  when_can_claim: {
    address: HumanAddr;
  };
};

export type HumanAddr = string;
