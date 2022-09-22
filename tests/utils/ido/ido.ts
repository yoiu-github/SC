import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getContractWithCheck,
  getExecuteMsg,
  Ido,
  Snip20,
} from "..";

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
    { whitelist_add: { addresses: [address] }, ido_id: idoId },
  );

  const response = await broadcastWithCheck(client, [addWhitelistMsg]);
  return response[0] as Ido.HandleAnswer.WhitelistAdd;
}

export async function buyTokens(
  client: SecretNetworkClient,
  idoId: number,
  amount: number,
  sscrtLabel = "sscrt",
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

  const buyTokensMsg = getExecuteMsg<Ido.HandleMsg>(
    idoContract,
    client.address,
    { buy_tokens: { ido_id: idoId, amount: amount.toString() } },
  );

  const messages = [depositMsg, increaseAllowanceMsg, buyTokensMsg];
  const response = await broadcastWithCheck(client, messages);
  return response[2] as Ido.HandleAnswer.BuyTokens;
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
