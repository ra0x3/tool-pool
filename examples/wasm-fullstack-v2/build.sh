#!/bin/bash

# Build script for wasm-fullstack v2
# Supports both standard WASM (simulated) and WasmEdge (real PostgreSQL/HTTP)

set -e

# Color codes for output
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building wasm-fullstack v2...${NC}"

# Set binary name
BINARY_NAME="wasm-fullstack-v2"
OUTPUT_NAME="${BINARY_NAME}_snapshot_preview.wasm"

# Always build with WasmEdge features since v2 is specifically for WasmEdge
echo -e "${BLUE}Building for WasmEdge with PostgreSQL support enabled...${NC}"
RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" cargo build --target wasm32-wasip1 --release --features wasmedge-postgres

echo ""
echo -e "${GREEN}✓ Built for WasmEdge!${NC}"
echo ""

# Note: Cannot create WASI component when using WasmEdge-specific features (sockets)
# The module uses WasmEdge extensions that aren't compatible with standard WASI adapters

echo -e "${BLUE}Run with WasmEdge (for real PostgreSQL):${NC}"
echo "  docker-compose up -d  # Start PostgreSQL"
echo "  DATABASE_URL=\"postgres://postgres@localhost/todos_db\" \\"
echo "  wasmedge --env DATABASE_URL target/wasm32-wasip1/release/${BINARY_NAME}.wasm"
echo ""
echo -e "${YELLOW}Note: This build uses WasmEdge-specific socket extensions.${NC}"
echo -e "${YELLOW}For standard WASM runtimes, use wasm-fullstack-v1 instead.${NC}"
