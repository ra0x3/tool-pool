#!/bin/bash

set -e

publish_if_not_exists() {
    local crate_name=$1
    local crate_dir=$2

    if cargo search "$crate_name" --limit 1 | grep -q "^$crate_name = \"0.15.0\""; then
        echo "Skipping $crate_name@0.15.0 - already published"
    else
        echo "Publishing $crate_name..."
        cd "$crate_dir"
        cargo publish --no-verify
        cd - > /dev/null
    fi
}

publish_if_not_exists "mcpkit-rs-policy" "crates/mcpkit-rs-policy"

publish_if_not_exists "mcpkit-rs-macros" "crates/mcpkit-rs-macros"

echo "Waiting for crates.io to index..."
sleep 30

publish_if_not_exists "mcpkit-rs-config" "crates/mcpkit-rs-config"

echo "Waiting for crates.io to index dependencies..."
sleep 30

publish_if_not_exists "mcpkit_rs" "crates/mcpkit-rs"

echo "Waiting for crates.io to index mcpkit_rs..."
sleep 30

publish_if_not_exists "mcpkit-rs-cli" "crates/mcpkit-rs-cli"

echo "All crates published successfully!"