#!/usr/bin/env bash
#
# compare-frameworks.sh — Harrow vs Axum load-test comparison
#
# Usage:
#   BENCH_BIN=/path/to/bench ./scripts/compare-frameworks.sh
#   ./scripts/compare-frameworks.sh --bench-bin /path/to/bench
#
# Requires: jq, curl

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

HARROW_PORT=3090
AXUM_PORT=3091
DURATION=10
WARMUP=3
CONCURRENCY_LEVELS=(1 4 8 16)
ENDPOINTS=("/" "/greet/bench" "/health")

OUTDIR="target/comparison"
REPORT="$OUTDIR/comparison-report.md"

# ---------------------------------------------------------------------------
# Locate bench binary
# ---------------------------------------------------------------------------

BENCH_BIN="${BENCH_BIN:-}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bench-bin)
            BENCH_BIN="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$BENCH_BIN" ]]; then
    # Auto-discover relative to repo root
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
    CANDIDATES=(
        "$REPO_ROOT/../mcp-servers/target/release/bench"
        "$REPO_ROOT/../mcp-load-tester/target/release/bench"
    )
    for candidate in "${CANDIDATES[@]}"; do
        if [[ -x "$candidate" ]]; then
            BENCH_BIN="$candidate"
            break
        fi
    done
fi

if [[ -z "$BENCH_BIN" || ! -x "$BENCH_BIN" ]]; then
    echo "Error: bench binary not found." >&2
    echo "Set BENCH_BIN env var or use --bench-bin /path/to/bench" >&2
    exit 1
fi

echo "Using bench binary: $BENCH_BIN"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

echo "Building both servers in release mode..."
cargo build --release --bin harrow-server --bin axum-server

HARROW_BIN="target/release/harrow-server"
AXUM_BIN="target/release/axum-server"

mkdir -p "$OUTDIR"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

wait_for_server() {
    local port="$1"
    local max_wait=5
    local i=0
    while ! curl -sf "http://127.0.0.1:$port/" >/dev/null 2>&1; do
        sleep 0.1
        i=$((i + 1))
        if [[ $i -ge $((max_wait * 10)) ]]; then
            echo "Error: server on port $port did not start within ${max_wait}s" >&2
            exit 1
        fi
    done
}

kill_server() {
    local pid="$1"
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi
}

run_bench() {
    local url="$1"
    local concurrency="$2"
    local output_file="$3"

    "$BENCH_BIN" -u "$url" -M -c "$concurrency" -d "$DURATION" -w "$WARMUP" -j -q > "$output_file"
}

# ---------------------------------------------------------------------------
# Run benchmarks
# ---------------------------------------------------------------------------

echo ""
echo "Starting framework comparison..."
echo "  Duration: ${DURATION}s per test, ${WARMUP}s warmup"
echo "  Concurrency levels: ${CONCURRENCY_LEVELS[*]}"
echo "  Endpoints: ${ENDPOINTS[*]}"
echo ""

for endpoint in "${ENDPOINTS[@]}"; do
    for concurrency in "${CONCURRENCY_LEVELS[@]}"; do
        # Sanitize endpoint for filename
        safe_endpoint=$(echo "$endpoint" | tr '/' '_' | sed 's/^_//')
        [[ -z "$safe_endpoint" ]] && safe_endpoint="root"

        echo "--- Endpoint: $endpoint, Concurrency: $concurrency ---"

        # Harrow
        echo "  Starting Harrow server..."
        $HARROW_BIN --port "$HARROW_PORT" &
        HARROW_PID=$!
        wait_for_server "$HARROW_PORT"

        harrow_file="$OUTDIR/harrow_${safe_endpoint}_c${concurrency}.json"
        echo "  Running bench against Harrow..."
        run_bench "http://127.0.0.1:$HARROW_PORT$endpoint" "$concurrency" "$harrow_file"

        kill_server "$HARROW_PID"
        sleep 0.5

        # Axum
        echo "  Starting Axum server..."
        $AXUM_BIN --port "$AXUM_PORT" &
        AXUM_PID=$!
        wait_for_server "$AXUM_PORT"

        axum_file="$OUTDIR/axum_${safe_endpoint}_c${concurrency}.json"
        echo "  Running bench against Axum..."
        run_bench "http://127.0.0.1:$AXUM_PORT$endpoint" "$concurrency" "$axum_file"

        kill_server "$AXUM_PID"
        sleep 0.5

        echo ""
    done
done

# ---------------------------------------------------------------------------
# Generate report
# ---------------------------------------------------------------------------

echo "Generating comparison report..."

cat > "$REPORT" <<'HEADER'
# Harrow vs Axum — Framework Comparison

**Generated:** $(date -u +"%Y-%m-%d %H:%M UTC")
**Duration:** DURATION seconds per test, WARMUP seconds warmup
**Tool:** mcp-load-tester bench (max-throughput mode)

---

HEADER

# Fix template values
sed -i '' "s/DURATION/$DURATION/g; s/WARMUP/$WARMUP/g" "$REPORT" 2>/dev/null || \
sed -i "s/DURATION/$DURATION/g; s/WARMUP/$WARMUP/g" "$REPORT"

# Replace date placeholder
GENERATED_DATE=$(date -u +"%Y-%m-%d %H:%M UTC")
sed -i '' "s/\$(date -u +\"%Y-%m-%d %H:%M UTC\")/$GENERATED_DATE/g" "$REPORT" 2>/dev/null || \
sed -i "s/\$(date -u +\"%Y-%m-%d %H:%M UTC\")/$GENERATED_DATE/g" "$REPORT"

for endpoint in "${ENDPOINTS[@]}"; do
    safe_endpoint=$(echo "$endpoint" | tr '/' '_' | sed 's/^_//')
    [[ -z "$safe_endpoint" ]] && safe_endpoint="root"

    echo "" >> "$REPORT"
    echo "## Endpoint: \`$endpoint\`" >> "$REPORT"
    echo "" >> "$REPORT"
    echo "| Concurrency | Framework | Req/s | p50 (ms) | p99 (ms) | p999 (ms) | Errors |" >> "$REPORT"
    echo "|-------------|-----------|-------|----------|----------|-----------|--------|" >> "$REPORT"

    for concurrency in "${CONCURRENCY_LEVELS[@]}"; do
        for framework in harrow axum; do
            json_file="$OUTDIR/${framework}_${safe_endpoint}_c${concurrency}.json"
            if [[ -f "$json_file" ]]; then
                rps=$(jq -r '.requests_per_second // .rps // "N/A"' "$json_file" 2>/dev/null || echo "N/A")
                p50=$(jq -r '.percentiles.p50 // .latency_percentiles.p50 // "N/A"' "$json_file" 2>/dev/null || echo "N/A")
                p99=$(jq -r '.percentiles.p99 // .latency_percentiles.p99 // "N/A"' "$json_file" 2>/dev/null || echo "N/A")
                p999=$(jq -r '.percentiles.p999 // .latency_percentiles.p999 // "N/A"' "$json_file" 2>/dev/null || echo "N/A")
                errors=$(jq -r '.errors // .error_count // 0' "$json_file" 2>/dev/null || echo "0")
                echo "| $concurrency | $framework | $rps | $p50 | $p99 | $p999 | $errors |" >> "$REPORT"
            else
                echo "| $concurrency | $framework | N/A | N/A | N/A | N/A | N/A |" >> "$REPORT"
            fi
        done
    done
done

echo "" >> "$REPORT"
echo "---" >> "$REPORT"
echo "" >> "$REPORT"
echo "*Raw JSON results are in \`target/comparison/\`.*" >> "$REPORT"

echo ""
echo "Done! Report written to: $REPORT"
echo "Raw JSON results in: $OUTDIR/"
