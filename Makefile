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
build: _build schema token.wasm.gz
_build:
	RUSTFLAGS='-C link-arg=-s' cargo build --release --target wasm32-unknown-unknown --locked
ifdef WASM_OPT
	wasm-opt -Oz ./target/wasm32-unknown-unknown/release/snip721_tier_token.wasm -o ./build/token.wasm
else
	mkdir -p ./build
	cp ./target/wasm32-unknown-unknown/release/snip721_tier_token.wasm ./build/token.wasm
endif

.PHONY: compile-optimized-reproducible
compile-optimized-reproducible:
	docker run --rm -v "$$(pwd)":/contract \
		--mount type=volume,source="$$(basename "$$(pwd)")_cache",target=/code/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		enigmampc/secret-contract-optimizer:1.0.6

token.wasm.gz: ./build/token.wasm
	cat ./build/token.wasm | gzip -9 > ./build/token.wasm.gz

.PHONY: start-server
start-server: # CTRL+C to stop
	docker run --rm -it -p 9091:9091 -p 26657:26657 -p 1317:1317 -p 5000:5000 \
		--name localsecret ghcr.io/scrtlabs/localsecret:v1.4.0-cw-v1-beta.2

.PHONY: schema
schema:
	cargo run --example schema

.PHONY: clean
clean:
	cargo clean
	rm -rf ./build/
