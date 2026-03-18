# Performance Test Results

Instance: c8g.12xlarge
Server: 34.244.235.41 (private: 172.31.2.238:3090)
Client: 34.251.252.173
Duration: 20s | Warmup: 2s
Spinr mode: docker
OS monitors: true
Perf stat: server only
Date: 2026-03-18 20:54:46 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| text-c128 | harrow | /text | 128 | 509706.600 | 0.240 | 0.500 | 0.620 |
| text-c128 | axum | /text | 128 | 1053411.400 | 0.120 | 0.220 | 0.260 |
| json-1kb-c128 | harrow | /json/1kb | 128 | 571097.350 | 0.220 | 0.440 | 0.550 |
| json-1kb-c128 | axum | /json/1kb | 128 | 992379.100 | 0.120 | 0.240 | 0.290 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % |
|-----------|------------|----------|---------|
| text-c128 | 509706.600 | 1053411.400 | -51.61% |
| json-1kb-c128 | 571097.350 | 992379.100 | -42.45% |
