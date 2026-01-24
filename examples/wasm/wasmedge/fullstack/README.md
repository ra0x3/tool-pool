# Fullstack (WasmEdge Example)

Full-featured MCP server with PostgreSQL integration running on WasmEdge runtime.

## 🚀 Quick Start

### Mode 1: Docker (Everything Automated)
```bash
$ docker-compose up                # PostgreSQL + MCP server + Inspector
# Access at http://localhost:5173
$ docker-compose down              # Stop services
$ docker-compose down -v           # Stop and remove data
```

### Mode 2: Manual (Direct Execution)
```bash
# Start PostgreSQL
$ docker-compose up -d             # Just database

# Build and run MCP server
$ ./build.sh                       # Build stdio transport (default)
$ ./build.sh -t http              # Or build HTTP transport
$ ./build.sh -t both              # Or build both

# Run stdio server
$ DATABASE_URL="postgres://postgres:postgres@localhost/todo" \
  npx @modelcontextprotocol/inspector wasmedge run \
  target/wasm32-wasip1/release/fullstack-stdio.wasm

# Or run HTTP server
$ DATABASE_URL="postgres://postgres:postgres@localhost/todo" \
  wasmedge run target/wasm32-wasip1/release/fullstack-http.wasm
# Access at http://127.0.0.1:8080/mcp
```

## Transport Modes

| Mode | Use Case | Isolation | Multi-tenant |
|------|----------|-----------|--------------|
| **stdio** | Module orchestration | Process-level | No |
| **HTTP** | API gateway | Shared process | Yes |

## Available Tools

| Tool | Description |
|------|------------|
| `fetch_todos` | Get all todos from database |
| `create_todo` | Create new todo with audit log |
| `update_todo` | Update existing todo |
| `delete_todo` | Delete todo with cascade |
| `batch_process` | Batch operations on multiple todos |
| `search_todos` | Search todos by title |
| `db_stats` | Get database statistics |
| `test_connection` | Test PostgreSQL connection |

## Test Harness

```bash
$ ./test_harness.sh   # Run automated tests (starts PostgreSQL if needed)
```

## Policy Configuration

Configured via `config.yaml`:
- **Allowed**: Database operations, localhost network, /tmp access
- **Denied**: External network, system directories, sensitive env vars
- **Limits**: 64MB memory, 30s execution time

## Technical Stack

- **WasmEdge**: TCP sockets + HTTP support in WASM
- **Database**: PostgreSQL with audit logging
- **Transport**: stdio (default) or HTTP
- **Schema**: See `init.sql` for database structure