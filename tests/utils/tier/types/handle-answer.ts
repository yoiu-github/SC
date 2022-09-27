export type ChangeAdmin = {
  change_admin: {
    status: ResponseStatus;
  };
};

export type ChangeStatus = {
  change_status: {
    status: ResponseStatus;
  };
};

export type Deposit = {
  deposit: {
    status: ResponseStatus;
  };
};

export type Withdraw = {
  withdraw: {
    status: ResponseStatus;
  };
};

export type Claim = {
  claim: {
    status: ResponseStatus;
  };
};

export type WithdrawRewards = {
  withdraw_rewards: {
    status: ResponseStatus;
  };
};

export type Redelegate = {
  redelegate: {
    status: ResponseStatus;
  };
};

export type ResponseStatus = "success" | "failure";
