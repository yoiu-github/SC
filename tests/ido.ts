import { SecretNetworkClient } from "secretjs";
import {
  airdrop,
  currentTime,
  getAdmin,
  Ido,
  newClient,
  Snip20,
  Snip721,
  Tier,
} from "./utils";
import * as assert from "assert";

describe("IDO", () => {
  let admin: SecretNetworkClient;
  let idoOwner: SecretNetworkClient;

  let ido_id: number;

  const tierDeposits = ["100", "200", "500", "1000"];
  const tierLockPeriods = [30, 40, 50, 60];

  const idoPayments = ["1000", "2000", "3000", "5000", "10000"];
  const idoLockPeriods = [20, 30, 40, 50, 60];

  let idoContract: Ido.IdoContract;
  let snip20Contract: Snip20.Snip20Contract;

  const tierContract = new Tier.TierContract("Tier");
  const nftContract = new Snip721.Snip721Contract("Snip721");
  const sscrtContract = new Snip20.Snip20Contract("SSCRT", "sscrt");

  it("Deploy IDO contract", async () => {
    admin = await getAdmin();
    await airdrop(admin);

    const validators = await admin.query.staking.validators({});
    const validator = validators.validators[0].operatorAddress;

    const initTierMsg: Tier.InitMsg = {
      validator,
      deposits: tierDeposits,
      lock_periods: tierLockPeriods,
    };

    await nftContract.init(admin);
    await tierContract.init(admin, initTierMsg);
    await sscrtContract.init(admin);

    const initIdoMsg: Ido.InitMsg = {
      max_payments: idoPayments,
      lock_periods: idoLockPeriods,
      tier_contract: tierContract.contractInfo.address,
      tier_contract_hash: tierContract.contractInfo.codeHash,
      nft_contract: nftContract.contractInfo.address,
      nft_contract_hash: nftContract.contractInfo.codeHash,
      token_contract: sscrtContract.contractInfo.address,
      token_contract_hash: sscrtContract.contractInfo.codeHash,
    };

    idoContract = new Ido.IdoContract(
      "IDO",
      sscrtContract.contractInfo,
      nftContract.contractInfo,
    );

    await idoContract.init(admin, initIdoMsg);
  });

  it("Start IDO", async () => {
    idoOwner = await newClient();
    await airdrop(idoOwner);

    snip20Contract = new Snip20.Snip20Contract("snip20");
    await snip20Contract.init(admin);
    await snip20Contract.mint(admin, idoOwner.address);

    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time,
        end_time: time + 60,
        token_contract: snip20Contract.contractInfo.address,
        token_contract_hash: snip20Contract.contractInfo.codeHash,
        price: "1",
        total_amount: "20000",
      },
    };

    const response = await idoContract.startIdo(
      idoOwner,
      startIdoMsg,
      snip20Contract.contractInfo,
    );

    ido_id = response.start_ido.ido_id;
  });

  it("Buy tokens with tier contract", async () => {
    const user = await newClient();
    await airdrop(user);

    const tier = 2;
    await tierContract.setTier(user, tier);
    await idoContract.addWhitelist(admin, user.address);

    const tierIndex = idoPayments.length - tier;
    const maxPayments = Number.parseInt(idoPayments[tierIndex]);

    await assert.rejects(async () => {
      await idoContract.buyTokens(
        user,
        ido_id,
        maxPayments + 1,
      );
    });

    await idoContract.buyTokens(
      user,
      ido_id,
      maxPayments,
    );

    const balance = await sscrtContract.getBalance(idoOwner);
    const balanceNumber = Number.parseInt(balance.balance.amount);
    assert.equal(balanceNumber, maxPayments);
  });

  it("Buy tokens with nft with private metadata", async () => {
    const user = await newClient();
    await airdrop(user);

    const tier = 4;
    const tierIndex = idoPayments.length - tier;
    const maxPayments = Number.parseInt(idoPayments[tierIndex]);

    await idoContract.addWhitelist(admin, user.address);
    const mintResponse = await nftContract.mint(admin, {
      mint_nft: {
        owner: user.address,
        private_metadata: {
          extension: {
            attributes: [{ value: "trait" }, {
              trait_type: "color",
              value: "green",
            }, { trait_type: "tier", value: tier.toString() }],
          },
        },
      },
    });

    await assert.rejects(async () => {
      await idoContract.buyTokens(
        user,
        ido_id,
        maxPayments + 1,
      );
    });

    await idoContract.buyTokens(
      user,
      ido_id,
      maxPayments,
      mintResponse.mint_nft.token_id,
    );
  });
});
