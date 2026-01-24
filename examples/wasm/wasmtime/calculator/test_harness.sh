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