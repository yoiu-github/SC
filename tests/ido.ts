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
  waitFor,
} from "./utils";
import * as assert from "assert";

async function checkMaxDeposit(
  client: SecretNetworkClient,
  contract: Ido.IdoContract,
  idoId: number,
  price: number,
  maxPayments: number,
  tokenId?: string,
) {
  const maxTokensAmount = Math.floor(maxPayments / price);

  await contract.buyTokens(
    client,
    idoId,
    maxTokensAmount,
    price,
    tokenId,
  );

  await assert.rejects(async () => {
    await contract.buyTokens(
      client,
      idoId,
      1,
      price,
      tokenId,
    );
  });
}

describe("IDO", () => {
  let admin: SecretNetworkClient;
  let user: SecretNetworkClient;
  let idoOwner: SecretNetworkClient;

  let idoId: number;
  let price: number;
  let tokenId: string;

  const tierDeposits = ["100", "200", "500", "1000"];
  const tierLockPeriods = [30, 40, 50, 60];

  const idoPayments = ["1000", "2000", "3000", "5000", "10000"];
  const idoLockPeriods = [115, 90, 75, 50, 25];
  const idoTotalAmount = 20000;
  const tokensPerTier = ["10", "20", "30", "40", "19900"];

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
    user = await newClient();
    await airdrop(user);

    idoOwner = await newClient();
    await airdrop(idoOwner);

    snip20Contract = new Snip20.Snip20Contract("snip20");
    await snip20Contract.init(admin);
    await snip20Contract.mint(admin, idoOwner.address);

    price = 10;
    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time + 20,
        end_time: time + 10_000,
        token_contract: snip20Contract.contractInfo.address,
        token_contract_hash: snip20Contract.contractInfo.codeHash,
        price: price.toString(),
        total_amount: idoTotalAmount.toString(),
      },
    };

    const response = await idoContract.startIdo(
      idoOwner,
      startIdoMsg,
      snip20Contract.contractInfo,
    );

    idoId = response.start_ido.ido_id;
  });

  it("Try to buy tokens before IDO starts", async () => {
    await assert.rejects(async () => {
      await idoContract.buyTokens(
        user,
        idoId,
        1,
        price,
      );
    });
  });

  it("Try to buy tokens not being whitelisted", async () => {
    const idoInfo = await idoContract.idoInfo(idoOwner, idoId);
    await waitFor(idoInfo.ido_info.start_time);

    const response = await idoContract.inWhitelist(user, idoId);
    assert.ok(!response.in_whitelist.in_whitelist);

    await assert.rejects(async () => {
      await idoContract.buyTokens(
        user,
        idoId,
        1,
        price,
      );
    });
  });

  it("Buy tokens with Tier = 0", async () => {
    await idoContract.addWhitelist(admin, user.address);
    const whitelistResponse = await idoContract.inWhitelist(user, idoId);
    assert.ok(whitelistResponse.in_whitelist.in_whitelist);

    const maxPayments = Number.parseInt(idoPayments[0]);
    await checkMaxDeposit(
      user,
      idoContract,
      idoId,
      price,
      maxPayments,
    );

    const userInfo = await idoContract.userInfo(user);
    assert.equal(userInfo.user_info.total_payment, maxPayments);
    assert.equal(userInfo.user_info.total_tokens_bought, maxPayments / price);
    assert.equal(userInfo.user_info.total_tokens_received, 0);

    const userInfoIdo = await idoContract.userInfo(user, idoId);
    assert.deepEqual(userInfo, userInfoIdo);

    const balance = await sscrtContract.getBalance(idoOwner);
    const balanceNumber = Number.parseInt(balance.balance.amount);
    assert.equal(balanceNumber, maxPayments);

    const response = await idoContract.purchases(user, idoId);
    const purchases = response.purchases.purchases;
    const lastPurchase = purchases[purchases.length - 1];
    const amount = response.purchases.amount;

    assert.equal(lastPurchase.tokens_amount, maxPayments / price);
    assert.equal(amount, 1);
    assert.equal(
      lastPurchase.timestamp + idoLockPeriods[0],
      lastPurchase.unlock_time,
    );
  });

  for (let tier = 4; tier >= 1; tier--) {
    it(`Buy tokens with Tier = ${tier}`, async () => {
      await tierContract.setTier(user, tier);
      const tierUserInfo = await tierContract.userInfo(user);
      assert.equal(tierUserInfo.user_info.tier, tier);

      const tierIndex = idoPayments.length - tier;
      const lastMaxPayments = Number.parseInt(idoPayments[tierIndex - 1]);
      const currentMaxPayments = Number.parseInt(idoPayments[tierIndex]);
      const payment = currentMaxPayments - lastMaxPayments;

      await checkMaxDeposit(
        user,
        idoContract,
        idoId,
        price,
        payment,
      );

      const userInfo = await idoContract.userInfo(user);
      assert.equal(userInfo.user_info.total_payment, currentMaxPayments);
      assert.equal(userInfo.user_info.total_tokens_received, 0);
      assert.equal(
        userInfo.user_info.total_tokens_bought,
        currentMaxPayments / price,
      );

      const userInfoIdo = await idoContract.userInfo(user, idoId);
      assert.deepEqual(userInfo, userInfoIdo);

      const balance = await sscrtContract.getBalance(idoOwner);
      const balanceNumber = Number.parseInt(balance.balance.amount);
      assert.equal(balanceNumber, currentMaxPayments);

      const idoInfo = await idoContract.idoInfo(idoOwner, idoId);
      assert.equal(idoInfo.ido_info.total_payment, currentMaxPayments);

      const response = await idoContract.purchases(user, idoId);
      const purchases = response.purchases.purchases;
      const lastPurchase = purchases[purchases.length - 1];
      const amount = response.purchases.amount;

      assert.equal(lastPurchase.tokens_amount, payment / price);
      assert.equal(amount, tierIndex + 1);
      assert.equal(
        lastPurchase.timestamp + idoLockPeriods[tierIndex],
        lastPurchase.unlock_time,
      );
    });
  }

  it("Try to receive tokens before lock period", async () => {
    await assert.rejects(async () => {
      await idoContract.recvTokens(user, idoId);
    });
  });

  it("Receive tokens after lock period", async () => {
    const response = await idoContract.purchases(user, idoId);
    const maxUnlockTime = response.purchases.purchases.reduce(
      (max, value) => Math.max(max, value.unlock_time),
      0,
    );

    const purchasesBeforeReceive = await idoContract.purchases(user, idoId);
    await waitFor(maxUnlockTime);

    const initialBalance = await snip20Contract.getBalance(user);
    await idoContract.recvTokens(user, idoId);

    const balance = await snip20Contract.getBalance(user);
    assert.equal(
      balance.balance.amount,
      Number.parseInt(initialBalance.balance.amount) +
      Number.parseInt(idoPayments[4]) / price,
    );

    const purchases = await idoContract.purchases(user, idoId);
    assert.equal(purchases.purchases.amount, 0);
    assert.equal(purchases.purchases.purchases.length, 0);

    const archivedPurchases = await idoContract.archivedPurchases(user, idoId);
    assert.deepEqual(
      archivedPurchases.archived_purchases,
      purchasesBeforeReceive.purchases,
    );
  });

  it("Buy tokens with NFT (private metadata)", async () => {
    user = await newClient();
    await airdrop(user);

    await idoContract.addWhitelist(admin, user.address);

    const tier = 1;
    const maxPayments = Number.parseInt(idoPayments[4]);

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

    await checkMaxDeposit(
      user,
      idoContract,
      idoId,
      price,
      maxPayments,
      mintResponse.mint_nft.token_id,
    );
  });

  it("Buy tokens with NFT (public metadata)", async () => {
    user = await newClient();
    await airdrop(user);

    await idoContract.addWhitelist(admin, user.address);

    const tier = 1;
    const mintResponse = await nftContract.mint(admin, {
      mint_nft: {
        owner: user.address,
        public_metadata: {
          extension: {
            attributes: [{ value: "public trait" }, {
              trait_type: "TIER",
              value: tier.toString(),
            }],
          },
        },
        private_metadata: {
          extension: {
            attributes: [{ value: "trait" }, {
              trait_type: "color",
              value: "green",
            }],
          },
        },
      },
    });

    const maxPayments = Number.parseInt(idoPayments[4]);
    tokenId = mintResponse.mint_nft.token_id;

    await checkMaxDeposit(
      user,
      idoContract,
      idoId,
      price,
      maxPayments,
      tokenId,
    );
  });

  it("Try to buy tokens with someone's NFT", async () => {
    user = await newClient();
    await airdrop(user);

    await idoContract.addWhitelist(admin, user.address);

    // Tier = 0
    const maxPayments = Number.parseInt(idoPayments[0]);
    await checkMaxDeposit(
      user,
      idoContract,
      idoId,
      price,
      maxPayments,
      tokenId,
    );
  });

  it("Start IDO with specified tokens per tier", async () => {
    snip20Contract = new Snip20.Snip20Contract("another Snip20");
    await snip20Contract.init(admin);
    await snip20Contract.mint(admin, idoOwner.address);

    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time,
        end_time: time + 10_000,
        token_contract: snip20Contract.contractInfo.address,
        token_contract_hash: snip20Contract.contractInfo.codeHash,
        price: price.toString(),
        total_amount: idoTotalAmount.toString(),
        tokens_per_tier: tokensPerTier,
      },
    };

    const response = await idoContract.startIdo(
      idoOwner,
      startIdoMsg,
      snip20Contract.contractInfo,
    );

    idoId = response.start_ido.ido_id;
  });

  it("Buy tokens with Tier = 0", async () => {
    const maxPayments = Number.parseInt(tokensPerTier[0]) * price;
    await checkMaxDeposit(
      user,
      idoContract,
      idoId,
      price,
      maxPayments,
    );
  });

  for (let tier = 4; tier >= 1; tier--) {
    it(`Buy tokens with Tier = ${tier}`, async () => {
      await tierContract.setTier(user, tier);
      const tierIndex = idoPayments.length - tier;

      let maxPayments: number;
      if (tier == 1) {
        maxPayments = Number.parseInt(idoPayments[tierIndex]) - 1000;
      } else {
        maxPayments = Number.parseInt(tokensPerTier[tierIndex]) * price;
      }

      await checkMaxDeposit(
        user,
        idoContract,
        idoId,
        price,
        maxPayments,
      );
    });
  }
});
