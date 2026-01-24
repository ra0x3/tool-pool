<div align="center">
  <h1 align="center">mcpkit-rs</h1>
  <p><b>Fork of Rust MCP SDK optimized for Rust integration with WebAssembly runtime support</b></p>

[Quick Start](#quick-start) | [FAQ](https://github.com/ra0x3/mcpkit-rs/issues) | [Documentation](#resources) | [Releases](https://github.com/ra0x3/mcpkit-rs/releases) | [Contributing](docs/CONTRIBUTE.MD) | [Discord](https://discord.gg/microsoft-open-source)
</div>

[![Crates.io Version](https://img.shields.io/crates/v/rmcp)](https://crates.io/crates/rmcp)
![Coverage](docs/coverage.svg)

> [!WARNING]
> **Early Development**: This repository is not production ready yet. It is in early development and may change significantly.

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
$ curl https://wasmtime.dev/install.sh -sSf | bash

# Add WASI compilation target
$ rustup target add wasm32-wasip2
```

#### WasmEdge Runtime
```bash
# Install WasmEdge with plugins
$ curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- --plugins wasmedge_rustls

# Add WasmEdge compilation target (uses wasm32-wasip1)
$ rustup target add wasm32-wasip1
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

See the [examples directory](examples/) for working implementations.

## Resources

- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [Original Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Tool Pool Design](docs/TOOL_POOL_TECH_DESIGN_v1.md)
- [WASI](https://wasi.dev/)

## Development

- [Contributing Guide](docs/CONTRIBUTE.MD)
- [Dev Container Setup](docs/DEVCONTAINER.md)