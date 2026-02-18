#!/bin/bash
# Test a single validator node

set -e

echo "=== Single Validator Test ==="

# Build
cargo build --release --bin validator-node

# Create data directory
mkdir -p data/test

# Run with debug logging
echo ""
echo "Starting validator..."
echo "Press Ctrl+C to stop"
echo ""

RUST_LOG=debug,platform_chain=debug ./target/release/validator-node \
    --data-dir ./data/test \
    --stake 100
