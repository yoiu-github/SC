export type QueryMsg =
  | {
    token_info: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    token_config: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    contract_status: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    exchange_rate: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    allowance: {
      key: string;
      owner: HumanAddr;
      spender: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    balance: {
      address: HumanAddr;
      key: string;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    transfer_history: {
      address: HumanAddr;
      key: string;
      page?: number | null;
      page_size: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    transaction_history: {
      address: HumanAddr;
      key: string;
      page?: number | null;
      page_size: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    minters: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    with_permit: {
      permit: Permit;
      query: QueryWithPermit;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };
export type HumanAddr = string;
export type Permission = "allowance" | "balance" | "history" | "owner";
/**
 * Binary is a wrapper around Vec<u8> to add base64 de/serialization with serde. It also adds some helper methods to help encode inline.
 *
 * This is only needed as serde-json-{core,wasm} has a horrible encoding for Vec<u8>
 */
export type Binary = string;
export type QueryWithPermit =
  | {
    allowance: {
      owner: HumanAddr;
      spender: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    balance: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    transfer_history: {
      page?: number | null;
      page_size: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    transaction_history: {
      page?: number | null;
      page_size: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };

export interface Permit {
  params: PermitParams;
  signature: PermitSignature;
  [k: string]: unknown;
}

export interface PermitParams {
  allowed_tokens: HumanAddr[];
  chain_id: string;
  permissions: Permission[];
  permit_name: string;
  [k: string]: unknown;
}

export interface PermitSignature {
  pub_key: PubKey;
  signature: Binary;
  [k: string]: unknown;
}

export interface PubKey {
  /**
   * ignored, but must be "tendermint/PubKeySecp256k1" otherwise the verification will fail
   */
  type: string;
  /**
   * Secp256k1 PubKey
   */
  value: Binary;
  [k: string]: unknown;
}
