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

### Runtime Functionality Comparison

| Problem | Wasmtime | WasmEdge |
|---------|----------|----------|
| Outbound TCP/HTTP | ✗ Pre-opened FDs only | ✓ Full sockets |
| Inbound TCP (servers) | ⚠ Awkward | ✓ Native |
| Database connections | ✗ | ✓ Postgres, MySQL |
| LLM inference (GGML) | ✗ | ✓ |
| Whisper/audio | ✗ | ✓ |
| TensorFlow/PyTorch | ⚠ OpenVINO only | ✓ Multiple backends |
| Docker Desktop | ✓ | ✓ (ships built-in) |
| Edge K8s (KubeEdge, etc.) | ⚠ | ✓ First-class |
| Plugin system | ✗ | ✓ |
| TLS built-in | ⚠ | ✓ |

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


## Comparison with Wassette

### mcpkit-rs vs Wassette Feature Comparison

| Feature | mcpkit-rs | Wassette |
|---------|-----------|----------|
| **Primary Focus** | Fast development with more features out-of-box | Enterprise-focused with minimal feature set |
| **Architecture** | MCP SDK that compiles to WASM | Bridge between WASM Components and MCP |
| **Language** | Rust | Rust |
| **MCP Implementation** | Full MCP SDK (client/server) | MCP server only |
| **WASM Runtime Support** | Wasmtime + WasmEdge | Wasmtime only |
| **Component Model** | Direct WASM compilation | WASM Components (WIT) |
| **Database Support** | ✓ (via WasmEdge) | ✗ |
| **HTTP Client** | ✓ (via WasmEdge) | ✗ (pre-opened FDs only) |
| **LLM/ML Support** | ✓ (via WasmEdge) | ✗ |
| **Permission System** | Fine-grained config.yaml | Fine-grained policy.yaml |
| **OCI Registry Support** | ✗ | ✓ |
| **Security Model** | Runtime sandboxing | Capability-based + interactive |
| **Network Hosting** | ✗ | ✓ (planned) |
| **Development Model** | Write MCP tools in any WASM language | Write generic WASM Components |
| **Tool Distribution** | Binary/source | OCI registry |
| **Zero Dependencies** | ✗ (requires WASI runtime) | ✗ (requires WASI runtime) |
| **Supported Languages** | JavaScript, Go, Python, Rust | JS, Python, Rust, Go |

### When to Use Which?

**Choose mcpkit-rs when:**
- Building MCP tools that need database or HTTP access
- Requiring ML/AI inference capabilities in tools
- Needing both MCP client and server functionality
- Wanting direct control over MCP implementation
- Needing flexibility to choose between multiple WASM runtimes

**Choose Wassette when:**
- Building language-agnostic WASM Components
- Requiring fine-grained security policies
- Distributing tools via OCI registries
- Needing network-hosted MCP servers
- Working with existing WASM Components

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
- [WASI](https://wasi.dev/)

## Development

- [Contributing Guide](docs/CONTRIBUTE.MD)
- [Dev Container Setup](docs/DEVCONTAINER.md)