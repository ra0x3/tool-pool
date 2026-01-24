#!/bin/bash

# Build script for calculator
# Simple calculator MCP server using WASI Component Model (preview 2) for Wasmtime runtime

set -e

# Color codes for output
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

show_help() {
    cat << EOF
${BLUE}Calculator Build Script${NC}

Builds a simple calculator MCP server as a WASI Component Model (preview 2) module
for Wasmtime runtime.

${BLUE}Usage:${NC}
  ./build.sh [OPTIONS]

${BLUE}Options:${NC}
  -h, --help    Show this help message

${BLUE}Examples:${NC}
  # Build the calculator module
  ./build.sh

${BLUE}Running the binary:${NC}
  # With MCP Inspector
  npx @modelcontextprotocol/inspector wasmtime run ./calculator.wasm

  # Direct execution
  wasmtime run ./calculator.wasm

${BLUE}Prerequisites:${NC}
  - Rust with wasm32-wasip2 target (rustup target add wasm32-wasip2)
  - Wasmtime runtime (optional for direct execution)

EOF
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

printf "${BLUE}=== Building Calculator ===${NC}\n"
echo ""

printf "${BLUE}Building calculator (WASI Component Model for Wasmtime)...${NC}\n"

# Build with release mode for smaller binary
# This is a cdylib that exports WASI Component Model interfaces
cargo build --target wasm32-wasip2 --release

# Set output name
OUTPUT_NAME="calculator.wasm"

# The workspace target directory is four levels up now
TARGET_DIR="../../../../target"

# The output is already a component when building for wasm32-wasip2
if [ -f "${TARGET_DIR}/wasm32-wasip2/release/calculator.wasm" ]; then
    cp "${TARGET_DIR}/wasm32-wasip2/release/calculator.wasm" "$OUTPUT_NAME"
    printf "${GREEN}✓ Built WASI Component: $OUTPUT_NAME${NC}\n"
else
    echo -e "${RED}✗ Failed to build WASI component${NC}"
    echo -e "${YELLOW}Note: This example uses WASI Component Model (preview 2)${NC}"
    echo -e "${YELLOW}Make sure you have the wasm32-wasip2 target installed:${NC}"
    echo "  rustup target add wasm32-wasip2"
    exit 1
fi

echo ""
printf "${GREEN}✓ Build complete!${NC}\n"
echo ""
printf "${BLUE}Built binary:${NC}\n"
echo "  calculator.wasm"
echo ""
printf "${BLUE}========================================${NC}\n"
printf "${BLUE}     DEPLOYMENT OPTIONS${NC}\n"
printf "${BLUE}========================================${NC}\n"
echo ""
printf "${GREEN}Mode 1: Docker Testing (Everything Automated)${NC}\n"
echo "----------------------------------------"
echo "Complete containerized deployment with automatic setup."
echo "Builds WASM module, installs Wasmtime, and launches Inspector UI."
echo ""
echo "  # Start calculator with Inspector UI"
echo "  docker-compose up"
echo ""
echo "  # Access Inspector at: http://localhost:5173"
echo ""
echo "  # Stop services"
echo "  docker-compose down"
echo ""
echo ""
printf "${GREEN}Mode 2: Manual Testing (Direct Execution)${NC}\n"
echo "----------------------------------------"
echo "Run the WASM module directly with your local Wasmtime installation."
echo "Perfect for development and debugging with live code changes."
echo ""
echo "  # Run with MCP Inspector"
echo "  npx @modelcontextprotocol/inspector wasmtime run ./calculator.wasm"
echo ""
echo "  # Or run directly with Wasmtime"
echo "  wasmtime run ./calculator.wasm"
echo ""