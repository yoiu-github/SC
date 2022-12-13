import { SecretNetworkClient } from "secretjs";
import { broadcastWithCheck, getExecuteMsg, Tier } from "..";
import { BaseContract } from "../baseContract";

export class Contract extends BaseContract {
  constructor(label = "tier") {
    super(label);
  }

  async init(client: SecretNetworkClient, initMsg: Tier.InitMsg) {
    await super.deploy(client, initMsg, "./build/tier.wasm");
  }

  async userInfo(
    client: SecretNetworkClient
  ): Promise<Tier.QueryAnswer.UserInfo> {
    const queryUserInfo: Tier.QueryMsg.UserInfo = {
      user_info: { address: client.address },
    };

    return await super.query(client, queryUserInfo);
  }

  async config(client: SecretNetworkClient): Promise<Tier.QueryAnswer.Config> {
    const queryConfig: Tier.QueryMsg.Config = { config: {} };
    return await super.query(client, queryConfig);
  }

  async withdrawals(
    client: SecretNetworkClient,
    start?: number,
    limit?: number
  ): Promise<Tier.QueryAnswer.Withdrawals> {
    const queryWithdrawals: Tier.QueryMsg.Withdrawals = {
      withdrawals: { address: client.address, start, limit },
    };

    return await super.query(client, queryWithdrawals);
  }

  async changeStatus(
    client: SecretNetworkClient,
    status: Tier.HandleMsg.ContractStatus
  ): Promise<Tier.HandleAnswer.ChangeStatus> {
    const changeStatusMsg = getExecuteMsg<Tier.HandleMsg.ChangeStatus>(
      this.contractInfo,
      client.address,
      { change_status: { status } }
    );

    const response = await broadcastWithCheck(client, [changeStatusMsg]);
    return response[0] as Tier.HandleAnswer.ChangeStatus;
  }

  async redelegate(
    client: SecretNetworkClient,
    validator: string
  ): Promise<Tier.HandleAnswer.Redelegate> {
    const changeStatusMsg = getExecuteMsg<Tier.HandleMsg.Redelegate>(
      this.contractInfo,
      client.address,
      { redelegate: { validator_address: validator } }
    );

    const response = await broadcastWithCheck(client, [changeStatusMsg]);
    return response[0] as Tier.HandleAnswer.Redelegate;
  }

  async deposit(
    client: SecretNetworkClient,
    amount: number,
    denom = "uscrt"
  ): Promise<Tier.HandleAnswer.Deposit> {
    const depositMsg = getExecuteMsg<Tier.HandleMsg.Deposit>(
      this.contractInfo,
      client.address,
      { deposit: {} },
      [
        {
          denom,
          amount: amount.toString(),
        },
      ]
    );

    const response = await broadcastWithCheck(client, [depositMsg]);
    return response[0] as Tier.HandleAnswer.Deposit;
  }

  async withdraw(
    client: SecretNetworkClient
  ): Promise<Tier.HandleAnswer.Withdraw> {
    const withdrawMsg = getExecuteMsg<Tier.HandleMsg.Withdraw>(
      this.contractInfo,
      client.address,
      { withdraw: {} }
    );

    const response = await broadcastWithCheck(client, [withdrawMsg]);
    return response[0] as Tier.HandleAnswer.Withdraw;
  }

  async setTier(client: SecretNetworkClient, tier: number) {
    const userInfo = await this.userInfo(client);
    const currentTier = userInfo.user_info.tier;
    if (currentTier == tier) {
      return;
    }

    if (currentTier != 0 && (currentTier < tier || tier == 0)) {
      throw new Error("Tier cannot be decreased");
    }

    const config = await this.config(client);
    const maxTier = config.config.tier_list.length;
    const tierInfo = config.config.tier_list[maxTier - tier];
    const tierExpectedDeposit = Number.parseInt(tierInfo.deposit);

    const currentDeposit = Number.parseInt(userInfo.user_info.deposit);
    const amount = tierExpectedDeposit - currentDeposit;

    await this.deposit(client, amount);
  }
}
