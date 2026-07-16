#!/bin/bash
# Quick benchmark script for Opus Rust
# Usage: ./bench_quick.sh [label]

set -e

LABEL=${1:-"baseline"}
DATE=$(date +%Y-%m-%d_%H-%M-%S)
RESULT_FILE="bench_results_${LABEL}_${DATE}.txt"

echo "=== Opus Rust Quick Benchmark ==="
echo "Label: $LABEL"
echo "Date: $DATE"
echo ""

# Build release
echo "Building release..."
cargo build --release --lib 2>&1 | grep -E "(Compiling|Finished|error)" || true
echo ""

# Run specific benchmarks
echo "Running benchmarks..."
echo "Results will be saved to: $RESULT_FILE"
echo ""

cargo bench -- opus_vs_c_real 2>&1 | tee "$RESULT_FILE" | grep -E "(opus_vs_c_real|time:.*ms)"

echo ""
echo "=== Summary ==="
grep -E "time:.*ms" "$RESULT_FILE" | head -10

echo ""
echo "Full results saved to: $RESULT_FILE"
