#!/bin/bash

# Build script for wasm-fullstack v2 - MCP Inspector compatible version
# This builds WITHOUT WasmEdge features for testing with standard WASM runtimes

set -e

# Color codes for output
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building wasm-fullstack v2 for MCP Inspector (without WasmEdge features)...${NC}"

# Set binary name
BINARY_NAME="wasm-fullstack-v2"
OUTPUT_NAME="${BINARY_NAME}_snapshot_preview.wasm"

# Build WITHOUT WasmEdge features for standard WASM compatibility
echo -e "${YELLOW}Note: Building without PostgreSQL support for MCP Inspector compatibility${NC}"
cargo build --target wasm32-wasip1 --release

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
    echo -e "${BLUE}Run with MCP Inspector:${NC}"
    echo "  npx @modelcontextprotocol/inspector wasmtime run ./$OUTPUT_NAME"
    echo ""
    echo -e "${YELLOW}Note: This build does NOT include PostgreSQL support.${NC}"
    echo -e "${YELLOW}For real PostgreSQL, use: ./build.sh (then run with WasmEdge)${NC}"
else
    echo -e "${RED}✗ Failed to create WASI component${NC}"
    exit 1
fi