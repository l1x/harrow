# Performance Test Results

Instance: c8g.12xlarge
Server: 34.244.235.41 (private: 172.31.2.238:3090)
Client: 34.251.252.173
Duration: 20s | Warmup: 2s
Spinr mode: docker
OS monitors: true
Perf stat: server only
Date: 2026-03-18 19:54:26 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| text-c128 | harrow | /text | 128 | 562681.800 | 0.220 | 0.440 | 0.550 |
| text-c128 | axum | /text | 128 | 1061353.800 | 0.120 | 0.220 | 0.250 |
| json-1kb-c128 | harrow | /json/1kb | 128 | 485948.600 | 0.260 | 0.520 | 0.640 |
| json-1kb-c128 | axum | /json/1kb | 128 | 992461.050 | 0.120 | 0.240 | 0.290 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % |
|-----------|------------|----------|---------|
| text-c128 | 562681.800 | 1061353.800 | -46.98% |
| json-1kb-c128 | 485948.600 | 992461.050 | -51.04% |
