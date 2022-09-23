import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getContractWithCheck,
  getExecuteMsg,
  Snip20,
} from "..";

export async function deploy(
  client: SecretNetworkClient,
  label = "snip20",
  initMsg?: Snip20.InitMsg,
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

export async function deploySscrt(
  client: SecretNetworkClient,
  label = "sscrt",
  initMsg?: Snip20.InitMsg,
  path?: string,
): Promise<ContractDeployInfo> {
  initMsg = initMsg || {
    name: "Tier token",
    symbol: "TTOKEN",
    decimals: 6,
    prng_seed: "seed",
  };

  path = path || "./build/sscrt.wasm";
  return await deployContractIfNeeded(
    client,
    path,
    initMsg,
    label,
  );
}

export async function mint(
  client: SecretNetworkClient,
  recipient: string,
  amount = 100_000_000,
  label = "snip20",
): Promise<Snip20.HandleAnswer.Mint> {
  const snip20 = await getContractWithCheck(client, label);
  const mintMsg = getExecuteMsg<Snip20.HandleMsg.Mint>(
    snip20,
    client.address,
    {
      mint: { recipient, amount: amount.toString() },
    },
  );

  const response = await broadcastWithCheck(client, [mintMsg]);
  return response[0] as Snip20.HandleAnswer.Mint;
}

export async function getBalance(
  client: SecretNetworkClient,
  label = "snip20",
): Promise<Snip20.QueryAnswer.Balance> {
  const snip20 = await getContractWithCheck(client, label);
  const key = "random key";

  const setViewingKey = getExecuteMsg<Snip20.HandleMsg.SetViewingKey>(
    snip20,
    client.address,
    { set_viewing_key: { key } },
  );

  await broadcastWithCheck(client, [setViewingKey]);
  const query: Snip20.QueryMsg.Balance = {
    balance: { address: client.address, key },
  };

  return client.query.compute.queryContract({
    contractAddress: snip20.address,
    codeHash: snip20.codeHash,
    query,
  });
}
