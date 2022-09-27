import * as fs from "fs";
import {
  CodeInfoResponse,
  Coin,
  JsonLog,
  MsgExecuteContract,
  SecretNetworkClient,
} from "secretjs";

export type ContractDeployInfo = {
  address: string;
  codeHash: string;
};

export class BaseContract {
  readonly label: string;
  contractInfo: ContractDeployInfo;

  constructor(label: string) {
    this.label = label;
  }

  async query<T extends object, R extends object>(
    client: SecretNetworkClient,
    query: T,
  ): Promise<R> {
    return await client.query.compute.queryContract({
      contractAddress: this.contractInfo.address,
      codeHash: this.contractInfo.codeHash,
      query,
    });
  }

  private async getContractInfo(
    client: SecretNetworkClient,
    label: string,
  ): Promise<ContractDeployInfo | undefined> {
    let codes: CodeInfoResponse[];
    try {
      codes = await client.query.compute.codes();
    } catch {
      return;
    }

    for (const code of codes) {
      const codeId = Number.parseInt(code.codeId);
      let contractByCode;
      try {
        contractByCode = await client.query.compute.contractsByCode(codeId);
      } catch {
        continue;
      }

      for (const contract of contractByCode.contractInfos) {
        if (contract.ContractInfo && contract.ContractInfo.label == label) {
          return { address: contract.address, codeHash: code.codeHash };
        }
      }
    }
  }

  async deploy(
    client: SecretNetworkClient,
    initMsg: object,
    path: string,
  ) {
    const deployedContract = await this.getContractInfo(client, this.label);

    if (deployedContract != null) {
      this.contractInfo = deployedContract;
      return;
    }

    const wasmByteCode = fs.readFileSync(path);
    const transaction = await client.tx.compute.storeCode({
      wasmByteCode,
      sender: client.address,
      source: "",
      builder: "",
    }, { gasLimit: 5000000 });

    if (transaction.code != 0) {
      throw new Error(`Cannot deploy smart contract: "${transaction.rawLog}"`);
    }

    const log: JsonLog = JSON.parse(transaction.rawLog);
    const codeIdKeyValue = log[0].events[0].attributes.find((
      a: { key: string; value: string },
    ) => a.key == "code_id");

    if (codeIdKeyValue == null) {
      throw new Error("Cannot find code_id");
    }

    const codeId = Number.parseInt(codeIdKeyValue.value);
    const codeHash = await client.query.compute.codeHash(codeId);

    const contract = await client.tx.compute.instantiateContract({
      sender: client.address,
      codeId,
      initMsg,
      codeHash,
      label: this.label,
    }, { gasLimit: 150000 });

    if (contract.code != 0) {
      throw new Error(
        `Failed to instantiate the contract with the following error ${contract.rawLog}`,
      );
    }

    const info = await client.query.compute.contractsByCode(codeId);
    const address = info.contractInfos[0].address;

    this.contractInfo = { address, codeHash };
  }
}
