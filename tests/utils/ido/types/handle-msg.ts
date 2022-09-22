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
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    whitelist_add: {
      addresses: HumanAddr[];
      ido_id?: number | null;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    whitelist_remove: {
      addresses: HumanAddr[];
      ido_id?: number | null;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    buy_tokens: {
      amount: Uint128;
      ido_id: number;
      padding?: string | null;
      token_id?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    recv_tokens: {
      ido_id: number;
      limit?: number | null;
      padding?: string | null;
      purchase_indices?: number[] | null;
      start?: number | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    withdraw: {
      ido_id: number;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };

export type Uint128 = string;
export type HumanAddr = string;
