import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getContractWithCheck,
  getExecuteMsg,
  Ido,
  Snip20,
  Snip721,
} from "..";
import { NftToken } from "./types/handle-msg";

export class IdoContract {
  label: string;
  contractInfo: ContractDeployInfo;
  sscrtContract: ContractDeployInfo;
  nftContract: ContractDeployInfo;

  constructor(
    label = "ido",
    sscrtContract: ContractDeployInfo,
    nftContract: ContractDeployInfo,
  ) {
    this.label = label;
    this.sscrtContract = sscrtContract;
    this.nftContract = nftContract;
  }

  async init(
    client: SecretNetworkClient,
    initMsg: Ido.InitMsg,
  ) {
    this.contractInfo = await deployContractIfNeeded(
      client,
      "./build/ido.wasm",
      initMsg,
      this.label,
    );
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
    token_id?: string,
  ): Promise<Ido.HandleAnswer.BuyTokens> {
    const depositMsg = getExecuteMsg<Snip20.HandleMsg.Deposit>(
      this.sscrtContract,
      client.address,
      { deposit: {} },
      [{ denom: "uscrt", amount: amount.toString() }],
    );

    const increaseAllowanceMsg = getExecuteMsg<
      Snip20.HandleMsg.IncreaseAllowance
    >(
      this.sscrtContract,
      client.address,
      {
        increase_allowance: {
          spender: this.contractInfo.address,
          amount: amount.toString(),
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
    snip20Contract: string | ContractDeployInfo = "snip20",
  ): Promise<Ido.HandleAnswer.StartIdo> {
    if (typeof snip20Contract == "string") {
      snip20Contract = await getContractWithCheck(client, snip20Contract);
    }

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
}
