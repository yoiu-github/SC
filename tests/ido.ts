import { SecretNetworkClient } from "secretjs";
import {
  airdrop,
  broadcastWithCheck,
  ContractDeployInfo,
  currentTime,
  getAdmin,
  getExecuteMsg,
  Ido,
  newClient,
  Snip20,
  Tier,
} from "./utils";

describe("Deploy", () => {
  let admin: SecretNetworkClient;
  let tierContractInfo: ContractDeployInfo;
  let sscrtContractInfo: ContractDeployInfo;
  let idoContractInfo: ContractDeployInfo;
  let ido_id: number;

  it("Initialize client", async () => {
    admin = await getAdmin();
    await airdrop(admin);
  });

  it("Deploy Tier contract", async () => {
    const validators = await admin.query.staking.validators({});
    const validator = validators.validators[0].operatorAddress;

    const initMsg: Tier.InitMsg = {
      validator,
      deposits: ["100", "200", "500", "1000"],
      lock_periods: [10, 20, 30, 40],
    };

    tierContractInfo = await Tier.deploy(admin, initMsg);
  });

  it("Deploy IDO contract", async () => {
    sscrtContractInfo = await Snip20.deploySscrt(admin);

    const initMsg: Ido.InitMsg = {
      max_payments: ["10", "20", "30", "40"],
      lock_periods: [10, 20, 30, 40],
      tier_contract: tierContractInfo.address,
      tier_contract_hash: tierContractInfo.codeHash,
      nft_contract: sscrtContractInfo.address,
      nft_contract_hash: sscrtContractInfo.codeHash,
      token_contract: sscrtContractInfo.address,
      token_contract_hash: sscrtContractInfo.codeHash,
    };

    idoContractInfo = await Ido.deploy(admin, initMsg);
  });

  it("Start IDO", async () => {
    const idoOwner = await newClient();
    await airdrop(idoOwner);

    const snip20 = await Snip20.deploy(admin);

    const mintMsg = getExecuteMsg<Snip20.HandleMsg>(snip20, admin.address, {
      mint: { recipient: idoOwner.address, amount: "100000000" },
    });

    await broadcastWithCheck(admin, [mintMsg]);

    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg = {
      start_ido: {
        start_time: time,
        end_time: time + 30,
        token_contract: snip20.address,
        token_contract_hash: snip20.codeHash,
        price: "1",
        total_amount: "20000",
      },
    };

    const messages = [];
    messages.push(
      getExecuteMsg<Snip20.HandleMsg>(snip20, idoOwner.address, {
        increase_allowance: {
          spender: idoContractInfo.address,
          amount: "20000",
        },
      }),
    );
    messages.push(
      getExecuteMsg(idoContractInfo, idoOwner.address, startIdoMsg),
    );

    const data = await broadcastWithCheck(idoOwner, messages);
    ido_id = data[1].start_ido.ido_id;
  });

  it("Buy tokens", async () => {
    const investor = await newClient();
    await airdrop(investor);

    await Tier.setTier(investor, 2);
    await Ido.addWhitelist(admin, investor.address);

    await Ido.buyTokens(
      investor,
      ido_id,
      30,
    );
  });
});
