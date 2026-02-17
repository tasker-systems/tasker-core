#!/usr/bin/env bash
# =============================================================================
# check-serde-contract.sh — Drift detection for serde FFI contracts
# =============================================================================
#
# The TypeScript (and Python/Ruby) workers build plain objects with snake_case
# keys that must match the Rust serde field layout exactly. If the Rust structs
# change and the language-side builders aren't updated, deserialization silently
# drops fields or fails at runtime.
#
# This script checks whether the Rust source-of-truth file has changed relative
# to the current branch's merge-base with main. If it has, it warns which
# language-side files need review.
#
# Usage:
#   ./scripts/check-serde-contract.sh          # warn mode (default)
#   ./scripts/check-serde-contract.sh --strict  # exit 1 on drift
#
# =============================================================================

set -euo pipefail

STRICT=false
if [[ "${1:-}" == "--strict" ]]; then
  STRICT=true
fi

# The single Rust file that defines the serde contract for step execution results.
# StepExecutionResult, StepExecutionError, StepExecutionMetadata all live here.
RUST_CONTRACT="tasker-shared/src/messaging/execution_types.rs"

# Language-side files that build serde-compatible objects for these structs.
# If the Rust contract changes, these files likely need updating.
DEPENDENT_FILES=(
  # TypeScript — builds snake_case objects in buildStepExecutionResult()
  "workers/typescript/src/subscriber/step-execution-subscriber.ts"
  # Python — builds snake_case dicts, deserialized via depythonize()
  "workers/python/python/tasker_core/step_execution_subscriber.py"
  # Ruby — builds snake_case hashes, deserialized via serde_magnus
  "workers/ruby/lib/tasker_core/subscriber.rb"
)

# Find repo root (script may be called from workers/typescript/)
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [[ -z "$REPO_ROOT" ]]; then
  echo "ERROR: Not in a git repository"
  exit 1
fi

cd "$REPO_ROOT"

# Determine comparison base — merge-base with main, or HEAD~1 as fallback
BASE_REF="main"
MERGE_BASE=$(git merge-base "$BASE_REF" HEAD 2>/dev/null || echo "")
if [[ -z "$MERGE_BASE" ]]; then
  # Fallback: compare against parent commit
  MERGE_BASE="HEAD~1"
fi

# Check if the Rust contract file has changed
if ! git diff --quiet "$MERGE_BASE" -- "$RUST_CONTRACT" 2>/dev/null; then
  echo ""
  echo "=========================================="
  echo "  WARN: Serde FFI contract has changed"
  echo "=========================================="
  echo ""
  echo "  $RUST_CONTRACT"
  echo "  has been modified since diverging from $BASE_REF."
  echo ""
  echo "  The following files build serde-compatible objects that must"
  echo "  match the Rust struct field names exactly. Please review:"
  echo ""

  HAS_DRIFT=false
  for dep in "${DEPENDENT_FILES[@]}"; do
    if [[ -f "$dep" ]]; then
      if git diff --quiet "$MERGE_BASE" -- "$dep" 2>/dev/null; then
        echo "    [NOT UPDATED]  $dep"
        HAS_DRIFT=true
      else
        echo "    [updated]      $dep"
      fi
    fi
  done

  echo ""

  if $HAS_DRIFT; then
    echo "  Files marked [NOT UPDATED] may need changes to match the new"
    echo "  Rust struct layout. Check field names, types, and optionality."
    echo ""
    echo "  Key structs to compare:"
    echo "    - StepExecutionResult  (fields: step_uuid, success, result, metadata, status, error, orchestration_metadata)"
    echo "    - StepExecutionError   (fields: message, error_type, backtrace, retryable, status_code, context)"
    echo "    - StepExecutionMetadata (fields: execution_time_ms, worker_id, completed_at, retryable, custom, error_type, error_code, ...)"
    echo ""

    if $STRICT; then
      echo "  --strict mode: exiting with error"
      exit 1
    fi
  else
    echo "  All dependent files have also been modified — looks good."
    echo "  (Still worth a manual review to confirm field-level alignment.)"
    echo ""
  fi
else
  echo "Serde FFI contract unchanged — no drift check needed."
fi
