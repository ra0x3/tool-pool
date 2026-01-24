# Multi-stage Dockerfile for WasmEdge fullstack MCP server

# Stage 1: Build the WASM module
FROM rust:slim as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Add WASM target
RUN rustup target add wasm32-wasip1

# Create app directory
WORKDIR /app

# Copy workspace files first (for caching)
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY examples/ ./examples/

# Build the WASM modules (both stdio and http versions) with verbose output
WORKDIR /app
RUN RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" \
    CARGO_TERM_COLOR=always \
    cargo build --manifest-path examples/wasm/wasmedge/fullstack/Cargo.toml \
    --target wasm32-wasip1 --release --features wasmedge-postgres \
    --bin fullstack-stdio --bin fullstack-http -vv

# Stage 2: Runtime with WasmEdge and MCP Inspector
FROM ubuntu:22.04

# Install WasmEdge, PostgreSQL client, and Node.js
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    postgresql-client \
    netcat \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install WasmEdge
RUN curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash

# Install MCP Inspector globally
RUN npm install -g @modelcontextprotocol/inspector

# Copy the built WASM modules (both stdio and http versions)
COPY --from=builder /app/examples/wasm/wasmedge/fullstack/target/wasm32-wasip1/release/fullstack-stdio.wasm /app/
COPY --from=builder /app/examples/wasm/wasmedge/fullstack/target/wasm32-wasip1/release/fullstack-http.wasm /app/
COPY --from=builder /app/examples/wasm/wasmedge/fullstack/config.yaml /app/

# Set environment variables
ENV PATH="/root/.wasmedge/bin:${PATH}"
ENV LD_LIBRARY_PATH="/root/.wasmedge/lib:${LD_LIBRARY_PATH}"
ENV DATABASE_URL="postgres://postgres:postgres@postgres:5432/todo"
ENV HOST="0.0.0.0"

# Create entrypoint script for running with Inspector
RUN echo '#!/bin/bash\n\
echo "Waiting for PostgreSQL to be ready..."\n\
while ! nc -z postgres 5432; do\n\
  sleep 1\n\
done\n\
echo "PostgreSQL is ready!"\n\
\n\
# Test database connection\n\
PGPASSWORD=postgres psql -h postgres -U postgres -d todo -c "SELECT 1" > /dev/null 2>&1 || exit 1\n\
echo "Database connection verified!"\n\
\n\
echo ""\n\
echo "========================================"\n\
echo "  MCP Fullstack Server Ready!"\n\
echo "  Inspector UI: http://127.0.0.1:6274"\n\
echo "========================================"\n\
echo ""\n\
echo "Available tools:"\n\
echo "  • fetch_todos    - Get todos from DB"\n\
echo "  • create_todo    - Create a new todo"\n\
echo "  • update_todo    - Update existing todo"\n\
echo "  • delete_todo    - Delete a todo"\n\
echo "  • batch_process  - Batch operations"\n\
echo "  • search_todos   - Search by title"\n\
echo "  • db_stats       - Database statistics"\n\
echo "  • test_connection - Test DB connection"\n\
echo ""\n\
echo "Starting MCP server with Inspector..."\n\
echo "Note: Inspector will be available at http://127.0.0.1:6274 from your host machine"\n\
export PATH="/root/.wasmedge/bin:${PATH}"\n\
export HOST="0.0.0.0"\n\
exec env DATABASE_URL="${DATABASE_URL}" DANGEROUSLY_OMIT_AUTH=true npx @modelcontextprotocol/inspector wasmedge run /app/fullstack-stdio.wasm\n\
' > /app/entrypoint.sh && chmod +x /app/entrypoint.sh

WORKDIR /app

# Create test entrypoint script that runs HTTP version directly
RUN echo '#!/bin/bash\n\
echo "Waiting for PostgreSQL to be ready..."\n\
while ! nc -z postgres 5432; do\n\
  sleep 1\n\
done\n\
echo "PostgreSQL is ready!"\n\
\n\
# Test database connection\n\
PGPASSWORD=postgres psql -h postgres -U postgres -d todo -c "SELECT 1" > /dev/null 2>&1 || exit 1\n\
echo "Database connection verified!"\n\
\n\
echo "Starting MCP HTTP server for testing..."\n\
export PATH="/root/.wasmedge/bin:${PATH}"\n\
exec env DATABASE_URL="${DATABASE_URL}" HOST="0.0.0.0" PORT="8080" wasmedge run /app/fullstack-http.wasm\n\
' > /app/entrypoint-test.sh && chmod +x /app/entrypoint-test.sh

# Inspector runs on ports 6274 (UI) and 6277 (proxy), HTTP server on 8080
EXPOSE 6274 6277 8080

# Use the entrypoint script
ENTRYPOINT ["/app/entrypoint.sh"]