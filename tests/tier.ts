import * as assert from "assert";
import { SecretNetworkClient } from "secretjs";
import { airdrop, getAdmin, newClient, Tier, waitFor } from "./utils";

describe("Tier", () => {
  let admin: SecretNetworkClient;
  let user: SecretNetworkClient;

  const tierDeposits = ["100", "200", "500", "1000"];
  const tierLockPeriods = [30, 40, 50, 20];
  const tierContract = new Tier.TierContract("Tier contract");
  let initialDelegation: number;
  let validator: string;

  it("Deploy Tier contract", async () => {
    admin = await getAdmin();
    await airdrop(admin);

    const validators = await admin.query.staking.validators({});
    validator = validators.validators[0].operatorAddress;

    const initTierMsg: Tier.InitMsg = {
      validator,
      deposits: tierDeposits,
      lock_periods: tierLockPeriods,
    };

    await tierContract.init(admin, initTierMsg);

    try {
      const delegation = await admin.query.staking.delegation({
        delegatorAddr: tierContract.contractInfo.address,
        validatorAddr: validator,
      });

      initialDelegation = Number.parseInt(
        delegation.delegationResponse?.balance?.amount || "0",
      );
    } catch {
      initialDelegation = 0;
    }
  });

  it("Deposit with wrong denom", async () => {
    user = await newClient();
    await airdrop(user);

    await assert.rejects(async () => {
      await tierContract.deposit(user, 100, "sscrt");
    });
  });

  it("Deposit less than min amount", async () => {
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
    assert.equal(
      userInfo.user_info.withdraw_time,
      userInfo.user_info.timestamp + tierLockPeriods[0],
    );
  });

  it("Tier 3", async () => {
    await tierContract.deposit(user, 100);
    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 3);
    assert.equal(userInfo.user_info.deposit, 200);
    assert.equal(
      userInfo.user_info.withdraw_time,
      userInfo.user_info.timestamp + tierLockPeriods[1],
    );
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
    assert.equal(
      userInfo.user_info.withdraw_time,
      userInfo.user_info.timestamp + tierLockPeriods[2],
    );
  });

  it("Tier 1", async () => {
    await tierContract.deposit(user, 500_000);
    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 1);
    assert.equal(userInfo.user_info.deposit, 1000);
    assert.equal(
      userInfo.user_info.withdraw_time,
      userInfo.user_info.timestamp + tierLockPeriods[3],
    );

    const delegation = await user.query.staking.delegation({
      delegatorAddr: tierContract.contractInfo.address,
      validatorAddr: validator,
    });

    assert.equal(
      delegation.delegationResponse?.balance?.amount,
      initialDelegation + Number.parseInt(tierDeposits[3]),
    );
  });

  it("Try to increase tier", async () => {
    await assert.rejects(async () => {
      await tierContract.deposit(user, 500_000);
    });
  });

  it("Withdraw tokens before lock period", async () => {
    await assert.rejects(async () => {
      await tierContract.withdraw(user);
    });
  });

  it("Withdraw tokens after lock period", async () => {
    let userInfo = await tierContract.userInfo(user);
    await waitFor(userInfo.user_info.withdraw_time);

    await tierContract.withdraw(user);
    userInfo = await tierContract.userInfo(user);

    assert.equal(userInfo.user_info.tier, 0);
    assert.equal(userInfo.user_info.deposit, 0);
    assert.equal(userInfo.user_info.timestamp, 0);
    assert.equal(userInfo.user_info.withdraw_time, 0);

    const withdrawals = await tierContract.withdrawals(user);
    const withdrawal = withdrawals.withdrawals.withdrawals[0];
    assert.equal(withdrawals.withdrawals.amount, 1);
    assert.equal(withdrawal.amount, 1000);

    let currentDelegation: number;

    try {
      const delegation = await user.query.staking.delegation({
        delegatorAddr: tierContract.contractInfo.address,
        validatorAddr: validator,
      });

      currentDelegation = Number.parseInt(
        delegation.delegationResponse?.balance?.amount || "0",
      );
    } catch {
      currentDelegation = 0;
    }

    assert.equal(currentDelegation, initialDelegation);
  });

  it("Deposit after withdraw", async () => {
    await tierContract.deposit(user, 500_000);
    const userInfo = await tierContract.userInfo(user);
    assert.equal(userInfo.user_info.tier, 1);
    assert.equal(userInfo.user_info.deposit, 1000);
    assert.equal(
      userInfo.user_info.withdraw_time,
      userInfo.user_info.timestamp + tierLockPeriods[3],
    );

    const withdrawals = await tierContract.withdrawals(user);
    const withdrawal = withdrawals.withdrawals.withdrawals[0];
    assert.equal(withdrawals.withdrawals.amount, 1);
    assert.equal(withdrawal.amount, 1000);

    const delegation = await user.query.staking.delegation({
      delegatorAddr: tierContract.contractInfo.address,
      validatorAddr: validator,
    });

    assert.equal(
      delegation.delegationResponse?.balance?.amount,
      initialDelegation + Number.parseInt(tierDeposits[3]),
    );
  });
});
