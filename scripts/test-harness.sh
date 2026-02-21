#!/bin/bash
# =============================================================================
# Term Challenge Test Harness Helpers
# =============================================================================
# Shared environment defaults and preflight checks for test entrypoints.
#
# Environment variables:
#   TC_TEST_ROOT             Repo root (auto-detected)
#   TC_TEST_ARTIFACTS_DIR    Base artifacts directory
#   TC_TEST_LOG_DIR          Log output directory
#   TC_TEST_TMP_BASE         Base temp directory
#   TC_TEST_RUN_DIR          Specific run directory
#   TC_TEST_COMPOSE_FILE     Docker compose file path
#   TC_TEST_COMPOSE_PROJECT  Compose project name
#   TC_TEST_NETWORK          Docker network name
#   TC_TEST_DOCKER_MODE      auto|skip|required
#   TC_TEST_PRESERVE_RUN_DIR true to skip cleanup
# =============================================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
    if [ -n "${PASSED+x}" ]; then
        PASSED=$((PASSED + 1))
    fi
}

log_failure() {
    echo -e "${RED}[FAIL]${NC} $1"
    if [ -n "${FAILED+x}" ]; then
        FAILED=$((FAILED + 1))
    fi
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
    if [ -n "${SKIPPED+x}" ]; then
        SKIPPED=$((SKIPPED + 1))
    fi
}

tc_test_init() {
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

    export TC_TEST_ROOT="${TC_TEST_ROOT:-$(cd "${script_dir}/.." && pwd)}"
    export TC_TEST_ARTIFACTS_DIR="${TC_TEST_ARTIFACTS_DIR:-${TC_TEST_ROOT}/artifacts/tests}"
    export TC_TEST_LOG_DIR="${TC_TEST_LOG_DIR:-${TC_TEST_ARTIFACTS_DIR}/logs}"
    export TC_TEST_TMP_BASE="${TC_TEST_TMP_BASE:-/tmp/tc-tests}"
    export TC_TEST_COMPOSE_FILE="${TC_TEST_COMPOSE_FILE:-${TC_TEST_ROOT}/tests/docker/docker-compose.test.yml}"
    export TC_TEST_COMPOSE_PROJECT="${TC_TEST_COMPOSE_PROJECT:-tc-test}"
    export TC_TEST_NETWORK="${TC_TEST_NETWORK:-tc-test}"
    export TC_TEST_DOCKER_MODE="${TC_TEST_DOCKER_MODE:-auto}"

    mkdir -p "${TC_TEST_ARTIFACTS_DIR}" "${TC_TEST_LOG_DIR}" "${TC_TEST_TMP_BASE}"

    if [ -z "${TC_TEST_RUN_DIR:-}" ]; then
        TC_TEST_RUN_DIR="$(mktemp -d "${TC_TEST_TMP_BASE}/run-XXXXXX")"
        export TC_TEST_RUN_DIR
    else
        mkdir -p "${TC_TEST_RUN_DIR}"
    fi

    if [ -z "${COMPOSE_PROJECT_NAME:-}" ]; then
        export COMPOSE_PROJECT_NAME="${TC_TEST_COMPOSE_PROJECT}"
    fi
}

tc_cleanup_run_dir() {
    if [ "${TC_TEST_PRESERVE_RUN_DIR:-false}" != "true" ] && [ -n "${TC_TEST_RUN_DIR:-}" ]; then
        rm -rf "${TC_TEST_RUN_DIR}" 2>/dev/null || true
    fi
}

tc_require_command() {
    local cmd="$1"
    if ! command -v "${cmd}" >/dev/null 2>&1; then
        log_failure "Required command not found: ${cmd}"
        return 1
    fi
}

tc_has_docker() {
    command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1
}

tc_has_compose() {
    if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
        return 0
    fi

    command -v docker-compose >/dev/null 2>&1
}

tc_compose() {
    if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
        docker compose "$@"
        return
    fi

    if command -v docker-compose >/dev/null 2>&1; then
        docker-compose "$@"
        return
    fi

    return 127
}

tc_should_run_docker() {
    case "${TC_TEST_DOCKER_MODE}" in
        skip)
            return 1
            ;;
        required)
            tc_require_docker
            ;;
        auto|*)
            tc_has_docker
            ;;
    esac
}

tc_require_docker() {
    if ! tc_has_docker; then
        log_failure "Docker daemon not available"
        return 1
    fi
}

tc_require_compose() {
    if ! tc_has_compose; then
        log_failure "Docker Compose not available"
        return 1
    fi
}

tc_ensure_network() {
    if ! tc_has_docker; then
        return 1
    fi

    if ! docker network inspect "${TC_TEST_NETWORK}" >/dev/null 2>&1; then
        log_info "Creating docker network ${TC_TEST_NETWORK}"
        docker network create "${TC_TEST_NETWORK}" >/dev/null
    fi
}
