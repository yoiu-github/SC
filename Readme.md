# Test

Run `make test` to test smart contracts

# Build

Run `make build` to build smart contract. Wasm files will be located in the
`./build` directory.

# Tier

The smart contract accepts delegations from users to define their `Tier`. `Tier`
value defines the amount of tokens the buyer can buy in the IDO contract.

## Deploy

Check secretcli version:

```bash
secretcli version
# Should return 1.4.0 or higher
```

Run:

```bash
WALLET="my wallet name"
WALLET_ADDRESS="my wallet address"
TIER_LABEL="tier contract"
VALIDATOR="secretvaloper1p0re3rp685fqsngfdvxg34wkwu9am2p4ckeq2h"

# Find code_id value from the output
secretcli tx compute store ./build/tier.wasm \
    --gas 1500000                            \
    --from "$WALLET"                         \
    --yes

# example code id
TIER_CODE_ID="165020"

secretcli tx compute instantiate                     \
    "$TIER_CODE_ID"                                  \
    '{
        "validator": "'"${VALIDATOR}"'",
        "deposits": ["500", "750", "5000", "10000"],
        "lock_periods": [60, 120, 180, 240]
    }'                                               \
    --gas 1500000                                    \
    --from "$WALLET"                                 \
    --label "$TIER_LABEL"                            \
    --yes
```

Check the initialization with:

```bash
# It will print the smart contract address
secretcli query compute list-contract-by-code "$TIER_CODE_ID"

TIER_ADDRESS=$(secretcli query compute list-contract-by-code "$TIER_CODE_ID" |
    jq -r '.[-1].contract_address')
```

## Usage

To deposit some SCRT, run:

```bash
secretcli tx compute execute "$TIER_ADDRESS" \
    '{ "deposit": {} }'                      \
    --from "$WALLET"                         \
    --amount 5000uscrt                       \
    --yes
```

To check your tier:

```bash
secretcli q compute query "$TIER_ADDRESS" \
    '{ "user_info": {"address":"'"$WALLET_ADDRESS"'"} }'

# {"user_info":{"tier":2,"deposit":"5000","withdraw_time":1664866566,"timestamp":1664866386}}
```

To withdraw your SCRT:

```bash
secretcli tx compute execute "$TIER_ADDRESS" \
    '{ "withdraw": {} }'                     \
    --from "$WALLET"                         \
    --yes
```

Claim your money after unbound period:

```bash
secretcli tx compute execute "$TIER_ADDRESS" \
    '{ "claim": {} }'                        \
    --from "$WALLET"                         \
    --yes
```

# IDO

The smart contract for the IDO platform.

## Deploy

Run:

```bash
secretcli tx compute store ./build/ido.wasm \
    --gas 2500000                           \
    --from "$WALLET"                        \
    --yes

# example code id
IDO_CODE_ID="165021"
```

Instantiate contract:

```bash
NFT_ADDRESS="nft contract address"

# testnet
SSCRT_ADDRESS="secret1umwqjum7f4zmp9alr2kpmq4y5j4hyxlam896r3"
# mainnet
# SSCRT_CONTRACT="secret1k0jntykt7e4g3y88ltc60czgjuqdy4c9e8fzek"

NFT_CONTRACT_HASH=$(secretcli query compute contract-hash "${NFT_ADDRESS}")
TIER_CONTRACT_HASH=$(secretcli query compute contract-hash "${TIER_ADDRESS}")
SSCRT_CONTRACT_HASH=$(secretcli query compute contract-hash "${SSCRT_ADDRESS}")

secretcli tx compute instantiate                             \
    "$IDO_CODE_ID"                                           \
    '{
        "max_payments": ["5000", "7500", "50000", "100000"],
        "lock_periods": [1728000, 1728000, 1728000, 864000],
        "nft_contract": "'"${NFT_ADDRESS}"'",
        "nft_contract_hash": "'"${NFT_CONTRACT_HASH}"'",
        "tier_contract": "'"${TIER_ADDRESS}"'",
        "tier_contract_hash": "'"${TIER_CONTRACT_HASH}"'",
        "token_contract": "'"${SSCRT_ADDRESS}"'",
        "token_contract_hash": "'"${SSCRT_CONTRACT_HASH}"'"
    }'                                                       \
    --gas 2000000                                            \
    --from "$WALLET"                                         \
    --label "$IDO_LABEL"                                     \
    --yes
```

Check the initialization with:

```bash
# It will print the smart contract address
secretcli query compute list-contract-by-code "$IDO_CODE_ID"

IDO_ADDRESS=$(secretcli query compute list-contract-by-code "$IDO_CODE_ID" |
    jq -r '.[-1].contract_address')
```

## Usage

Create IDO:

```bash
AMOUNT=10000

secretcli tx compute execute "$SNIP_20_ADDRESS"    \
    '{
        "increase_allowance": {
            "spender": "'"$IDO_ADDRESS"'",
            "amount": "'"$AMOUNT"'"
        }
    }'                                             \
    --from "$WALLET"                               \
    --yes

secretcli tx compute execute "$IDO_ADDRESS"                \
    '{
        "start_ido": {
            "start_time": 1234,
            "end_time": 1234,
            "total_amount": "'"$AMOUNT"'",
            "price": "100",
            "token_contract": "snip 20 contract address",
            "token_contract_hash": "snip 20 contract hash"
        }
    }'                                                     \
    --from "$WALLET"                                       \
    --yes
```

Add whitelist:

```bash
secretcli tx compute execute "$IDO_ADDRESS" \
    '{
        "whitelist_add": {
            "addresses": ["user address"],
            "ido_id": ido_id
        }
    }'                                      \
    --from "$WALLET"                        \
    --yes
```

Buy some tokens:

```bash
IDO_ID=0
AMOUNT=20

# amount * price
MONEY=2000

secretcli tx compute execute "$SSCRT_ADDRESS" \
    '{ "deposit": {} }'                       \
    --from "$WALLET"                          \
    --amount "${MONEY}"uscrt                  \
    --yes

secretcli tx compute execute "$SSCRT_ADDRESS" \
    '{
        "increase_allowance": {
            "spender": "'"$IDO_ADDRESS"'",
            "amount": "'"$MONEY"'"
        }
    }'                                        \
    --from "$WALLET"                          \
    --yes

secretcli tx compute execute "$IDO_ADDRESS" \
    '{
        "buy_tokens": {
            "amount": "'"$AMOUNT"'",
            "ido_id": '"$IDO_ID"'
        }
    }'                                      \
    --from "$WALLET"                        \
    --gas 2500000                           \
    --yes
```

Receive tokens after lock period:

```bash
secretcli tx compute execute "$IDO_ADDRESS" \
    '{
        "recv_tokens": {
            "ido_id": '"$IDO_ID"'
        }
    }'                                      \
    --from "$WALLET"                        \
    --gas 2500000                           \
    --yes
```
