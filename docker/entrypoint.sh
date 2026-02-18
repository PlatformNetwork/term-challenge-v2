#!/bin/bash
set -e

# Mini-Chain Validator Entrypoint

echo "=== Mini-Chain Validator ==="
echo "Version: ${VERSION:-unknown}"
echo "P2P Port: ${P2P_PORT:-9000}"
echo "RPC Port: ${RPC_PORT:-8080}"
echo ""

# Check for required environment variables
if [ -z "$VALIDATOR_SECRET_KEY" ]; then
    echo "ERROR: VALIDATOR_SECRET_KEY is required"
    exit 1
fi

# Build arguments
ARGS="--secret-key $VALIDATOR_SECRET_KEY"

# Optional arguments
if [ -n "$P2P_PORT" ]; then
    ARGS="$ARGS --p2p-port $P2P_PORT"
fi

if [ -n "$RPC_PORT" ]; then
    ARGS="$ARGS --rpc-port $RPC_PORT"
fi

if [ -n "$SUBTENSOR_ENDPOINT" ]; then
    ARGS="$ARGS --subtensor-endpoint $SUBTENSOR_ENDPOINT"
fi

if [ -n "$BOOTSTRAP_PEERS" ]; then
    for peer in $BOOTSTRAP_PEERS; do
        ARGS="$ARGS --bootstrap-peer $peer"
    done
fi

if [ -n "$DATA_DIR" ]; then
    ARGS="$ARGS --data-dir $DATA_DIR"
fi

# Execute validator
exec /app/validator-node $ARGS "$@"
