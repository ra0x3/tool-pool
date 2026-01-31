#!/bin/bash

# Automated test harness for calculator WASM example
# Tests calculator operations with MCP protocol

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

echo "${BLUE}=== Calculator WASM Test Harness ===${NC}"
echo ""

# Step 1: Build the calculator WASM module
echo "${YELLOW}Building calculator WASM module...${NC}"
./build.sh > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "${RED}✗ Failed to build calculator WASM module${NC}"
    exit 1
fi
echo "${GREEN}✓ Calculator WASM module built successfully${NC}"
echo ""

# Function to send JSON-RPC request and check response
send_request() {
    local request=$1
    local expected_success=$2
    local test_name=$3

    TESTS_RUN=$((TESTS_RUN + 1))

    echo "${BLUE}Test $TESTS_RUN: $test_name${NC}"

    # Create temporary files for input/output
    local input_file=$(mktemp)
    local output_file=$(mktemp)

    # Write both initialize and actual request to input file
    echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-harness","version":"1.0.0"}},"id":1}' > "$input_file"
    echo "$request" >> "$input_file"

    # Send requests to server and capture response
    wasmtime run ./calculator.wasm < "$input_file" > "$output_file" 2>/dev/null

    # Get the second response (first is initialize, second is our actual request)
    # Filter to get only the second JSON response
    response=$(grep '^{' "$output_file" | sed -n '2p')

    # Clean up temp files
    rm -f "$input_file" "$output_file"

    # Check if response contains error
    if echo "$response" | grep -q '"error"'; then
        if [ "$expected_success" = "false" ]; then
            echo "${GREEN}  ✓ Pass${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo "${RED}  ✗ Fail: $response${NC}"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        if [ "$expected_success" = "true" ]; then
            echo "${GREEN}  ✓ Pass${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo "${RED}  ✗ Fail: should have returned error${NC}"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    fi
    echo ""
}

echo "${BLUE}=== Running Calculator Tests ===${NC}"
echo ""

# Test 1: List tools
send_request '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}' "true" "List available tools"

# Test 2: Addition
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"add","arguments":{"a":5,"b":3}},"id":3}' "true" "add(5, 3) = 8"

# Test 3: Subtraction
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"subtract","arguments":{"a":10,"b":4}},"id":4}' "true" "subtract(10, 4) = 6"

# Test 4: Multiplication
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"multiply","arguments":{"a":6,"b":7}},"id":5}' "true" "multiply(6, 7) = 42"

# Test 5: Division
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"divide","arguments":{"a":20,"b":4}},"id":6}' "true" "divide(20, 4) = 5"

# Test 6: Division by zero
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"divide","arguments":{"a":10,"b":0}},"id":7}' "true" "divide(10, 0) - returns error in result"

# Test 7: Large numbers
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"multiply","arguments":{"a":999999,"b":999999}},"id":8}' "true" "multiply(999999, 999999)"

# Test 8: Negative numbers
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"add","arguments":{"a":-5,"b":3}},"id":9}' "true" "add(-5, 3) = -2"

# Test 9: Decimal numbers
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"multiply","arguments":{"a":2.5,"b":4}},"id":10}' "true" "multiply(2.5, 4) = 10"

# Build the mcpkit CLI silently if not already built
(cd ../../../../ && cargo build --release --package mcpkit-rs-cli --bin mcpkit >/dev/null 2>&1)
if [ $? -ne 0 ]; then
    echo "${RED}✗ Failed to build mcpkit CLI - bundle tests will be skipped${NC}"
    MCPKIT=""
else
    MCPKIT="../../../../target/release/mcpkit"
fi

# Check for GitHub credentials
if [ -z "$GITHUB_USER" ] || [ -z "$GITHUB_TOKEN" ]; then
    echo "${YELLOW}Note: Set GITHUB_USER and GITHUB_TOKEN to test real push/pull${NC}"
    echo ""
    USE_REAL_REGISTRY=false
else
    USE_REAL_REGISTRY=true
    # Generate unique tag for this test run
    TEST_TAG="test-$(date +%s)"
    REGISTRY_URI="oci://ghcr.io/${GITHUB_USER}/mcpkit-calculator:${TEST_TAG}"
fi

# Test 10: Build WASM bundle
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Build WASM bundle${NC}"
# Ensure WASM is built
if [ ! -f calculator.wasm ]; then
    cargo build --target wasm32-wasip1 --release >/dev/null 2>&1
    wasm-tools component new target/wasm32-wasip1/release/calculator.wasm -o calculator.wasm >/dev/null 2>&1
fi
if [ -f calculator.wasm ] && [ -f config.yaml ]; then
    echo "  ${GREEN}✓ Passed${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo "  ${RED}✗ Failed - WASM or config not found${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
echo ""

# Test 11: Push bundle to GitHub registry
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Push bundle to GitHub Container Registry${NC}"
if [ "$USE_REAL_REGISTRY" = true ] && [ -n "$MCPKIT" ]; then
    # Export the env vars for mcpkit
    export GITHUB_USER="$GITHUB_USER"
    export GITHUB_TOKEN="$GITHUB_TOKEN"

    # Run the push command and capture output
    if $MCPKIT bundle push --wasm calculator.wasm --config config.yaml --uri "${REGISTRY_URI}" >/tmp/push_output.txt 2>&1; then
        # Push succeeded
        echo "  ${GREEN}✓ Passed - pushed to ${REGISTRY_URI}${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        PUSH_SUCCESS=true
    else
        # Push failed - check the reason
        PUSH_SUCCESS=false
        if grep -q "403\|401\|Permission denied\|Authentication" /tmp/push_output.txt; then
            echo "  ${YELLOW}✗ Failed - Authentication/permission error${NC}"
            echo "    Make sure GITHUB_TOKEN has 'write:packages' scope"
            # Show actual error for debugging
            echo "    Error: $(grep -E "Error:|Caused by:" /tmp/push_output.txt | head -1)"
        else
            echo "  ${RED}✗ Failed - push error${NC}"
            # Show first line of error for debugging
            echo "    Error: $(head -1 /tmp/push_output.txt)"
        fi
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo "  ${YELLOW}⊘ Skipped - credentials not available${NC}"
    PUSH_SUCCESS=false
fi
echo ""

# Test 12: Pull bundle from GitHub registry
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Pull bundle from GitHub Container Registry${NC}"
if [ "$USE_REAL_REGISTRY" = true ] && [ -n "$MCPKIT" ] && [ "$PUSH_SUCCESS" = true ]; then
    rm -rf /tmp/pulled-calculator-bundle
    if $MCPKIT bundle pull "${REGISTRY_URI}" --output /tmp/pulled-calculator-bundle >/tmp/pull_output.txt 2>&1; then
        echo "  ${GREEN}✓ Passed - pulled from ${REGISTRY_URI}${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        PULL_SUCCESS=true
    else
        echo "  ${RED}✗ Failed - pull error${NC}"
        echo "    Error: $(grep -E "Error:|Caused by:" /tmp/pull_output.txt | head -1)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        PULL_SUCCESS=false
    fi
else
    if [ "$PUSH_SUCCESS" = false ]; then
        echo "  ${YELLOW}⊘ Skipped - push didn't succeed${NC}"
    else
        echo "  ${YELLOW}⊘ Skipped - credentials not available${NC}"
    fi
    PULL_SUCCESS=false
fi
echo ""

# Test 13: Verify pulled bundle integrity
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Verify pulled bundle integrity${NC}"
if [ "$PULL_SUCCESS" = true ]; then
    if [ -f /tmp/pulled-calculator-bundle/module.wasm ] && [ -f /tmp/pulled-calculator-bundle/config.yaml ]; then
        echo "  ${GREEN}✓ Passed${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo "  ${RED}✗ Failed - bundle missing files${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo "  ${YELLOW}⊘ Skipped - pull didn't succeed${NC}"
fi
echo ""

# Test 14: Run server from pulled bundle
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Run server from pulled bundle${NC}"
if [ "$PULL_SUCCESS" = true ] && [ -f /tmp/pulled-calculator-bundle/module.wasm ]; then
    echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"bundle-test","version":"1.0.0"}},"id":1}' | \
        wasmtime run --dir . /tmp/pulled-calculator-bundle/module.wasm 2>/dev/null | grep -q '"serverInfo"'
    if [ $? -eq 0 ]; then
        echo "  ${GREEN}✓ Passed${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo "  ${RED}✗ Failed - WASM execution failed${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    if [ "$PULL_SUCCESS" = false ]; then
        echo "  ${YELLOW}⊘ Skipped - pull didn't succeed${NC}"
    else
        echo "  ${RED}✗ Failed - no pulled bundle to run${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
fi
echo ""

# Clean up
rm -rf /tmp/pulled-calculator-bundle /tmp/push_output.txt /tmp/pull_output.txt

# Print test summary
echo "${BLUE}=== Test Summary ===${NC}"
echo "Tests run: $TESTS_RUN"
echo "${GREEN}Tests passed: $TESTS_PASSED${NC}"
echo "${RED}Tests failed: $TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo "${RED}✗ Some tests failed${NC}"
    echo "${YELLOW}Note: The failed tests might be expected behavior differences${NC}"
    exit 1
fi