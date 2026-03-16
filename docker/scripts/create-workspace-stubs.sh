#!/bin/bash
# =============================================================================
# Create Workspace Stubs
# =============================================================================
# Creates minimal stub files for Cargo workspace members that aren't needed
# by a particular Docker build. This satisfies Cargo's workspace validation
# without copying full source code.
#
# Usage: create-workspace-stubs.sh [crate1] [crate2] ...
#   Arguments are the crates that need stubs (not the ones being built)
#
# Example:
#   # For orchestration build (needs stubs for worker crates)
#   ./create-workspace-stubs.sh tasker-worker tasker-example-rs tasker-rb tasker-py tasker-ts
#
# Each stub consists of:
#   - A minimal src/lib.rs with a stub function
#   - The actual Cargo.toml (must be COPY'd separately)

set -e

STUB_CONTENT='pub fn stub() {}'

# Map of crate names to their paths in the workspace
declare -A CRATE_PATHS=(
    ["tasker-orchestration"]="crates/tasker-orchestration"
    ["tasker-worker"]="crates/tasker-worker"
    ["tasker-shared"]="crates/tasker-shared"
    ["tasker-client"]="crates/tasker-client"
    ["tasker-ctl"]="crates/tasker-ctl"
    ["tasker-pgmq"]="crates/tasker-pgmq"
    ["tasker-sdk"]="crates/tasker-sdk"
    ["tasker-mcp"]="crates/tasker-mcp"
    ["tasker-grammar"]="crates/tasker-grammar"
    ["tasker-secure"]="crates/tasker-secure"
    ["tasker-runtime"]="crates/tasker-runtime"
    ["tasker-example-rs"]="crates/tasker-example-rs"
    ["tasker-rb"]="crates/tasker-rb/ext/tasker_core"
    ["tasker-py"]="crates/tasker-py"
    ["tasker-ts"]="crates/tasker-ts"
)

create_stub() {
    local crate_key="$1"
    local crate_path="${CRATE_PATHS[$crate_key]}"

    if [ -z "$crate_path" ]; then
        echo "Warning: Unknown crate '$crate_key', using as literal path"
        crate_path="$crate_key"
    fi

    echo "Creating stub for: $crate_path"
    mkdir -p "$crate_path/src"
    echo "$STUB_CONTENT" > "$crate_path/src/lib.rs"
}

# Process all arguments as crates needing stubs
for crate in "$@"; do
    create_stub "$crate"
done

echo "Workspace stubs created successfully"
