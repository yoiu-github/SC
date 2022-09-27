export type ChangeAdmin = {
  change_admin: {
    admin: string;
    padding?: string | null;
  };
};

export type ChangeStatus = {
  change_status: {
    status: ContractStatus;
    padding?: string | null;
  };
};

export type StartIdo = {
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
  };
};

export type WhitelistAdd = {
  whitelist_add: {
    addresses: HumanAddr[];
    ido_id?: number | null;
    padding?: string | null;
  };
};

export type WhitelistRemove = {
  whitelist_remove: {
    addresses: HumanAddr[];
    ido_id?: number | null;
    padding?: string | null;
  };
};

export type BuyTokens = {
  buy_tokens: {
    amount: Uint128;
    ido_id: number;
    padding?: string | null;
    token?: NftToken | null;
  };
};

export type RecvTokens = {
  recv_tokens: {
    ido_id: number;
    limit?: number | null;
    padding?: string | null;
    purchase_indices?: number[] | null;
    start?: number | null;
  };
};

export type Withdraw = {
  withdraw: {
    ido_id: number;
    padding?: string | null;
  };
};

export interface NftToken {
  token_id: string;
  viewing_key: string;
}

export type Uint128 = string;
export type HumanAddr = string;
export type ContractStatus = "active" | "stopped";
