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
build: _build compress schema
_build:
	cargo build --release --target wasm32-unknown-unknown
	mkdir --parents ./build
ifdef WASM_OPT
	wasm-opt -Oz ./target/wasm32-unknown-unknown/release/tier.wasm -o ./build/tier.wasm
	wasm-opt -Oz ./target/wasm32-unknown-unknown/release/ido.wasm -o ./build/ido.wasm
else
	cp ./target/wasm32-unknown-unknown/release/tier.wasm ./build/tier.wasm
	cp ./target/wasm32-unknown-unknown/release/ido.wasm ./build/ido.wasm
endif

.PHONY: start-server
start-server:
	docker-compose up -d

.PHONY: stop-server
stop-server:
	docker-compose down

.PHONY: compress
compress: tier.wasm.gz ido.wasm.gz

tier.wasm.gz: build/tier.wasm
	cat ./build/tier.wasm | gzip -9 > ./build/tier.wasm.gz

ido.wasm.gz: build/ido.wasm
	cat ./build/ido.wasm | gzip -9 > ./build/ido.wasm.gz

.PHONY: schema
schema:
	cargo run --release --example schema-tier
	cargo run --release --example schema-ido

.PHONY: clean
clean:
	cargo clean
	rm -rf ./build/
