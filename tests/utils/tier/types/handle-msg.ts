export type Deposit = {
  deposit: {
    padding?: string | null;
  };
};

export type Withdraw = {
  withdraw: {
    padding?: string | null;
  };
};

export type Claim = {
  claim: {
    padding?: string | null;
    recipient?: HumanAddr | null;
  };
};

export type WithdrawRewards = {
  withdraw_rewards: {
    padding?: string | null;
    recipient?: HumanAddr | null;
  };
};

export type Redelegate = {
  redelegate: {
    padding?: string | null;
    recipient?: HumanAddr | null;
    validator_address: HumanAddr;
  };
};

export type HumanAddr = string;
