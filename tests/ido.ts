import { SecretNetworkClient } from "secretjs";
import {
  airdrop,
  broadcastWithCheck,
  ContractDeployInfo,
  currentTime,
  deployIdo,
  deploySnip20,
  deployTier,
  getAdmin,
  getExecuteMsg,
  IdoExecuteMsg,
  IdoInitMsg,
  newClient,
  Snip20ExecuteMsg,
  Snip20InitMsg,
  TierInitMsg,
} from "./utils";

describe("Deploy", () => {
  let admin: SecretNetworkClient;
  let tierContractInfo: ContractDeployInfo;
  let sscrtContractInfo: ContractDeployInfo;
  let idoContractInfo: ContractDeployInfo;

  it("Initialize client", async () => {
    admin = await getAdmin();
    await airdrop(admin);
  });

  it("Deploy Tier contract", async () => {
    const initMsg: TierInitMsg = {
      validator: "",
      deposits: ["100", "200", "500", "1000"],
      lock_periods: [10, 20, 30, 40],
    };

    tierContractInfo = await deployTier(admin, initMsg);
  });

  it("Deploy SSCRT contract", async () => {
    const initMsg: Snip20InitMsg = {
      name: "Tier token",
      symbol: "TTOKEN",
      decimals: 6,
      prng_seed: "seed",
    };

    sscrtContractInfo = await deploySnip20(
      admin,
      "sscrt",
      initMsg,
      "./build/sscrt.wasm",
    );
  });

  it("Deploy IDO contract", async () => {
    const initMsg: IdoInitMsg = {
      max_payments: ["10", "20", "30", "40"],
      lock_periods: [10, 20, 30, 40],
      tier_contract: tierContractInfo.address,
      tier_contract_hash: tierContractInfo.codeHash,
      nft_contract: sscrtContractInfo.address,
      nft_contract_hash: sscrtContractInfo.codeHash,
      token_contract: sscrtContractInfo.address,
      token_contract_hash: sscrtContractInfo.codeHash,
    };

    idoContractInfo = await deployIdo(admin, initMsg);
  });

  it("Start IDO", async () => {
    const idoOwner = await newClient();
    await airdrop(idoOwner);

    const snip20 = await deploySnip20(admin);

    const mintMsg = getExecuteMsg<Snip20ExecuteMsg>(snip20, admin.address, {
      mint: { recipient: idoOwner.address, amount: "100000000" },
    });

    await broadcastWithCheck(admin, [mintMsg]);

    const time = currentTime();
    const startIdoMsg: IdoExecuteMsg = {
      start_ido: {
        start_time: time,
        end_time: time + 20,
        token_contract: snip20.address,
        token_contract_hash: snip20.codeHash,
        price: "1000",
        total_amount: "20000",
      },
    };

    const messages = [];
    messages.push(
      getExecuteMsg<Snip20ExecuteMsg>(snip20, idoOwner.address, {
        increase_allowance: {
          spender: idoContractInfo.address,
          amount: "20000",
        },
      }),
    );
    messages.push(
      getExecuteMsg(idoContractInfo, idoOwner.address, startIdoMsg),
    );

    await broadcastWithCheck(idoOwner, messages);
  });
});
