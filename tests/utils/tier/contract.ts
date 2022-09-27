import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getExecuteMsg,
  Tier,
} from "..";

export class TierContract {
  label: string;
  contractInfo: ContractDeployInfo;

  constructor(label = "tier") {
    this.label = label;
  }

  async init(
    client: SecretNetworkClient,
    initMsg: Tier.InitMsg,
  ) {
    this.contractInfo = await deployContractIfNeeded(
      client,
      "./build/tier.wasm",
      initMsg,
      this.label,
    );
  }

  async userInfo(
    client: SecretNetworkClient,
  ): Promise<Tier.QueryAnswer.UserInfo> {
    const queryUserInfo: Tier.QueryMsg.UserInfo = {
      user_info: { address: client.address },
    };

    const userInfo = await client.query.compute
      .queryContract({
        contractAddress: this.contractInfo.address,
        codeHash: this.contractInfo.codeHash,
        query: queryUserInfo,
      });

    return userInfo as Tier.QueryAnswer.UserInfo;
  }

  async config(
    client: SecretNetworkClient,
  ): Promise<Tier.QueryAnswer.Config> {
    const queryConfig: Tier.QueryMsg.Config = { config: {} };
    const config: Tier.QueryAnswer.Config = await client.query.compute
      .queryContract({
        contractAddress: this.contractInfo.address,
        codeHash: this.contractInfo.codeHash,
        query: queryConfig,
      });

    return config as Tier.QueryAnswer.Config;
  }

  async changeStatus(
    client: SecretNetworkClient,
    status: Tier.HandleMsg.ContractStatus,
  ): Promise<Tier.HandleAnswer.ChangeStatus> {
    const changeStatusMsg = getExecuteMsg<Tier.HandleMsg.ChangeStatus>(
      this.contractInfo,
      client.address,
      { change_status: { status } },
    );

    const response = await broadcastWithCheck(client, [changeStatusMsg]);
    return response[0] as Tier.HandleAnswer.ChangeStatus;
  }

  async deposit(
    client: SecretNetworkClient,
    amount: number,
    denom = "uscrt",
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
      ],
    );

    const response = await broadcastWithCheck(client, [depositMsg]);
    return response[0] as Tier.HandleAnswer.Deposit;
  }

  async setTier(
    client: SecretNetworkClient,
    tier: number,
  ) {
    const queryUserInfo: Tier.QueryMsg.UserInfo = {
      user_info: { address: client.address },
    };

    const userInfoResponse: Tier.QueryAnswer.UserInfo = await client.query
      .compute
      .queryContract({
        contractAddress: this.contractInfo.address,
        codeHash: this.contractInfo.codeHash,
        query: queryUserInfo,
      });

    const currentTier = userInfoResponse.user_info.tier;
    if (currentTier == tier) {
      return;
    }

    if (currentTier > tier) {
      throw new Error("Tier cannot be decreased");
    }

    const config = await this.config(client);
    const tierInfo = config.config.tier_list[tier - 1];
    const tierExpectedDeposit = Number.parseInt(tierInfo.deposit);

    const currentDeposit = Number.parseInt(userInfoResponse.user_info.deposit);
    const amount = tierExpectedDeposit - currentDeposit;

    await this.deposit(client, amount);
  }
}
