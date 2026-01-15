# WASM Calculator

Minimal MCP server compiled to WebAssembly demonstrating WASI compilation.

## Prerequisites

- Rust with `wasm32-wasip2` target
- Wasmtime or Wasmer runtime

## Build

```bash
cargo build -p wasi-mcp-example --target wasm32-wasip2 --release
```

Output: `target/wasm32-wasip2/release/wasi_mcp_example.wasm`

## Run

```bash
# Wasmtime
npx @modelcontextprotocol/inspector wasmtime target/wasm32-wasip2/release/wasi_mcp_example.wasm

# Wasmer
npx @modelcontextprotocol/inspector wasmer target/wasm32-wasip2/release/wasi_mcp_example.wasm
```

## Tools

| Tool | Parameters | Returns |
|------|-----------|---------|
| `add` | `a: f64, b: f64` | Sum |
| `subtract` | `a: f64, b: f64` | Difference |
| `multiply` | `a: f64, b: f64` | Product |
| `divide` | `a: f64, b: f64` | Quotient (error if b=0) |

## Technical Details

- Runtime: WASI Preview 2
- Protocol: MCP over stdio
- Binary size: ~2.5 MB (release)
- Limitations: No networking (WASI constraint)