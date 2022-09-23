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

describe("Buy tokens", () => {
  let admin: SecretNetworkClient;
  let idoOwner: SecretNetworkClient;

  let ido_id: number;

  const tierDeposits = ["100", "200", "500", "1000"];
  const tierLockPeriods = [30, 40, 50, 60];

  const idoPayments = ["1000", "2000", "3000", "5000", "10000"];
  const idoLockPeriods = [20, 30, 40, 50, 60];

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

    const tierContractInfo = await Tier.deploy(admin, initTierMsg);
    const sscrtContractInfo = await Snip20.deploySscrt(admin);
    const nftContractInfo = await Snip721.deploy(admin);

    const initIdoMsg: Ido.InitMsg = {
      max_payments: idoPayments,
      lock_periods: idoLockPeriods,
      tier_contract: tierContractInfo.address,
      tier_contract_hash: tierContractInfo.codeHash,
      nft_contract: nftContractInfo.address,
      nft_contract_hash: nftContractInfo.codeHash,
      token_contract: sscrtContractInfo.address,
      token_contract_hash: sscrtContractInfo.codeHash,
    };

    await Ido.deploy(admin, initIdoMsg);
  });

  it("Start IDO", async () => {
    idoOwner = await newClient();
    await airdrop(idoOwner);

    const snip20 = await Snip20.deploy(admin);
    await Snip20.mint(admin, idoOwner.address);

    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time,
        end_time: time + 60,
        token_contract: snip20.address,
        token_contract_hash: snip20.codeHash,
        price: "1",
        total_amount: "20000",
      },
    };

    const response = await Ido.startIdo(idoOwner, startIdoMsg);
    ido_id = response.start_ido.ido_id;
  });

  it("Buy tokens with tier contract", async () => {
    const user = await newClient();
    await airdrop(user);

    const tier = 2;
    // TODO: replace 3 => tier
    await Tier.setTier(user, 3);
    await Ido.addWhitelist(admin, user.address);

    const tierIndex = idoPayments.length - tier;
    const maxPayments = Number.parseInt(idoPayments[tierIndex]);

    await assert.rejects(async () => {
      await Ido.buyTokens(
        user,
        ido_id,
        maxPayments + 1,
      );
    });

    await Ido.buyTokens(
      user,
      ido_id,
      maxPayments,
    );

    const balance = await Snip20.getBalance(idoOwner, "sscrt");
    const balanceNumber = Number.parseInt(balance.balance.amount);
    assert.equal(balanceNumber, maxPayments);
  });

  it("Buy tokens with nft with private metadata", async () => {
    const user = await newClient();
    await airdrop(user);

    const tier = 4;
    const tierIndex = idoPayments.length - tier;
    const maxPayments = Number.parseInt(idoPayments[tierIndex]);

    await Ido.addWhitelist(admin, user.address);
    const mintResponse = await Snip721.mint(admin, {
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
      await Ido.buyTokens(
        user,
        ido_id,
        maxPayments + 1,
      );
    });

    await Ido.buyTokens(
      user,
      ido_id,
      maxPayments,
      mintResponse.mint_nft.token_id,
    );
  });
});
