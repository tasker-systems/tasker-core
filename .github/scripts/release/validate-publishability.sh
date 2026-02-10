#!/usr/bin/env bash
set -euo pipefail

# Validates Rust crate publishability via cargo publish --dry-run.
#
# Only validates tasker-pgmq (the dependency chain root with zero workspace
# dependencies). All other crates depend on unpublished workspace crates,
# so cargo publish --dry-run fails resolving them from the registry — a
# chicken-and-egg problem that only resolves once we publish in order.
#
# Build correctness for all crates is already validated by clippy.
# The actual publish step runs crates in dependency order and will fail
# fast if any crate has metadata or packaging issues.

echo "Validating tasker-pgmq (dependency chain root)..."
cargo publish -p tasker-pgmq --dry-run --allow-dirty

echo ""
echo "Remaining crates (tasker-shared, tasker-client, tasker-orchestration,"
echo "tasker-worker, tasker-ctl) depend on unpublished workspace crates and"
echo "cannot be validated until published in order. Build correctness is"
echo "covered by clippy."
