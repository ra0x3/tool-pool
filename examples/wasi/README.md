# WASI Calculator Example

A simple calculator MCP server demonstrating WASI compilation and cross-platform compatibility.

## Overview

This example shows how to build MCP servers as WebAssembly modules that run anywhere with a WASI runtime. The calculator provides basic arithmetic operations through the MCP protocol.

## Features

- **Basic Operations**: Addition, subtraction, multiplication, division
- **Cross-Platform**: Single WASM binary runs on any OS
- **Sandboxed**: Secure execution in WASI runtime
- **No Dependencies**: Pure WASI implementation

## Quick Start

### Build

```bash
cargo build -p wasi-mcp-example --target wasm32-wasip2 --release
```

Creates: `target/wasm32-wasip2/release/wasi_mcp_example.wasm`

### Run

Using wasmtime:
```bash
npx @modelcontextprotocol/inspector wasmtime target/wasm32-wasip2/release/wasi_mcp_example.wasm
```

Using wasmer:
```bash
npx @modelcontextprotocol/inspector wasmer target/wasm32-wasip2/release/wasi_mcp_example.wasm
```

## Available Tools

### `add`
Add two numbers
```json
{
  "a": 5,
  "b": 3
}
// Returns: 8
```

### `subtract`
Subtract two numbers
```json
{
  "a": 10,
  "b": 4
}
// Returns: 6
```

### `multiply`
Multiply two numbers
```json
{
  "a": 6,
  "b": 7
}
// Returns: 42
```

### `divide`
Divide two numbers (with zero check)
```json
{
  "a": 20,
  "b": 4
}
// Returns: 5
```

## Testing

Open the MCP Inspector URL and connect via STDIO. Test each operation:
1. Select a tool (add, subtract, multiply, divide)
2. Enter parameter values
3. Execute and see results

## Binary Size

- Debug: ~3.5 MB
- Release: ~2.5 MB (with optimizations)

## Why WASI?

- **Portability**: Same binary works everywhere
- **Security**: Sandboxed execution by default
- **Distribution**: Single file, no runtime dependencies
- **Performance**: Near-native speed