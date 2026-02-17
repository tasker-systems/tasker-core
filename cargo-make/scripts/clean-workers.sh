#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "🧹 Cleaning workers..."

# Python worker
echo "  Cleaning Python worker..."
cd "$WORKSPACE_ROOT/workers/python"
rm -rf .venv/ target/ .ruff_cache/ .mypy_cache/ .pytest_cache/ .coverage htmlcov/ *.egg-info/
rm -f python/tasker_core/*.so python/tasker_core/*.pyd python/tasker_core/*.dylib
find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true

# Ruby worker
echo "  Cleaning Ruby worker..."
cd "$WORKSPACE_ROOT/workers/ruby"
bundle exec rake clean 2>/dev/null || true
rm -rf tmp/
rm -f lib/*.bundle lib/*.so lib/*.dylib 2>/dev/null || true
(cd ext/tasker_core && cargo clean) 2>/dev/null || true

# TypeScript worker
echo "  Cleaning TypeScript worker..."
cd "$WORKSPACE_ROOT/workers/typescript"
rm -rf dist/
rm -rf node_modules/
rm -f *.node
cargo clean -p tasker-ts 2>/dev/null || true

echo "✓ Workers cleaned"
