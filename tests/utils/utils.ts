import axios from "axios";
import {
  Coin,
  Msg,
  MsgExecuteContract,
  SecretNetworkClient,
  Wallet,
} from "secretjs";
import { ContractDeployInfo } from "./baseContract";

export function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function waitFor(timestamp: number) {
  const currentTimestamp = currentTime();
  const waitTime = timestamp - currentTimestamp + 1;
  return new Promise((resolve) => setTimeout(resolve, waitTime * 1000));
}

export function currentTime(): number {
  return Math.floor(new Date().getTime() / 1000);
}

export async function broadcastWithCheck(
  client: SecretNetworkClient,
  messages: Msg[],
  gasLimit = 200_000,
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
  address?: string,
): Promise<number> {
  address = address || client.address;
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
