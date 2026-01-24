# Multi-stage Dockerfile for Wasmtime calculator MCP server

# Stage 1: Build the WASM module
FROM rust:slim as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Add WASM target for WASI Preview 2
RUN rustup target add wasm32-wasip2

# Create app directory
WORKDIR /app

# Copy workspace files first (for caching)
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY examples/ ./examples/

# Build the WASM module
WORKDIR /app
RUN cargo build --manifest-path examples/wasm/wasmtime/calculator/Cargo.toml --target wasm32-wasip2 --release

# Stage 2: Runtime with Wasmtime and MCP Inspector
FROM ubuntu:22.04

# Install Node.js, Wasmtime, and utilities
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    netcat \
    xz-utils \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install Wasmtime
RUN curl https://wasmtime.dev/install.sh -sSf | bash \
    && mv /root/.wasmtime /opt/wasmtime \
    && chmod +x /opt/wasmtime/bin/wasmtime

# Install MCP Inspector globally
RUN npm install -g @modelcontextprotocol/inspector

# Copy the built WASM module
COPY --from=builder /app/target/wasm32-wasip2/release/calculator.wasm /app/
COPY --from=builder /app/examples/wasm/wasmtime/calculator/config.yaml /app/

# Set environment variables
ENV PATH="/opt/wasmtime/bin:${PATH}"
ENV RUST_LOG="info"
ENV HOST="0.0.0.0"

# Create entrypoint script
RUN echo '#!/bin/bash\n\
echo "Starting Calculator MCP server with Inspector..."\n\
echo ""\n\
echo "========================================"\n\
echo "  MCP Calculator Server Ready!"\n\
echo "  Inspector UI: http://127.0.0.1:6274"\n\
echo "========================================"\n\
echo ""\n\
echo "Available tools:"\n\
echo "  • sum(a, b)      - Add two numbers"\n\
echo "  • sub(a, b)      - Subtract b from a"\n\
echo ""\n\
echo "Starting server..."\n\
export PATH="/opt/wasmtime/bin:${PATH}"\n\
# Verify wasmtime is available\n\
echo "Wasmtime location: $(which wasmtime)"\n\
# Start MCP Inspector with explicit command path\n\
exec env DATABASE_URL="${DATABASE_URL}" npx @modelcontextprotocol/inspector wasmtime run /app/calculator.wasm\n\
' > /app/entrypoint.sh && chmod +x /app/entrypoint.sh

WORKDIR /app

# Inspector runs on ports 6274 (UI) and 6277 (proxy)
EXPOSE 6274 6277

# Use the entrypoint script
ENTRYPOINT ["/app/entrypoint.sh"]