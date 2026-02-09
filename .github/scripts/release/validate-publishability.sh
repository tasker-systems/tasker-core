#!/usr/bin/env bash
set -euo pipefail

# Validates that all Rust crates can be published via cargo publish --dry-run.
#
# Uses --no-verify to skip build verification because workspace crates depend
# on each other and aren't on crates.io yet. Without --no-verify, cargo tries
# to resolve dependencies from the registry and fails. Build correctness is
# already validated by the clippy step. This step validates metadata, package
# structure, and license compliance.

for crate in tasker-pgmq tasker-shared tasker-client tasker-orchestration tasker-worker tasker-cli; do
  echo "Validating ${crate}..."
  cargo publish -p "$crate" --dry-run --allow-dirty --no-verify
done
