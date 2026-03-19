# Performance Test Results

Instance: c8g.12xlarge
Server: 63.33.64.12 (private: 172.31.1.247:3090)
Client: 54.194.23.97
Duration: 60s | Warmup: 5s
Spinr mode: docker
OS monitors: true
Perf: record (server only)
Date: 2026-03-19 07:46:15 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| text-c128 | harrow | /text | 128 | 501742.230 | 0.250 | 0.520 | 0.650 |
| text-c128 | axum | /text | 128 | 1019224.030 | 0.120 | 0.230 | 0.270 |
| json-1kb-c128 | harrow | /json/1kb | 128 | 589757.170 | 0.210 | 0.430 | 0.530 |
| json-1kb-c128 | axum | /json/1kb | 128 | 998524.180 | 0.120 | 0.240 | 0.280 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % |
|-----------|------------|----------|---------|
| text-c128 | 501742.230 | 1019224.030 | -50.77% |
| json-1kb-c128 | 589757.170 | 998524.180 | -40.94% |
