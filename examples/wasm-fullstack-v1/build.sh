#!/bin/bash

# Build script for wasm-fullstack v1
# Simple in-memory version without database dependencies

set -e

# Color codes for output
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building wasm-fullstack v1...${NC}"

# Build with release mode for smaller binary
cargo build --target wasm32-wasip1 --release

# Set output names using canonical naming
BINARY_NAME="wasm-fullstack-v1"
OUTPUT_NAME="${BINARY_NAME}_snapshot_preview.wasm"

# Check for wasm-tools
if ! command -v wasm-tools &> /dev/null; then
    echo -e "${YELLOW}⚠ wasm-tools not found. Installing...${NC}"
    cargo install wasm-tools
fi

# Check for WASI adapter and download if needed
if [ ! -f "wasi_snapshot_preview1.command.wasm" ]; then
    echo -e "${YELLOW}⚠ WASI adapter not found. Downloading...${NC}"
    curl -LO https://github.com/bytecodealliance/wasmtime/releases/latest/download/wasi_snapshot_preview1.command.wasm
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ WASI adapter downloaded successfully${NC}"
    else
        echo -e "${RED}✗ Failed to download WASI adapter${NC}"
        exit 1
    fi
fi

# Create component
echo -e "${BLUE}Creating WASI component with canonical name: $OUTPUT_NAME${NC}"
wasm-tools component new "target/wasm32-wasip1/release/${BINARY_NAME}.wasm" \
    -o "$OUTPUT_NAME" \
    --adapt wasi_snapshot_preview1.command.wasm

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ Built and created component: $OUTPUT_NAME${NC}"
    echo ""
    echo -e "${BLUE}Run with:${NC}"
    echo "  npx @modelcontextprotocol/inspector wasmtime run ./$OUTPUT_NAME"
    echo "  # or"
    echo "  wasmtime run ./$OUTPUT_NAME"
else
    echo -e "${RED}✗ Failed to create WASI component${NC}"
    exit 1
fi