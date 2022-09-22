export type HandleMsg =
  | {
    deposit: {
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    withdraw: {
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    claim: {
      padding?: string | null;
      recipient?: HumanAddr | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    withdraw_rewards: {
      padding?: string | null;
      recipient?: HumanAddr | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    redelegate: {
      padding?: string | null;
      recipient?: HumanAddr | null;
      validator_address: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };

export type HumanAddr = string;
