#!/usr/bin/env bash
# scripts/release/publish-crates.sh
#
# Publish Rust crates to crates.io in dependency order.
#
# Usage:
#   ./scripts/release/publish-crates.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]
#
# Publishes six crates in four phases respecting dependency ordering:
#   Phase 1: tasker-pgmq
#   Phase 2: tasker-shared
#   Phase 3: tasker-client, tasker-orchestration
#   Phase 4: tasker-worker, tasker-ctl
#
# Requires CARGO_REGISTRY_TOKEN (skipped in dry-run mode).
# Requires SQLX_OFFLINE=true (no database in release runner).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
VERSION="${1:-}"
shift || true

DRY_RUN=false
ON_DUPLICATE="warn"

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)          DRY_RUN=true; shift ;;
        --on-duplicate=*)   ON_DUPLICATE="${1#*=}"; shift ;;
        *)                  die "Unknown argument: $1" ;;
    esac
done

[[ -z "$VERSION" ]] && die "Usage: $0 VERSION [--dry-run] [--on-duplicate=skip|warn|fail]"

log_header "Publish Rust Crates (crates.io)"
log_info "Version: ${VERSION}"
log_info "Dry run: ${DRY_RUN}"
log_info "On duplicate: ${ON_DUPLICATE}"

# ---------------------------------------------------------------------------
# Pre-flight: verify credentials
# ---------------------------------------------------------------------------
if [[ "$DRY_RUN" != "true" ]]; then
    require_env "CARGO_REGISTRY_TOKEN" "crates.io publishing"
fi

# ---------------------------------------------------------------------------
# Publish in dependency order
# ---------------------------------------------------------------------------
PHASE_1=("tasker-pgmq")
PHASE_2=("tasker-shared")
PHASE_3=("tasker-client" "tasker-orchestration" "tasker-tooling")
PHASE_4=("tasker-worker" "tasker-ctl" "tasker-mcp")

publish_phase() {
    local phase_name="$1"
    shift
    local crates=("$@")

    log_section "Phase ${phase_name}"

    for crate in "${crates[@]}"; do
        if [[ "$DRY_RUN" == "true" ]]; then
            # Only tasker-pgmq (dependency chain root with zero workspace deps)
            # can be validated via --dry-run. All other crates depend on
            # unpublished workspace crates, causing cargo publish --dry-run to
            # fail resolving them from the registry (chicken-and-egg problem).
            if [[ "$crate" == "tasker-pgmq" ]]; then
                log_info "[dry-run] Validating ${crate}@${VERSION}"
                cargo publish -p "$crate" --dry-run
            else
                log_info "[dry-run] Skipping ${crate}@${VERSION} (depends on unpublished workspace crates)"
            fi
            continue
        fi

        # Check if already published
        if crate_exists_on_registry "$crate" "$VERSION"; then
            handle_duplicate "$ON_DUPLICATE" "$crate" "$VERSION" "crates.io"
            continue
        fi

        log_info "Publishing ${crate}@${VERSION}..."
        cargo publish -p "$crate"
    done
}

publish_phase "1" "${PHASE_1[@]}"

# Wait for crates.io index propagation between phases
if [[ "$DRY_RUN" != "true" ]]; then
    log_info "Waiting 30s for crates.io index propagation..."
    sleep 30
fi

publish_phase "2" "${PHASE_2[@]}"

if [[ "$DRY_RUN" != "true" ]]; then
    log_info "Waiting 30s for crates.io index propagation..."
    sleep 30
fi

publish_phase "3" "${PHASE_3[@]}"

if [[ "$DRY_RUN" != "true" ]]; then
    log_info "Waiting 30s for crates.io index propagation..."
    sleep 30
fi

publish_phase "4" "${PHASE_4[@]}"

log_section "Done"
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "Dry run complete — no crates were published"
else
    log_info "All crates published successfully"
fi
