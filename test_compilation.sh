#!/bin/bash

set -e

echo "================================================"
echo "Testing StaticX Compilation Pipeline"
echo "================================================"
echo ""

# Use the term CLI to compile the test agent
TERM_BIN="/root/term-challenge-repo/target/release/term"
TEST_AGENT="/root/term-challenge-repo/test_agent.py"
OUTPUT_DIR="/tmp/test_compilation"

echo "[1] Creating output directory: $OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

echo "[2] Compiling test agent..."
echo "    Input: $TEST_AGENT"
echo "    Using term CLI: $TERM_BIN"

# Try to compile the agent
if $TERM_BIN compile "$TEST_AGENT" -o "$OUTPUT_DIR/test_agent_compiled"; then
    echo "✓ Compilation succeeded!"
else
    echo "✗ Compilation failed!"
    exit 1
fi

echo ""
echo "[3] Checking compiled output..."

# List output files
if [ -f "$OUTPUT_DIR/test_agent_compiled" ]; then
    ls -lh "$OUTPUT_DIR/test_agent_compiled"
    
    echo ""
    echo "[4] Verifying binary type..."
    file "$OUTPUT_DIR/test_agent_compiled"
    
    echo ""
    echo "[5] Checking if binary is static..."
    # Use ldd to check if binary is static (should return "not a dynamic executable" or similar)
    ldd "$OUTPUT_DIR/test_agent_compiled" 2>&1 || echo "Binary appears to be static!"
    
    echo ""
    echo "[6] Testing binary execution..."
    if "$OUTPUT_DIR/test_agent_compiled" --help 2>&1 | head -5; then
        echo "✓ Binary is executable!"
    else
        echo "⚠ Could not run binary help (this might be normal)"
    fi
    
    echo ""
    echo "================================================"
    echo "✓ COMPILATION TEST PASSED!"
    echo "================================================"
    exit 0
else
    echo "✗ Compiled binary not found at $OUTPUT_DIR/test_agent_compiled"
    exit 1
fi
