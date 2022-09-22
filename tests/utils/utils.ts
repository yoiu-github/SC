import * as fs from "fs";
import axios from "axios";
import {
  CodeInfoResponse,
  Coin,
  JsonLog,
  Msg,
  MsgExecuteContract,
  SecretNetworkClient,
  Wallet,
} from "secretjs";

export function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function waitFor(timestamp: number) {
  const currentTimestamp = currentTime();
  const waitTime = timestamp - currentTimestamp;
  return new Promise((resolve) => setTimeout(resolve, waitTime * 1000));
}

export function currentTime(): number {
  return Math.floor(new Date().getTime() / 1000);
}

export async function broadcastWithCheck(
  client: SecretNetworkClient,
  messages: Msg[],
  gasLimit = 150_000,
) {
  const transaction = await client.tx.broadcast(messages, { gasLimit });
  if (transaction.code != 0) {
    throw new Error(transaction.rawLog);
  }

  return transaction.data.map((d) =>
    JSON.parse(Buffer.from(d).toString("utf8"))
  );
}

export async function airdrop(
  client: SecretNetworkClient,
  address?: string,
  url = "http://localhost:5000",
) {
  address = address || client.address;
  const initial_balance = await getBalance(client, address);
  await axios.get(`${url}/faucet?address=${address}`);

  let balance = await getBalance(client, address);
  while (balance == initial_balance) {
    await delay(100);
    balance = await getBalance(client, address);
  }
}

export async function getBalance(
  client: SecretNetworkClient,
  address: string,
): Promise<number> {
  const response = await client.query.bank.balance({ address, denom: "uscrt" });
  return response.balance && Number.parseInt(response.balance.amount) || 0;
}

export async function newClient(
  endpoint = "http://localhost:9091",
  chainId = "secretdev-1",
  mnemonic?: string,
): Promise<SecretNetworkClient> {
  const wallet = new Wallet(mnemonic);
  const accAddress = wallet.address;

  const client = await SecretNetworkClient.create({
    grpcWebUrl: endpoint,
    chainId: chainId,
    wallet: wallet,
    walletAddress: accAddress,
  });

  return client;
}

export async function getAdmin(
  endpoint = "http://localhost:9091",
  chainId = "secretdev-1",
): Promise<SecretNetworkClient> {
  const mnemonic = [
    "liquid",
    "poet",
    "polar",
    "arrive",
    "embody",
    "steak",
    "athlete",
    "cloth",
    "reopen",
    "divorce",
    "bundle",
    "yard",
    "collect",
    "click",
    "rug",
    "able",
    "secret",
    "maximum",
    "valid",
    "nephew",
    "recall",
    "speak",
    "mammal",
    "more",
  ];

  return await newClient(endpoint, chainId, mnemonic.join(" "));
}

export type ContractDeployInfo = {
  address: string;
  codeHash: string;
};

export async function getContractInfo(
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

export async function getContractWithCheck(
  client: SecretNetworkClient,
  label: string,
): Promise<ContractDeployInfo> {
  const contractInfo = await getContractInfo(client, label);
  if (contractInfo == null) {
    throw new Error(`Deploy contract ${label}`);
  }

  return contractInfo;
}

export async function deployContractIfNeeded(
  client: SecretNetworkClient,
  path: string,
  initMsg: object,
  label: string,
): Promise<ContractDeployInfo> {
  const deployedContract = await getContractInfo(client, label);

  if (deployedContract != null) {
    return deployedContract;
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
    label,
  }, { gasLimit: 100000 });

  if (contract.code != 0) {
    throw new Error(
      `Failed to instantiate the contract with the following error ${contract.rawLog}`,
    );
  }

  const info = await client.query.compute.contractsByCode(codeId);
  const address = info.contractInfos[0].address;

  return { address, codeHash };
}

export function getExecuteMsg<T extends object>(
  contract: ContractDeployInfo,
  sender: string,
  msg: T,
  sentFunds?: Coin[],
): MsgExecuteContract<T> {
  return new MsgExecuteContract({
    sender,
    contractAddress: contract.address,
    codeHash: contract.codeHash,
    msg,
    sentFunds,
  });
}
