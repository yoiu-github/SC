import { SecretNetworkClient } from "secretjs";
import { ContractDeployInfo, deployContractIfNeeded } from "./utils";

export type Snip20ExecuteMsg =
  | {
    redeem: {
      amount: Uint128;
      denom?: string | null;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    deposit: {
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    transfer: {
      amount: Uint128;
      memo?: string | null;
      padding?: string | null;
      recipient: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    send: {
      amount: Uint128;
      memo?: string | null;
      msg?: Binary | null;
      padding?: string | null;
      recipient: HumanAddr;
      recipient_code_hash?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    batch_transfer: {
      actions: TransferAction[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    batch_send: {
      actions: SendAction[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    burn: {
      amount: Uint128;
      memo?: string | null;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    register_receive: {
      code_hash: string;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    create_viewing_key: {
      entropy: string;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    set_viewing_key: {
      key: string;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    increase_allowance: {
      amount: Uint128;
      expiration?: number | null;
      padding?: string | null;
      spender: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    decrease_allowance: {
      amount: Uint128;
      expiration?: number | null;
      padding?: string | null;
      spender: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    transfer_from: {
      amount: Uint128;
      memo?: string | null;
      owner: HumanAddr;
      padding?: string | null;
      recipient: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    send_from: {
      amount: Uint128;
      memo?: string | null;
      msg?: Binary | null;
      owner: HumanAddr;
      padding?: string | null;
      recipient: HumanAddr;
      recipient_code_hash?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    batch_transfer_from: {
      actions: TransferFromAction[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    batch_send_from: {
      actions: SendFromAction[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    burn_from: {
      amount: Uint128;
      memo?: string | null;
      owner: HumanAddr;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    batch_burn_from: {
      actions: BurnFromAction[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    mint: {
      amount: Uint128;
      memo?: string | null;
      padding?: string | null;
      recipient: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    batch_mint: {
      actions: MintAction[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    add_minters: {
      minters: HumanAddr[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    remove_minters: {
      minters: HumanAddr[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    set_minters: {
      minters: HumanAddr[];
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    change_admin: {
      address: HumanAddr;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    set_contract_status: {
      level: ContractStatusLevel;
      padding?: string | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    revoke_permit: {
      permit_name: string;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };
export type Uint128 = string;
export type HumanAddr = string;
export type Binary = string;
export type ContractStatusLevel =
  | "normal_run"
  | "stop_all_but_redeems"
  | "stop_all";

export interface TransferAction {
  amount: Uint128;
  memo?: string | null;
  recipient: HumanAddr;
  [k: string]: unknown;
}
export interface SendAction {
  amount: Uint128;
  memo?: string | null;
  msg?: Binary | null;
  recipient: HumanAddr;
  recipient_code_hash?: string | null;
  [k: string]: unknown;
}
export interface TransferFromAction {
  amount: Uint128;
  memo?: string | null;
  owner: HumanAddr;
  recipient: HumanAddr;
  [k: string]: unknown;
}
export interface SendFromAction {
  amount: Uint128;
  memo?: string | null;
  msg?: Binary | null;
  owner: HumanAddr;
  recipient: HumanAddr;
  recipient_code_hash?: string | null;
  [k: string]: unknown;
}
export interface BurnFromAction {
  amount: Uint128;
  memo?: string | null;
  owner: HumanAddr;
  [k: string]: unknown;
}
export interface MintAction {
  amount: Uint128;
  memo?: string | null;
  recipient: HumanAddr;
  [k: string]: unknown;
}

export type InitialBalance = {
  address: string;
  amount: string;
};

export type Snip20Config = {
  public_total_supply?: boolean;
  enable_deposit?: boolean;
  enable_redeem?: boolean;
  enable_mint?: boolean;
  enable_burn?: boolean;
};

export type Snip20InitMsg = {
  name: string;
  admin?: string;
  symbol: string;
  decimals: number;
  initial_balances?: InitialBalance[];
  prng_seed: string;
  config?: Snip20Config;
};

export async function deploySnip20(
  client: SecretNetworkClient,
  label = "snip20",
  initMsg?: Snip20InitMsg,
  path?: string,
): Promise<ContractDeployInfo> {
  initMsg = initMsg || {
    name: "Snip20Token",
    symbol: "SNIP",
    decimals: 6,
    prng_seed: "seed",
    config: {
      enable_mint: true,
    },
  };

  path = path || "./build/snip20.wasm";
  return await deployContractIfNeeded(
    client,
    path,
    initMsg,
    label,
  );
}
