export type HandleMsg =
  | {
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
  }
  | {
    whitelist_add: {
      addresses: HumanAddr[];
      ido_id?: number | null;
      padding?: string | null;
    };
  }
  | {
    whitelist_remove: {
      addresses: HumanAddr[];
      ido_id?: number | null;
      padding?: string | null;
    };
  }
  | {
    buy_tokens: {
      amount: Uint128;
      ido_id: number;
      padding?: string | null;
      token?: NftToken | null;
    };
  }
  | {
    recv_tokens: {
      ido_id: number;
      limit?: number | null;
      padding?: string | null;
      purchase_indices?: number[] | null;
      start?: number | null;
    };
  }
  | {
    withdraw: {
      ido_id: number;
      padding?: string | null;
    };
  };

export type Uint128 = string;
export type HumanAddr = string;

export interface NftToken {
  token_id: string;
  viewing_key: string;
}
