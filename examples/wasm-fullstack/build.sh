#!/bin/bash

# Build script for wasm-fullstack v2
# Builds MCP servers with PostgreSQL support for WasmEdge runtime

set -e

# Color codes for output
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
TRANSPORT="stdio"  # Default to stdio for better module isolation
FEATURES="wasmedge-postgres"

show_help() {
    cat << EOF
${BLUE}wasm-fullstack Build Script${NC}

Builds MCP server binaries with PostgreSQL support for WasmEdge runtime.

${BLUE}Usage:${NC}
  ./build.sh [OPTIONS]

${BLUE}Options:${NC}
  -h, --help              Show this help message
  -t, --transport TYPE    Transport type: stdio, http, or both (default: stdio)
                          stdio: stdin/stdout transport (better for parallel modules)
                          http:  HTTP transport (multi-tenant, network-accessible)
                          both:  Build both binaries
  -f, --features          Comma-separated features (default: wasmedge-postgres)

${BLUE}Examples:${NC}
  # Build both binaries with PostgreSQL support
  ./build.sh

  # Build only the HTTP server
  ./build.sh --transport http

  # Build only the stdio server
  ./build.sh --transport stdio

  # Build without PostgreSQL (mock mode)
  ./build.sh --features ""

${BLUE}Running the binaries:${NC}
  # stdio transport
  npx @modelcontextprotocol/inspector wasmedge target/wasm32-wasip1/release/wasm-fullstack-stdio.wasm

  # HTTP transport
  wasmedge target/wasm32-wasip1/release/wasm-fullstack-http.wasm
  # Then access at: http://127.0.0.1:8080/mcp

${BLUE}Prerequisites:${NC}
  - WasmEdge runtime installed
  - PostgreSQL running (docker-compose up -d)
  - Rust with wasm32-wasip1 target (rustup target add wasm32-wasip1)

EOF
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -t|--transport)
            TRANSPORT="$2"
            shift 2
            ;;
        -f|--features)
            FEATURES="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Validate transport
if [[ ! "$TRANSPORT" =~ ^(stdio|http|both)$ ]]; then
    echo -e "${RED}Error: Invalid transport '$TRANSPORT'. Must be 'stdio', 'http', or 'both'${NC}"
    exit 1
fi

build_binary() {
    local mode=$1
    local binary_name="wasm-fullstack-${mode}"
    printf "${BLUE}Building ${binary_name} for WasmEdge...${NC}\n"

    if [ -n "$FEATURES" ]; then
        RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" \
            cargo build --bin "$binary_name" --target wasm32-wasip1 --release --features "$FEATURES"
    else
        RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" \
            cargo build --bin "$binary_name" --target wasm32-wasip1 --release
    fi

    printf "${GREEN}✓ Built ${binary_name}.wasm${NC}\n"
}

printf "${BLUE}=== Building wasm-fullstack ===${NC}\n"
echo ""

if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "stdio" ]; then
    build_binary "stdio"
    echo ""
fi

if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "http" ]; then
    build_binary "http"
    echo ""
fi

printf "${GREEN}✓ Build complete!${NC}\n"
echo ""
printf "${BLUE}Built binaries:${NC}\n"
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "stdio" ]; then
    echo "  target/wasm32-wasip1/release/wasm-fullstack-stdio.wasm"
fi
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "http" ]; then
    echo "  target/wasm32-wasip1/release/wasm-fullstack-http.wasm"
fi
echo ""
printf "${BLUE}Quick start:${NC}\n"
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "stdio" ]; then
    printf "  ${GREEN}# stdio transport:${NC}\n"
    echo "  npx @modelcontextprotocol/inspector wasmedge target/wasm32-wasip1/release/wasm-fullstack-stdio.wasm"
    echo ""
fi
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "http" ]; then
    printf "  ${GREEN}# HTTP transport:${NC}\n"
    echo "  wasmedge target/wasm32-wasip1/release/wasm-fullstack-http.wasm"
    echo "  # Access at: http://127.0.0.1:8080/mcp"
    echo ""
fi
echo ""
printf "${BLUE}Database configuration:${NC}\n"
echo "  # Start PostgreSQL (required)"
echo "  docker-compose up -d"
echo ""
echo "  # Optional: Set custom database URL"
echo "  DATABASE_URL=\"postgres://postgres:postgres@localhost/todo\" \\"
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "stdio" ]; then
    echo "    wasmedge --env DATABASE_URL target/wasm32-wasip1/release/wasm-fullstack-stdio.wasm"
fi
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "http" ]; then
    echo "    wasmedge --env DATABASE_URL target/wasm32-wasip1/release/wasm-fullstack-http.wasm"
fi
