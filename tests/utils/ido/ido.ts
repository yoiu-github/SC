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

export async function deploy(
  client: SecretNetworkClient,
  initMsg: Ido.InitMsg,
  label = "ido",
): Promise<ContractDeployInfo> {
  return await deployContractIfNeeded(
    client,
    "./build/ido.wasm",
    initMsg,
    label,
  );
}

export async function addWhitelist(
  client: SecretNetworkClient,
  address: string,
  idoId?: number,
  label = "ido",
): Promise<Ido.HandleAnswer.WhitelistAdd> {
  const idoContract = await getContractWithCheck(client, label);
  const addWhitelistMsg = getExecuteMsg<Ido.HandleMsg>(
    idoContract,
    client.address,
    { whitelist_add: { addresses: [address], ido_id: idoId } },
  );

  const response = await broadcastWithCheck(client, [addWhitelistMsg]);
  return response[0] as Ido.HandleAnswer.WhitelistAdd;
}

export async function buyTokens(
  client: SecretNetworkClient,
  idoId: number,
  amount: number,
  token_id?: string,
  sscrtLabel = "sscrt",
  nftLabel = "snip721",
  idoLabel = "ido",
): Promise<Ido.HandleAnswer.BuyTokens> {
  const sscrtContract = await getContractWithCheck(client, sscrtLabel);
  const idoContract = await getContractWithCheck(client, idoLabel);
  const depositMsg = getExecuteMsg<Snip20.HandleMsg>(
    sscrtContract,
    client.address,
    { deposit: {} },
    [{ denom: "uscrt", amount: amount.toString() }],
  );

  const increaseAllowanceMsg = getExecuteMsg<Snip20.HandleMsg>(
    sscrtContract,
    client.address,
    {
      increase_allowance: {
        spender: idoContract.address,
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

    const nftContract = await getContractWithCheck(client, nftLabel);
    const setViewingKey = getExecuteMsg<Snip721.HandleMsg>(
      nftContract,
      client.address,
      { set_viewing_key: { key: token.viewing_key } },
    );

    messages.push(setViewingKey);
  }

  const buyTokensMsg = getExecuteMsg<Ido.HandleMsg>(
    idoContract,
    client.address,
    { buy_tokens: { ido_id: idoId, amount: amount.toString(), token } },
  );

  messages.push(buyTokensMsg);
  const response = await broadcastWithCheck(client, messages, 200_000);
  return response[messages.length - 1] as Ido.HandleAnswer.BuyTokens;
}

export async function recvTokens(
  client: SecretNetworkClient,
  ido_id: number,
  label = "ido",
): Promise<Ido.HandleAnswer.RecvTokens> {
  const idoContract = await getContractWithCheck(client, label);
  const response = await client.query.compute
    .queryContract({
      contractAddress: idoContract.address,
      codeHash: idoContract.codeHash,
      query: {
        purchases: {
          ido_id,
          address: client.address,
          start: 0,
          limit: 100,
        },
      },
    });

  return response as Ido.HandleAnswer.RecvTokens;
}
