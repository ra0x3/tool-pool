#!/bin/bash

set -e

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Check for --dry-run flag
DRY_RUN=""
if [ "$1" = "--dry-run" ]; then
    DRY_RUN="--dry-run"
    echo -e "${YELLOW}Running in dry-run mode...${NC}"
fi

publish_if_not_exists() {
    local crate_name=$1
    local crate_dir=$2

    if [ -z "$DRY_RUN" ] && cargo search "$crate_name" --limit 1 | grep -q "^$crate_name = \"0.15.0\""; then
        echo -e "${YELLOW}Skipping ${CYAN}$crate_name@0.15.0${NC} - already published"
    else
        echo -e "${BLUE}Publishing ${CYAN}$crate_name${NC}..."
        cd "$crate_dir"
        cargo publish --no-verify $DRY_RUN
        cd - > /dev/null
        echo -e "${GREEN}✓ Successfully published ${CYAN}$crate_name${NC}"
    fi
}

publish_if_not_exists "mcpkit-rs-policy" "crates/mcpkit-rs-policy"

publish_if_not_exists "mcpkit-rs-macros" "crates/mcpkit-rs-macros"

echo -e "${CYAN}Waiting for crates.io to index...${NC}"
sleep 30

publish_if_not_exists "mcpkit-rs-config" "crates/mcpkit-rs-config"

echo -e "${CYAN}Waiting for crates.io to index dependencies...${NC}"
sleep 30

publish_if_not_exists "mcpkit_rs" "crates/mcpkit-rs"

echo -e "${CYAN}Waiting for crates.io to index mcpkit_rs...${NC}"
sleep 30

publish_if_not_exists "mcpkit-rs-cli" "crates/mcpkit-rs-cli"

echo -e "${GREEN}✓ All crates published successfully!${NC}"