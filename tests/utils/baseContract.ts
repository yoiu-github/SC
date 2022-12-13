import * as fs from "fs";
import { JsonLog, SecretNetworkClient } from "secretjs";
import { QueryContractAddressResponse } from "secretjs/dist/grpc_gateway/secret/compute/v1beta1/query.pb";

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

  setContractInfo(contractInfo: ContractDeployInfo) {
    this.contractInfo = contractInfo;
  }

  async query<T extends object, R extends object>(
    client: SecretNetworkClient,
    query: T
  ): Promise<R> {
    return await client.query.compute.queryContract({
      contract_address: this.contractInfo.address,
      code_hash: this.contractInfo.codeHash,
      query,
    });
  }

  private async getContractInfo(
    client: SecretNetworkClient,
    label: string
  ): Promise<ContractDeployInfo | undefined> {
    let contract: QueryContractAddressResponse;
    let contractAddress: string;

    try {
      contract = await client.query.compute.addressByLabel({ label });
      contractAddress = contract.contract_address!;
    } catch {
      return;
    }

    if (contractAddress == null) {
      return;
    }

    const codeHash = await client.query.compute.codeHashByContractAddress({
      contract_address: contractAddress,
    });

    return { address: contractAddress, codeHash: codeHash.code_hash! };
  }

  async deploy(client: SecretNetworkClient, initMsg: object, path: string) {
    const deployedContract = await this.getContractInfo(client, this.label);

    if (deployedContract != null) {
      this.contractInfo = deployedContract;
      return;
    }

    const wasmByteCode = fs.readFileSync(path);
    const transaction = await client.tx.compute.storeCode(
      {
        wasm_byte_code: wasmByteCode,
        sender: client.address,
        source: "",
        builder: "",
      },
      { gasLimit: 5000000 }
    );

    if (transaction.code != 0) {
      throw new Error(`Cannot deploy smart contract: "${transaction.rawLog}"`);
    }

    const log: JsonLog = JSON.parse(transaction.rawLog);
    const codeIdKeyValue = log[0].events[0].attributes.find(
      (a: { key: string; value: string }) => a.key == "code_id"
    );

    if (codeIdKeyValue == null) {
      throw new Error("Cannot find code_id");
    }

    const codeId = codeIdKeyValue.value;
    const codeHash = await client.query.compute
      .codeHashByCodeId({
        code_id: codeId,
      })
      .then((c) => c.code_hash!);

    const contract = await client.tx.compute.instantiateContract(
      {
        sender: client.address,
        code_id: codeId,
        init_msg: initMsg,
        code_hash: codeHash,
        label: this.label,
      },
      { gasLimit: 150000 }
    );

    if (contract.code != 0) {
      throw new Error(
        `Failed to instantiate the contract with the following error ${contract.rawLog}`
      );
    }

    const info = await client.query.compute.contractsByCodeId({
      code_id: codeId,
    });

    const address = info.contract_infos![0].contract_address!;
    this.contractInfo = { address, codeHash };
  }
}
