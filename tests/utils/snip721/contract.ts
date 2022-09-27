import { SecretNetworkClient } from "secretjs";
import { broadcastWithCheck, getExecuteMsg, Snip721 } from "..";
import { BaseContract } from "../baseContract";

export class Snip721Contract extends BaseContract {
  constructor(label: string) {
    super(label);
  }

  async init(
    client: SecretNetworkClient,
    initMsg?: Snip721.InitMsg,
    path?: string,
  ) {
    initMsg = initMsg || {
      entropy: "entropy",
      name: "Nft collection",
      symbol: "NFT",
    };

    path = path || "./build/snip721.wasm";
    await super.deploy(client, initMsg, path);
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
