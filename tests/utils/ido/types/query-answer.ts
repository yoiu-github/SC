export type Config = {
  config: {
    lock_periods: number[];
    max_payments: Uint128[];
    nft_contract: HumanAddr;
    nft_contract_hash: string;
    owner: HumanAddr;
    tier_contract: HumanAddr;
    tier_contract_hash: string;
    token_contract: HumanAddr;
    token_contract_hash: string;
  };
};

export type IdoAmount = {
  ido_amount: {
    amount: number;
  };
};

export type IdoInfo = {
  ido_info: {
    end_time: number;
    owner: HumanAddr;
    participants: number;
    price: Uint128;
    sold_amount: Uint128;
    start_time: number;
    token_contract: HumanAddr;
    token_contract_hash: string;
    total_payment: Uint128;
    total_tokens_amount: Uint128;
    withdrawn: boolean;
  };
};

export type InWhitelist = {
  in_whitelist: {
    in_whitelist: boolean;
  };
};

export type Whitelist = {
  whitelist: {
    addresses: HumanAddr[];
    amount: number;
  };
};

export type IdoAmountOwnedBy = {
  ido_amount_owned_by: {
    amount: number;
  };
};

export type IdoListOwnerBy = {
  ido_list_owned_by: {
    ido_ids: number[];
  };
};

export type Purchases = {
  purchases: {
    purchases: PurchaseAnswer[];
    amount: number;
  };
};

export type ArchivedPurchases = {
  archived_purchases: {
    purchases: PurchaseAnswer[];
    amount: number;
  };
};

export type UserInfo = {
  user_info: {
    total_payment: Uint128;
    total_tokens_bought: Uint128;
    total_tokens_received: Uint128;
  };
};

export type Uint128 = string;
export type HumanAddr = string;

export interface PurchaseAnswer {
  timestamp: number;
  tokens_amount: Uint128;
  unlock_time: number;
}
