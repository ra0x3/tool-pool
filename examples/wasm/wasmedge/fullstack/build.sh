#!/bin/bash

# Build script for fullstack
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
HOST="127.0.0.1"  # Default host for HTTP transport

show_help() {
    cat << EOF
${BLUE}fullstack Build Script${NC}

Builds MCP server binaries with PostgreSQL support for WasmEdge runtime.

${BLUE}Usage:${NC}
  ./build.sh [OPTIONS]

${BLUE}Options:${NC}
  --help                  Show this help message
  -h, --host HOST         Host address for HTTP transport (default: 127.0.0.1)
  -t, --transport TYPE    Transport type: stdio, http, or both (default: stdio)
                          stdio: stdin/stdout transport (better for parallel modules)
                          http:  HTTP transport (multi-tenant, network-accessible)
                          both:  Build both binaries
  -f, --features          Comma-separated features (default: wasmedge-postgres)

${BLUE}Examples:${NC}
  # Build both binaries with PostgreSQL support
  ./build.sh

  # Build only the HTTP server with custom host
  ./build.sh --transport http --host 0.0.0.0

  # Build only the stdio server
  ./build.sh --transport stdio

  # Build without PostgreSQL (mock mode)
  ./build.sh --features ""

${BLUE}Running the binaries:${NC}
  # stdio transport with MCP Inspector (all-in-one)
  DATABASE_URL="postgres://postgres:postgres@localhost/todo" \\
    npx @modelcontextprotocol/inspector wasmedge run target/wasm32-wasip1/release/fullstack-stdio.wasm

  # HTTP transport requires two terminals:
  # Terminal 1: Run the HTTP server
  HOST="${HOST}" PORT="8080" DATABASE_URL="postgres://postgres:postgres@localhost/todo" \\
    wasmedge target/wasm32-wasip1/release/fullstack-http.wasm

  # Terminal 2: Run MCP Inspector
  npx @modelcontextprotocol/inspector

  # Then connect via Inspector UI at http://localhost:6274
  # Select 'Streamable HTTP' and enter URL: http://${HOST}:8080/mcp

${BLUE}Prerequisites:${NC}
  - WasmEdge runtime installed
  - PostgreSQL running (docker-compose up -d)
  - Rust with wasm32-wasip1 target (rustup target add wasm32-wasip1)

EOF
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --help)
            show_help
            exit 0
            ;;
        -h|--host)
            HOST="$2"
            shift 2
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
    local binary_name="fullstack-${mode}"
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

printf "${BLUE}=== Building fullstack ===${NC}\n"
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "http" ]; then
    echo "HTTP host configured: $HOST"
fi
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
    echo "  target/wasm32-wasip1/release/fullstack-stdio.wasm"
fi
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "http" ]; then
    echo "  target/wasm32-wasip1/release/fullstack-http.wasm"
fi
echo ""
printf "${BLUE}========================================${NC}\n"
printf "${BLUE}     DEPLOYMENT OPTIONS${NC}\n"
printf "${BLUE}========================================${NC}\n"
echo ""
printf "${GREEN}Mode 1: Manual Testing (PostgreSQL only)${NC}\n"
echo "----------------------------------------"
echo "Perfect for development and debugging. PostgreSQL runs in Docker,"
echo "while you run the MCP server manually with live code changes."
echo ""
echo "  # Start just PostgreSQL"
echo "  docker-compose up -d"
echo ""
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "stdio" ]; then
    echo "  # Run stdio server with MCP Inspector (all-in-one)"
    echo "  # Uses config.stdio.yaml for stdio transport"
    echo "  DATABASE_URL=\"postgres://postgres:postgres@localhost/todo\" \\"
    echo "    npx @modelcontextprotocol/inspector wasmedge --dir .:. run target/wasm32-wasip1/release/fullstack-stdio.wasm"
    echo "  # Access Inspector UI at: http://localhost:6274"
    echo ""
fi
if [ "$TRANSPORT" = "both" ] || [ "$TRANSPORT" = "http" ]; then
    echo "  # Run HTTP server (Terminal 1)"
    echo "  # Uses config.http.yaml for HTTP transport"
    echo "  HOST=\"${HOST}\" PORT=\"8080\" DATABASE_URL=\"postgres://postgres:postgres@localhost/todo\" \\"
    echo "    wasmedge --dir .:. target/wasm32-wasip1/release/fullstack-http.wasm"
    echo ""
    echo "  # Run MCP Inspector separately (Terminal 2)"
    echo "  npx @modelcontextprotocol/inspector"
    echo ""
    echo "  # Then in Inspector UI at http://localhost:6274:"
    echo "  # - Select 'Streamable HTTP' transport"
    echo "  # - Enter URL: http://${HOST}:8080/mcp"
    echo "  # - Click Connect"
    echo ""
fi
echo ""
printf "${GREEN}Mode 2: Full Stack Testing (Everything in Docker)${NC}\n"
echo "---------------------------------------------------"
echo "Complete containerized deployment. Both PostgreSQL and MCP server"
echo "run in Docker with automatic health checks and networking."
echo ""
echo "  # Start PostgreSQL + MCP server in Docker"
echo "  docker-compose --profile full up"
echo ""
echo "  # This mode runs the server through docker-compose"
echo "  # Note: Inspector integration in Docker mode requires additional setup"
echo ""
echo ""
printf "${BLUE}STOPPING SERVICES:${NC}\n"
echo "------------------"
echo "  # Stop PostgreSQL only (Mode 1)"
echo "  docker-compose down"
echo ""
echo "  # Stop full stack (Mode 2)"
echo "  docker-compose --profile full down"
echo ""
echo "  # Stop and remove volumes (clean slate)"
echo "  docker-compose --profile full down -v"
echo ""
