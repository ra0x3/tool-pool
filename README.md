# mcpkit-rs

Fork of [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) optimized for Rust integration with WebAssembly runtime support.

[![Crates.io Version](https://img.shields.io/crates/v/rmcp)](https://crates.io/crates/rmcp)
![Coverage](docs/coverage.svg)

## Overview

This fork provides a Rust-native MCP SDK with:
- **Multiple WASM Runtime Support**: Both WASI (wasm32-wasip2) and WasmEdge runtimes
- **Tokio 1.36**: Downgraded from 1.40+ for broader compatibility
- **Portable Tool Development**: Build once, run anywhere with WebAssembly

## WebAssembly Runtime Support

This fork enables portable MCP tools through WebAssembly, supporting multiple runtimes for different use cases:

### Supported Runtimes

- **WASI (wasm32-wasip2)**: Standard WebAssembly System Interface for general-purpose tools
- **WasmEdge**: Extended runtime with PostgreSQL and HTTP client support for full-stack applications

### Why WebAssembly?

Current MCP ecosystem suffers from massive duplication - everyone writes the same tools. These tools are:
- Deterministic and non-differentiating
- Expensive to maintain relative to value
- Perfect candidates for community reuse

WebAssembly provides:
- **Portability**: Compile once, run on any runtime
- **Security**: Sandboxed execution with explicit capabilities
- **Simplicity**: Standard interfaces, no FFI complexity
- **Reproducibility**: Same tool version = same behavior
- **Distribution**: Share tools as binaries, not services

See [Tool Pool Technical Design](docs/TOOL_POOL_TECH_DESIGN_v1.md) for details.

## Installation

### Prerequisites

- Rust 1.75+
- Tokio 1.36 (included in dependencies)
- WebAssembly runtime (choose based on your needs):
  - **WASI/Wasmtime**: For standard WASI tools
  - **WasmEdge**: For tools requiring PostgreSQL or HTTP client

### Runtime Installation

#### WASI Runtime (Wasmtime)
```bash
# Install wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash

# Add WASI compilation target
rustup target add wasm32-wasip2
```

#### WasmEdge Runtime
```bash
# Install WasmEdge with plugins
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- --plugins wasmedge_rustls

# Add WasmEdge compilation target (uses wasm32-wasip1)
rustup target add wasm32-wasip1
```

### Cargo Dependencies

```toml
rmcp = { version = "0.13.0", features = ["server"] }

# For WASI compilation
[target.wasm32-wasip2.dependencies]
rmcp = { version = "0.13.0", features = ["server", "wasi"] }

# For WasmEdge compilation
[target.wasm32-wasip1.dependencies]
rmcp = { version = "0.13.0", features = ["server", "wasmedge"] }
```

## Quick Start

### Minimal WASI Tool

```rust
use rmcp::{handler::server::ServerHandler, protocol::*, ServiceExt};
use serde_json::Value;
use tokio::io::{stdin, stdout};

#[derive(Clone)]
struct HelloTool;

#[rmcp::async_trait]
impl ServerHandler for HelloTool {
    async fn list_tools(&self) -> ServerResult<ListToolsResponse> {
        Ok(ListToolsResponse {
            tools: vec![Tool {
                name: "hello".into(),
                description: Some("Greet someone".into()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    },
                    "required": ["name"]
                }),
            }],
            ..Default::default()
        })
    }

    async fn call_tool(&self, params: CallToolParams) -> ServerResult<CallToolResponse> {
        if params.name == "hello" {
            let name = params.arguments
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("World");

            Ok(CallToolResponse {
                content: vec![Content::Text(TextContent {
                    text: format!("Hello, {}!", name),
                    annotations: None,
                })],
                ..Default::default()
            })
        } else {
            Err(ServerError::MethodNotFound)
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = (stdin(), stdout());
    let server = HelloTool.serve(transport).await?;
    server.waiting().await?;
    Ok(())
}
```

### Build and Run

```bash
# Add WASI target
rustup target add wasm32-wasip2

# Build
cargo build --target wasm32-wasip2 --release

# Run with wasmtime
wasmtime target/wasm32-wasip2/release/your_tool.wasm

# Test with MCP Inspector
npx @modelcontextprotocol/inspector wasmtime target/wasm32-wasip2/release/your_tool.wasm
```

## Examples

| Example | Runtime | Description | Location |
|---------|---------|-------------|----------|
| WASM Calculator | WASI | Basic arithmetic operations | [examples/wasm-calculator](examples/wasm-calculator) |
| WASM Fullstack | WasmEdge | PostgreSQL + HTTP/stdio | [examples/wasm-fullstack](examples/wasm-fullstack) |
| Native Examples | Native | Client/server implementations | [examples/README.md](examples/README.md) |

### Using WASM Artifacts with Claude CLI

Add the compiled WASM server to Claude CLI:

```bash
# Build the wasm-fullstack example first
cd examples/wasm-fullstack
cargo build --target wasm32-wasip1 --release

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
```

Test the connection:

```bash
# Connect and verify
claude --debug "mcp"
# Use /mcp to connect to wasm-fullstack server

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


## Resources

- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [Original Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Tool Pool Design](docs/TOOL_POOL_TECH_DESIGN_v1.md)
- [WASI](https://wasi.dev/)

## Development

- [Contributing Guide](docs/CONTRIBUTE.MD)
- [Dev Container Setup](docs/DEVCONTAINER.md)