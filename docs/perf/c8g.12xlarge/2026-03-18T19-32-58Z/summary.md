# Performance Test Results

Instance: c8g.12xlarge
Server: 34.244.235.41 (private: 172.31.2.238:3090)
Client: 34.251.252.173
Duration: 60s | Warmup: 5s
Spinr mode: docker
OS monitors: true
Perf stat: server only
Date: 2026-03-18 19:40:02 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| text-c128 | harrow | /text | 128 | 543278.700 | 0.230 | 0.450 | 0.560 |
| text-c128 | axum | /text | 128 | 1054846.550 | 0.120 | 0.220 | 0.250 |
| json-1kb-c128 | harrow | /json/1kb | 128 | 503983.400 | 0.250 | 0.490 | 0.600 |
| json-1kb-c128 | axum | /json/1kb | 128 | 1043349.480 | 0.120 | 0.220 | 0.270 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % |
|-----------|------------|----------|---------|
| text-c128 | 543278.700 | 1054846.550 | -48.50% |
| json-1kb-c128 | 503983.400 | 1043349.480 | -51.70% |
