#!/bin/bash
# Build script to compile the calculator to WASM

# Ensure we have the wasm32-wasip1 target installed
rustup target add wasm32-wasip1

# Build the WASM module
cargo build --target wasm32-wasip1 --release

# Copy the compiled WASM to the test fixtures directory
mkdir -p ../../tests/fixtures/wasm-tools/calculator
cp target/wasm32-wasip1/release/calculator_wasm.wasm ../../tests/fixtures/wasm-tools/calculator/calculator.wasm

echo "Calculator WASM module built successfully!"
echo "Output: ../../tests/fixtures/wasm-tools/calculator/calculator.wasm"