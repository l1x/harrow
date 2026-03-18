# Performance Test Results

Instance: c8g.12xlarge
Server: 34.244.235.41 (private: 172.31.2.238:3090)
Client: 34.251.252.173
Duration: 20s | Warmup: 2s
Spinr mode: docker
OS monitors: true
Perf stat: server only
Date: 2026-03-18 19:47:01 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| text-c128 | harrow | /text | 128 | 568341 | 0.220 | 0.430 | 0.540 |
| text-c128 | axum | /text | 128 | 1069605.400 | 0.120 | 0.220 | 0.250 |
| json-1kb-c128 | harrow | /json/1kb | 128 | 558080.950 | 0.220 | 0.440 | 0.550 |
| json-1kb-c128 | axum | /json/1kb | 128 | 1008897.100 | 0.120 | 0.230 | 0.280 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % |
|-----------|------------|----------|---------|
| text-c128 | 568341.000 | 1069605.400 | -46.86% |
| json-1kb-c128 | 558080.950 | 1008897.100 | -44.68% |
