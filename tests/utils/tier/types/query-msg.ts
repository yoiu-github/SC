export type QueryMsg =
  | {
    tier_info: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    tier_of: {
      address: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    deposit_of: {
      address: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    when_can_withdraw: {
      address: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    when_can_claim: {
      address: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };

export type HumanAddr = string;
