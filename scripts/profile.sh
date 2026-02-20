#!/usr/bin/env bash
set -euo pipefail

# profile.sh — Run all benchmark workloads under cargo-flamegraph.
# Outputs SVGs to flamegraphs/.
#
# Prerequisites:
#   cargo install flamegraph
#   On macOS: dtrace must be available (ships with Xcode CLI tools)
#   On Linux: perf must be available

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$ROOT_DIR/flamegraphs"

mkdir -p "$OUT_DIR"

WORKLOADS=("echo" "middleware_chain" "full_stack")

echo "=== Harrow profiling ==="
echo "Output: $OUT_DIR"
echo ""

for workload in "${WORKLOADS[@]}"; do
    echo "--- Profiling: $workload ---"
    cargo flamegraph \
        --bench "$workload" \
        --features profiling \
        -o "$OUT_DIR/${workload}.svg" \
        --root \
        -- --bench 2>&1 | tail -5

    # Also save the folded stacks for diffing
    if [ -f "$ROOT_DIR/perf.data" ]; then
        # Linux: perf script + stackcollapse
        perf script -i "$ROOT_DIR/perf.data" \
            | inferno-collapse-perf \
            > "$OUT_DIR/${workload}.folded" 2>/dev/null || true
    fi

    echo "  -> $OUT_DIR/${workload}.svg"
    echo ""
done

echo "=== Done. Flamegraphs written to $OUT_DIR ==="
