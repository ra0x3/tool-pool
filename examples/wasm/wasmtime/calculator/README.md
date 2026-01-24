# Calculator (Wasmtime Example)

A minimal MCP server compiled to WebAssembly using WASI Component Model (preview 2) for Wasmtime.

## 🚀 Quick Start

### Mode 1: Docker (Everything Automated)
```bash
$ docker-compose up                # Start with Inspector UI
# Access at http://localhost:5173
$ docker-compose down              # Stop services
```

### Mode 2: Manual (Direct Execution)
```bash
$ ./build.sh                       # Build WASM module
$ npx @modelcontextprotocol/inspector wasmtime run ./calculator.wasm
```

## Tools

| Tool | Description | Example |
|------|------------|---------|
| `add` | Addition | `add(5, 3) = 8` |
| `subtract` | Subtraction | `subtract(10, 4) = 6` |
| `multiply` | Multiplication | `multiply(6, 7) = 42` |
| `divide` | Division | `divide(20, 4) = 5` |

## Test Harness

```bash
$ ./test_harness.sh   # Run automated tests
```

## Technical Stack
- **Runtime**: WASI Component Model (preview 2)
- **Transport**: MCP over stdio
- **Policy**: Configurable via config.yaml