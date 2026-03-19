# Performance Test Results

![Run Dashboard](summary.svg)

Instance: c8g.12xlarge
Server: 63.33.64.12
Client: 54.194.23.97
Duration: 60s | Warmup: 5s
Spinr mode: docker
OS monitors: true
Perf: record (server only)
Date: 2026-03-19 13:46:09 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| json-1kb-c128 | axum | /json/1kb | 128 | 1017349.970 | 0.120 | 0.240 | 0.280 |
| json-1kb-c128 | harrow | /json/1kb | 128 | 967500.580 | 0.130 | 0.250 | 0.290 |
| text-c128 | axum | /text | 128 | 1055730.220 | 0.120 | 0.220 | 0.250 |
| text-c128 | harrow | /text | 128 | 1041052.280 | 0.120 | 0.220 | 0.260 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % | Harrow p99 (ms) | Axum p99 (ms) |
|-----------|------------|----------|---------|------------------|---------------|
| json-1kb-c128 | 967500.580 | 1017349.970 | -4.90% | 0.250 | 0.240 |
| text-c128 | 1041052.280 | 1055730.220 | -1.39% | 0.220 | 0.220 |

## Telemetry Digest

| Run | Server CPU (user/sys/wait/idle) | Client CPU (user/sys/wait/idle) | Server Net (rx/tx MB/s, retrans/s) | Client Net (rx/tx MB/s, retrans/s) | Top Perf Hotspot |
|-----|----------------------------------|----------------------------------|------------------------------------|------------------------------------|------------------|
| axum_json_1kb_c128 | 10.3% / 10.0% / 0.0% / 79.6% | 3.5% / 16.9% / 0.0% / 79.6% | 108.7 / 1094.3 MB/s · retrans 0.00/s | 1152.1 / 128.5 MB/s · retrans 0.00/s | - |
| harrow_json_1kb_c128 | 10.5% / 9.7% / 0.0% / 79.8% | 4.0% / 19.3% / 0.0% / 76.7% | 103.5 / 1041.5 MB/s · retrans 0.00/s | 1096.0 / 122.3 MB/s · retrans 0.00/s | - |
| axum_text_c128 | 7.2% / 10.0% / 0.0% / 82.7% | 3.7% / 17.6% / 0.0% / 78.6% | 109.2 / 163.3 MB/s · retrans 0.00/s | 160.8 / 129.6 MB/s · retrans 0.00/s | - |
| harrow_text_c128 | 7.4% / 9.8% / 0.0% / 82.8% | 4.3% / 19.9% / 0.0% / 75.8% | 107.9 / 161.5 MB/s · retrans 0.00/s | 158.7 / 127.9 MB/s · retrans 0.00/s | - |

## Telemetry Charts

### json-1kb-c128

![json-1kb-c128 telemetry](./json-1kb-c128.server.telemetry.svg)

### text-c128

![text-c128 telemetry](./text-c128.server.telemetry.svg)

## Artifacts

| Run | JSON | Perf Report | Perf Script | Perf SVG | Server CPU | Server Net | Client CPU | Client Net |
|-----|------|-------------|-------------|----------|------------|------------|------------|------------|
| axum_json_1kb_c128 | [json](./axum_json_1kb_c128.json) | [perf-report](./axum_json_1kb_c128.server.perf-report.txt) | [perf-script](./axum_json_1kb_c128.server.perf.script) | [perf.svg](./axum_json_1kb_c128.server.perf.svg) | [server cpu](./axum_json_1kb_c128.server.sar-u.txt) | [server net](./axum_json_1kb_c128.server.sar-net.txt) | [client cpu](./axum_json_1kb_c128.client.sar-u.txt) | [client net](./axum_json_1kb_c128.client.sar-net.txt) |
| harrow_json_1kb_c128 | [json](./harrow_json_1kb_c128.json) | [perf-report](./harrow_json_1kb_c128.server.perf-report.txt) | [perf-script](./harrow_json_1kb_c128.server.perf.script) | [perf.svg](./harrow_json_1kb_c128.server.perf.svg) | [server cpu](./harrow_json_1kb_c128.server.sar-u.txt) | [server net](./harrow_json_1kb_c128.server.sar-net.txt) | [client cpu](./harrow_json_1kb_c128.client.sar-u.txt) | [client net](./harrow_json_1kb_c128.client.sar-net.txt) |
| axum_text_c128 | [json](./axum_text_c128.json) | [perf-report](./axum_text_c128.server.perf-report.txt) | [perf-script](./axum_text_c128.server.perf.script) | [perf.svg](./axum_text_c128.server.perf.svg) | [server cpu](./axum_text_c128.server.sar-u.txt) | [server net](./axum_text_c128.server.sar-net.txt) | [client cpu](./axum_text_c128.client.sar-u.txt) | [client net](./axum_text_c128.client.sar-net.txt) |
| harrow_text_c128 | [json](./harrow_text_c128.json) | [perf-report](./harrow_text_c128.server.perf-report.txt) | [perf-script](./harrow_text_c128.server.perf.script) | [perf.svg](./harrow_text_c128.server.perf.svg) | [server cpu](./harrow_text_c128.server.sar-u.txt) | [server net](./harrow_text_c128.server.sar-net.txt) | [client cpu](./harrow_text_c128.client.sar-u.txt) | [client net](./harrow_text_c128.client.sar-net.txt) |

## Flamegraphs

### axum_json_1kb_c128

![axum_json_1kb_c128 flamegraph](./axum_json_1kb_c128.server.perf.svg)

### harrow_json_1kb_c128

![harrow_json_1kb_c128 flamegraph](./harrow_json_1kb_c128.server.perf.svg)

### axum_text_c128

![axum_text_c128 flamegraph](./axum_text_c128.server.perf.svg)

### harrow_text_c128

![harrow_text_c128 flamegraph](./harrow_text_c128.server.perf.svg)

