#!/usr/bin/env bash
set -euo pipefail

# Validates that all Rust crates can be published via cargo publish --dry-run.

for crate in tasker-pgmq tasker-shared tasker-client tasker-orchestration tasker-worker tasker-cli; do
  echo "Validating ${crate}..."
  cargo publish -p "$crate" --dry-run --allow-dirty
done
