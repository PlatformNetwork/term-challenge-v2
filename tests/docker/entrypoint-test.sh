#!/bin/bash
# =============================================================================
# Term Challenge Test Server Entrypoint
# =============================================================================
# Handles environment variables and starts the challenge server.
# =============================================================================

set -euo pipefail

ARGS=""

if [ -n "${CHALLENGE_HOST:-}" ]; then
    ARGS="${ARGS} --host ${CHALLENGE_HOST}"
fi

if [ -n "${CHALLENGE_PORT:-}" ]; then
    ARGS="${ARGS} --port ${CHALLENGE_PORT}"
fi

if [ -n "${DATABASE_PATH:-}" ]; then
    ARGS="${ARGS} --db-path ${DATABASE_PATH}"
fi

if [ -n "${CHALLENGE_ID:-}" ]; then
    ARGS="${ARGS} --challenge-id ${CHALLENGE_ID}"
fi

echo "[entrypoint] Starting term-challenge-server with args:${ARGS}"
# shellcheck disable=SC2086
exec term-challenge-server ${ARGS}
