import { SecretNetworkClient } from "secretjs";
import {
  Band,
  currentTime,
  getAdmin,
  getBalance,
  getUser,
  Ido,
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
  tokenId?: string
) {
  const maxTokensAmount = Math.floor(maxPayments / price);
  await contract.buyTokens(client, idoId, maxTokensAmount, tokenId);
  await assert.rejects(async () => {
    await contract.buyTokens(client, idoId, 1, tokenId);
  });
}

describe("IDO", () => {
  let admin: SecretNetworkClient;
  let user: SecretNetworkClient;
  let idoOwner: SecretNetworkClient;

  let idoId: number;
  let price: number;
  let tokenId: string;

  const tierDeposits = ["1000", "500", "200", "100"];

  const idoPayments = ["10000", "5000", "3000", "2000", "1000"];
  const idoLockPeriods = [30, 60, 90, 120, 150];
  const idoTotalAmount = 20000;
  const tokensPerTier = ["19900", "40", "30", "20", "10"];

  let idoContract: Ido.IdoContract;

  const totalIdoPayment = idoPayments.reduce(
    (s, value) => s + Number.parseInt(value),
    0
  );

  const endpoint = "https://api.pulsar.scrttestnet.com";
  const chainId = "pulsar-2";

  const tierContract = new Tier.Contract();
  const idoToken = new Snip20.Snip20Contract();
  const paymentToken = new Snip20.Snip20Contract();
  const nftContract = new Snip721.Snip721Contract();
  const bandContract = new Band.Contract();

  const mintTo = async (
    user: SecretNetworkClient,
    amount: number,
    token = paymentToken
  ) => {
    await token.mint(admin, user.address, amount);
    await token.increaseAllowance(
      user,
      idoContract.contractInfo.address,
      amount
    );
  };

  idoToken.setContractInfo({
    address: "secret1kenn60zdhlqu6lc0gmjjwmswrfvglep6lswupm",
    codeHash:
      "eedf6770184f2ebfa4d331cdc7cb3c0f6bfffa8847567cc5b1ff8c0edf462736",
  });

  paymentToken.setContractInfo({
    address: "secret1txd47t355qqjvseg4hetcp524gnsejhg4vxvx4",
    codeHash:
      "eedf6770184f2ebfa4d331cdc7cb3c0f6bfffa8847567cc5b1ff8c0edf462736",
  });

  nftContract.setContractInfo({
    address: "secret159hrs96qs5cqug6asc8c0svzgwpfpn9gdgx3x8",
    codeHash:
      "a41f4cedabcb4585ab263ae014ef654ec6fd4f9cfc9f51dcade69efbdf514db7",
  });

  bandContract.setContractInfo({
    address: "secret14swdnnllsfvtnvwmtvnvcj2zu0njsl9cdkk5xp",
    codeHash:
      "00230665fa8dc8bb3706567cf0a61f282edc34d2f7df56192b2891fd9cd27b06",
  });

  it("Deploy IDO contract", async () => {
    admin = await getAdmin(endpoint, chainId);

    const validators = await admin.query.staking.validators({});
    const validator = validators.validators![0].operator_address!;

    const initTierMsg: Tier.InitMsg = {
      validator,
      deposits: tierDeposits,
      band_oracle: bandContract.contractInfo.address,
      band_code_hash: bandContract.contractInfo.codeHash,
    };

    await tierContract.init(admin, initTierMsg);

    const initIdoMsg: Ido.InitMsg = {
      max_payments: idoPayments,
      lock_periods: idoLockPeriods,
      tier_contract: tierContract.contractInfo.address,
      tier_contract_hash: tierContract.contractInfo.codeHash,
      nft_contract: nftContract.contractInfo.address,
      nft_contract_hash: nftContract.contractInfo.codeHash,
    };

    idoContract = new Ido.IdoContract(nftContract.contractInfo);
    await idoContract.init(admin, initIdoMsg);
  });

  it("Start IDO", async () => {
    idoOwner = await getUser(endpoint, chainId, 0);
    await mintTo(idoOwner, idoTotalAmount, idoToken);

    price = 10;
    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time + 20,
        end_time: time + 10_000,
        token_contract: idoToken.contractInfo.address,
        token_contract_hash: idoToken.contractInfo.codeHash,
        price: price.toString(),
        total_amount: idoTotalAmount.toString(),
        whitelist: { empty: {} },
        payment: {
          token: {
            contract: paymentToken.contractInfo.address,
            code_hash: paymentToken.contractInfo.codeHash,
          },
        },
      },
    };

    const response = await idoContract.startIdo(idoOwner, startIdoMsg);
    idoId = response.start_ido.ido_id;
  });

  it("Try to buy tokens before IDO starts", async () => {
    user = await getUser(endpoint, chainId, 1);

    await mintTo(user, totalIdoPayment);
    await assert.rejects(async () => {
      await idoContract.buyTokens(user, idoId, 1);
    });
  });

  it("Try to buy tokens not being whitelisted", async () => {
    const idoInfo = await idoContract.idoInfo(idoOwner, idoId);
    await waitFor(idoInfo.ido_info.start_time);

    const response = await idoContract.inWhitelist(user, idoId);
    assert.ok(!response.in_whitelist.in_whitelist);

    await assert.rejects(async () => {
      await idoContract.buyTokens(user, idoId, 1);
    });
  });

  it("Add user to whitelist", async () => {
    await idoContract.addWhitelist(idoOwner, user.address, idoId);
  });

  for (let tier = 5; tier >= 1; tier--) {
    it(`Buy tokens with Tier = ${tier}`, async () => {
      await tierContract.setTier(user, tier, bandContract);
      const tierUserInfo = await tierContract.userInfo(user);
      assert.equal(tierUserInfo.user_info.tier, tier);

      const tierIndex = tier - 1;
      const lastMaxPayments = Number.parseInt(
        idoPayments.at(tierIndex + 1) || "0"
      );

      const currentMaxPayments = Number.parseInt(idoPayments[tierIndex]);
      const payment = currentMaxPayments - lastMaxPayments;

      const initialIdoOwnerBalance = await paymentToken.getBalance(idoOwner);
      await checkMaxDeposit(user, idoContract, idoId, price, payment);

      const userInfo = await idoContract.userInfo(user);
      assert.equal(userInfo.user_info.total_payment, currentMaxPayments);
      assert.equal(userInfo.user_info.total_tokens_received, 0);
      assert.equal(
        userInfo.user_info.total_tokens_bought,
        currentMaxPayments / price
      );

      const userInfoIdo = await idoContract.userInfo(user, idoId);
      assert.deepEqual(userInfo, userInfoIdo);

      const balance = await paymentToken.getBalance(idoOwner);
      assert.equal(balance - initialIdoOwnerBalance, payment);

      const idoInfo = await idoContract.idoInfo(idoOwner, idoId);
      assert.equal(idoInfo.ido_info.total_payment, currentMaxPayments);

      const response = await idoContract.purchases(user, idoId);
      const purchases = response.purchases.purchases;
      const lastPurchase = purchases[purchases.length - 1];
      const amount = response.purchases.amount;

      assert.equal(lastPurchase.tokens_amount, payment / price);
      assert.equal(amount, 5 - tierIndex);
      assert.equal(
        lastPurchase.timestamp + idoLockPeriods[tierIndex],
        lastPurchase.unlock_time
      );
    });
  }

  it("Try to receive tokens before lock period", async () => {
    await assert.rejects(async () => {
      console.log(await idoContract.recvTokens(user, idoId));
    });
  });

  it("Receive tokens after lock period", async () => {
    const response = await idoContract.purchases(user, idoId);
    const maxUnlockTime = response.purchases.purchases.reduce(
      (max, value) => Math.max(max, value.unlock_time),
      0
    );

    const purchasesBeforeReceive = await idoContract.purchases(user, idoId);
    await waitFor(maxUnlockTime);

    const initialBalance = await idoToken.getBalance(user);
    await idoContract.recvTokens(user, idoId);

    const balance = await idoToken.getBalance(user);
    assert.equal(
      balance,
      initialBalance + Number.parseInt(idoPayments[0]) / price
    );

    const purchases = await idoContract.purchases(user, idoId);
    assert.equal(purchases.purchases.amount, 0);
    assert.equal(purchases.purchases.purchases.length, 0);

    const archivedPurchases = await idoContract.archivedPurchases(user, idoId);
    assert.deepEqual(
      archivedPurchases.archived_purchases,
      purchasesBeforeReceive.purchases
    );
  });

  it("Buy tokens with NFT (private metadata)", async () => {
    const mintAmount = Number.parseInt(idoPayments[0]);
    user = await getUser(endpoint, chainId, 2);

    await mintTo(user, mintAmount);
    await idoContract.addWhitelist(idoOwner, user.address, idoId);

    const tier = 1;
    const maxPayments = Number.parseInt(idoPayments[0]);

    const mintResponse = await nftContract.mint(admin, {
      mint_nft: {
        owner: user.address,
        private_metadata: {
          extension: {
            attributes: [
              { value: "trait" },
              {
                trait_type: "color",
                value: "green",
              },
              { trait_type: "tier", value: tier.toString() },
            ],
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
      mintResponse.mint_nft.token_id
    );
  });

  it("Buy tokens with NFT (public metadata)", async () => {
    const mintAmount = Number.parseInt(idoPayments[0]);
    user = await getUser(endpoint, chainId, 3);

    await idoContract.addWhitelist(idoOwner, user.address, idoId);
    await mintTo(user, mintAmount);

    const tier = 1;
    const mintResponse = await nftContract.mint(admin, {
      mint_nft: {
        owner: user.address,
        public_metadata: {
          extension: {
            attributes: [
              { value: "public trait" },
              {
                trait_type: "TIER",
                value: tier.toString(),
              },
            ],
          },
        },
        private_metadata: {
          extension: {
            attributes: [
              { value: "trait" },
              {
                trait_type: "color",
                value: "green",
              },
            ],
          },
        },
      },
    });

    const maxPayments = Number.parseInt(idoPayments[0]);
    tokenId = mintResponse.mint_nft.token_id;

    await checkMaxDeposit(
      user,
      idoContract,
      idoId,
      price,
      maxPayments,
      tokenId
    );
  });

  it("Try to buy tokens with someone's NFT", async () => {
    user = await getUser(endpoint, chainId, 0);
    await idoContract.addWhitelist(idoOwner, user.address, idoId);

    const mintAmount = Number.parseInt(idoPayments[0]);
    await mintTo(user, mintAmount);

    // Tier = 5
    const maxPayments = Number.parseInt(idoPayments[4]);
    await checkMaxDeposit(
      user,
      idoContract,
      idoId,
      price,
      maxPayments,
      tokenId
    );
  });

  it("Start IDO with specified tokens per tier", async () => {
    await paymentToken.mint(admin, idoOwner.address);
    await mintTo(idoOwner, idoTotalAmount, idoToken);
    await mintTo(idoOwner, totalIdoPayment);

    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time,
        end_time: time + 10_000,
        token_contract: idoToken.contractInfo.address,
        token_contract_hash: idoToken.contractInfo.codeHash,
        price: price.toString(),
        total_amount: idoTotalAmount.toString(),
        tokens_per_tier: tokensPerTier,
        whitelist: { empty: { with: [user.address] } },
        payment: {
          token: {
            contract: paymentToken.contractInfo.address,
            code_hash: paymentToken.contractInfo.codeHash,
          },
        },
      },
    };

    const response = await idoContract.startIdo(idoOwner, startIdoMsg);
    idoId = response.start_ido.ido_id;
  });

  for (let tier = 5; tier >= 1; tier--) {
    it(`Buy tokens with Tier = ${tier}`, async () => {
      await tierContract.setTier(user, tier, bandContract);
      const tierIndex = tier - 1;

      let maxPayments: number;
      if (tier == 1) {
        maxPayments = Number.parseInt(idoPayments[tierIndex]) - 1000;
      } else {
        maxPayments = Number.parseInt(tokensPerTier[tierIndex]) * price;
      }

      await checkMaxDeposit(user, idoContract, idoId, price, maxPayments);
    });
  }

  it("Start IDO with native payment", async () => {
    idoOwner = await getUser(endpoint, chainId, 0);
    await mintTo(idoOwner, idoTotalAmount, idoToken);

    price = 2;
    const time = currentTime();
    const startIdoMsg: Ido.HandleMsg.StartIdo = {
      start_ido: {
        start_time: time,
        end_time: time + 10_000,
        token_contract: idoToken.contractInfo.address,
        token_contract_hash: idoToken.contractInfo.codeHash,
        price: price.toString(),
        total_amount: idoTotalAmount.toString(),
        payment: "native",
        whitelist: { empty: {} },
      },
    };

    const response = await idoContract.startIdo(idoOwner, startIdoMsg);
    idoId = response.start_ido.ido_id;
  });

  it("Buy some tokens", async () => {
    user = await getUser(endpoint, chainId, 1);
    await idoContract.addWhitelist(idoOwner, user.address, idoId);

    const initialIdoOwnerBalance = await getBalance(idoOwner);
    await idoContract.buyTokens(user, idoId, 1);

    const balance = await getBalance(idoOwner);
    assert.equal(balance - initialIdoOwnerBalance, 1);
  });
});
