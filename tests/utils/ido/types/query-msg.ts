export type QueryMsg =
  | {
    config: never;
  }
  | {
    ido_amount: never;
  }
  | {
    ido_info: {
      ido_id: number;
    };
  }
  | {
    whitelist_amount: {
      ido_id?: number | null;
    };
  }
  | {
    whitelist: {
      ido_id?: number | null;
      limit: number;
      start: number;
    };
  }
  | {
    ido_amount_owned_by: {
      address: HumanAddr;
    };
  }
  | {
    ido_list_owned_by: {
      address: HumanAddr;
      limit: number;
      start: number;
    };
  }
  | {
    purchases_amount: {
      address: HumanAddr;
      ido_id: number;
    };
  }
  | {
    purchases: {
      address: HumanAddr;
      ido_id: number;
      limit: number;
      start: number;
    };
  }
  | {
    archived_purchases_amount: {
      address: HumanAddr;
      ido_id: number;
    };
  }
  | {
    archived_purchases: {
      address: HumanAddr;
      ido_id: number;
      limit: number;
      start: number;
    };
  }
  | {
    user_info: {
      address: HumanAddr;
      ido_id?: number | null;
    };
  };

export type HumanAddr = string;
