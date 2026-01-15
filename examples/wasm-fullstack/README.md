# WASM Fullstack

PostgreSQL MCP server via WasmEdge runtime. Two transports: **stdio** (default) and **HTTP**.

## Transport Selection

**stdio (default)**: Process isolation for parallel WASM modules. Each instance gets isolated memory, preventing race conditions on shared storage. OS handles lifecycle/cleanup.

**HTTP**: Multi-tenant server for centralized access, load balancing, remote APIs.

## Prerequisites

```bash
# WasmEdge (required for networking)
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- -v 0.14.0
source $HOME/.wasmedge/env

# PostgreSQL
docker-compose up -d
```

## Build

```bash
./build.sh                    # stdio only (default)
./build.sh -t http           # HTTP only
./build.sh -t both           # Both transports
```

## Run

### stdio Transport
```bash
npx @modelcontextprotocol/inspector wasmedge target/wasm32-wasip1/release/wasm-fullstack-stdio.wasm
```

### HTTP Transport
```bash
# Server must run first
wasmedge target/wasm32-wasip1/release/wasm-fullstack-http.wasm

# Connect at http://127.0.0.1:8080/mcp
```


## Using Artifacts with Claude CLI

```bash
# Build first
./build.sh

# Add to Claude CLI
claude mcp add-json wasm-fullstack '{
  "type":"stdio",
  "command":"wasmedge",
  "args":[
    "--dir","/path/to/home:/path/to/home",
    "/path/to/mcpkit-rs/examples/wasm-fullstack/target/wasm32-wasip1/release/wasm-fullstack-stdio.wasm"
  ],
  "env":{"RUST_LOG":"info"}
}'

# Test connection
claude --debug "mcp"
# Use /mcp to connect to wasm-fullstack

# Call a tool
claude -p \
  --allowedTools "mcp__wasm-fullstack__db_stats" \
  --debug mcp \
  "Call the db_stats tool and return the raw JSON result only."
```

Expected output:
```json
{
  "completed": 1,
  "completion_rate": 25.0,
  "pending": 3,
  "source": "postgresql",
  "total": 4,
  "unique_users": 2
}
```

## Available Tools

| Tool | Description |
|------|------------|
| `fetch_todos` | SELECT from PostgreSQL or JSONPlaceholder API |
| `create_todo` | INSERT with audit logging |
| `update_todo` | UPDATE existing todo |
| `delete_todo` | DELETE with cascade |
| `batch_process` | Batch operations on multiple todos |
| `search_todos` | LIKE query on titles |
| `db_stats` | Aggregated statistics |
| `test_connection` | PostgreSQL health check |

## Architecture Comparison

| Aspect | stdio | HTTP |
|--------|-------|------|
| Isolation | Process-level | Shared process |
| Parallelism | Native | Requires mutex/channels |
| Multi-tenant | No | Yes |
| Remote access | No | Yes |
| Resource cleanup | Automatic | Manual |
| Use case | Module orchestration | API gateway |

## Technical Stack

- **WasmEdge patches**: TCP sockets + HTTP in WASM
- **Second State forks**: tokio/axum/tokio-postgres with networking
- **StreamableHTTP**: Session-based SSE transport
- **Database**: PostgreSQL with audit logging via WAL

Schema in `init.sql`: `todos`, `wal_entries`, `todo_stats` view.