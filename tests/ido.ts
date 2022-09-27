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
    user = await newClient();
    await airdrop(user);
    await idoContract.addWhitelist(admin, user.address);

    idoOwner = await newClient();
    await airdrop(idoOwner);

    snip20Contract = new Snip20.Snip20Contract("snip20");
    await snip20Contract.init(admin);
    await snip20Contract.mint(admin, idoOwner.address);

    price = 10;
    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time,
        end_time: time + 10_000,
        token_contract: snip20Contract.contractInfo.address,
        token_contract_hash: snip20Contract.contractInfo.codeHash,
        price: price.toString(),
        total_amount: "20000",
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
});
