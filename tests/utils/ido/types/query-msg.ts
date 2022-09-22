export type QueryMsg =
  | {
    config: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    ido_amount: {
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    ido_info: {
      ido_id: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    whitelist_amount: {
      ido_id?: number | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    whitelist: {
      ido_id?: number | null;
      limit: number;
      start: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    ido_amount_owned_by: {
      address: HumanAddr;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    ido_list_owned_by: {
      address: HumanAddr;
      limit: number;
      start: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    purchases_amount: {
      address: HumanAddr;
      ido_id: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    purchases: {
      address: HumanAddr;
      ido_id: number;
      limit: number;
      start: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    archived_purchases_amount: {
      address: HumanAddr;
      ido_id: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    archived_purchases: {
      address: HumanAddr;
      ido_id: number;
      limit: number;
      start: number;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  }
  | {
    user_info: {
      address: HumanAddr;
      ido_id?: number | null;
      [k: string]: unknown;
    };
    [k: string]: unknown;
  };

export type HumanAddr = string;
