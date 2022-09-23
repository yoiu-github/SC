import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getContractWithCheck,
  getExecuteMsg,
  Snip721,
} from "..";

export async function deploy(
  client: SecretNetworkClient,
  label = "snip721",
  initMsg?: Snip721.InitMsg,
  path?: string,
): Promise<ContractDeployInfo> {
  path = path || "./build/snip721.wasm";
  initMsg = initMsg || {
    entropy: "entropy",
    name: "Nft collection",
    symbol: "NFT",
  };

  return await deployContractIfNeeded(
    client,
    path,
    initMsg,
    label,
  );
}

export async function mint(
  client: SecretNetworkClient,
  msg: Snip721.HandleMsg.MintNft,
  label = "snip721",
): Promise<Snip721.HandleAnswer.MintNft> {
  const nftContract = await getContractWithCheck(client, label);
  const mintNftMsg = getExecuteMsg<Snip721.HandleMsg.MintNft>(
    nftContract,
    client.address,
    msg,
  );

  const response = await broadcastWithCheck(client, [mintNftMsg]);
  return response[0] as Snip721.HandleAnswer.MintNft;
}
