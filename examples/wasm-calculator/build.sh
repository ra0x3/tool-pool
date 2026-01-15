#!/bin/bash

# Build script for wasm-calculator
# Simple calculator MCP server using WASI Component Model (preview 2)

set -e

# Color codes for output
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building wasm-calculator (WASI Component Model)...${NC}"

# Build with release mode for smaller binary
# This is a cdylib that exports WASI Component Model interfaces
cargo build --target wasm32-wasip2 --release

# Set output name
OUTPUT_NAME="wasm-calculator.wasm"

# The workspace target directory is two levels up
TARGET_DIR="../../target"

# The output is already a component when building for wasm32-wasip2
if [ -f "${TARGET_DIR}/wasm32-wasip2/release/wasm_calculator.wasm" ]; then
    cp "${TARGET_DIR}/wasm32-wasip2/release/wasm_calculator.wasm" "$OUTPUT_NAME"
    echo ""
    echo -e "${GREEN}✓ Built WASI Component: $OUTPUT_NAME${NC}"
    echo ""
    echo -e "${BLUE}Run with:${NC}"
    echo "  npx @modelcontextprotocol/inspector wasmtime run ./$OUTPUT_NAME"
    echo "  # or"
    echo "  wasmtime run ./$OUTPUT_NAME"
else
    echo -e "${RED}✗ Failed to build WASI component${NC}"
    echo -e "${YELLOW}Note: This example uses WASI Component Model (preview 2)${NC}"
    echo -e "${YELLOW}Make sure you have the wasm32-wasip2 target installed:${NC}"
    echo "  rustup target add wasm32-wasip2"
    exit 1
fi