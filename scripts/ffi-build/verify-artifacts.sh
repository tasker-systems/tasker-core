#!/usr/bin/env bash
# scripts/ffi-build/verify-artifacts.sh
# Post-build verification of FFI artifacts.
#
# Usage:
#   ./scripts/ffi-build/verify-artifacts.sh --target TARGET_TRIPLE [--language LANG]
#
# Checks:
#   - Each artifact exists
#   - It's a shared library (via `file` command)
#   - Targets the correct architecture
#   - Meets minimum file size (FFI libs should be >1MB)
#   - Python wheel platform tags match target triple

set -euo pipefail

source "$(dirname "$0")/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
TARGET=""
LANGUAGE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)
            TARGET="$2"
            shift 2
            ;;
        --language)
            LANGUAGE="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

if [[ -z "$TARGET" ]]; then
    TARGET="$(detect_arch)"
fi

FAILURES=0
CHECKS=0

# ---------------------------------------------------------------------------
# Verification helpers
# ---------------------------------------------------------------------------

fail_check() {
    log_error "FAIL: $*"
    FAILURES=$((FAILURES + 1))
}

pass_check() {
    log_info "PASS: $*"
}

# Check that a file exists and is non-empty
check_exists() {
    local path="$1" description="$2"
    CHECKS=$((CHECKS + 1))
    if [[ -f "$path" ]]; then
        pass_check "${description} exists"
    else
        fail_check "${description} not found: ${path}"
        return 1
    fi
}

# Check minimum file size (default 1MB = 1048576 bytes)
check_min_size() {
    local path="$1" description="$2" min_bytes="${3:-1048576}"
    CHECKS=$((CHECKS + 1))
    if [[ ! -f "$path" ]]; then
        fail_check "${description} missing (cannot check size)"
        return 1
    fi

    local size
    if [[ "$(uname -s)" == "Darwin" ]]; then
        size=$(stat -f%z "$path")
    else
        size=$(stat -c%s "$path")
    fi

    if [[ "$size" -ge "$min_bytes" ]]; then
        pass_check "${description} size OK ($(( size / 1024 ))KB >= $(( min_bytes / 1024 ))KB)"
    else
        fail_check "${description} too small: ${size} bytes (min: ${min_bytes})"
    fi
}

# Check that `file` output indicates correct architecture
check_architecture() {
    local path="$1" target="$2" description="$3"
    CHECKS=$((CHECKS + 1))
    if [[ ! -f "$path" ]]; then
        fail_check "${description} missing (cannot check arch)"
        return 1
    fi

    local file_output
    file_output="$(file "$path")"

    case "$target" in
        x86_64-unknown-linux-gnu)
            if echo "$file_output" | grep -qi "x86.64\|x86-64\|AMD64"; then
                pass_check "${description} arch matches x86_64"
            else
                fail_check "${description} expected x86_64, got: ${file_output}"
            fi
            ;;
        aarch64-apple-darwin)
            if echo "$file_output" | grep -qi "arm64\|aarch64"; then
                pass_check "${description} arch matches arm64"
            else
                fail_check "${description} expected arm64, got: ${file_output}"
            fi
            ;;
        *)
            log_warn "Unknown target for arch verification: ${target}"
            ;;
    esac
}

# Check that `file` output indicates a shared library
check_shared_lib() {
    local path="$1" description="$2"
    CHECKS=$((CHECKS + 1))
    if [[ ! -f "$path" ]]; then
        fail_check "${description} missing (cannot check type)"
        return 1
    fi

    local file_output
    file_output="$(file "$path")"

    if echo "$file_output" | grep -qi "shared object\|dynamically linked\|Mach-O.*dynamically linked\|Mach-O.*bundle\|Mach-O.*dylib"; then
        pass_check "${description} is a shared library"
    else
        fail_check "${description} not a shared library: ${file_output}"
    fi
}

# ---------------------------------------------------------------------------
# Python verification
# ---------------------------------------------------------------------------
verify_python() {
    local dir
    dir="$(artifact_dir "$TARGET" "python")"
    log_section "Verifying Python artifacts in ${dir}"

    # Find wheel files
    local wheel_found=false
    for whl in "${dir}"/tasker_py-*.whl; do
        if [[ -f "$whl" ]]; then
            wheel_found=true
            local basename
            basename="$(basename "$whl")"

            check_exists "$whl" "Python wheel ${basename}"
            check_min_size "$whl" "Python wheel ${basename}" 1048576

            # Verify platform tag in wheel filename
            CHECKS=$((CHECKS + 1))
            case "$TARGET" in
                x86_64-unknown-linux-gnu)
                    if echo "$basename" | grep -qE "manylinux.*x86_64|linux_x86_64"; then
                        pass_check "Wheel platform tag matches x86_64 Linux"
                    else
                        fail_check "Wheel platform tag mismatch for x86_64 Linux: ${basename}"
                    fi
                    ;;
                aarch64-apple-darwin)
                    if echo "$basename" | grep -q "macosx.*arm64"; then
                        pass_check "Wheel platform tag matches macOS arm64"
                    else
                        fail_check "Wheel platform tag mismatch for macOS arm64: ${basename}"
                    fi
                    ;;
            esac
        fi
    done

    if [[ "$wheel_found" == "false" ]]; then
        CHECKS=$((CHECKS + 1))
        fail_check "No Python wheel files found in ${dir}"
    fi
}

# ---------------------------------------------------------------------------
# TypeScript verification
# ---------------------------------------------------------------------------
verify_typescript() {
    local dir ext lib_path
    dir="$(artifact_dir "$TARGET" "typescript")"
    log_section "Verifying TypeScript artifacts in ${dir}"

    # Determine expected extension
    case "$TARGET" in
        *-darwin) ext="dylib" ;;
        *)        ext="so" ;;
    esac

    lib_path="${dir}/libtasker_ts-${TARGET}.${ext}"

    check_exists "$lib_path" "TypeScript FFI library"
    check_min_size "$lib_path" "TypeScript FFI library" 1048576
    check_shared_lib "$lib_path" "TypeScript FFI library"
    check_architecture "$lib_path" "$TARGET" "TypeScript FFI library"
}

# ---------------------------------------------------------------------------
# Ruby verification
# ---------------------------------------------------------------------------
verify_ruby() {
    local dir ext lib_path
    dir="$(artifact_dir "$TARGET" "ruby")"
    log_section "Verifying Ruby artifacts in ${dir}"

    # Determine expected extension
    case "$TARGET" in
        *-darwin) ext="bundle" ;;
        *)        ext="so" ;;
    esac

    lib_path="${dir}/tasker_rb-${TARGET}.${ext}"

    check_exists "$lib_path" "Ruby FFI extension"
    check_min_size "$lib_path" "Ruby FFI extension" 1048576
    check_shared_lib "$lib_path" "Ruby FFI extension"
    check_architecture "$lib_path" "$TARGET" "Ruby FFI extension"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
log_header "Artifact Verification"
log_info "Target:    ${TARGET}"
log_info "Language:  ${LANGUAGE:-all}"
log_info "Artifacts: ${ARTIFACTS_DIR}"

if [[ -n "$LANGUAGE" ]]; then
    case "$LANGUAGE" in
        python)     verify_python ;;
        typescript) verify_typescript ;;
        ruby)       verify_ruby ;;
        *)          die "Unknown language: ${LANGUAGE}" ;;
    esac
else
    verify_python
    verify_typescript
    verify_ruby
fi

# Summary
log_section "Verification Summary"
log_info "Checks: ${CHECKS}, Failures: ${FAILURES}"

if [[ "$FAILURES" -gt 0 ]]; then
    die "${FAILURES} verification check(s) failed"
fi
log_info "All verification checks passed"
