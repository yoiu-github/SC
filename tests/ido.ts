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
  Snip721,
  Tier,
} from "./utils";

describe("Buy tokens", () => {
  let admin: SecretNetworkClient;
  let user: SecretNetworkClient;

  let idoContractInfo: ContractDeployInfo;
  let ido_id: number;

  it("Deploy IDO contract", async () => {
    admin = await getAdmin();
    await airdrop(admin);

    user = await newClient();
    await airdrop(user);

    const validators = await admin.query.staking.validators({});
    const validator = validators.validators[0].operatorAddress;

    const initTierMsg: Tier.InitMsg = {
      validator,
      deposits: ["100", "200", "500", "1000"],
      lock_periods: [10, 20, 30, 40],
    };

    const tierContractInfo = await Tier.deploy(admin, initTierMsg);
    const sscrtContractInfo = await Snip20.deploySscrt(admin);
    const nftContractInfo = await Snip721.deploy(admin);

    const initIdoMsg: Ido.InitMsg = {
      max_payments: ["10", "20", "30", "40", "50"],
      lock_periods: [10, 20, 30, 40, 50],
      tier_contract: tierContractInfo.address,
      tier_contract_hash: tierContractInfo.codeHash,
      nft_contract: nftContractInfo.address,
      nft_contract_hash: nftContractInfo.codeHash,
      token_contract: sscrtContractInfo.address,
      token_contract_hash: sscrtContractInfo.codeHash,
    };

    idoContractInfo = await Ido.deploy(admin, initIdoMsg);
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
        end_time: time + 60,
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

  it("Buy tokens with tier contract", async () => {
    const investor = await newClient();
    await airdrop(investor);

    await Tier.setTier(investor, 3);
    await Ido.addWhitelist(admin, investor.address);

    await Ido.buyTokens(
      investor,
      ido_id,
      30,
    );
  });

  it("Buy tokens with nft", async () => {
    const investor = await newClient();
    await airdrop(investor);

    await Ido.addWhitelist(admin, investor.address);
    const mintResponse = await Snip721.mint(admin, {
      mint_nft: {
        owner: investor.address,
        private_metadata: {
          extension: {
            attributes: [{ value: "trait" }, {
              trait_type: "color",
              value: "green",
            }, { trait_type: "tier", value: "3" }],
          },
        },
      },
    });

    await Ido.buyTokens(
      investor,
      ido_id,
      30,
      mintResponse.mint_nft.token_id,
    );
  });
});
