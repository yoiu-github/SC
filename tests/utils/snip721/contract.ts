import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getContractWithCheck,
  getExecuteMsg,
  Snip721,
} from "..";

export class Snip721Contract {
  label: string;
  contractInfo: ContractDeployInfo;

  constructor(label: string) {
    this.label = label;
  }

  async init(
    client: SecretNetworkClient,
    initMsg?: Snip721.InitMsg,
    path?: string,
  ) {
    path = path || "./build/snip721.wasm";
    initMsg = initMsg || {
      entropy: "entropy",
      name: "Nft collection",
      symbol: "NFT",
    };

    this.contractInfo = await deployContractIfNeeded(
      client,
      path,
      initMsg,
      this.label,
    );
  }

  async mint(
    client: SecretNetworkClient,
    msg: Snip721.HandleMsg.MintNft,
  ): Promise<Snip721.HandleAnswer.MintNft> {
    const mintNftMsg = getExecuteMsg<Snip721.HandleMsg.MintNft>(
      this.contractInfo,
      client.address,
      msg,
    );

    const response = await broadcastWithCheck(client, [mintNftMsg]);
    return response[0] as Snip721.HandleAnswer.MintNft;
  }
}
