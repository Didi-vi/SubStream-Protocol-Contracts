#!/bin/bash
# Generate TypeScript bindings for the SubStream Soroban contract

CONTRACT_WASM="./target/wasm32-unknown-unknown/release/substream_contracts.wasm"
OUTPUT_DIR="./bindings/substream"

# 1. Build contract if wasm doesn't exist
if [ ! -f "$CONTRACT_WASM" ]; then
    echo "WASM not found. Building contract..."
    cargo build --target wasm32-unknown-unknown --release
fi

# 2. Check for stellar-cli
if ! command -v stellar &> /dev/null
then
    echo "stellar-cli could not be found. Please install it with 'cargo install --locked stellar-cli'"
    exit 1
fi

# 3. Generate bindings
echo "Generating TypeScript bindings for $CONTRACT_WASM..."
mkdir -p $OUTPUT_DIR
stellar contract bindings typescript --wasm $CONTRACT_WASM --output-dir $OUTPUT_DIR --overwrite

echo "Bindings generated in $OUTPUT_DIR"
