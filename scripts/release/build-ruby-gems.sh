#!/usr/bin/env bash
# scripts/release/build-ruby-gems.sh
#
# Build platform-specific Ruby gems from pre-built FFI artifacts,
# plus a source gem fallback.
#
# Usage:
#   ./scripts/release/build-ruby-gems.sh VERSION ARTIFACTS_DIR
#
# Outputs gems to gem-output/ in the current directory.
#
# Platform gems are derived from the canonical gemspec (workers/ruby/tasker-rb.gemspec)
# with sed patches to: hardcode version, add platform, remove extensions/rb_sys,
# and simplify the file list to lib/**/* only. This avoids maintaining a separate
# gemspec that can drift.
#
# Source gem ships Cargo.toml with explicit crates.io deps (requires Rust).
#
# Runs on ubuntu-22.04 in CI — bash 4+ features are safe to use.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

VERSION="${1:-}"
ARTIFACTS_DIR="${2:-}"

[[ -z "$VERSION" ]] && die "Usage: $0 VERSION ARTIFACTS_DIR"
[[ -z "$ARTIFACTS_DIR" ]] && die "Usage: $0 VERSION ARTIFACTS_DIR"
[[ ! -d "$ARTIFACTS_DIR" ]] && die "Artifacts directory not found: $ARTIFACTS_DIR"

GEM_NAME="tasker-rb"
RUBY_DIR="${REPO_ROOT}/workers/ruby"
OUTPUT_DIR="${REPO_ROOT}/gem-output"
SOURCE_GEMSPEC="${RUBY_DIR}/${GEM_NAME}.gemspec"

[[ ! -f "$SOURCE_GEMSPEC" ]] && die "Source gemspec not found: $SOURCE_GEMSPEC"

log_header "Building Ruby Platform Gems"
log_info "Version: ${VERSION}"
log_info "Artifacts: ${ARTIFACTS_DIR}"
log_info "Output: ${OUTPUT_DIR}"

mkdir -p "$OUTPUT_DIR"

# ---------------------------------------------------------------------------
# Platform mapping: Rust triple -> RubyGems platform + extension
# ---------------------------------------------------------------------------
declare -A PLATFORM_MAP=(
    ["x86_64-unknown-linux-gnu"]="x86_64-linux"
    ["aarch64-apple-darwin"]="arm64-darwin"
)

declare -A EXTENSION_MAP=(
    ["x86_64-unknown-linux-gnu"]="so"
    ["aarch64-apple-darwin"]="bundle"
)

# ---------------------------------------------------------------------------
# patch_gemspec_for_platform
#
# Derives a platform gemspec from the canonical source gemspec via sed.
# Changes:
#   1. Replace require_relative + TaskerCore::VERSION with hardcoded version
#   2. Add spec.platform after spec.version
#   3. Replace multi-line spec.files block with simple lib/**/* glob
#   4. Remove spec.extensions line
#   5. Remove rb_sys dependency line and its comment
# ---------------------------------------------------------------------------
patch_gemspec_for_platform() {
    local src="$1" dest="$2" version="$3" platform="$4"

    cp "$src" "$dest"

    # 1. Remove require_relative line (version comes from hardcoded string now)
    sed_i "/^require_relative 'lib\/tasker_core\/version'/d" "$dest"

    # 2. Replace TaskerCore::VERSION with hardcoded version string
    sed_i "s/TaskerCore::VERSION/'${version}'/" "$dest"

    # 3. Add spec.platform after spec.version line
    sed_i "/spec\.version/a\\
  spec.platform      = '${platform}'" "$dest"

    # 4. Replace the multi-line spec.files block with a simple glob.
    #    The block spans from "spec.files = Dir[" to the closing "end" of the reject block.
    #    We use sed to delete from the spec.files line through the reject block's end,
    #    then insert a simple replacement.
    #    Pattern: delete from "spec.files" line to next "end" line (the reject block close).
    sed_i "/spec\.files = Dir\[/,/^  end$/c\\
  spec.files = Dir['lib/**/*', 'README.md', 'CHANGELOG.md', 'LICENSE'].select { |f| File.file?(f) }" "$dest"

    # 5. Remove spec.extensions line
    sed_i "/spec\.extensions/d" "$dest"

    # 6. Remove rb_sys dependency and its comment
    sed_i "/# Magnus and Rust compilation dependencies/d" "$dest"
    sed_i "/rb_sys/d" "$dest"

    # 7. Remove "Ensure we have a Rust toolchain" comment
    sed_i "/# Ensure we have a Rust toolchain/d" "$dest"

    log_info "Patched gemspec: ${dest}"
}

# ---------------------------------------------------------------------------
# Build platform gems
# ---------------------------------------------------------------------------
for triple in "${!PLATFORM_MAP[@]}"; do
    ruby_platform="${PLATFORM_MAP[$triple]}"
    ext="${EXTENSION_MAP[$triple]}"

    # Find the pre-built binary
    binary="${ARTIFACTS_DIR}/ruby/tasker_rb-${triple}.${ext}"
    if [[ ! -f "$binary" ]]; then
        log_warn "No artifact for ${triple}: ${binary} — skipping platform gem"
        continue
    fi

    log_section "Building platform gem: ${GEM_NAME}-${VERSION}-${ruby_platform}"

    STAGING="${REPO_ROOT}/tmp/gem-staging/${ruby_platform}"
    rm -rf "$STAGING"
    mkdir -p "$STAGING"

    # Copy the Ruby lib tree
    cp -r "${RUBY_DIR}/lib" "$STAGING/lib"

    # Place the pre-built binary where Ruby expects it
    mkdir -p "$STAGING/lib/tasker_core"
    cp "$binary" "$STAGING/lib/tasker_core/tasker_rb.${ext}"

    # Copy supporting files
    for f in README.md CHANGELOG.md LICENSE; do
        [[ -f "${RUBY_DIR}/${f}" ]] && cp "${RUBY_DIR}/${f}" "$STAGING/"
    done

    # Derive platform gemspec from canonical source
    patch_gemspec_for_platform \
        "$SOURCE_GEMSPEC" \
        "$STAGING/${GEM_NAME}.gemspec" \
        "$VERSION" \
        "$ruby_platform"

    # Build the gem
    cd "$STAGING"
    gem build "${GEM_NAME}.gemspec"
    cp "${GEM_NAME}-${VERSION}-${ruby_platform}.gem" "$OUTPUT_DIR/"
    log_info "Built: ${GEM_NAME}-${VERSION}-${ruby_platform}.gem"
    cd "$REPO_ROOT"
done

# ---------------------------------------------------------------------------
# Build source gem (fallback for unsupported platforms)
# ---------------------------------------------------------------------------
log_section "Building source gem: ${GEM_NAME}-${VERSION}"

cd "${RUBY_DIR}"
gem build "${GEM_NAME}.gemspec"
cp "${GEM_NAME}-${VERSION}.gem" "$OUTPUT_DIR/"
log_info "Built: ${GEM_NAME}-${VERSION}.gem (source)"
cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
log_section "Gem build summary"
log_info "Output directory: ${OUTPUT_DIR}"
ls -la "$OUTPUT_DIR"/*.gem

# Cleanup staging
rm -rf "${REPO_ROOT}/tmp/gem-staging"
