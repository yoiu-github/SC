import * as assert from "assert";
import { SecretNetworkClient } from "secretjs";
import { airdrop, getAdmin, newClient, Tier } from "./utils";

describe("Tier", () => {
  let admin: SecretNetworkClient;
  let user: SecretNetworkClient;

  const tierDeposits = ["100", "200", "500", "1000"];
  const tierLockPeriods = [30, 40, 50, 60];
  const tierContract = new Tier.TierContract("Tier contract");

  it("Deploy Tier contract", async () => {
    admin = await getAdmin();
    await airdrop(admin);

    const validators = await admin.query.staking.validators({});
    const validator = validators.validators[0].operatorAddress;

    const initTierMsg: Tier.InitMsg = {
      validator,
      deposits: tierDeposits,
      lock_periods: tierLockPeriods,
    };

    await tierContract.init(admin, initTierMsg);
  });

  it("Deposit with wrong denom", async () => {
    user = await newClient();
    await airdrop(user);

    await assert.rejects(async () => {
      await tierContract.deposit(user, 100, "sscrt");
    });
  });

  it("Deposit less than min amount", async () => {
    user = await newClient();
    await airdrop(user);

    await assert.rejects(async () => {
      await tierContract.deposit(user, 99);
    });

    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 0);
    assert.equal(userInfo.user_info.deposit, 0);
  });

  it("Tier 4", async () => {
    await tierContract.deposit(user, 100);
    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 4);
    assert.equal(userInfo.user_info.deposit, 100);
  });

  it("Tier 3", async () => {
    await tierContract.deposit(user, 100);
    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 3);
    assert.equal(userInfo.user_info.deposit, 200);
  });

  it("Try to change status with user", async () => {
    await assert.rejects(async () => {
      await tierContract.changeStatus(user, "stopped");
    });
  });

  it("Change status to stopped", async () => {
    await tierContract.changeStatus(admin, "stopped");
    const config = await tierContract.config(user);
    assert.equal(config.config.status, "stopped");
  });

  it("Try to deposit with stopped contract", async () => {
    await assert.rejects(async () => {
      await tierContract.deposit(user, 300);
    });
  });

  it("Change status to active", async () => {
    await tierContract.changeStatus(admin, "active");
    const config = await tierContract.config(user);
    assert.equal(config.config.status, "active");
  });

  it("Tier 2", async () => {
    await tierContract.deposit(user, 300);
    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 2);
    assert.equal(userInfo.user_info.deposit, 500);
  });

  it("Tier 1", async () => {
    await tierContract.deposit(user, 5000);
    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 1);
    assert.equal(userInfo.user_info.deposit, 1000);
  });
});
