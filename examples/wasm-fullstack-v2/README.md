# WASM Fullstack V2 Example - Real PostgreSQL & HTTP

## ⚠️ IMPORTANT: WasmEdge Runtime Required ⚠️

**This example is WasmEdge-only and will NOT work with:**
- ❌ Wasmtime
- ❌ MCP Inspector (uses wasmtime)
- ❌ Any standard WASM runtime

**The ENTIRE PURPOSE of this v2 example is to demonstrate real TCP and PostgreSQL connectivity through WasmEdge. These features are NOT optional.**

## Why WasmEdge?

Standard WASI runtimes cannot:
- Connect to databases over TCP
- Make HTTP requests
- Use network sockets

WasmEdge extends WASI with real networking capabilities, enabling:
- ✓ Real PostgreSQL connections via TCP
- ✓ Real HTTP requests to external APIs
- ✓ Full socket support

## Overview

Two versions demonstrating WasmEdge capabilities:
- **v1**: Basic in-memory storage (simpler example)
- **v2**: Real PostgreSQL database + HTTP API calls

## Prerequisites

1. **Install WasmEdge**:
```bash
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- -v 0.14.0
source $HOME/.wasmedge/env
```

2. **Start PostgreSQL**:
```bash
docker-compose up -d
```

## Build & Run

### Standard WASM (Simulation Mode)
```bash
# Build and create component with canonical naming
./build-v2.sh

# Creates: wasm-fullstack-v2_snapshot_preview.wasm
# Run with MCP inspector
npx @modelcontextprotocol/inspector wasmtime run wasm-fullstack-v2_snapshot_preview.wasm
```

### Build for WasmEdge (Real PostgreSQL/HTTP)
```bash
# Build with WasmEdge support
./build-v2.sh wasmedge

# Run with WasmEdge
DATABASE_URL="postgres://wasi_user:wasi_password@localhost/todos_db" \
wasmedge --env DATABASE_URL target/wasm32-wasip1/release/wasm-fullstack-v2.wasm
```

## Features

| Tool | Implementation |
|------|---------------|
| **fetch_todos** | Real PostgreSQL SELECT query |
| **fetch_todos {"from_api": true}** | Real HTTP to JSONPlaceholder API |
| **create_todo** | PostgreSQL INSERT + WAL table |
| **update_todo** | PostgreSQL UPDATE |
| **delete_todo** | PostgreSQL DELETE |
| **batch_process** | Batch PostgreSQL operations |
| **search_todos** | PostgreSQL LIKE query |
| **db_stats** | PostgreSQL view aggregation |
| **test_connection** | Real PostgreSQL version check |

## Example Usage

```bash
# Test database connection
test_connection {}

# Fetch todos from real API and save to PostgreSQL
fetch_todos {"from_api": true}

# Create todo in PostgreSQL
create_todo {"title": "Test WasmEdge networking", "user_id": 1}

# Search in PostgreSQL
search_todos {"title_contains": "WasmEdge"}

# Batch operations
batch_process {"ids": ["todo-1", "todo-2"], "operation": "complete"}
```

## Architecture

```
┌─────────────────────────┐
│     MCP Client          │
└────────┬────────────────┘
         │
┌────────┴────────────────┐
│  WASI Server v2         │
│  (WasmEdge Runtime)     │
├─────────────────────────┤
│ • Real TCP sockets      │
│ • PostgreSQL client     │
│ • HTTP client           │
└────────┬────────────────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
PostgreSQL   JSONPlaceholder
(Docker)     (Internet)
```

## Database Schema

See `init.sql` for PostgreSQL schema with:
- `todos` table for todo items
- `wal_entries` table for audit log
- `todo_stats` view for aggregations

## Notes

- The WasmEdge patches in the workspace `Cargo.toml` enable PostgreSQL and HTTP support
- These are forks from Second State that add socket support to tokio, reqwest, and tokio-postgres
- Without WasmEdge, this example will not compile or run