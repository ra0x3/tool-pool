#!/bin/bash

# Build script for wasm-fullstack v2
# Supports both standard WASM (simulated) and WasmEdge (real PostgreSQL/HTTP)

set -e

echo "Building wasm-fullstack v2..."

# Check if building for WasmEdge
if [ "$1" = "wasmedge" ]; then
    echo "Building for WasmEdge with real PostgreSQL and HTTP support..."
    RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" cargo build --bin v2 --target wasm32-wasip1
    echo ""
    echo "✓ Built for WasmEdge!"
    echo ""
    echo "Run with:"
    echo "  docker-compose up -d  # Start PostgreSQL"
    echo "  DATABASE_URL=\"postgres://wasi_user:wasi_password@localhost/todos_db\" \\"
    echo "  wasmedge --env DATABASE_URL ../../target/wasm32-wasip1/debug/v2.wasm"
else
    echo "Building for standard WASM (simulation mode)..."
    cargo build --bin v2 --target wasm32-wasip1

    # Create component if wasm-tools is available
    if command -v wasm-tools &> /dev/null; then
        if [ -f "wasi_snapshot_preview1.command.wasm" ]; then
            echo "Creating WASI component..."
            wasm-tools component new ../../target/wasm32-wasip1/debug/v2.wasm \
                -o v2-component.wasm \
                --adapt wasi_snapshot_preview1.command.wasm
            echo ""
            echo "✓ Built and created component!"
            echo ""
            echo "Run with:"
            echo "  wasmtime run v2-component.wasm"
            echo "  # or"
            echo "  npx @modelcontextprotocol/inspector wasmtime run v2-component.wasm"
        else
            echo ""
            echo "⚠ WASI adapter not found. Download with:"
            echo "  curl -LO https://github.com/bytecodealliance/wasmtime/releases/latest/download/wasi_snapshot_preview1.command.wasm"
        fi
    else
        echo ""
        echo "⚠ wasm-tools not found. Install with:"
        echo "  cargo install wasm-tools"
    fi
fi

echo ""
echo "Note: For real PostgreSQL/HTTP, use: ./build-v2.sh wasmedge"
