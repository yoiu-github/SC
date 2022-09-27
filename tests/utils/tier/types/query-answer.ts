export type Config = {
  config: {
    admin: HumanAddr;
    validator: HumanAddr;
    status: ContractStatus;
    tier_list: TierInfo[];
  };
};

export type UserInfo = {
  user_info: {
    tier: number;
    deposit: Uint128;
    withdraw_time: number;
    timestamp: number;
  };
};

export type Withdrawals = {
  withdrawals: {
    amount: number;
    withdrawals: SerializedWithdrawals[];
  };
};

export type HumanAddr = string;
export type Uint128 = string;

export interface TierInfo {
  deposit: Uint128;
  lock_period: number;
}

export interface SerializedWithdrawals {
  amount: Uint128;
  claim_time: number;
  timestamp: number;
}

export type ContractStatus = "active" | "stopped";
