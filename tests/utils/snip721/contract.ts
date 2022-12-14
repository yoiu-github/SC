import { SecretNetworkClient } from "secretjs";
import { broadcastWithCheck, getExecuteMsg, Snip721 } from "..";
import { BaseContract } from "../baseContract";

export class Snip721Contract extends BaseContract {
  constructor(label?: string, path?: string) {
    super(label, path);
  }

  async init(client: SecretNetworkClient, initMsg?: Snip721.InitMsg) {
    initMsg = initMsg || {
      entropy: "entropy",
      name: "Nft collection",
      symbol: "NFT",
    };

    await super.init(client, initMsg);
  }

  async mint(
    client: SecretNetworkClient,
    msg: Snip721.HandleMsg.MintNft
  ): Promise<Snip721.HandleAnswer.MintNft> {
    const mintNftMsg = getExecuteMsg<Snip721.HandleMsg.MintNft>(
      this.contractInfo,
      client.address,
      msg
    );

    const response = await broadcastWithCheck(client, [mintNftMsg]);
    return response[0] as Snip721.HandleAnswer.MintNft;
  }
}
