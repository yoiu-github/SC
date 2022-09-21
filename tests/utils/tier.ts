import { SecretNetworkClient } from "secretjs";
import { ContractDeployInfo, deployContractIfNeeded } from "./utils";

export type TierInitMsg = {
  owner?: string;
  validator: string;
  deposits: string[];
  lock_periods: number[];
};

export async function deployTier(
  client: SecretNetworkClient,
  initMsg: TierInitMsg,
  label = "tier",
): Promise<ContractDeployInfo> {
  return await deployContractIfNeeded(
    client,
    "./build/tier.wasm",
    initMsg,
    label,
  );
}
