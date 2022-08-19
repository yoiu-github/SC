SECRETCLI := docker exec -it secretdev /usr/bin/secretcli
WASM_OPT := $(shell wasm-opt --version 2>/dev/null)

.PHONY: all
all: clippy test

.PHONY: check
check:
	cargo check

.PHONY: clippy
clippy:
	cargo clippy

.PHONY: test
test:
	cargo test

.PHONY: list-code
list-code:
	$(SECRETCLI) query compute list-code

.PHONY: build _build
build: _build schema
_build:
	RUSTFLAGS='-C link-arg=-s' cargo build --release --target wasm32-unknown-unknown --locked --package snip721-tier-token
	RUSTFLAGS='-C link-arg=-s' cargo build --release --target wasm32-unknown-unknown --locked --package tier
ifdef WASM_OPT
	wasm-opt -Oz ./target/wasm32-unknown-unknown/release/snip721_tier_token.wasm -o ./build/token.wasm
	wasm-opt -Oz ./target/wasm32-unknown-unknown/release/tier.wasm -o ./build/tier.wasm
else
	mkdir -p ./build
	cp ./target/wasm32-unknown-unknown/release/snip721_tier_token.wasm ./build/token.wasm
	cp ./target/wasm32-unknown-unknown/release/tier.wasm ./build/tier.wasm
endif

.PHONY: compile-optimized-reproducible
compile-optimized-reproducible:
	docker run --rm -v "$$(pwd)":/contract \
		--mount type=volume,source="$$(basename "$$(pwd)")_cache",target=/code/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		enigmampc/secret-contract-optimizer:1.0.6

.PHONY: start-server
start-server: # CTRL+C to stop
	docker run -it -p 9091:9091 -p 26657:26657 -p 1317:1317 -p 5000:5000 \
		-v $(pwd):/root/code \
		--name localsecret ghcr.io/scrtlabs/localsecret:v1.4.0-cw-v1-beta.2

.PHONY: schema
schema:
	cargo run --bin schema-builder

.PHONY: clean
clean:
	cargo clean
	rm -rf ./build/
