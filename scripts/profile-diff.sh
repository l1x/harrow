#!/usr/bin/env bash
set -euo pipefail

# profile-diff.sh — Compare current flamegraphs against a baseline.
# Produces red/blue differential SVGs.
#
# Usage:
#   ./scripts/profile-diff.sh <baseline-dir> [current-dir]
#
# Example:
#   ./scripts/profile-diff.sh flamegraphs/baseline flamegraphs/
#
# Prerequisites:
#   cargo install inferno

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

BASELINE_DIR="${1:?Usage: profile-diff.sh <baseline-dir> [current-dir]}"
CURRENT_DIR="${2:-$ROOT_DIR/flamegraphs}"

WORKLOADS=("echo" "middleware_chain" "full_stack")

echo "=== Harrow flamegraph diff ==="
echo "Baseline: $BASELINE_DIR"
echo "Current:  $CURRENT_DIR"
echo ""

for workload in "${WORKLOADS[@]}"; do
    baseline="$BASELINE_DIR/${workload}.folded"
    current="$CURRENT_DIR/${workload}.folded"
    output="$CURRENT_DIR/diff-${workload}.svg"

    if [ ! -f "$baseline" ]; then
        echo "SKIP $workload — no baseline at $baseline"
        continue
    fi

    if [ ! -f "$current" ]; then
        echo "SKIP $workload — no current profile at $current"
        continue
    fi

    echo "--- Diffing: $workload ---"
    inferno-diff-folded "$baseline" "$current" \
        | inferno-flamegraph --title "diff: $workload" \
        > "$output"

    echo "  -> $output"
    echo ""
done

echo "=== Done ==="
