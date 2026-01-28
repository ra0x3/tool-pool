#!/bin/bash

# Test harness for fullstack WASM example using HTTP transport with policy enforcement

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

SESSION_ID=""
# Notification format (no id field) for initialization
INIT_NOTIFICATION='{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}'
# Request format (with id) for getting capabilities
INIT_REQUEST='{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"1.0.0","capabilities":{},"clientInfo":{"name":"test-harness","version":"1.0.0"}},"id":1}'
MCP_URL="http://localhost:8080/mcp"

log_session() {
    if [ -n "$SESSION_ID" ]; then
        echo "${CYAN}  Session: $SESSION_ID${NC}"
    fi
}

extract_session_from_headers() {
    local header_file=$1
    local header
    header=$(grep -i '^mcp-session-id:' "$header_file" | tail -n 1)
    if [ -n "$header" ]; then
        local new_session
        new_session=${header#*: }
        new_session=$(echo "$new_session" | tr -d '\r\n ')
        if [ -n "$new_session" ] && [ "$new_session" != "$SESSION_ID" ]; then
            SESSION_ID="$new_session"
            log_session
        fi
    fi
}

extract_payload() {
    local raw=$1
    local payload
    # Extract JSON data from SSE format (lines starting with "data: {")
    payload=$(echo "$raw" | grep '^data: {' | head -1 | sed 's/^data: //')
    # If no JSON data found in SSE, check if it's a plain response
    if [ -z "$payload" ]; then
        # Check if the raw response is JSON
        if echo "$raw" | head -1 | grep -q '^{'; then
            payload="$raw"
        fi
    fi
    echo "$payload"
}

start_session() {
    echo "${CYAN}  Initializing session...${NC}"
    SESSION_ID=""
    local headers
    headers=$(mktemp)
    local response
    response=$(curl -s -D "$headers" -X POST \
        -H "Content-Type: application/json" \
        -H "Accept: text/event-stream, application/json" \
        -d "$INIT_REQUEST" "$MCP_URL" 2>/dev/null)
    local status
    status=$(head -n 1 "$headers" | awk '{print $2}')
    extract_session_from_headers "$headers"
    rm -f "$headers"
    local payload
    payload=$(extract_payload "$response")
    if echo "$payload" | grep -q '"error"'; then
        echo "${RED}✗ Initialize failed: $(echo "$payload" | jq -r '.error.message' 2>/dev/null || echo "$payload")${NC}"
        exit 1
    fi
    if [ -n "$status" ] && [ "$status" -ge 400 ] 2>/dev/null; then
        echo "${RED}✗ Initialize failed with HTTP $status${NC}"
        echo "$payload"
        exit 1
    fi
    if [ -z "$SESSION_ID" ]; then
        echo "${RED}✗ Initialize did not return session ID${NC}"
        exit 1
    fi
}

send_raw_request() {
    local json=$1
    local headers
    headers=$(mktemp)
    local curl_args=(-s -D "$headers" -X POST \
        -H "Content-Type: application/json" \
        -H "Accept: text/event-stream, application/json")
    if [ -n "$SESSION_ID" ]; then
        curl_args+=(-H "Mcp-Session-Id: $SESSION_ID")
    fi
    local response
    response=$(curl "${curl_args[@]}" -d "$json" "$MCP_URL" 2>/dev/null)
    local status
    status=$(head -n 1 "$headers" | awk '{print $2}')
    extract_session_from_headers "$headers"
    rm -f "$headers"
    local payload
    payload=$(extract_payload "$response")
    echo "$status"
    echo "$payload"
}

send_request() {
    local json=$1
    local expect_success=$2
    local name=$3

    TESTS_RUN=$((TESTS_RUN + 1))
    echo "${BLUE}Test $TESTS_RUN: $name${NC}"

    local method
    method=$(echo "$json" | jq -r '.method' 2>/dev/null || echo "")
    if [ -z "$SESSION_ID" ] || [ "$method" = "initialize" ]; then
        start_session
        # If this was an initialize request, we're done - start_session already handled it
        if [ "$method" = "initialize" ]; then
            echo "${GREEN}  ✓ Pass${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
            return
        fi
    fi

    # For non-initialize requests, we need to send notification to establish the connection
    # because the server requires an initialized notification on every HTTP connection
    if [ "$method" != "initialize" ]; then
        # First send initialized notification to establish the connection
        local init_temp=$(mktemp)
        send_raw_request "$INIT_NOTIFICATION" > "$init_temp"
        rm -f "$init_temp"
    fi

    local attempt=1
    local status payload
    while [ $attempt -le 2 ]; do
        local temp_file=$(mktemp)
        send_raw_request "$json" > "$temp_file"
        status=$(head -n 1 "$temp_file")
        payload=$(tail -n +2 "$temp_file")
        rm -f "$temp_file"
        if echo "$payload" | grep -qi 'session not found'; then
            SESSION_ID=""
            start_session
            # Re-send notification after getting new session
            if [ "$method" != "initialize" ]; then
                local init_temp2=$(mktemp)
                send_raw_request "$INIT_NOTIFICATION" > "$init_temp2"
                rm -f "$init_temp2"
            fi
            attempt=$((attempt + 1))
            continue
        fi
        if [ -n "$status" ] && { [ "$status" = "401" ] || [ "$status" = "422" ]; }; then
            SESSION_ID=""
            start_session
            # Re-send notification after getting new session
            if [ "$method" != "initialize" ]; then
                local init_temp3=$(mktemp)
                send_raw_request "$INIT_NOTIFICATION" > "$init_temp3"
                rm -f "$init_temp3"
            fi
            attempt=$((attempt + 1))
            continue
        fi
        break
    done

    local has_error=false
    if [ -n "$status" ] && [ "$status" -ge 400 ] 2>/dev/null; then
        has_error=true
    elif echo "$payload" | grep -q '"error"'; then
        has_error=true
    elif [ -z "$payload" ]; then
        # Empty payload for policy enforcement tests means violation
        # But for regular tests, it's a connection issue
        if [ "$expect_success" = "false" ]; then
            has_error=true
            payload='{"error":{"message":"Server rejected request (policy violation or invalid request)"}}'
        else
            has_error=true
            payload='{"error":{"message":"Server connection issue - no response received"}}'
        fi
    fi

    if [ "$has_error" = true ]; then
        if [ "$expect_success" = "false" ]; then
            echo "${GREEN}  ✓ Pass${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo "${RED}  ✗ Fail: $(echo "$payload" | jq -r '.error.message' 2>/dev/null || echo "$payload")${NC}"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        if [ "$expect_success" = "true" ]; then
            echo "${GREEN}  ✓ Pass${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo "${RED}  ✗ Fail: should have returned error${NC}"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    fi
}

echo "${BLUE}=== Fullstack WASM Test Harness (HTTP Transport) ===${NC}"
echo ""

if docker ps | grep -q fullstack-postgres; then
    echo "${GREEN}✓ PostgreSQL is already running${NC}"
else
    echo "${YELLOW}Starting PostgreSQL container...${NC}"
    docker run -d \
        --name fullstack-postgres \
        -e POSTGRES_DB=testdb \
        -e POSTGRES_USER=postgres \
        -e POSTGRES_PASSWORD=postgres \
        -p 5432:5432 \
        postgres:alpine > /dev/null 2>&1
    echo "${CYAN}  Waiting for PostgreSQL to start...${NC}"
    sleep 5
    docker exec fullstack-postgres psql -U postgres -d testdb -c "SELECT 1" > /dev/null 2>&1
    if [ $? -eq 0 ]; then
        echo "${GREEN}✓ PostgreSQL started successfully${NC}"
    else
        echo "${RED}✗ Failed to start PostgreSQL${NC}"
        exit 1
    fi
fi

echo "${YELLOW}Building fullstack WASM module...${NC}"
./build.sh --transport http > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "${RED}✗ Failed to build fullstack WASM module${NC}"
    exit 1
fi
echo "${GREEN}✓ Fullstack WASM module built successfully${NC}"

HTTP_WASM_FILE="target/wasm32-wasip1/release/fullstack-http.wasm"
if [ ! -f "$HTTP_WASM_FILE" ]; then
    echo "${RED}✗ HTTP WASM file not found at $HTTP_WASM_FILE${NC}"
    exit 1
fi
echo "${GREEN}✓ Found HTTP WASM file: $HTTP_WASM_FILE${NC}"
echo ""

echo "${CYAN}Starting HTTP server...${NC}"
DATABASE_URL="postgres://postgres:postgres@localhost/testdb" HOST="0.0.0.0" PORT="8080" wasmedge --dir .:. "$HTTP_WASM_FILE" > /tmp/http_server.log 2>&1 &
HTTP_PID=$!
sleep 3
if ! ps -p $HTTP_PID > /dev/null; then
    echo "${RED}✗ Failed to start HTTP server${NC}"
    cat /tmp/http_server.log
    exit 1
fi
echo "${GREEN}✓ HTTP server started (PID: $HTTP_PID)${NC}"
echo ""

echo "${BLUE}=== Running Tests ===${NC}"

echo "${YELLOW}Initializing MCP connection...${NC}"
send_request "$INIT_REQUEST" "true" "Initialize MCP connection"

send_request '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}' "true" "List available tools"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"test_connection","arguments":{}},"id":3}' "true" "Test PostgreSQL connection"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"create_todo","arguments":{"title":"Test Todo 1","user_id":1}},"id":4}' "true" "Create first todo"
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"create_todo","arguments":{"title":"Test Todo 2","user_id":1}},"id":5}' "true" "Create second todo"
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"create_todo","arguments":{"title":"Test Todo 3","user_id":2}},"id":6}' "true" "Create third todo"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"fetch_todos","arguments":{}},"id":7}' "true" "Fetch all todos"
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"fetch_todos","arguments":{"user_id":1}},"id":8}' "true" "Fetch todos for user 1"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search_todos","arguments":{"title_contains":"Test"}},"id":9}' "true" "Search todos containing 'Test'"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"update_todo","arguments":{"id":"1","title":"Updated Todo","completed":true}},"id":10}' "true" "Update todo ID 1"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"delete_todo","arguments":{"id":"2"}},"id":11}' "true" "Delete todo ID 2"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"batch_process","arguments":{"operation":"complete","ids":["1","3"]}},"id":12}' "true" "Batch complete todos"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"db_stats","arguments":{}},"id":13}' "true" "Get database statistics"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"read_wal","arguments":{}},"id":14}' "true" "Read WAL statistics"

echo ""
echo "${BLUE}=== Policy Enforcement Tests ===${NC}"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_shell","arguments":{"cmd":"ls"}},"id":15}' "false" "Call non-existent tool (should fail)"

send_request '{"jsonrpc":"2.0","method":"resources/read","params":{"uri":"file:///etc/passwd"},"id":16}' "false" "Access forbidden resource (should fail)"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"create_todo","arguments":{"invalid_field":"test"}},"id":17}' "false" "Invalid tool arguments (should fail)"

send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"delete_todo","arguments":{}},"id":18}' "false" "Missing required arguments (should fail)"

echo ""
echo "${YELLOW}Cleaning up...${NC}"
kill $HTTP_PID 2>/dev/null
wait $HTTP_PID 2>/dev/null || true
echo "${GREEN}✓ HTTP server stopped${NC}"

echo ""
read -p "Stop PostgreSQL container? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    docker stop fullstack-postgres > /dev/null 2>&1
    docker rm fullstack-postgres > /dev/null 2>&1
    echo "${GREEN}✓ PostgreSQL stopped and removed${NC}"
fi

echo ""
echo "${BLUE}=== Test Summary ===${NC}"
echo "Tests run: $TESTS_RUN"
echo "${GREEN}Tests passed: $TESTS_PASSED${NC}"
echo "${RED}Tests failed: $TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo "${GREEN}✓ All tests passed!${NC}"
else
    echo "${RED}✗ Some tests failed${NC}"
    exit 1
fi
