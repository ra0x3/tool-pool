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
        -e POSTGRES_DB=todo \
        -e POSTGRES_USER=postgres \
        -e POSTGRES_PASSWORD=postgres \
        -p 5432:5432 \
        postgres:alpine > /dev/null 2>&1
    echo "${CYAN}  Waiting for PostgreSQL to start...${NC}"
    sleep 5
    docker exec fullstack-postgres psql -U postgres -d todo -c "SELECT 1" > /dev/null 2>&1
    if [ $? -eq 0 ]; then
        echo "${GREEN}✓ PostgreSQL started successfully${NC}"
    else
        echo "${RED}✗ Failed to start PostgreSQL${NC}"
        exit 1
    fi
fi

echo "${YELLOW}Building fullstack WASM modules (HTTP + stdio)...${NC}"
./build.sh --transport both > /dev/null 2>&1
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

STDIO_WASM_FILE="target/wasm32-wasip1/release/fullstack-stdio.wasm"
if [ ! -f "$STDIO_WASM_FILE" ]; then
    echo "${RED}✗ Stdio WASM file not found at $STDIO_WASM_FILE${NC}"
    exit 1
fi
echo "${GREEN}✓ Found stdio WASM file: $STDIO_WASM_FILE${NC}"
echo ""

echo "${CYAN}Starting HTTP server...${NC}"
# Explicitly forward the runtime configuration into WasmEdge. Environment variables
# must be declared with --env for the sandbox, so derive the effective values and
# expose them both to the spawning shell process and the WASM module.
SERVER_DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost/todo}"
SERVER_HOST="${HOST:-0.0.0.0}"
SERVER_PORT="${PORT:-8080}"

HOST="$SERVER_HOST" PORT="$SERVER_PORT" DATABASE_URL="$SERVER_DATABASE_URL" \
wasmedge \
    --env "DATABASE_URL=$SERVER_DATABASE_URL" \
    --env "HOST=$SERVER_HOST" \
    --env "PORT=$SERVER_PORT" \
    --dir .:. \
    --dir /tmp:/tmp \
    --dir /var/tmp:/var/tmp \
    "$HTTP_WASM_FILE" > /tmp/http_server.log 2>&1 &
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
echo "${BLUE}=== Filesystem Access Tests ===${NC}"

# Test filesystem access with policies
# These tests verify that the WASI filesystem permissions are correctly enforced

# Create test directories and files for filesystem tests
TEST_FS_DIR="/tmp/wasm-fs-test"
mkdir -p "$TEST_FS_DIR/allowed" "$TEST_FS_DIR/forbidden" 2>/dev/null || true
echo "test content" > "$TEST_FS_DIR/allowed/test.txt"
echo "secret" > "$TEST_FS_DIR/forbidden/secret.txt"

# Test 1: Read from allowed directory (should succeed)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_read","arguments":{"path":"/tmp/wasm-fs-test/allowed/test.txt"}},"id":24}' "true" "Read from allowed directory"

# Test 2: Write to allowed directory (should succeed)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_write","arguments":{"path":"/tmp/wasm-fs-test/allowed/output.txt","content":"hello world from WASM"}},"id":25}' "true" "Write to allowed directory"

# Test 3: List allowed directory (should succeed)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_list","arguments":{"path":"/tmp/wasm-fs-test/allowed"}},"id":26}' "true" "List files in allowed directory"

# Test 4: Read from forbidden directory (should fail due to policy)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_read","arguments":{"path":"/tmp/wasm-fs-test/forbidden/secret.txt"}},"id":27}' "false" "Read from forbidden directory (should fail)"

# Test 5: Write to forbidden directory (should fail due to policy)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_write","arguments":{"path":"/tmp/wasm-fs-test/forbidden/bad.txt","content":"should not work"}},"id":28}' "false" "Write to forbidden directory (should fail)"

# Test 6: Read system file (should fail - not in allowed paths)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_read","arguments":{"path":"/etc/passwd"}},"id":29}' "false" "Read system file (should fail)"

# Test 7: Write to system directory (should fail - not in allowed paths)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_write","arguments":{"path":"/etc/test.txt","content":"should not work"}},"id":30}' "false" "Write to system directory (should fail)"

# Test 8: List forbidden directory (should fail)
send_request '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"file_list","arguments":{"path":"/tmp/wasm-fs-test/forbidden"}},"id":31}' "false" "List forbidden directory (should fail)"

# Verify the file was actually written
if [ -f "$TEST_FS_DIR/allowed/output.txt" ]; then
    echo "${GREEN}  ✓ File was successfully written to allowed directory${NC}"
    CONTENT=$(cat "$TEST_FS_DIR/allowed/output.txt")
    if [ "$CONTENT" = "hello world from WASM" ]; then
        echo "${GREEN}  ✓ File content matches expected value${NC}"
    else
        echo "${RED}  ✗ File content does not match: '$CONTENT'${NC}"
    fi
else
    echo "${RED}  ✗ File was not written to allowed directory${NC}"
fi

# Clean up test filesystem
rm -rf "$TEST_FS_DIR" 2>/dev/null || true

echo ""
echo "${YELLOW}Cleaning up HTTP server...${NC}"
kill $HTTP_PID 2>/dev/null
wait $HTTP_PID 2>/dev/null || true
echo "${GREEN}✓ HTTP server stopped${NC}"
echo ""

echo "${BLUE}=== Distribution Tests ===${NC}"

# Build the mcpk CLI (required for bundle tests)
if ! (cd ../../../../ && cargo build --release --package mcpkit-rs-cli --bin mcpk >/dev/null 2>&1); then
    echo "${RED}✗ Failed to build mcpk CLI${NC}"
    exit 1
fi
MCPK="../../../../target/release/mcpk"
if [ ! -x "$MCPK" ]; then
    echo "${RED}✗ mcpk binary not found at $MCPK${NC}"
    exit 1
fi

REGISTRY_MODE="local"
REGISTRY_STARTED=false
REGISTRY_CONTAINER="fullstack-test-registry"
REGISTRY_PORT="${FULLSTACK_REGISTRY_PORT:-5050}"
REGISTRY_URI=""
PULLED_BUNDLE_DIR=""

# Test 19: Prepare OCI registry target
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Prepare OCI registry target${NC}"
if [ -n "$GITHUB_USER" ] && [ -n "$GITHUB_TOKEN" ]; then
    REGISTRY_MODE="github"
    TEST_TAG="test-$(date +%s)"
    REGISTRY_URI="oci://ghcr.io/${GITHUB_USER}/mcpkit-fullstack:${TEST_TAG}"
    echo "  Using GitHub Container Registry: ${REGISTRY_URI}"
    echo "  ${GREEN}✓ Pass${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo "  Starting local OCI registry on port ${REGISTRY_PORT}..."
    REGISTRY_URI="oci://localhost:${REGISTRY_PORT}/mcpkit/fullstack:test-$(date +%s)"
    # Stop any existing container with the same name
    if docker ps -aq -f name="^${REGISTRY_CONTAINER}$" >/dev/null 2>&1 && \
       [ -n "$(docker ps -aq -f name="^${REGISTRY_CONTAINER}$")" ]; then
        docker rm -f "$REGISTRY_CONTAINER" >/dev/null 2>&1 || true
    fi
    if docker run -d -p ${REGISTRY_PORT}:5000 --name "$REGISTRY_CONTAINER" registry:2 >/dev/null 2>&1; then
        REGISTRY_STARTED=true
        sleep 3
        echo "  Local registry URI: ${REGISTRY_URI}"
        echo "  ${GREEN}✓ Pass${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo "  ${RED}✗ Fail - could not start local registry${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
fi

# Test 20: Push bundle to OCI registry
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Push bundle to OCI registry${NC}"
PUSH_LOG=$(mktemp)
if GITHUB_USER="$GITHUB_USER" GITHUB_TOKEN="$GITHUB_TOKEN" \
    "$MCPK" bundle push --wasm "$STDIO_WASM_FILE" --config config.stdio.yaml --uri "$REGISTRY_URI" >"$PUSH_LOG" 2>&1; then
    echo "  ${GREEN}✓ Pass${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo "  ${RED}✗ Fail - bundle push failed${NC}"
    tail -n 20 "$PUSH_LOG"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Test 21: Pull bundle from OCI registry
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Pull bundle from OCI registry${NC}"
PULLED_BUNDLE_DIR=$(mktemp -d /tmp/fullstack-bundle.XXXXXX)
PULL_LOG=$(mktemp)
if GITHUB_USER="$GITHUB_USER" GITHUB_TOKEN="$GITHUB_TOKEN" \
    "$MCPK" bundle pull "$REGISTRY_URI" --output "$PULLED_BUNDLE_DIR" --force >"$PULL_LOG" 2>&1; then
    echo "  ${GREEN}✓ Pass${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo "  ${RED}✗ Fail - bundle pull failed${NC}"
    tail -n 20 "$PULL_LOG"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Test 22: Verify pulled bundle integrity
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Verify pulled bundle integrity${NC}"
if [ -n "$PULLED_BUNDLE_DIR" ] && \
   [ -f "$PULLED_BUNDLE_DIR/module.wasm" ] && \
   [ -f "$PULLED_BUNDLE_DIR/config.yaml" ]; then
    echo "  ${GREEN}✓ Pass${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo "  ${RED}✗ Fail - pulled bundle missing module or config${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Test 23: Run server from pulled bundle
TESTS_RUN=$((TESTS_RUN + 1))
echo "${BLUE}Test $TESTS_RUN: Run server from pulled bundle${NC}"
RUN_LOG=$(mktemp)
RUN_OUTPUT=$(mktemp)
RUN_INPUT=$(mktemp)
if [ -n "$PULLED_BUNDLE_DIR" ] && [ -f "$PULLED_BUNDLE_DIR/module.wasm" ]; then
    cp "$PULLED_BUNDLE_DIR/config.yaml" "$PULLED_BUNDLE_DIR/config.stdio.yaml" 2>/dev/null || true
    echo "$INIT_REQUEST" > "$RUN_INPUT"
    if (cd "$PULLED_BUNDLE_DIR" && \
        HOST="$SERVER_HOST" PORT="$SERVER_PORT" DATABASE_URL="$SERVER_DATABASE_URL" \
            wasmedge \
            --env "DATABASE_URL=$SERVER_DATABASE_URL" \
            --env "HOST=$SERVER_HOST" \
            --env "PORT=$SERVER_PORT" \
            --dir .:. module.wasm \
            < "$RUN_INPUT") > "$RUN_OUTPUT" 2>"$RUN_LOG"; then
        if grep -q '"serverInfo"' "$RUN_OUTPUT"; then
            echo "  ${GREEN}✓ Pass${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo "  ${RED}✗ Fail - initialization response missing${NC}"
            tail -n 20 "$RUN_LOG"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo "  ${RED}✗ Fail - pulled WASM did not initialize${NC}"
        tail -n 20 "$RUN_LOG"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo "  ${RED}✗ Fail - no pulled bundle to execute${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
rm -f "$RUN_LOG" "$RUN_OUTPUT" "$RUN_INPUT" "$PUSH_LOG" "$PULL_LOG"
if [ -n "$PULLED_BUNDLE_DIR" ]; then
    rm -rf "$PULLED_BUNDLE_DIR"
fi

if [ "$REGISTRY_MODE" = "local" ] && [ "$REGISTRY_STARTED" = "true" ]; then
    echo "${YELLOW}Stopping local OCI registry...${NC}"
    docker stop "$REGISTRY_CONTAINER" >/dev/null 2>&1 || true
    docker rm "$REGISTRY_CONTAINER" >/dev/null 2>&1 || true
    echo "${GREEN}✓ Local registry stopped${NC}"
fi

echo ""
if [ -t 0 ]; then
    read -p "Stop PostgreSQL container? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        docker stop fullstack-postgres > /dev/null 2>&1
        docker rm fullstack-postgres > /dev/null 2>&1
        echo "${GREEN}✓ PostgreSQL stopped and removed${NC}"
    fi
else
    echo "${YELLOW}Skipping PostgreSQL shutdown prompt (non-interactive mode)${NC}"
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
