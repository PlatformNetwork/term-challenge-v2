#!/bin/bash
# =============================================================================
# Platform Standard Test Suite
# =============================================================================
# Entry point for local/unit test runs. Docker is not required.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./test-harness.sh
source "${SCRIPT_DIR}/test-harness.sh"

PASSED=0
FAILED=0
SKIPPED=0

platform_test_init
trap platform_cleanup_run_dir EXIT
log_info "Defaults: nightly toolchain uses -Z threads=0"
log_info "Defaults: fast linker flags opt-in via env"
log_info "Opt-out: PLATFORM_DISABLE_NIGHTLY=1"
log_info "Override: PLATFORM_RUST_NIGHTLY=1"
log_info "Opt-out: PLATFORM_DISABLE_FAST_LINKER=1"
log_info "Override: PLATFORM_FAST_LINKER_RUSTFLAGS/PLATFORM_FAST_LINKER_RUSTFLAGS_DARWIN"
log_info "Override: PLATFORM_LINKER_RUSTFLAGS/PLATFORM_LINKER_RUSTFLAGS_DARWIN"
if [ "${PLATFORM_DISABLE_NIGHTLY:-0}" = "1" ]; then
    export PLATFORM_NIGHTLY_RUSTFLAGS=""
    export RUSTUP_TOOLCHAIN=""
    log_info "Nightly Rust disabled via opt-out"
elif [ "${PLATFORM_RUST_NIGHTLY:-0}" = "1" ] || [ "${RUSTUP_TOOLCHAIN:-}" = "nightly" ]; then
    export RUSTUP_TOOLCHAIN="nightly"
    export PLATFORM_NIGHTLY_RUSTFLAGS="${PLATFORM_NIGHTLY_RUSTFLAGS:--Z threads=0}"
    log_info "Nightly Rust enabled (parallel rustc)"
else
    export PLATFORM_NIGHTLY_RUSTFLAGS=""
    log_info "Nightly Rust not requested; clearing nightly flags"
fi

if [ "${PLATFORM_DISABLE_FAST_LINKER:-0}" = "1" ]; then
    export PLATFORM_FAST_LINKER_RUSTFLAGS=""
    export PLATFORM_FAST_LINKER_RUSTFLAGS_DARWIN=""
    export PLATFORM_LINKER_RUSTFLAGS=""
    export PLATFORM_LINKER_RUSTFLAGS_DARWIN=""
    log_info "Fast linker disabled via opt-out"
fi
log_info "=== Platform Test Suite ==="
log_info "Artifacts: ${PLATFORM_TEST_ARTIFACTS_DIR}"
log_info "Run dir: ${PLATFORM_TEST_RUN_DIR}"

log_info "[1/2] Building workspace"
if cargo build --release 2>&1 | tee "${PLATFORM_TEST_LOG_DIR}/build.log"; then
    log_success "Build completed"
else
    log_failure "Build failed"
    exit 1
fi

log_info "[2/2] Running unit tests"
if cargo test --workspace --release 2>&1 | tee "${PLATFORM_TEST_LOG_DIR}/unit-tests.log"; then
    log_success "Unit tests completed"
else
    log_failure "Unit tests failed"
fi

log_info "Test summary"
log_info "Passed: ${PASSED}"
log_info "Failed: ${FAILED}"
log_info "Skipped: ${SKIPPED}"

if [ "${FAILED}" -ne 0 ]; then
    exit 1
fi