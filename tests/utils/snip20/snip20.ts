import { SecretNetworkClient } from "secretjs";
import { ContractDeployInfo, deployContractIfNeeded, Snip20 } from "..";

export async function deploy(
  client: SecretNetworkClient,
  label = "snip20",
  initMsg?: Snip20.InitMsg,
  path?: string,
): Promise<ContractDeployInfo> {
  initMsg = initMsg || {
    name: "Snip20Token",
    symbol: "SNIP",
    decimals: 6,
    prng_seed: "seed",
    config: {
      enable_mint: true,
    },
  };

  path = path || "./build/snip20.wasm";
  return await deployContractIfNeeded(
    client,
    path,
    initMsg,
    label,
  );
}

export async function deploySscrt(
  client: SecretNetworkClient,
  label = "sscrt",
  initMsg?: Snip20.InitMsg,
  path?: string,
): Promise<ContractDeployInfo> {
  initMsg = initMsg || {
    name: "Tier token",
    symbol: "TTOKEN",
    decimals: 6,
    prng_seed: "seed",
  };

  path = path || "./build/sscrt.wasm";
  return await deployContractIfNeeded(
    client,
    path,
    initMsg,
    label,
  );
}
