import { SecretNetworkClient } from "secretjs";
import {
  broadcastWithCheck,
  ContractDeployInfo,
  deployContractIfNeeded,
  getContractWithCheck,
  getExecuteMsg,
  Tier,
} from "..";

export async function deploy(
  client: SecretNetworkClient,
  initMsg: Tier.InitMsg,
  label = "tier",
): Promise<ContractDeployInfo> {
  return await deployContractIfNeeded(
    client,
    "./build/tier.wasm",
    initMsg,
    label,
  );
}

export async function setTier(
  client: SecretNetworkClient,
  tier: number,
  label = "tier",
) {
  const tierContract = await getContractWithCheck(client, label);
  const queryTierOf: Tier.QueryMsg.TierOf = {
    tier_of: { address: client.address },
  };

  const tierOfResponse: Tier.QueryAnswer.TierOf = await client.query.compute
    .queryContract({
      contractAddress: tierContract.address,
      codeHash: tierContract.codeHash,
      query: queryTierOf,
    });

  const currentTier = tierOfResponse.tier_of.tier;
  if (currentTier == tier) {
    return;
  }

  if (currentTier > tier) {
    throw new Error("Tier cannot be decreased");
  }

  const queryInfoMsg: Tier.QueryMsg.TierInfo = { tier_info: {} };
  const tierInfoResponse: Tier.QueryAnswer.TierInfo = await client.query.compute
    .queryContract({
      contractAddress: tierContract.address,
      codeHash: tierContract.codeHash,
      query: queryInfoMsg,
    });

  const tierInfo = tierInfoResponse.tier_info.tier_list[tier - 1];
  const tierExpectedDeposit = Number.parseInt(tierInfo.deposit);

  const queryDepositOf: Tier.QueryMsg.DepositOf = {
    deposit_of: { address: client.address },
  };

  const depositOfResponse: Tier.QueryAnswer.DepositOf = await client.query
    .compute
    .queryContract({
      contractAddress: tierContract.address,
      codeHash: tierContract.codeHash,
      query: queryDepositOf,
    });

  const currentDeposit = Number.parseInt(depositOfResponse.deposit_of.deposit);
  const amount = tierExpectedDeposit - currentDeposit;

  const depositMsg = getExecuteMsg<Tier.HandleMsg.Deposit>(
    tierContract,
    client.address,
    { deposit: {} },
    [
      {
        denom: "uscrt",
        amount: amount.toString(),
      },
    ],
  );

  return await broadcastWithCheck(client, [depositMsg]);
}
