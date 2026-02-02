# TAS-170: Automated Release Management System for Tasker Ecosystem (Revised)

## Overview

Build an automated release management system for the Tasker ecosystem that handles version management, selective package publishing, and cross-language dependency coordination across crates.io, RubyGems, PyPI, and npm.

## Context & Rationale

Tasker Core is approaching alpha release readiness. The ecosystem consists of:

**Rust crates in monorepo (all currently at 0.1.0):**

| Crate | Description | Internal Dependencies | Publish Order |
|-------|-------------|----------------------|---------------|
| `tasker-pgmq` | PGMQ wrapper with NOTIFY support | None | 1 |
| `tasker-shared` | Foundational types, models, messaging | tasker-pgmq | 2 |
| `tasker-client` | API client (REST + gRPC transport) | tasker-shared | 3 |
| `tasker-orchestration` | Orchestration system | tasker-pgmq, tasker-shared | 3 |
| `tasker-worker` | FFI-capable worker foundation (cdylib) | tasker-pgmq, tasker-client, tasker-shared | 4 |
| `tasker-cli` | CLI utilities, config explorer, plugin foundation | tasker-client, tasker-shared | 4 |

**FFI language bindings (published to language-specific registries, not crates.io):**

| Package | Registry | Rust Crate | Build Tool | Package Manager |
|---------|----------|------------|------------|-----------------|
| `tasker-worker-rb` | RubyGems | `tasker-worker-rb` (cdylib via magnus) | rake compile (rb_sys) | bundle |
| `tasker-worker-py`\* | PyPI | `tasker-worker-py` (cdylib via pyo3) | maturin | uv |
| `@tasker-systems/worker` | npm | `tasker-worker-ts` (cdylib, C ABI) | cargo build + tsup | bun |

\* Currently named `tasker-core-py` on PyPI -- recommend renaming to `tasker-worker-py` for consistency with the `tasker-worker-` prefix, especially given future `tasker-client-{rb,py,ts}` packages.

**Not published:**

- `tasker-worker-rust` (workers/rust) -- example crate showing how to wire up a Rust worker. Users should depend on `tasker-worker` directly. Consider renaming to `tasker-worker-rs` for naming consistency.
- Root `tasker-core` crate -- workspace root, not independently useful.

**Future (in monorepo):**

- Client bindings: `tasker-client-{rb,py,ts}` -- bare-metal FFI wrappers, same pattern as `tasker-worker-{rb,py,ts}`

**Future (tasker-contrib, separate repo):**

- Unified language packages: `tasker-{rb,py,ts}` -- combine worker + client for each language
- Framework integrations: `tasker-rails`, `tasker-fastapi`, etc. -- depend on `tasker-{rb,py}` and add framework niceties (ActiveJob integration, Celery integration, native config generation, CLI templating)
- Web UI (Bun + SvelteKit)
- TUI (ratatui)

> **Extensibility note:** The change detection, version calculation, and publish pipeline are designed to accommodate additional FFI crates. Adding `tasker-client-{rb,py,ts}` later means adding entries to the detection paths, version file lists, and publish scripts -- no structural changes needed.

**Key challenges:**

1. Changes to FFI-facing crates (`tasker-shared`, `tasker-worker`, `tasker-pgmq`) require rebuilding and republishing ALL FFI bindings
2. Language-specific fixes in bindings need independent patch releases
3. Weekly release cadence during alpha with multiple daily merges to main
4. Need automation from the start -- manual processes won't scale
5. Alpha stays in 0.1.N range; no strict semver yet

## Goals

**Primary:**

- Minimize executive function cost of releases (one command, everything automated)
- Prevent version drift across ecosystem
- Only publish packages that actually changed
- Support "edge" deployment (GitHub references to main) alongside published packages

**Secondary:**

- Establish patterns that scale to LTS support later
- Create clear audit trail of what was published when
- Make it easy to roll back or skip problematic releases

---

## Technical Design

### Versioning Strategy

#### Core Rust Crates (0.1.N)

All six publishable Rust crates version together during alpha:

- `tasker-pgmq`, `tasker-shared`, `tasker-worker` (FFI-facing)
- `tasker-orchestration`, `tasker-client`, `tasker-cli` (server/client-facing)

Post-alpha, orchestration/client/cli may diverge to independent versioning.

#### FFI Language Bindings (0.1.N.P)

Two-tier version format where:

- `N` = tracks core Rust version compatibility
- `P` = language-specific patch level

**Examples:**

- Core at 0.1.8, Ruby binding with 2 Ruby-side patches: `0.1.8.2`
- Core bumps to 0.1.9, Ruby binding resets: `0.1.9.0`
- Python-only fix: `0.1.8.0` -> `0.1.8.1` (core still at 0.1.8)

**Benefits:**

- Clear compatibility signal (0.1.8.x works with core 0.1.8)
- Independent patching capability
- Prevents unnecessary republishing of unchanged bindings

#### Version Files to Update

A release touches these files:

| File | Field | Example |
|------|-------|---------|
| `VERSION` (new, root) | Central source of truth | `0.1.8` |
| `Cargo.toml` (root) | `version` | `"0.1.8"` |
| `tasker-pgmq/Cargo.toml` | `version` | `"0.1.8"` |
| `tasker-shared/Cargo.toml` | `version` | `"0.1.8"` |
| `tasker-client/Cargo.toml` | `version` | `"0.1.8"` |
| `tasker-cli/Cargo.toml` | `version` | `"0.1.8"` |
| `tasker-orchestration/Cargo.toml` | `version` | `"0.1.8"` |
| `tasker-worker/Cargo.toml` | `version` | `"0.1.8"` |
| `workers/ruby/lib/tasker_core/version.rb` | `VERSION`, `RUST_CORE_VERSION` | `'0.1.8.2'`, `'0.1.8'` |
| `workers/ruby/ext/tasker_core/Cargo.toml` | `version` | `"0.1.8"` |
| `workers/python/pyproject.toml` | `version` | `"0.1.8.1"` |
| `workers/python/Cargo.toml` | `version` | `"0.1.8"` |
| `workers/typescript/package.json` | `version` | `"0.1.8.0"` |
| `workers/typescript/Cargo.toml` | `version` | `"0.1.8"` |

Note: Python exposes version at runtime via `env!("CARGO_PKG_VERSION")` in `src/lib.rs` -- this is automatically correct when the Cargo.toml version is updated.

### Prerequisites for Publishing

**Inter-crate dependency version fields:** Currently, workspace dependencies use path-only references (e.g., `tasker-pgmq = { path = "tasker-pgmq" }` in workspace deps). For `cargo publish`, each path dependency must also include a version field:

```toml
# Before (current state - won't publish)
tasker-pgmq = { path = "tasker-pgmq" }

# After (publishable)
tasker-pgmq = { path = "tasker-pgmq", version = "=0.1.8" }
```

The version update script must update both the `[package] version` and all inter-crate dependency version fields.

### Git Tagging Convention

**Release tags (human-initiated):**

- Format: `release-YYYYMMDD-HHMM` or `vX.Y.Z`
- Triggers full release process via CI

**Package-specific tags (created by CI after successful publish):**

- `core-vX.Y.Z` -- Core Rust crates published
- `ruby-vX.Y.Z.P` -- Ruby gem published
- `python-vX.Y.Z.P` -- Python package published
- `typescript-vX.Y.Z.P` -- TypeScript package published

### Change Detection Logic

CI determines what to publish by comparing HEAD to last release tag:

```
FFI-facing core changed if any of:
  - tasker-pgmq/**
  - tasker-shared/**
  - tasker-worker/**
  -> Publish ALL core crates + ALL FFI bindings (reset binding patches to .0)

Server/client core changed if any of:
  - tasker-orchestration/**
  - tasker-client/**
  - tasker-cli/**
  (and FFI-facing core did NOT change)
  -> Publish core crates only (no FFI rebuild needed)

Ruby binding changed if:
  - workers/ruby/** (and FFI-facing core did NOT change)
  -> Publish Ruby gem only (increment .P)

Python binding changed if:
  - workers/python/** (and FFI-facing core did NOT change)
  -> Publish Python package only (increment .P)

TypeScript binding changed if:
  - workers/typescript/** (and FFI-facing core did NOT change)
  -> Publish TypeScript package only (increment .P)
```

### Publish Order (Dependency-Respecting)

```
Phase 1: tasker-pgmq              (no internal deps)
Phase 2: tasker-shared             (depends on pgmq)
Phase 3: tasker-client             (depends on shared)
         tasker-orchestration      (depends on pgmq, shared)
         [Phase 3 crates can publish in parallel]
Phase 4: tasker-worker             (depends on pgmq, client, shared)
         tasker-cli                (depends on client, shared)
         [Phase 4 crates can publish in parallel]
Phase 5: FFI bindings              (depend on shared + worker)
         Ruby:       bundle exec rake compile && gem build && gem push
         Python:     uv run maturin build --release && uv run maturin publish
         TypeScript: cargo build -p tasker-worker-ts --release && bun run build && npm publish
         [All FFI bindings can publish in parallel]
```

### Pre-Publish Validation

Two safety checks run before any publish attempt:

#### 1. Credential Verification

Before doing any work, verify all required tokens are present for the packages about to be published. Fail fast with a clear listing of what's missing.

```
Required credentials by target:
  crates.io:  CARGO_REGISTRY_TOKEN
  RubyGems:   GEM_HOST_API_KEY
  PyPI:       MATURIN_PYPI_TOKEN (or PYPI_TOKEN)
  npm:        NPM_TOKEN
```

The check only validates credentials for packages that will actually be published in this release (e.g., if only Ruby changed, only `GEM_HOST_API_KEY` is required).

#### 2. Already-Published Detection (Idempotent Publishing)

Before each package publish, query the target registry to check if the exact version already exists. This handles the partial-failure retry case: some packages published successfully, then CI died, and re-running should skip the successful ones and continue.

**Registry queries:**

| Registry | Check Command |
|----------|--------------|
| crates.io | `cargo search <crate> --limit 1` or `https://crates.io/api/v1/crates/<crate>/versions` |
| RubyGems | `gem search <gem> --exact --versions` or `https://rubygems.org/api/v1/versions/<gem>.json` |
| PyPI | `https://pypi.org/pypi/<package>/json` |
| npm | `npm view <package> versions --json` |

**Behavior controlled by `--on-duplicate` flag:**

| Mode | Behavior | Use Case |
|------|----------|----------|
| `skip` | Log and continue to next package | CI retries after partial failure |
| `warn` (default) | Log warning and continue | Normal operation -- safe default |
| `fail` | Exit with error immediately | Strict mode for catching version calculation bugs |

**Example output:**

```
  [warn] tasker-pgmq@0.1.9 already exists on crates.io, skipping
  [warn] tasker-shared@0.1.9 already exists on crates.io, skipping
  Publishing tasker-client@0.1.9...
  Publishing tasker-orchestration@0.1.9...
```

This ensures that re-running a failed release is always safe and picks up where it left off.

---

## Implementation Plan

### Phase 1: Foundation -- Dry-Run Tooling

**Goal:** Scripts that detect changes, calculate versions, update files, and report what *would* be published -- all verifiable locally without touching any registry or CI pipeline.

#### Deliverables

1. **`VERSION` file** -- Central version source of truth (initial: `0.1.0`)
2. **`scripts/release/detect-changes.sh`** -- Identify what changed since last release tag
3. **`scripts/release/calculate-versions.sh`** -- Determine next version numbers for each component
4. **`scripts/release/update-versions.sh`** -- Update all version files across workspace
5. **`scripts/release/release.sh`** -- Single-command orchestrator with `--dry-run` mode
6. **`scripts/release/lib/`** -- Shared functions (version parsing, git helpers, logging)

#### `VERSION` File

```
0.1.0
```

Simple single-line file at repo root. This is the source of truth for the core version.

#### `scripts/release/detect-changes.sh`

```bash
#!/usr/bin/env bash
# Detect what changed since last release
# Usage: ./scripts/release/detect-changes.sh [--from TAG]
# Output: KEY=VALUE pairs for each component

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

LAST_TAG=${FROM:-$(git describe --tags --match 'release-*' --abbrev=0 HEAD 2>/dev/null || \
                   git describe --tags --match 'v*' --abbrev=0 HEAD 2>/dev/null || \
                   git rev-list --max-parents=0 HEAD)}

log_info "Comparing HEAD to $LAST_TAG"

CHANGED_FILES=$(git diff "$LAST_TAG" HEAD --name-only)

# FFI-facing core (triggers FFI rebuilds)
if echo "$CHANGED_FILES" | grep -qE '^(tasker-pgmq|tasker-shared|tasker-worker)/'; then
    echo "FFI_CORE_CHANGED=true"
else
    echo "FFI_CORE_CHANGED=false"
fi

# Server/client core (no FFI rebuild needed)
if echo "$CHANGED_FILES" | grep -qE '^(tasker-orchestration|tasker-client|tasker-cli)/'; then
    echo "SERVER_CORE_CHANGED=true"
else
    echo "SERVER_CORE_CHANGED=false"
fi

# Any core change = all Rust crates publish
if [[ "$FFI_CORE_CHANGED" == "true" || "$SERVER_CORE_CHANGED" == "true" ]]; then
    echo "CORE_CHANGED=true"
else
    echo "CORE_CHANGED=false"
fi

# Language bindings (only relevant if FFI core did NOT change)
for lang in ruby python typescript; do
    if echo "$CHANGED_FILES" | grep -qE "^workers/$lang/"; then
        echo "${lang^^}_CHANGED=true"
    else
        echo "${lang^^}_CHANGED=false"
    fi
done
```

#### `scripts/release/calculate-versions.sh`

```bash
#!/usr/bin/env bash
# Calculate next version numbers
# Usage: ./scripts/release/calculate-versions.sh
# Reads: VERSION file, git tags, detect-changes.sh output
# Output: KEY=VALUE pairs with next versions

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

CURRENT_CORE=$(cat VERSION)

# Source change detection
eval "$(./scripts/release/detect-changes.sh)"

if [[ "$CORE_CHANGED" == "true" ]]; then
    # Bump core patch: 0.1.8 -> 0.1.9
    NEXT_CORE=$(bump_patch "$CURRENT_CORE")
else
    NEXT_CORE="$CURRENT_CORE"
fi
echo "NEXT_CORE_VERSION=$NEXT_CORE"

# Calculate FFI binding versions
for lang in ruby python typescript; do
    LANG_UPPER="${lang^^}"
    LANG_CHANGED_VAR="${LANG_UPPER}_CHANGED"

    if [[ "$FFI_CORE_CHANGED" == "true" ]]; then
        # Core changed: reset binding patch to .0
        echo "NEXT_${LANG_UPPER}_VERSION=${NEXT_CORE}.0"
    elif [[ "${!LANG_CHANGED_VAR}" == "true" ]]; then
        # Binding-only change: increment .P
        LAST_TAG=$(git tag -l "${lang}-v*" --sort=-version:refname | head -n1)
        if [[ -n "$LAST_TAG" && "$LAST_TAG" =~ ${lang}-v([0-9]+\.[0-9]+\.[0-9]+)\.([0-9]+) ]]; then
            PATCH=$(( ${BASH_REMATCH[2]} + 1 ))
            echo "NEXT_${LANG_UPPER}_VERSION=${CURRENT_CORE}.${PATCH}"
        else
            echo "NEXT_${LANG_UPPER}_VERSION=${CURRENT_CORE}.0"
        fi
    else
        echo "NEXT_${LANG_UPPER}_VERSION=unchanged"
    fi
done
```

#### `scripts/release/update-versions.sh`

```bash
#!/usr/bin/env bash
# Update all version files across the workspace
# Usage: ./scripts/release/update-versions.sh --core 0.1.9 [--ruby 0.1.9.0] \
#        [--python 0.1.9.0] [--typescript 0.1.9.0] [--dry-run]

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

# Parse arguments
CORE_VERSION=""
RUBY_VERSION=""
PYTHON_VERSION=""
TS_VERSION=""
DRY_RUN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --core) CORE_VERSION="$2"; shift 2 ;;
        --ruby) RUBY_VERSION="$2"; shift 2 ;;
        --python) PYTHON_VERSION="$2"; shift 2 ;;
        --typescript) TS_VERSION="$2"; shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        *) die "Unknown argument: $1" ;;
    esac
done

[[ -z "$CORE_VERSION" ]] && die \
    "Usage: $0 --core VERSION [--ruby VER] [--python VER] [--typescript VER] [--dry-run]"

# --- Core Rust crates ---
update_version_file "$CORE_VERSION"
update_cargo_version "Cargo.toml" "$CORE_VERSION"

for crate in tasker-pgmq tasker-shared tasker-client tasker-cli tasker-orchestration tasker-worker; do
    update_cargo_version "$crate/Cargo.toml" "$CORE_VERSION"
done

# Update inter-crate dependency versions in workspace deps
update_workspace_dep_versions "$CORE_VERSION"

# --- FFI Rust crates (not published to crates.io, but version should track) ---
for ffi_crate in workers/ruby/ext/tasker_core workers/python workers/typescript; do
    update_cargo_version "$ffi_crate/Cargo.toml" "$CORE_VERSION"
done

# --- Ruby ---
if [[ -n "$RUBY_VERSION" ]]; then
    update_ruby_version "$RUBY_VERSION" "$CORE_VERSION"
fi

# --- Python ---
if [[ -n "$PYTHON_VERSION" ]]; then
    update_python_version "$PYTHON_VERSION"
fi

# --- TypeScript ---
if [[ -n "$TS_VERSION" ]]; then
    update_typescript_version "$TS_VERSION"
fi
```

#### `scripts/release/release.sh`

```bash
#!/usr/bin/env bash
# Tasker release orchestrator
# Usage: ./scripts/release/release.sh [--dry-run]

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

DRY_RUN=false
[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=true

log_header "Tasker Release Manager"

# --- Pre-flight checks ---
log_section "Pre-flight checks"

if ! git diff-index --quiet HEAD --; then
    die "Uncommitted changes detected. Commit or stash first."
fi

BRANCH=$(git branch --show-current)
if [[ "$BRANCH" != "main" && "$DRY_RUN" == "false" ]]; then
    log_warn "On branch '$BRANCH', not 'main'"
    confirm "Continue anyway?"
fi

# --- Change detection ---
log_section "Detecting changes"
eval "$(./scripts/release/detect-changes.sh)"

# --- Version calculation ---
log_section "Calculating versions"
eval "$(./scripts/release/calculate-versions.sh)"

# --- Summary ---
log_section "Release Summary"
echo ""
echo "  Core version:       $(cat VERSION) -> ${NEXT_CORE_VERSION}"
echo ""
echo "  Rust crates to publish:"
if [[ "$CORE_CHANGED" == "true" ]]; then
    echo "    Phase 1: tasker-pgmq"
    echo "    Phase 2: tasker-shared"
    echo "    Phase 3: tasker-client, tasker-orchestration"
    echo "    Phase 4: tasker-worker, tasker-cli"
else
    echo "    (none - no core changes detected)"
fi
echo ""
echo "  FFI bindings to publish:"
for lang in ruby python typescript; do
    LANG_UPPER="${lang^^}"
    VERSION_VAR="NEXT_${LANG_UPPER}_VERSION"
    if [[ "${!VERSION_VAR}" != "unchanged" ]]; then
        echo "    $lang: ${!VERSION_VAR}"
    else
        echo "    $lang: (unchanged)"
    fi
done

if [[ "$DRY_RUN" == "true" ]]; then
    echo ""
    log_info "DRY RUN -- no files modified, no tags created"

    # Show what files would change
    log_section "Files that would be modified"
    UPDATE_ARGS="--core $NEXT_CORE_VERSION --dry-run"
    [[ "${NEXT_RUBY_VERSION:-unchanged}" != "unchanged" ]] && \
        UPDATE_ARGS+=" --ruby $NEXT_RUBY_VERSION"
    [[ "${NEXT_PYTHON_VERSION:-unchanged}" != "unchanged" ]] && \
        UPDATE_ARGS+=" --python $NEXT_PYTHON_VERSION"
    [[ "${NEXT_TYPESCRIPT_VERSION:-unchanged}" != "unchanged" ]] && \
        UPDATE_ARGS+=" --typescript $NEXT_TYPESCRIPT_VERSION"

    ./scripts/release/update-versions.sh $UPDATE_ARGS
    exit 0
fi

# --- Apply changes ---
log_section "Updating version files"
UPDATE_ARGS="--core $NEXT_CORE_VERSION"
[[ "${NEXT_RUBY_VERSION:-unchanged}" != "unchanged" ]] && \
    UPDATE_ARGS+=" --ruby $NEXT_RUBY_VERSION"
[[ "${NEXT_PYTHON_VERSION:-unchanged}" != "unchanged" ]] && \
    UPDATE_ARGS+=" --python $NEXT_PYTHON_VERSION"
[[ "${NEXT_TYPESCRIPT_VERSION:-unchanged}" != "unchanged" ]] && \
    UPDATE_ARGS+=" --typescript $NEXT_TYPESCRIPT_VERSION"

./scripts/release/update-versions.sh $UPDATE_ARGS

# --- Tag creation ---
TAG="release-$(date +%Y%m%d-%H%M)"
git tag "$TAG"

log_section "Ready"
echo "  Tag created: $TAG"
echo ""
echo "  To trigger CI release:"
echo "    git push origin $TAG"
echo ""
echo "  To abort:"
echo "    git tag -d $TAG"
```

#### `scripts/release/lib/common.sh` (shared functions)

```bash
#!/usr/bin/env bash
# Shared release tooling functions

set -euo pipefail

# --- Logging ---
log_info()    { echo "  [info] $*"; }
log_warn()    { echo "  [warn] $*" >&2; }
log_error()   { echo "  [error] $*" >&2; }
log_header()  { echo ""; echo "== $* =="; echo ""; }
log_section() { echo ""; echo "-- $* --"; }

die() { log_error "$*"; exit 1; }

confirm() {
    read -p "  $1 (y/N) " -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]] || exit 1
}

# --- Version arithmetic ---
bump_patch() {
    local version="$1"
    local major minor patch
    IFS='.' read -r major minor patch <<< "$version"
    echo "${major}.${minor}.$((patch + 1))"
}

# --- File updates ---
update_version_file() {
    local version="$1"
    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update VERSION -> $version"
    else
        echo "$version" > VERSION
        log_info "Updated VERSION -> $version"
    fi
}

update_cargo_version() {
    local file="$1" version="$2"

    if [[ ! -f "$file" ]]; then
        log_warn "File not found: $file"
        return
    fi

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update $file version -> $version"
    else
        sed -i "s/^version = \".*\"/version = \"$version\"/" "$file"
        log_info "Updated $file -> $version"
    fi
}

update_workspace_dep_versions() {
    local version="$1"
    local root_toml="Cargo.toml"

    # Update workspace dependency declarations that include version fields
    # e.g., tasker-pgmq = { path = "tasker-pgmq", version = "=0.1.0" }
    #
    # Also handles individual crate Cargo.toml files that reference
    # other workspace crates with version constraints
    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update inter-crate dependency versions -> =$version"
    else
        # Implementation: find and update version fields in workspace dep declarations
        log_info "Updated inter-crate dependency versions -> =$version"
    fi
}

update_ruby_version() {
    local binding_version="$1" core_version="$2"
    local file="workers/ruby/lib/tasker_core/version.rb"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update $file -> VERSION=$binding_version, RUST_CORE_VERSION=$core_version"
    else
        sed -i "s/VERSION = '.*'/VERSION = '$binding_version'/" "$file"
        sed -i "s/Version = '.*'/Version = '$binding_version'/" "$file"
        sed -i "s/RUST_CORE_VERSION = '.*'/RUST_CORE_VERSION = '$core_version'/" "$file"
        log_info "Updated Ruby version -> $binding_version (core: $core_version)"
    fi
}

update_python_version() {
    local version="$1"
    local file="workers/python/pyproject.toml"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update $file -> $version"
    else
        sed -i "s/^version = \".*\"/version = \"$version\"/" "$file"
        log_info "Updated Python version -> $version"
    fi
}

update_typescript_version() {
    local version="$1"
    local file="workers/typescript/package.json"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update $file version -> $version"
    else
        # Use a targeted replacement to avoid touching dependency versions
        sed -i '0,/"version": ".*"/{s/"version": ".*"/"version": "'"$version"'"/}' "$file"
        log_info "Updated TypeScript version -> $version"
    fi
}

# --- Credential verification ---
require_env() {
    local var_name="$1" purpose="$2"
    if [[ -z "${!var_name:-}" ]]; then
        die "Missing $var_name (required for $purpose)"
    fi
    log_info "Verified $var_name is set ($purpose)"
}

# --- Registry duplicate detection ---
crate_exists_on_registry() {
    local crate="$1" version="$2"
    local url="https://crates.io/api/v1/crates/${crate}/${version}"
    curl -sf "$url" > /dev/null 2>&1
}

gem_exists_on_registry() {
    local gem="$1" version="$2"
    local url="https://rubygems.org/api/v1/versions/${gem}.json"
    curl -sf "$url" 2>/dev/null | grep -q "\"number\":\"${version}\""
}

pypi_exists_on_registry() {
    local package="$1" version="$2"
    local url="https://pypi.org/pypi/${package}/${version}/json"
    curl -sf "$url" > /dev/null 2>&1
}

npm_exists_on_registry() {
    local package="$1" version="$2"
    npm view "${package}@${version}" version > /dev/null 2>&1
}

handle_duplicate() {
    local mode="$1" package="$2" version="$3" registry="$4"
    case "$mode" in
        skip)
            log_info "$package@$version already on $registry, skipping"
            ;;
        warn)
            log_warn "$package@$version already on $registry, skipping"
            ;;
        fail)
            die "$package@$version already exists on $registry (--on-duplicate=fail)"
            ;;
        *)
            die "Unknown --on-duplicate mode: $mode (expected skip|warn|fail)"
            ;;
    esac
}
```

#### Acceptance Criteria (Phase 1)

- [ ] `VERSION` file exists at repo root with `0.1.0`
- [ ] `./scripts/release/detect-changes.sh` correctly identifies modified components against any git ref
- [ ] `./scripts/release/calculate-versions.sh` produces correct core and binding version numbers
- [ ] `./scripts/release/update-versions.sh --dry-run` shows what would change without modifying files
- [ ] `./scripts/release/update-versions.sh` correctly updates all version files listed above
- [ ] `./scripts/release/release.sh --dry-run` produces an accurate summary of what would be published
- [ ] All scripts are idempotent (safe to re-run)
- [ ] Scripts handle edge cases: no prior tags, first release, binding-only changes

---

### Phase 2: Rust Publishing

#### Deliverables

1. **`scripts/release/publish-crates.sh`** -- Publish Rust crates to crates.io in dependency order
2. **Inter-crate version field fix** -- Add `version = "=X.Y.Z"` to all path dependencies
3. **Core tagging** -- Create `core-vX.Y.Z` tags after successful publish

#### Publishing Prerequisites

Before first publish, all inter-crate `path` dependencies need version fields:

```toml
# Root Cargo.toml [workspace.dependencies]
tasker-pgmq = { path = "tasker-pgmq", version = "=0.1.0" }

# Individual crate Cargo.toml files inherit from workspace
# or use direct path + version references:
tasker-shared = { path = "../tasker-shared", version = "=0.1.0" }
```

Using exact version pins (`=0.1.0`) during alpha to prevent accidental resolution mismatches.

#### `scripts/release/publish-crates.sh`

```bash
#!/usr/bin/env bash
# Publish Rust crates to crates.io in dependency order
# Usage: ./scripts/release/publish-crates.sh [--dry-run] [--on-duplicate=skip|warn|fail]

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

DRY_RUN=false
ON_DUPLICATE="warn"

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --on-duplicate=*) ON_DUPLICATE="${arg#*=}" ;;
    esac
done

VERSION=$(cat VERSION)

# Pre-flight: verify credentials
if [[ "$DRY_RUN" != "true" ]]; then
    require_env "CARGO_REGISTRY_TOKEN" "crates.io publishing"
fi

CRATE_PHASES=(
    "tasker-pgmq"
    "tasker-shared"
    "tasker-client tasker-orchestration"
    "tasker-worker tasker-cli"
)

for phase in "${CRATE_PHASES[@]}"; do
    for crate in $phase; do
        if [[ "$DRY_RUN" == "true" ]]; then
            echo "  [dry-run] Would publish $crate@$VERSION"
            cargo publish -p "$crate" --dry-run
            continue
        fi

        # Check if already published
        if crate_exists_on_registry "$crate" "$VERSION"; then
            handle_duplicate "$ON_DUPLICATE" "$crate" "$VERSION" "crates.io"
            continue
        fi

        echo "  Publishing $crate@$VERSION..."
        cargo publish -p "$crate" --token "$CARGO_REGISTRY_TOKEN"
        sleep 15  # Wait for crates.io index propagation
    done
done
```

#### Acceptance Criteria

- [ ] `cargo publish --dry-run` succeeds for all crates in correct order
- [ ] Inter-crate version fields are present and updated by version scripts
- [ ] `--dry-run` mode validates publishability without uploading
- [ ] Missing `CARGO_REGISTRY_TOKEN` fails fast with clear message before any work
- [ ] Already-published versions are detected and handled per `--on-duplicate` mode

---

### Phase 3: FFI Bindings Publishing

#### `scripts/release/publish-ruby.sh`

```bash
#!/usr/bin/env bash
# Build and publish Ruby gem
# Usage: ./scripts/release/publish-ruby.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

VERSION="$1"; shift
DRY_RUN=false
ON_DUPLICATE="warn"

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --on-duplicate=*) ON_DUPLICATE="${arg#*=}" ;;
    esac
done

if [[ "$DRY_RUN" != "true" ]]; then
    require_env "GEM_HOST_API_KEY" "RubyGems publishing"
fi

cd workers/ruby

# Compile native extension
bundle exec rake compile

# Build gem
gem build tasker-worker-rb.gemspec

if [[ "$DRY_RUN" == "true" ]]; then
    echo "  [dry-run] Would publish tasker-worker-rb-${VERSION}.gem"
    echo "  [dry-run] Gem contents:"
    gem specification "tasker-worker-rb-${VERSION}.gem" | head -20
else
    if gem_exists_on_registry "tasker-worker-rb" "$VERSION"; then
        handle_duplicate "$ON_DUPLICATE" "tasker-worker-rb" "$VERSION" "RubyGems"
    else
        gem push "tasker-worker-rb-${VERSION}.gem"
    fi
fi
```

#### `scripts/release/publish-python.sh`

```bash
#!/usr/bin/env bash
# Build and publish Python package
# Usage: ./scripts/release/publish-python.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

VERSION="$1"; shift
DRY_RUN=false
ON_DUPLICATE="warn"

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --on-duplicate=*) ON_DUPLICATE="${arg#*=}" ;;
    esac
done

PYPI_PACKAGE="tasker-worker-py"  # or tasker-core-py until renamed

if [[ "$DRY_RUN" != "true" ]]; then
    require_env "MATURIN_PYPI_TOKEN" "PyPI publishing"
fi

cd workers/python

# Build wheel with maturin via uv
uv run maturin build --release

if [[ "$DRY_RUN" == "true" ]]; then
    echo "  [dry-run] Would publish ${PYPI_PACKAGE}==${VERSION}"
    echo "  [dry-run] Built wheel:"
    ls -la ../../target/wheels/tasker_*.whl
else
    if pypi_exists_on_registry "$PYPI_PACKAGE" "$VERSION"; then
        handle_duplicate "$ON_DUPLICATE" "$PYPI_PACKAGE" "$VERSION" "PyPI"
    else
        uv run maturin publish
    fi
fi
```

#### `scripts/release/publish-typescript.sh`

```bash
#!/usr/bin/env bash
# Build and publish TypeScript package
# Usage: ./scripts/release/publish-typescript.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]

set -euo pipefail
source "$(dirname "$0")/lib/common.sh"

VERSION="$1"; shift
DRY_RUN=false
ON_DUPLICATE="warn"

NPM_PACKAGE="@tasker-systems/worker"

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --on-duplicate=*) ON_DUPLICATE="${arg#*=}" ;;
    esac
done

if [[ "$DRY_RUN" != "true" ]]; then
    require_env "NPM_TOKEN" "npm publishing"
fi

cd workers/typescript

# Build Rust FFI library
cargo build -p tasker-worker-ts --release

# Build TypeScript
bun run build

if [[ "$DRY_RUN" == "true" ]]; then
    echo "  [dry-run] Would publish ${NPM_PACKAGE}@${VERSION}"
    npm pack --dry-run
else
    if npm_exists_on_registry "$NPM_PACKAGE" "$VERSION"; then
        handle_duplicate "$ON_DUPLICATE" "$NPM_PACKAGE" "$VERSION" "npm"
    else
        npm publish --access public
    fi
fi
```

#### Acceptance Criteria

- [ ] Ruby gem builds with native extension and `--dry-run` reports correctly
- [ ] Python wheel builds via maturin and `--dry-run` reports correctly
- [ ] TypeScript package builds (Rust cdylib + tsup) and `--dry-run` reports correctly
- [ ] Version calculation handles core-triggered resets vs binding-only patches
- [ ] Each publish script validates its required credential before building
- [ ] Already-published versions detected for each registry (RubyGems, PyPI, npm)
- [ ] Re-running after partial failure skips already-published packages and continues

---

### Phase 4: CI Integration & Guardrails

#### `.github/workflows/release.yml`

```yaml
name: Release
on:
  push:
    tags:
      - 'release-*'
      - 'v*'

jobs:
  detect-changes:
    runs-on: ubuntu-latest
    outputs:
      core_changed: ${{ steps.changes.outputs.CORE_CHANGED }}
      ffi_core_changed: ${{ steps.changes.outputs.FFI_CORE_CHANGED }}
      ruby_changed: ${{ steps.changes.outputs.RUBY_CHANGED }}
      python_changed: ${{ steps.changes.outputs.PYTHON_CHANGED }}
      typescript_changed: ${{ steps.changes.outputs.TYPESCRIPT_CHANGED }}
      core_version: ${{ steps.versions.outputs.NEXT_CORE_VERSION }}
      ruby_version: ${{ steps.versions.outputs.NEXT_RUBY_VERSION }}
      python_version: ${{ steps.versions.outputs.NEXT_PYTHON_VERSION }}
      typescript_version: ${{ steps.versions.outputs.NEXT_TYPESCRIPT_VERSION }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Detect changes
        id: changes
        run: ./scripts/release/detect-changes.sh >> $GITHUB_OUTPUT
      - name: Calculate versions
        id: versions
        run: ./scripts/release/calculate-versions.sh >> $GITHUB_OUTPUT

  pre-flight:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: cargo check + clippy + fmt
        run: |
          cargo fmt --all -- --check
          cargo clippy --all-features --all-targets -- -D warnings
      - name: Tests
        run: cargo test --features test-messaging --lib

  publish-core:
    needs: [detect-changes, pre-flight]
    if: needs.detect-changes.outputs.core_changed == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Update versions
        run: |
          ./scripts/release/update-versions.sh \
            --core ${{ needs.detect-changes.outputs.core_version }}
      - name: Publish crates
        run: ./scripts/release/publish-crates.sh
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
      - name: Tag core release
        run: |
          git tag "core-v${{ needs.detect-changes.outputs.core_version }}"
          git push origin "core-v${{ needs.detect-changes.outputs.core_version }}"

  publish-ruby:
    needs: [detect-changes, pre-flight, publish-core]
    if: |
      always() && !cancelled() && !failure() &&
      needs.detect-changes.outputs.ruby_version != 'unchanged'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: ruby/setup-ruby@v1
        with:
          ruby-version: '3.4'
      - name: Build and publish
        run: |
          ./scripts/release/publish-ruby.sh \
            ${{ needs.detect-changes.outputs.ruby_version }}
        env:
          GEM_HOST_API_KEY: ${{ secrets.RUBYGEMS_API_KEY }}
      - name: Tag ruby release
        run: |
          git tag "ruby-v${{ needs.detect-changes.outputs.ruby_version }}"
          git push origin "ruby-v${{ needs.detect-changes.outputs.ruby_version }}"

  # Similar jobs for publish-python and publish-typescript...

  notify-completion:
    needs: [detect-changes, publish-core, publish-ruby]
    if: always()
    runs-on: ubuntu-latest
    steps:
      - name: Summary
        run: |
          echo "Release complete"
          echo "  Core: ${{ needs.publish-core.result }}"
          echo "  Ruby: ${{ needs.publish-ruby.result }}"
```

#### Acceptance Criteria

- [ ] Pre-flight checks prevent publishing if tests/lints fail
- [ ] CI workflow triggers on release tags
- [ ] Package-specific tags created after successful publishes
- [ ] Failure in one binding doesn't block others
- [ ] RELEASING.md and VERSIONING.md documented

---

## Decision Points

### Open: Python Package Name

Currently `tasker-core-py` in pyproject.toml. Options:

1. **Rename to `tasker-worker-py`** -- consistent with Ruby (`tasker-worker-rb`) and future `tasker-client-py`
2. **Keep `tasker-core-py`** -- if the Python package will always bundle more than just worker functionality

**Recommendation:** Rename to `tasker-worker-py` since nothing is published yet and it creates a clean namespace pattern.

### Open: workers/rust Naming

Currently `tasker-worker-rust`. Consider renaming to `tasker-worker-rs` for consistency with `tasker-worker-rb`, `tasker-worker-py`, `tasker-worker-ts`. This is cosmetic and low priority since the crate isn't published.

---

## Success Metrics

**For Alpha (next 6 months):**

- Weekly releases happen consistently
- Zero manual version editing required
- No version drift incidents across packages
- Release process takes <5 minutes of human time
- `--dry-run` provides accurate preview every time

**For Production (later):**

- LTS versions maintained
- Security patches backported within 24 hours
- Community can predict release schedule
- Clear upgrade paths documented

---

## Dependencies

**Secrets (Phase 4 only):**

- `CARGO_REGISTRY_TOKEN` -- crates.io API token
- `RUBYGEMS_API_KEY` -- RubyGems API key
- `PYPI_TOKEN` -- PyPI API token
- `NPM_TOKEN` -- npm publish token

**Prerequisites:**

- VERSION file in repo (Phase 1)
- Inter-crate version fields in Cargo.toml files (Phase 2)
- Package registry accounts created (Phase 4)

---

## Dry-Run Child Ticket Scope

For the initial implementation branch (`claude/tas-170-dry-run-*`), scope is limited to:

- **In scope:** Phase 1 deliverables + `--dry-run` paths from Phases 2-3
- **Out of scope:** Actual publishing, CI wiring, registry credentials, GitHub Actions workflow

The dry-run should be fully verifiable by running `./scripts/release/release.sh --dry-run` from any branch and seeing an accurate summary of what would happen.
