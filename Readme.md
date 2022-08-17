# Test

Run `make test` to test smart contracts

# Build

Run `make build` to build smart contract. Wasm files will be located in
the `./build` directory.

# Nft token

The smart contract implements the SNIP-721 standard. In addition, each minted
token has addiitonal field `Tier`. Minters can mint tokens with the tier value,
change the tier, and query the tier value of a token.

## Deploy

[Set
up](https://docs.scrt.network/secret-network-documentation/development/getting-started/setting-up-your-environment)
your environment. Initialize a wallet with `secretcli keys add <wallet-name>`.
Configure the `secretcli` utility (select node, chain-id, etc.).

Run:

```bash
secretcli tx compute store ./build/token.wasm --gas 5000000 --from <wallet-name>
```

`secretcli query compute list-code` will return the code id. Initialize the
contract. All parameters are presented
[here](https://github.com/baedrik/snip721-reference-impl#Instantiating-The-Token-Contract).
There is an additional parameter `config.max_tier_value` with the default
value = 4.

```bash
secretcli tx compute instantiate <code-id> '{ "name": "My token contract", "symbol": "MYNFT", "entropy": "random string" }' --from <wallet-name> --label <label>
```

Check the initialization with:

```bash
# It will print the smart contract address
secretcli query compute list-contract-by-code <code-id>
```

## Usage

To mint tokens with the tier, run:

```bash
secretcli tx compute execute <smart-contract-address> '{ "mint_nft": { "token_id": "NFT", "tier": 3 } }' --from <wallet-name>
```

To change the tier, run:

```bash
secretcli tx compute execute <smart-contract-address> '{ "set_tier": { "token_id": "NFT", "tier": 2 } }' --from <wallet-name>
```

To query the tier, run:

```bash
secretcli q compute query <smart-contract-address> '{ "tier_of": { "token_id": "NFT" } }'
```
