# WASM Fullstack V1 Example

This is the V1 implementation of the WASM fullstack example, which provides a simple in-memory todo list management system.

## Features

- In-memory storage (no database required)
- Basic CRUD operations for todos
- UUID generation
- Runs in standard WASM runtimes

## Known Issues

### Tokio 1.36 Compatibility

This example uses **tokio 1.36** for WASM compatibility, which has a known issue with task completion during runtime shutdown:

- **Symptom**: The `tools/list` response may not appear in the MCP Inspector, showing only POST messages
- **Root Cause**: Tokio 1.36's runtime doesn't properly flush pending async tasks when stdin closes
- **Impact**: Tools are registered correctly internally but the list response may not be sent to stdout
- **Workaround**: The initialization handshake works, and tools can be called directly if you know their names

This issue is resolved in tokio 1.46+, but that version currently has compatibility issues with WASM targets. The issue will be resolved once the WASM ecosystem updates.

## Building

```bash
# Build and create the WASM component with canonical naming
./build.sh

# This creates: wasm-fullstack-v1_snapshot_preview.wasm
```

## Running

```bash
# Using npx with MCP inspector (run the component, not the raw wasm)
npx @modelcontextprotocol/inspector wasmtime run ./wasm-fullstack-v1_snapshot_preview.wasm

# Or directly with wasmtime
wasmtime run ./wasm-fullstack-v1_snapshot_preview.wasm

# Using WasmEdge (if available)
wasmedge ./wasm-fullstack-v1_snapshot_preview.wasm

# Note: Do NOT run the raw wasm file in target/wasm32-wasip1/release/
# Always use the component file created by the build script
```

## Available Tools

- `create_todo`: Create a new todo item
- `list_todos`: List all todos (optionally filtered by user)
- `update_todo`: Update an existing todo
- `delete_todo`: Delete a todo by ID

## Notes

This version does not require any external dependencies or database connections, making it suitable for simple demonstrations and testing.