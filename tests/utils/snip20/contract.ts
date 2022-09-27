import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getExecuteMsg,
  Snip20,
} from "..";

export type Snip20ContractType = "snip20" | "sscrt";

export class Snip20Contract {
  label: string;
  path: string;
  contractInfo: ContractDeployInfo;
  contractType: Snip20ContractType;

  constructor(label = "snip20", contractType: Snip20ContractType = "snip20") {
    this.label = label;
    this.contractType = contractType;
  }

  async init(
    client: SecretNetworkClient,
    initMsg?: Snip20.InitMsg,
  ) {
    initMsg = initMsg || {
      name: "Snip20Token",
      symbol: "SNIP",
      decimals: 6,
      prng_seed: "seed",
      config: {
        enable_mint: true,
      },
    };

    let path: string;
    if (this.contractType == "snip20") {
      path = "./build/snip20.wasm";
    } else {
      path = "./build/sscrt.wasm";
    }

    this.contractInfo = await deployContractIfNeeded(
      client,
      path,
      initMsg,
      this.label,
    );
  }

  async mint(
    client: SecretNetworkClient,
    recipient: string,
    amount = 100_000_000,
  ): Promise<Snip20.HandleAnswer.Mint> {
    const mintMsg = getExecuteMsg<Snip20.HandleMsg.Mint>(
      this.contractInfo,
      client.address,
      {
        mint: { recipient, amount: amount.toString() },
      },
    );

    const response = await broadcastWithCheck(client, [mintMsg]);
    return response[0] as Snip20.HandleAnswer.Mint;
  }

  async getBalance(
    client: SecretNetworkClient,
    key?: string,
  ): Promise<Snip20.QueryAnswer.Balance> {
    key = key || "random string";

    const setViewingKey = getExecuteMsg<Snip20.HandleMsg.SetViewingKey>(
      this.contractInfo,
      client.address,
      { set_viewing_key: { key } },
    );

    await broadcastWithCheck(client, [setViewingKey]);
    const query: Snip20.QueryMsg.Balance = {
      balance: { address: client.address, key },
    };

    return client.query.compute.queryContract({
      contractAddress: this.contractInfo.address,
      codeHash: this.contractInfo.codeHash,
      query,
    });
  }
}
