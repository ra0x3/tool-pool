#!/bin/bash
# Test script for bundle distribution with local OCI registry

set -e

echo "=== MCPKit-RS Bundle Distribution Test ==="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Docker is required but not installed${NC}"
    exit 1
fi

# Start local OCI registry if not running
REGISTRY_NAME="mcpkit-test-registry"
REGISTRY_PORT=5000

echo -e "${YELLOW}Starting local OCI registry...${NC}"
if [ ! "$(docker ps -q -f name=$REGISTRY_NAME)" ]; then
    if [ "$(docker ps -aq -f status=exited -f name=$REGISTRY_NAME)" ]; then
        # Clean up old container
        docker rm $REGISTRY_NAME
    fi
    # Start new registry
    docker run -d -p $REGISTRY_PORT:5000 --name $REGISTRY_NAME registry:2
    echo "Registry started on localhost:$REGISTRY_PORT"
    sleep 2
else
    echo "Registry already running"
fi

# Set test environment
export TEST_REGISTRY_URL="localhost:$REGISTRY_PORT"

# Build calculator example if needed
echo -e "${YELLOW}Building calculator example...${NC}"
cd examples/wasm/wasmtime/calculator
if [ ! -f calculator.wasm ]; then
    ./build.sh
fi
cd -

# Run Rust tests
echo -e "${YELLOW}Running distribution tests...${NC}"
cargo test --features distribution -- --nocapture bundle

# Test with ignored tests (requires registry)
echo -e "${YELLOW}Running integration tests with local registry...${NC}"
cargo test --features distribution -- --ignored test_push_pull_cycle --nocapture

# Optional: Test with GitHub registry if credentials are set
if [ -n "$GITHUB_USER" ] && [ -n "$GITHUB_TOKEN" ]; then
    echo -e "${YELLOW}Testing with GitHub Container Registry...${NC}"
    cargo test --features distribution -- --ignored test_github_registry --nocapture
else
    echo -e "${YELLOW}Skipping GitHub registry test (set GITHUB_USER and GITHUB_TOKEN to enable)${NC}"
fi

# Clean up
echo -e "${YELLOW}Cleaning up...${NC}"
docker stop $REGISTRY_NAME
docker rm $REGISTRY_NAME

echo -e "${GREEN}✓ All distribution tests completed successfully!${NC}"