#!/usr/bin/env bash
set -euo pipefail

echo "📦 Preparing SQLX cache for all crates..."
echo ""

# List of crates that use SQLX and need cache generation
CRATES=(
  "."
  "crates/tasker-shared"
  "crates/tasker-orchestration"
  "crates/tasker-worker"
  "crates/tasker-client"
  "crates/tasker-ctl"
  "crates/tasker-pgmq"
  "crates/workers/ruby/ext/tasker_core"
  "crates/workers/rust"
  "crates/workers/python"
  "crates/workers/typescript"
)

# Store the workspace root
_WORKSPACE_ROOT=$(pwd)

for crate in "${CRATES[@]}"; do
  if [ "$crate" = "." ]; then
    echo "📦 Preparing SQLX cache for workspace root..."
    cargo sqlx prepare -- --all-targets --all-features
  else
    echo "📦 Preparing SQLX cache for $crate..."
    (cd "$crate" && cargo sqlx prepare -- --all-targets --all-features)
  fi
  echo "✓ SQLX cache prepared for $crate"
  echo ""
done

echo "✅ All SQLX caches prepared"
echo ""
echo "Don't forget to commit the .sqlx directories!"
