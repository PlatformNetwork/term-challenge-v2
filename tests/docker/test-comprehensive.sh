#!/bin/bash
# =============================================================================
# Term Challenge Comprehensive Docker Integration Test
# =============================================================================
# Spins up a 3-instance challenge-server network via Docker Compose and runs
# 44 tests covering:
#
#   1.  Server health & startup
#   2.  Challenge configuration
#   3.  Validation API
#   4.  Evaluation API & scoring
#   5.  Custom challenge routes (leaderboard, stats, decay, agent)
#   6.  Leaderboard & scoring consistency
#   7.  Multi-instance consistency
#   8.  Fault tolerance (stop/restart)
#   9.  Resource & stability checks
#   10. Edge cases & error handling
#
# Usage:
#   bash tests/docker/test-comprehensive.sh
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../../scripts/test-harness.sh
source "${SCRIPT_DIR}/../../scripts/test-harness.sh"

tc_test_init

ARTIFACT_DIR="${TC_TEST_ARTIFACTS_DIR}/comprehensive"
LOG_DIR="${ARTIFACT_DIR}/logs"
mkdir -p "${ARTIFACT_DIR}" "${LOG_DIR}"

COMPOSE_FILE="${TC_TEST_COMPOSE_FILE}"

PASSED=0
FAILED=0
SKIPPED=0
TOTAL=0

CHALLENGE_ID="a1b2c3d4-e5f6-7890-abcd-ef1234567890"
SERVER_PORTS=(8081 8082 8083)
CONTAINER_NAMES=("tc-challenge-server-1" "tc-challenge-server-2" "tc-challenge-server-3")

run_test() {
    local name="$1"
    shift
    TOTAL=$((TOTAL + 1))
    log_info "TEST ${TOTAL}: ${name}"
    if "$@"; then
        log_success "${name}"
    else
        log_failure "${name}"
    fi
}

cleanup_compose() {
    log_info "Collecting final compose logs..."
    if tc_has_compose; then
        tc_compose -f "${COMPOSE_FILE}" logs --no-color > "${LOG_DIR}/compose-final.log" 2>&1 || true
        for i in 1 2 3; do
            tc_compose -f "${COMPOSE_FILE}" logs --no-color "challenge-server-${i}" > "${LOG_DIR}/challenge-server-${i}.log" 2>&1 || true
        done
        tc_compose -f "${COMPOSE_FILE}" down -v > "${LOG_DIR}/compose-down.log" 2>&1 || true
    fi
    tc_cleanup_run_dir
}

trap cleanup_compose EXIT

if ! tc_should_run_docker; then
    log_skip "Docker not available; skipping comprehensive docker test"
    exit 0
fi

tc_require_compose

# =============================================================================
# Phase 0: Build and start Docker stack
# =============================================================================

tc_ensure_network

log_info "Building docker images (this may take a while)..."
tc_compose -f "${COMPOSE_FILE}" build > "${LOG_DIR}/compose-build.log" 2>&1

log_info "Starting 3-server compose stack..."
tc_compose -f "${COMPOSE_FILE}" up -d > "${LOG_DIR}/compose-up.log" 2>&1

# =============================================================================
# Wait for all services
# =============================================================================

wait_for_health() {
    local url="$1"
    local timeout_seconds="$2"
    local start
    start=$(date +%s)

    while true; do
        if curl -fsS "${url}" > /dev/null 2>&1; then
            return 0
        fi

        local now
        now=$(date +%s)
        if [ $((now - start)) -ge "${timeout_seconds}" ]; then
            return 1
        fi

        sleep 3
    done
}

log_info "Waiting for all 3 challenge servers to become healthy..."
for port in "${SERVER_PORTS[@]}"; do
    if ! wait_for_health "http://localhost:${port}/health" 180; then
        log_failure "Challenge server on port ${port} did not become healthy"
        tc_compose -f "${COMPOSE_FILE}" logs --no-color > "${LOG_DIR}/compose-startup-fail.log" 2>&1 || true
        exit 1
    fi
    log_info "Challenge server on port ${port} is healthy"
done

sleep 3

log_info "Collecting initial compose logs..."
tc_compose -f "${COMPOSE_FILE}" logs --no-color > "${LOG_DIR}/compose.log" 2>&1

# =============================================================================
# Helper functions
# =============================================================================

curl_json() {
    curl -fsS -H "Content-Type: application/json" "$@" 2>/dev/null
}

curl_json_quiet() {
    curl -s -H "Content-Type: application/json" "$@" 2>/dev/null
}

# =============================================================================
# TEST SUITE 1: Server Health & Startup (4 tests)
# =============================================================================

test_all_servers_started() {
    for container in "${CONTAINER_NAMES[@]}"; do
        local running
        running=$(docker inspect --format '{{.State.Running}}' "${container}" 2>/dev/null || echo "false")
        if [ "${running}" != "true" ]; then
            log_info "Container ${container} is not running"
            return 1
        fi
    done
    log_info "All 3 challenge server containers running"
    return 0
}

test_health_endpoint_responds() {
    for port in "${SERVER_PORTS[@]}"; do
        local response
        response=$(curl_json "http://localhost:${port}/health")
        if [ -z "${response}" ]; then
            log_info "No response from port ${port}"
            return 1
        fi
        local healthy
        healthy=$(echo "${response}" | jq -r '.healthy' 2>/dev/null)
        if [ "${healthy}" != "true" ]; then
            log_info "Server on port ${port} reports unhealthy: ${response}"
            return 1
        fi
    done
    log_info "All servers respond healthy on /health"
    return 0
}

test_server_version_matches() {
    local expected_version="4.0.0"
    for port in "${SERVER_PORTS[@]}"; do
        local version
        version=$(curl_json "http://localhost:${port}/health" | jq -r '.version' 2>/dev/null)
        if [ "${version}" != "${expected_version}" ]; then
            log_info "Server on port ${port} has version ${version}, expected ${expected_version}"
            return 1
        fi
    done
    log_info "All servers report version ${expected_version}"
    return 0
}

test_challenge_id_consistent() {
    for port in "${SERVER_PORTS[@]}"; do
        local cid
        cid=$(curl_json "http://localhost:${port}/health" | jq -r '.challenge_id' 2>/dev/null)
        if [ "${cid}" != "${CHALLENGE_ID}" ]; then
            log_info "Server on port ${port} has challenge_id ${cid}, expected ${CHALLENGE_ID}"
            return 1
        fi
    done
    log_info "All servers report challenge_id=${CHALLENGE_ID}"
    return 0
}

run_test "All 3 challenge servers started successfully" test_all_servers_started
run_test "Health endpoint responds on all instances" test_health_endpoint_responds
run_test "Server version matches expected (4.0.0)" test_server_version_matches
run_test "Challenge ID is consistent across instances" test_challenge_id_consistent

# =============================================================================
# TEST SUITE 2: Health Response Schema (4 tests)
# =============================================================================

test_health_has_load_field() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/health")
    local load
    load=$(echo "${response}" | jq '.load' 2>/dev/null)
    if [ "${load}" = "null" ] || [ -z "${load}" ]; then
        log_info "Health response missing 'load' field"
        return 1
    fi
    log_info "Health load field present: ${load}"
    return 0
}

test_health_has_pending_field() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/health")
    local pending
    pending=$(echo "${response}" | jq '.pending' 2>/dev/null)
    if [ "${pending}" = "null" ] || [ -z "${pending}" ]; then
        log_info "Health response missing 'pending' field"
        return 1
    fi
    log_info "Health pending field present: ${pending}"
    return 0
}

test_health_has_uptime_field() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/health")
    local uptime
    uptime=$(echo "${response}" | jq '.uptime_secs' 2>/dev/null)
    if [ "${uptime}" = "null" ] || [ -z "${uptime}" ]; then
        log_info "Health response missing 'uptime_secs' field"
        return 1
    fi
    if [ "${uptime}" -lt 0 ] 2>/dev/null; then
        log_info "Uptime is negative: ${uptime}"
        return 1
    fi
    log_info "Health uptime_secs field present: ${uptime}s"
    return 0
}

test_health_uptime_increases() {
    local uptime1
    uptime1=$(curl_json "http://localhost:${SERVER_PORTS[0]}/health" | jq '.uptime_secs' 2>/dev/null)
    sleep 2
    local uptime2
    uptime2=$(curl_json "http://localhost:${SERVER_PORTS[0]}/health" | jq '.uptime_secs' 2>/dev/null)
    if [ "${uptime2}" -ge "${uptime1}" ] 2>/dev/null; then
        log_info "Uptime increased: ${uptime1}s -> ${uptime2}s"
        return 0
    fi
    log_info "Uptime did not increase: ${uptime1}s -> ${uptime2}s"
    return 1
}

run_test "Health response includes load field" test_health_has_load_field
run_test "Health response includes pending field" test_health_has_pending_field
run_test "Health response includes uptime_secs field" test_health_has_uptime_field
run_test "Server uptime increases over time" test_health_uptime_increases

# =============================================================================
# TEST SUITE 3: Validation API (5 tests)
# =============================================================================

test_validate_valid_submission() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "val-test-1",
            "submission_id": "sub-val-1",
            "participant_id": "miner-val-1",
            "data": {
                "agent_hash": "abc123",
                "miner_hotkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "epoch": 1,
                "task_results": [
                    {"task_id": "task-1", "passed": true, "score": 0.9, "execution_time_ms": 1000, "test_output": "ok", "agent_output": "done", "error": null}
                ]
            },
            "metadata": null,
            "epoch": 1,
            "deadline": null
        }')
    local success
    success=$(echo "${response}" | jq -r '.success' 2>/dev/null)
    if [ "${success}" = "true" ]; then
        log_info "Valid submission accepted with success=true"
        return 0
    fi
    log_info "Valid submission rejected: ${response}"
    return 1
}

test_validate_empty_task_results() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "val-test-2",
            "submission_id": "sub-val-2",
            "participant_id": "miner-val-2",
            "data": {
                "agent_hash": "abc123",
                "miner_hotkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "epoch": 1,
                "task_results": []
            },
            "metadata": null,
            "epoch": 1,
            "deadline": null
        }')
    local success
    success=$(echo "${response}" | jq -r '.success' 2>/dev/null)
    if [ "${success}" = "false" ]; then
        log_info "Empty task results correctly rejected"
        return 0
    fi
    log_info "Empty task results not rejected: ${response}"
    return 1
}

test_validate_invalid_json() {
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -H "Content-Type: application/json" \
        -d 'not valid json' 2>/dev/null)
    if [ "${http_code}" -ge 400 ]; then
        log_info "Invalid JSON correctly rejected with HTTP ${http_code}"
        return 0
    fi
    log_info "Invalid JSON returned HTTP ${http_code} (expected >= 400)"
    return 1
}

test_validate_missing_required_fields() {
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -H "Content-Type: application/json" \
        -d '{"request_id": "test"}' 2>/dev/null)
    if [ "${http_code}" -ge 400 ]; then
        log_info "Missing fields correctly rejected with HTTP ${http_code}"
        return 0
    fi
    log_info "Missing fields returned HTTP ${http_code} (expected >= 400)"
    return 1
}

test_validate_invalid_score_range() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "val-test-score",
            "submission_id": "sub-score",
            "participant_id": "miner-score",
            "data": {
                "agent_hash": "abc123",
                "miner_hotkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "epoch": 1,
                "task_results": [
                    {"task_id": "task-1", "passed": true, "score": 2.5, "execution_time_ms": 1000, "test_output": "ok", "agent_output": "done", "error": null}
                ]
            },
            "metadata": null,
            "epoch": 1,
            "deadline": null
        }')
    local success
    success=$(echo "${response}" | jq -r '.success' 2>/dev/null)
    if [ "${success}" = "false" ]; then
        log_info "Invalid score range correctly rejected"
        return 0
    fi
    log_info "Invalid score range not rejected: ${response}"
    return 1
}

run_test "Valid submission passes evaluation" test_validate_valid_submission
run_test "Empty task results returns error" test_validate_empty_task_results
run_test "Invalid JSON is rejected" test_validate_invalid_json
run_test "Missing required fields rejected" test_validate_missing_required_fields
run_test "Invalid score range (>1.0) rejected" test_validate_invalid_score_range

# =============================================================================
# TEST SUITE 4: Evaluation API & Scoring (5 tests)
# =============================================================================

test_eval_all_tasks_passed() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "eval-all-pass",
            "submission_id": "sub-all-pass",
            "participant_id": "miner-all-pass",
            "data": {
                "agent_hash": "hash-all-pass",
                "miner_hotkey": "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty",
                "epoch": 10,
                "task_results": [
                    {"task_id": "t1", "passed": true, "score": 1.0, "execution_time_ms": 100, "test_output": "", "agent_output": "", "error": null},
                    {"task_id": "t2", "passed": true, "score": 1.0, "execution_time_ms": 200, "test_output": "", "agent_output": "", "error": null},
                    {"task_id": "t3", "passed": true, "score": 1.0, "execution_time_ms": 150, "test_output": "", "agent_output": "", "error": null}
                ]
            },
            "metadata": null,
            "epoch": 10,
            "deadline": null
        }')
    local score
    score=$(echo "${response}" | jq '.score' 2>/dev/null)
    if [ -z "${score}" ] || [ "${score}" = "null" ]; then
        log_info "No score returned: ${response}"
        return 1
    fi
    local is_high
    is_high=$(echo "${score}" | awk '{print ($1 >= 0.9) ? "yes" : "no"}')
    if [ "${is_high}" = "yes" ]; then
        log_info "All tasks passed, score=${score} (>= 0.9)"
        return 0
    fi
    log_info "Score too low for all-pass: ${score}"
    return 1
}

test_eval_no_tasks_passed() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "eval-none-pass",
            "submission_id": "sub-none-pass",
            "participant_id": "miner-none-pass",
            "data": {
                "agent_hash": "hash-none-pass",
                "miner_hotkey": "5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy",
                "epoch": 10,
                "task_results": [
                    {"task_id": "t1", "passed": false, "score": 0.0, "execution_time_ms": 100, "test_output": "", "agent_output": "", "error": "timeout"},
                    {"task_id": "t2", "passed": false, "score": 0.0, "execution_time_ms": 200, "test_output": "", "agent_output": "", "error": "timeout"}
                ]
            },
            "metadata": null,
            "epoch": 10,
            "deadline": null
        }')
    local score
    score=$(echo "${response}" | jq '.score' 2>/dev/null)
    local is_zero
    is_zero=$(echo "${score}" | awk '{print ($1 <= 0.01) ? "yes" : "no"}')
    if [ "${is_zero}" = "yes" ]; then
        log_info "No tasks passed, score=${score} (<= 0.01)"
        return 0
    fi
    log_info "Expected near-zero score, got: ${score}"
    return 1
}

test_eval_mixed_results() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "eval-mixed",
            "submission_id": "sub-mixed",
            "participant_id": "miner-mixed",
            "data": {
                "agent_hash": "hash-mixed",
                "miner_hotkey": "5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw",
                "epoch": 10,
                "task_results": [
                    {"task_id": "t1", "passed": true, "score": 1.0, "execution_time_ms": 100, "test_output": "", "agent_output": "", "error": null},
                    {"task_id": "t2", "passed": false, "score": 0.0, "execution_time_ms": 200, "test_output": "", "agent_output": "", "error": null}
                ]
            },
            "metadata": null,
            "epoch": 10,
            "deadline": null
        }')
    local score
    score=$(echo "${response}" | jq '.score' 2>/dev/null)
    local is_mid
    is_mid=$(echo "${score}" | awk '{print ($1 > 0.1 && $1 < 0.9) ? "yes" : "no"}')
    if [ "${is_mid}" = "yes" ]; then
        log_info "Mixed results, proportional score=${score}"
        return 0
    fi
    log_info "Expected mid-range score, got: ${score}"
    return 1
}

test_eval_returns_execution_time() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "eval-time",
            "submission_id": "sub-time",
            "participant_id": "miner-time",
            "data": {
                "agent_hash": "hash-time",
                "miner_hotkey": "5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y",
                "epoch": 10,
                "task_results": [
                    {"task_id": "t1", "passed": true, "score": 0.8, "execution_time_ms": 500, "test_output": "", "agent_output": "", "error": null}
                ]
            },
            "metadata": null,
            "epoch": 10,
            "deadline": null
        }')
    local exec_time
    exec_time=$(echo "${response}" | jq '.execution_time_ms' 2>/dev/null)
    if [ "${exec_time}" != "null" ] && [ -n "${exec_time}" ]; then
        log_info "Evaluation returned execution_time_ms=${exec_time}"
        return 0
    fi
    log_info "No execution_time_ms in response: ${response}"
    return 1
}

test_eval_returns_request_id() {
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d '{
            "request_id": "eval-reqid-check",
            "submission_id": "sub-reqid",
            "participant_id": "miner-reqid",
            "data": {
                "agent_hash": "hash-reqid",
                "miner_hotkey": "5GNJqTPyNqANBkUVMN1LPPrxXnFouWA2MRQg3gKrUYgw6J9i",
                "epoch": 10,
                "task_results": [
                    {"task_id": "t1", "passed": true, "score": 0.7, "execution_time_ms": 100, "test_output": "", "agent_output": "", "error": null}
                ]
            },
            "metadata": null,
            "epoch": 10,
            "deadline": null
        }')
    local request_id
    request_id=$(echo "${response}" | jq -r '.request_id' 2>/dev/null)
    if [ "${request_id}" = "eval-reqid-check" ]; then
        log_info "Response echoes back request_id correctly"
        return 0
    fi
    log_info "Expected request_id=eval-reqid-check, got: ${request_id}"
    return 1
}

run_test "All tasks passed returns high score (>= 0.9)" test_eval_all_tasks_passed
run_test "No tasks passed returns near-zero score" test_eval_no_tasks_passed
run_test "Mixed results return proportional score" test_eval_mixed_results
run_test "Evaluation returns execution_time_ms" test_eval_returns_execution_time
run_test "Evaluation echoes back request_id" test_eval_returns_request_id

# =============================================================================
# TEST SUITE 5: Custom Challenge Routes (5 tests)
# =============================================================================

test_leaderboard_returns_json_array() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/leaderboard")
    local is_array
    is_array=$(echo "${response}" | jq 'type == "array"' 2>/dev/null)
    if [ "${is_array}" = "true" ]; then
        local count
        count=$(echo "${response}" | jq 'length' 2>/dev/null)
        log_info "Leaderboard returns JSON array with ${count} entries"
        return 0
    fi
    log_info "Leaderboard did not return JSON array: ${response}"
    return 1
}

test_stats_returns_submission_counts() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/stats")
    local total
    total=$(echo "${response}" | jq '.total_submissions' 2>/dev/null)
    local miners
    miners=$(echo "${response}" | jq '.active_miners' 2>/dev/null)
    if [ "${total}" != "null" ] && [ "${miners}" != "null" ]; then
        log_info "Stats: total_submissions=${total}, active_miners=${miners}"
        return 0
    fi
    log_info "Stats missing expected fields: ${response}"
    return 1
}

test_decay_returns_state() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/decay")
    if [ -n "${response}" ]; then
        local has_decay
        has_decay=$(echo "${response}" | jq 'has("decay_active")' 2>/dev/null)
        if [ "${has_decay}" = "true" ]; then
            log_info "Decay endpoint returns valid state"
            return 0
        fi
    fi
    log_info "Decay response unexpected: ${response}"
    return 1
}

test_agent_unknown_hotkey_returns_404() {
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:${SERVER_PORTS[0]}/agent/unknown-hotkey-xyz/score" 2>/dev/null)
    if [ "${http_code}" = "404" ]; then
        log_info "Unknown hotkey correctly returns 404"
        return 0
    fi
    log_info "Unknown hotkey returned HTTP ${http_code} (expected 404)"
    return 1
}

test_agent_submissions_endpoint() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/agent/miner-all-pass/submissions")
    local submissions
    submissions=$(echo "${response}" | jq '.submissions' 2>/dev/null)
    if [ "${submissions}" != "null" ] && [ -n "${submissions}" ]; then
        log_info "Agent submissions endpoint returns count=${submissions}"
        return 0
    fi
    log_info "Agent submissions response: ${response}"
    return 1
}

run_test "GET /leaderboard returns valid JSON array" test_leaderboard_returns_json_array
run_test "GET /stats returns submission/miner counts" test_stats_returns_submission_counts
run_test "GET /decay returns decay state" test_decay_returns_state
run_test "GET /agent/:hotkey/score returns 404 for unknown" test_agent_unknown_hotkey_returns_404
run_test "GET /agent/:hotkey/submissions returns count" test_agent_submissions_endpoint

# =============================================================================
# TEST SUITE 6: Leaderboard & Scoring Consistency (4 tests)
# =============================================================================

test_leaderboard_has_entries_after_evaluations() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/leaderboard")
    local count
    count=$(echo "${response}" | jq 'length' 2>/dev/null)
    if [ "${count}" -gt 0 ] 2>/dev/null; then
        log_info "Leaderboard has ${count} entries after evaluations"
        return 0
    fi
    log_info "Leaderboard is empty after evaluations"
    return 1
}

test_leaderboard_entries_have_required_fields() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/leaderboard")
    local count
    count=$(echo "${response}" | jq 'length' 2>/dev/null)
    if [ "${count}" -lt 1 ] 2>/dev/null; then
        log_info "No leaderboard entries to check"
        return 1
    fi
    local has_rank has_hotkey has_score
    has_rank=$(echo "${response}" | jq '.[0] | has("rank")' 2>/dev/null)
    has_hotkey=$(echo "${response}" | jq '.[0] | has("hotkey")' 2>/dev/null)
    has_score=$(echo "${response}" | jq '.[0] | has("score")' 2>/dev/null)
    if [ "${has_rank}" = "true" ] && [ "${has_hotkey}" = "true" ] && [ "${has_score}" = "true" ]; then
        log_info "Leaderboard entries have rank, hotkey, score fields"
        return 0
    fi
    log_info "Leaderboard entry missing fields: rank=${has_rank}, hotkey=${has_hotkey}, score=${has_score}"
    return 1
}

test_leaderboard_sorted_by_score_descending() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/leaderboard")
    local count
    count=$(echo "${response}" | jq 'length' 2>/dev/null)
    if [ "${count}" -lt 2 ] 2>/dev/null; then
        log_info "Need at least 2 entries to check sort order (have ${count})"
        return 0
    fi
    local sorted
    sorted=$(echo "${response}" | jq '[.[].score] | . as $orig | sort | reverse | . == $orig' 2>/dev/null)
    if [ "${sorted}" = "true" ]; then
        log_info "Leaderboard is sorted by score descending"
        return 0
    fi
    log_info "Leaderboard is NOT sorted by score descending"
    return 1
}

test_leaderboard_ranks_sequential() {
    local response
    response=$(curl_json "http://localhost:${SERVER_PORTS[0]}/leaderboard")
    local count
    count=$(echo "${response}" | jq 'length' 2>/dev/null)
    if [ "${count}" -lt 1 ] 2>/dev/null; then
        log_info "No entries to check ranks"
        return 1
    fi
    local first_rank
    first_rank=$(echo "${response}" | jq '.[0].rank' 2>/dev/null)
    if [ "${first_rank}" = "1" ]; then
        log_info "First leaderboard entry has rank=1"
        return 0
    fi
    log_info "First rank is ${first_rank}, expected 1"
    return 1
}

run_test "Leaderboard has entries after evaluations" test_leaderboard_has_entries_after_evaluations
run_test "Leaderboard entries have required fields" test_leaderboard_entries_have_required_fields
run_test "Leaderboard sorted by score descending" test_leaderboard_sorted_by_score_descending
run_test "Leaderboard ranks start at 1" test_leaderboard_ranks_sequential

# =============================================================================
# TEST SUITE 7: Multi-Instance Consistency (4 tests)
# =============================================================================

test_all_servers_same_version() {
    local versions=()
    for port in "${SERVER_PORTS[@]}"; do
        local version
        version=$(curl_json "http://localhost:${port}/health" | jq -r '.version' 2>/dev/null)
        versions+=("${version}")
    done
    local first="${versions[0]}"
    for v in "${versions[@]}"; do
        if [ "${v}" != "${first}" ]; then
            log_info "Version mismatch: ${versions[*]}"
            return 1
        fi
    done
    log_info "All servers report same version: ${first}"
    return 0
}

test_all_servers_same_challenge_id() {
    local ids=()
    for port in "${SERVER_PORTS[@]}"; do
        local cid
        cid=$(curl_json "http://localhost:${port}/health" | jq -r '.challenge_id' 2>/dev/null)
        ids+=("${cid}")
    done
    local first="${ids[0]}"
    for id in "${ids[@]}"; do
        if [ "${id}" != "${first}" ]; then
            log_info "Challenge ID mismatch: ${ids[*]}"
            return 1
        fi
    done
    log_info "All servers report same challenge_id: ${first}"
    return 0
}

test_all_servers_healthy_simultaneously() {
    local healthy_count=0
    for port in "${SERVER_PORTS[@]}"; do
        local healthy
        healthy=$(curl_json "http://localhost:${port}/health" | jq -r '.healthy' 2>/dev/null)
        if [ "${healthy}" = "true" ]; then
            healthy_count=$((healthy_count + 1))
        fi
    done
    if [ "${healthy_count}" -eq 3 ]; then
        log_info "All 3 servers simultaneously healthy"
        return 0
    fi
    log_info "Only ${healthy_count}/3 servers healthy"
    return 1
}

test_independent_evaluation_on_each_server() {
    local scores=()
    for port in "${SERVER_PORTS[@]}"; do
        local response
        response=$(curl_json_quiet -X POST "http://localhost:${port}/evaluate" \
            -d '{
                "request_id": "multi-eval-'"${port}"'",
                "submission_id": "sub-multi-'"${port}"'",
                "participant_id": "miner-multi-'"${port}"'",
                "data": {
                    "agent_hash": "hash-multi",
                    "miner_hotkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                    "epoch": 20,
                    "task_results": [
                        {"task_id": "t1", "passed": true, "score": 0.8, "execution_time_ms": 100, "test_output": "", "agent_output": "", "error": null}
                    ]
                },
                "metadata": null,
                "epoch": 20,
                "deadline": null
            }')
        local score
        score=$(echo "${response}" | jq '.score' 2>/dev/null)
        scores+=("${score}")
    done
    local first="${scores[0]}"
    for s in "${scores[@]}"; do
        if [ "${s}" != "${first}" ]; then
            log_info "Score mismatch across servers: ${scores[*]}"
            return 1
        fi
    done
    log_info "Same evaluation produces same score across all servers: ${first}"
    return 0
}

run_test "All servers report same version" test_all_servers_same_version
run_test "All servers report same challenge ID" test_all_servers_same_challenge_id
run_test "All 3 servers healthy simultaneously" test_all_servers_healthy_simultaneously
run_test "Independent evaluation produces same score" test_independent_evaluation_on_each_server

# =============================================================================
# TEST SUITE 8: Fault Tolerance (4 tests)
# =============================================================================

test_network_survives_single_server_stop() {
    docker stop tc-challenge-server-3 > /dev/null 2>&1

    sleep 3

    local healthy=0
    for port in 8081 8082; do
        local is_healthy
        is_healthy=$(curl_json "http://localhost:${port}/health" 2>/dev/null | jq -r '.healthy' 2>/dev/null || echo "false")
        if [ "${is_healthy}" = "true" ]; then
            healthy=$((healthy + 1))
        fi
    done

    docker start tc-challenge-server-3 > /dev/null 2>&1

    if [ "${healthy}" -ge 2 ]; then
        log_info "Network survived with ${healthy}/2 remaining servers healthy"
        return 0
    fi
    log_info "Only ${healthy}/2 servers healthy after stopping server-3"
    return 1
}

test_stopped_server_restarts_cleanly() {
    if ! wait_for_health "http://localhost:8083/health" 60; then
        log_info "Server-3 did not become healthy within 60s after restart"
        return 1
    fi
    log_info "Server-3 restarted and is healthy"
    return 0
}

test_restarted_server_serves_requests() {
    local response
    response=$(curl_json "http://localhost:8083/health")
    local healthy
    healthy=$(echo "${response}" | jq -r '.healthy' 2>/dev/null)
    if [ "${healthy}" = "true" ]; then
        local eval_response
        eval_response=$(curl_json_quiet -X POST "http://localhost:8083/evaluate" \
            -d '{
                "request_id": "restart-eval",
                "submission_id": "sub-restart",
                "participant_id": "miner-restart",
                "data": {
                    "agent_hash": "hash-restart",
                    "miner_hotkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                    "epoch": 30,
                    "task_results": [
                        {"task_id": "t1", "passed": true, "score": 0.5, "execution_time_ms": 100, "test_output": "", "agent_output": "", "error": null}
                    ]
                },
                "metadata": null,
                "epoch": 30,
                "deadline": null
            }')
        local success
        success=$(echo "${eval_response}" | jq -r '.success' 2>/dev/null)
        if [ "${success}" = "true" ]; then
            log_info "Restarted server-3 serves evaluation requests"
            return 0
        fi
    fi
    log_info "Restarted server-3 not serving requests properly"
    return 1
}

test_all_servers_recover_after_restart() {
    local all_healthy=true
    for port in "${SERVER_PORTS[@]}"; do
        local healthy
        healthy=$(curl_json "http://localhost:${port}/health" 2>/dev/null | jq -r '.healthy' 2>/dev/null || echo "false")
        if [ "${healthy}" != "true" ]; then
            all_healthy=false
        fi
    done
    if [ "${all_healthy}" = true ]; then
        log_info "All 3 servers recovered and healthy"
        return 0
    fi
    log_info "Not all servers recovered"
    return 1
}

run_test "Network survives single server stop" test_network_survives_single_server_stop
run_test "Stopped server restarts cleanly" test_stopped_server_restarts_cleanly
run_test "Restarted server serves requests" test_restarted_server_serves_requests
run_test "All servers recover after restart" test_all_servers_recover_after_restart

# =============================================================================
# TEST SUITE 9: Resource & Stability (4 tests)
# =============================================================================

test_server_memory_within_limits() {
    for container in "${CONTAINER_NAMES[@]}"; do
        local mem_usage
        mem_usage=$(docker stats --no-stream --format '{{.MemUsage}}' "${container}" 2>/dev/null | cut -d'/' -f1 | tr -d ' ')
        if [ -n "${mem_usage}" ]; then
            local mem_mb
            if echo "${mem_usage}" | grep -qi "gib"; then
                mem_mb=$(echo "${mem_usage}" | sed 's/[^0-9.]//g' | awk '{printf "%.0f", $1 * 1024}')
            else
                mem_mb=$(echo "${mem_usage}" | sed 's/[^0-9.]//g' | awk '{printf "%.0f", $1}')
            fi
            if [ -n "${mem_mb}" ] && [ "${mem_mb}" -gt 2048 ] 2>/dev/null; then
                log_info "${container} using excessive memory: ${mem_usage}"
                return 1
            fi
        fi
    done
    log_info "All server memory usage within 2GB limit"
    return 0
}

test_containers_not_oom_killed() {
    for container in "${CONTAINER_NAMES[@]}"; do
        local oom
        oom=$(docker inspect --format '{{.State.OOMKilled}}' "${container}" 2>/dev/null || echo "false")
        if [ "${oom}" = "true" ]; then
            log_info "${container} was OOM-killed!"
            return 1
        fi
    done
    log_info "No containers OOM-killed"
    return 0
}

test_no_panics_in_logs() {
    local log_file="${LOG_DIR}/compose.log"
    tc_compose -f "${COMPOSE_FILE}" logs --no-color > "${log_file}" 2>&1
    local panic_count
    panic_count=$(grep -c -E "panic|PANIC|thread .* panicked" "${log_file}" || true)
    if [ "${panic_count}" -gt 0 ]; then
        log_info "Found ${panic_count} panic(s) in logs!"
        grep -E "panic|PANIC|thread .* panicked" "${log_file}" | head -5 >> "${ARTIFACT_DIR}/panics.txt"
        return 1
    fi
    local fatal_count
    fatal_count=$(grep -c -E "FATAL|fatal error" "${log_file}" || true)
    if [ "${fatal_count}" -gt 0 ]; then
        log_info "Found ${fatal_count} fatal error(s) in logs!"
        return 1
    fi
    log_info "No panics or fatal errors in server logs"
    return 0
}

test_no_crash_loops() {
    for container in "${CONTAINER_NAMES[@]}"; do
        local restarts
        restarts=$(docker inspect --format '{{.RestartCount}}' "${container}" 2>/dev/null || echo "0")
        if [ "${restarts}" -gt 3 ]; then
            log_info "${container} restarted ${restarts} times (crash loop)"
            return 1
        fi
    done
    log_info "No containers in crash loops (all restarts <= 3)"
    return 0
}

run_test "Server memory usage within limits" test_server_memory_within_limits
run_test "No containers OOM-killed" test_containers_not_oom_killed
run_test "No panics or fatal errors in logs" test_no_panics_in_logs
run_test "No crash loops detected" test_no_crash_loops

# =============================================================================
# TEST SUITE 10: Edge Cases & Error Handling (5 tests)
# =============================================================================

test_unknown_route_returns_404() {
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:${SERVER_PORTS[0]}/nonexistent/route" 2>/dev/null)
    if [ "${http_code}" = "404" ]; then
        log_info "Unknown route correctly returns 404"
        return 0
    fi
    log_info "Unknown route returned HTTP ${http_code} (expected 404)"
    return 1
}

test_post_to_get_endpoint_fails() {
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "http://localhost:${SERVER_PORTS[0]}/leaderboard" \
        -H "Content-Type: application/json" -d '{}' 2>/dev/null)
    if [ "${http_code}" = "404" ] || [ "${http_code}" = "405" ]; then
        log_info "POST to GET-only endpoint correctly returns HTTP ${http_code}"
        return 0
    fi
    log_info "POST to GET endpoint returned HTTP ${http_code} (expected 404 or 405)"
    return 1
}

test_empty_body_to_evaluate() {
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -H "Content-Type: application/json" 2>/dev/null)
    if [ "${http_code}" -ge 400 ]; then
        log_info "Empty body to /evaluate correctly returns HTTP ${http_code}"
        return 0
    fi
    log_info "Empty body to /evaluate returned HTTP ${http_code} (expected >= 400)"
    return 1
}

test_large_number_of_task_results() {
    local tasks=""
    for i in $(seq 1 50); do
        if [ -n "${tasks}" ]; then tasks="${tasks},"; fi
        tasks="${tasks}{\"task_id\":\"task-${i}\",\"passed\":true,\"score\":0.8,\"execution_time_ms\":100,\"test_output\":\"\",\"agent_output\":\"\",\"error\":null}"
    done
    local response
    response=$(curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
        -d "{
            \"request_id\": \"eval-large\",
            \"submission_id\": \"sub-large\",
            \"participant_id\": \"miner-large\",
            \"data\": {
                \"agent_hash\": \"hash-large\",
                \"miner_hotkey\": \"5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY\",
                \"epoch\": 50,
                \"task_results\": [${tasks}]
            },
            \"metadata\": null,
            \"epoch\": 50,
            \"deadline\": null
        }")
    local success
    success=$(echo "${response}" | jq -r '.success' 2>/dev/null)
    if [ "${success}" = "true" ]; then
        local score
        score=$(echo "${response}" | jq '.score' 2>/dev/null)
        log_info "50-task evaluation succeeded with score=${score}"
        return 0
    fi
    log_info "50-task evaluation failed: ${response}"
    return 1
}

test_concurrent_evaluations() {
    local pids=()
    local results_dir
    results_dir=$(mktemp -d)

    for i in $(seq 1 5); do
        (
            curl_json_quiet -X POST "http://localhost:${SERVER_PORTS[0]}/evaluate" \
                -d "{
                    \"request_id\": \"concurrent-${i}\",
                    \"submission_id\": \"sub-concurrent-${i}\",
                    \"participant_id\": \"miner-concurrent-${i}\",
                    \"data\": {
                        \"agent_hash\": \"hash-concurrent-${i}\",
                        \"miner_hotkey\": \"5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY\",
                        \"epoch\": 60,
                        \"task_results\": [
                            {\"task_id\": \"t1\", \"passed\": true, \"score\": 0.7, \"execution_time_ms\": 100, \"test_output\": \"\", \"agent_output\": \"\", \"error\": null}
                        ]
                    },
                    \"metadata\": null,
                    \"epoch\": 60,
                    \"deadline\": null
                }" > "${results_dir}/result-${i}.json"
        ) &
        pids+=($!)
    done

    for pid in "${pids[@]}"; do
        wait "${pid}" 2>/dev/null || true
    done

    local success_count=0
    for i in $(seq 1 5); do
        local success
        success=$(jq -r '.success' "${results_dir}/result-${i}.json" 2>/dev/null)
        if [ "${success}" = "true" ]; then
            success_count=$((success_count + 1))
        fi
    done

    rm -rf "${results_dir}"

    if [ "${success_count}" -eq 5 ]; then
        log_info "All 5 concurrent evaluations succeeded"
        return 0
    fi
    log_info "Only ${success_count}/5 concurrent evaluations succeeded"
    return 1
}

run_test "Unknown route returns 404" test_unknown_route_returns_404
run_test "POST to GET-only endpoint fails appropriately" test_post_to_get_endpoint_fails
run_test "Empty body to /evaluate returns error" test_empty_body_to_evaluate
run_test "Large evaluation (50 tasks) succeeds" test_large_number_of_task_results
run_test "Concurrent evaluations all succeed" test_concurrent_evaluations

# =============================================================================
# Collect final logs per server
# =============================================================================

log_info "Collecting per-server logs..."
for i in 1 2 3; do
    tc_compose -f "${COMPOSE_FILE}" logs --no-color "challenge-server-${i}" > "${LOG_DIR}/challenge-server-${i}.log" 2>&1 || true
done

# =============================================================================
# Results summary
# =============================================================================

echo ""
echo "============================================================================="
echo "  TERM CHALLENGE INTEGRATION TEST RESULTS"
echo "============================================================================="
echo "  Total:   ${TOTAL}"
echo "  Passed:  ${PASSED}"
echo "  Failed:  ${FAILED}"
echo "  Skipped: ${SKIPPED}"
echo ""
echo "  Artifacts: ${ARTIFACT_DIR}"
echo "  Logs:      ${LOG_DIR}"
echo "============================================================================="
echo ""

if [ "${FAILED}" -gt 0 ]; then
    echo -e "${RED}[FAIL]${NC} Integration test completed with ${FAILED} failure(s)"
    exit 1
fi

echo -e "${GREEN}[PASS]${NC} All ${PASSED}/${TOTAL} integration tests passed (3-server network)"
