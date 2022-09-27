import { SecretNetworkClient } from "secretjs";
import { broadcastWithCheck, getExecuteMsg, Ido, Snip20, Snip721 } from "..";
import { BaseContract, ContractDeployInfo } from "../baseContract";
import { NftToken } from "./types/handle-msg";

export class IdoContract extends BaseContract {
  sscrtContract: ContractDeployInfo;
  nftContract: ContractDeployInfo;

  constructor(
    label = "ido",
    sscrtContract: ContractDeployInfo,
    nftContract: ContractDeployInfo,
  ) {
    super(label);
    this.sscrtContract = sscrtContract;
    this.nftContract = nftContract;
  }

  async init(
    client: SecretNetworkClient,
    initMsg: Ido.InitMsg,
  ) {
    await super.deploy(client, initMsg, "./build/ido.wasm");
  }

  async addWhitelist(
    client: SecretNetworkClient,
    address: string,
    idoId?: number,
  ): Promise<Ido.HandleAnswer.WhitelistAdd> {
    const addWhitelistMsg = getExecuteMsg<Ido.HandleMsg.WhitelistAdd>(
      this.contractInfo,
      client.address,
      { whitelist_add: { addresses: [address], ido_id: idoId } },
    );

    const response = await broadcastWithCheck(client, [addWhitelistMsg]);
    return response[0] as Ido.HandleAnswer.WhitelistAdd;
  }

  async buyTokens(
    client: SecretNetworkClient,
    idoId: number,
    amount: number,
    price: number,
    token_id?: string,
  ): Promise<Ido.HandleAnswer.BuyTokens> {
    const sscrtAmount = amount * price;
    const depositMsg = getExecuteMsg<Snip20.HandleMsg.Deposit>(
      this.sscrtContract,
      client.address,
      { deposit: {} },
      [{ denom: "uscrt", amount: sscrtAmount.toString() }],
    );

    const increaseAllowanceMsg = getExecuteMsg<
      Snip20.HandleMsg.IncreaseAllowance
    >(
      this.sscrtContract,
      client.address,
      {
        increase_allowance: {
          spender: this.contractInfo.address,
          amount: sscrtAmount.toString(),
        },
      },
    );

    const messages = [];
    messages.push(depositMsg);
    messages.push(increaseAllowanceMsg);

    let token: NftToken | undefined;
    if (token_id != null) {
      token = {
        token_id,
        viewing_key: "random key",
      };

      const setViewingKey = getExecuteMsg<Snip721.HandleMsg.SetViewingKey>(
        this.nftContract,
        client.address,
        { set_viewing_key: { key: token.viewing_key } },
      );

      messages.push(setViewingKey);
    }

    const buyTokensMsg = getExecuteMsg<Ido.HandleMsg.BuyTokens>(
      this.contractInfo,
      client.address,
      { buy_tokens: { ido_id: idoId, amount: amount.toString(), token } },
    );

    messages.push(buyTokensMsg);
    const response = await broadcastWithCheck(client, messages, 200_000);
    return response[messages.length - 1] as Ido.HandleAnswer.BuyTokens;
  }

  async startIdo(
    client: SecretNetworkClient,
    startIdoMsg: Ido.HandleMsg.StartIdo,
    snip20Contract: ContractDeployInfo,
  ): Promise<Ido.HandleAnswer.StartIdo> {
    const amount = startIdoMsg.start_ido.total_amount;

    const messages = [];
    messages.push(
      getExecuteMsg<Snip20.HandleMsg.IncreaseAllowance>(
        snip20Contract,
        client.address,
        {
          increase_allowance: {
            spender: this.contractInfo.address,
            amount,
          },
        },
      ),
    );

    messages.push(
      getExecuteMsg(this.contractInfo, client.address, startIdoMsg),
    );

    const response = await broadcastWithCheck(client, messages);
    return response[1] as Ido.HandleAnswer.StartIdo;
  }

  async recvTokens(
    client: SecretNetworkClient,
    idoId: number,
  ): Promise<Ido.HandleAnswer.RecvTokens> {
    const recvTokensMsg = getExecuteMsg<Ido.HandleMsg.RecvTokens>(
      this.contractInfo,
      client.address,
      {
        recv_tokens: { ido_id: idoId },
      },
    );

    const response = await broadcastWithCheck(client, [recvTokensMsg]);
    return response[0] as Ido.HandleAnswer.RecvTokens;
  }

  async idoInfo(
    client: SecretNetworkClient,
    idoId: number,
  ): Promise<Ido.QueryAnswer.IdoInfo> {
    const query: Ido.QueryMsg.IdoInfo = { ido_info: { ido_id: idoId } };
    return await super.query(client, query);
  }

  async purchases(
    client: SecretNetworkClient,
    idoId: number,
    start = 0,
    limit = 50,
  ): Promise<Ido.QueryAnswer.Purchases> {
    const query: Ido.QueryMsg.Purchases = {
      purchases: { address: client.address, ido_id: idoId, start, limit },
    };

    return await super.query(client, query);
  }

  async archivedPurchases(
    client: SecretNetworkClient,
    idoId: number,
    start = 0,
    limit = 50,
  ): Promise<Ido.QueryAnswer.Purchases> {
    const query: Ido.QueryMsg.ArchivedPurchases = {
      archived_purchases: {
        address: client.address,
        ido_id: idoId,
        start,
        limit,
      },
    };

    return await super.query(client, query);
  }

  async userInfo(
    client: SecretNetworkClient,
    idoId?: number,
  ): Promise<Ido.QueryAnswer.UserInfo> {
    const query: Ido.QueryMsg.UserInfo = {
      user_info: { address: client.address, ido_id: idoId },
    };

    return await super.query(client, query);
  }
}
