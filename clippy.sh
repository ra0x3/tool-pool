#!/bin/bash

# This script runs all clippy variations used in CI
# Based on .github/workflows/ci.yml

set -e  # Exit on error

echo "Running all clippy variations from CI..."
echo "========================================="

# Counter for tracking which variation is running
count=0
total=10

run_clippy() {
    count=$((count + 1))
    local features="$1"
    local desc="$2"

    echo ""
    echo "[$count/$total] Running clippy with: $desc"
    echo "Command: cargo clippy --all-targets $features -- -D warnings"
    echo "---------------------------------------------------------"

    cargo clippy --all-targets $features -- -D warnings

    if [ $? -eq 0 ]; then
        echo "✓ Clippy passed for: $desc"
    else
        echo "✗ Clippy failed for: $desc"
        exit 1
    fi
}

# Run all clippy variations from CI
run_clippy "" "default features"
run_clippy "--all-features" "all features"
run_clippy "--no-default-features --features client" "no defaults + client"
run_clippy "--no-default-features --features server" "no defaults + server"
run_clippy "--no-default-features --features macros,server" "no defaults + macros,server"
run_clippy "--features client" "client feature"
run_clippy "--features server" "server feature"
run_clippy "--features client,transport-child-process" "client + transport-child-process"
run_clippy "--features server,transport-streamable-http-server" "server + transport-streamable-http-server"
run_clippy "--features server,wasm-tools" "server + wasm-tools"

echo ""
echo "========================================="
echo "✓ All clippy checks passed successfully!"
echo "========================================="