# Performance Test Results

Instance: c8g.12xlarge
Server: 34.244.235.41 (private: 172.31.2.238:3090)
Client: 34.251.252.173
Duration: 20s | Warmup: 2s
Spinr mode: docker
OS monitors: true
Perf stat: server only
Date: 2026-03-18 21:06:39 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| text-c128 | harrow | /text | 128 | 536204.450 | 0.230 | 0.460 | 0.570 |
| text-c128 | axum | /text | 128 | 1067539 | 0.120 | 0.220 | 0.250 |
| json-1kb-c128 | harrow | /json/1kb | 128 | 575236.300 | 0.210 | 0.460 | 0.570 |
| json-1kb-c128 | axum | /json/1kb | 128 | 1005271.400 | 0.120 | 0.240 | 0.280 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % |
|-----------|------------|----------|---------|
| text-c128 | 536204.450 | 1067539.000 | -49.77% |
| json-1kb-c128 | 575236.300 | 1005271.400 | -42.78% |
