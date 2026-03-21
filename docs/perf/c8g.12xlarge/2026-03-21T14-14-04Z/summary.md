# Performance Test Results

![Run Dashboard](summary.svg)

Instance: c8g.12xlarge
Server: 3.249.174.124
Client: 34.243.15.89
Duration: 20s | Warmup: 2s
Spinr mode: docker
OS monitors: true
Perf: record (server only)
Date: 2026-03-21 14:40:56 UTC

## Runs

| Test case | Framework | Path | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |
|-----------|-----------|------|-------------|-----|----------|----------|-----------|
| json-10kb-c128 | axum | harrow-bench/spinr/json-10kb-c128.toml | 0 | 803473.900 | 0.150 | 0.300 | 0.390 |
| json-10kb-c128 | harrow | harrow-bench/spinr/json-10kb-c128.toml | 0 | 801449.450 | 0.150 | 0.300 | 0.380 |
| json-10kb-c32 | axum | harrow-bench/spinr/json-10kb-c32.toml | 0 | 504245.450 | 0.090 | 0.160 | 0.210 |
| json-10kb-c32 | harrow | harrow-bench/spinr/json-10kb-c32.toml | 0 | 505325.300 | 0.090 | 0.150 | 0.180 |
| json-1kb-c128 | axum | harrow-bench/spinr/json-1kb-c128.toml | 0 | 1008408.750 | 0.120 | 0.250 | 0.280 |
| json-1kb-c128 | harrow | harrow-bench/spinr/json-1kb-c128.toml | 0 | 987281.950 | 0.120 | 0.250 | 0.290 |
| json-1kb-c32 | axum | harrow-bench/spinr/json-1kb-c32.toml | 0 | 739483.750 | 0.060 | 0.110 | 0.130 |
| json-1kb-c32 | harrow | harrow-bench/spinr/json-1kb-c32.toml | 0 | 739182.100 | 0.060 | 0.110 | 0.130 |
| msgpack-1kb-c128 | axum | harrow-bench/spinr/msgpack-1kb-c128.toml | 0 | 1024233.600 | 0.120 | 0.240 | 0.270 |
| msgpack-1kb-c128 | harrow | harrow-bench/spinr/msgpack-1kb-c128.toml | 0 | 998676.200 | 0.120 | 0.240 | 0.280 |
| msgpack-1kb-c32 | axum | harrow-bench/spinr/msgpack-1kb-c32.toml | 0 | 729859.450 | 0.060 | 0.110 | 0.130 |
| msgpack-1kb-c32 | harrow | harrow-bench/spinr/msgpack-1kb-c32.toml | 0 | 739334.500 | 0.060 | 0.110 | 0.130 |
| text-c128 | axum | harrow-bench/spinr/text-c128.toml | 0 | 1041665.000 | 0.120 | 0.230 | 0.260 |
| text-c128 | harrow | harrow-bench/spinr/text-c128.toml | 0 | 1049590.550 | 0.120 | 0.230 | 0.260 |
| text-c32 | axum | harrow-bench/spinr/text-c32.toml | 0 | 752642.200 | 0.060 | 0.110 | 0.130 |
| text-c32 | harrow | harrow-bench/spinr/text-c32.toml | 0 | 720376.700 | 0.060 | 0.110 | 0.130 |

## Comparison

| Test case | Harrow RPS | Axum RPS | Delta % | Harrow p99 (ms) | Axum p99 (ms) |
|-----------|------------|----------|---------|------------------|---------------|
| json-10kb-c128 | 801449.450 | 803473.900 | -0.25% | 0.300 | 0.300 |
| json-10kb-c32 | 505325.300 | 504245.450 | +0.21% | 0.150 | 0.160 |
| json-1kb-c128 | 987281.950 | 1008408.750 | -2.10% | 0.250 | 0.250 |
| json-1kb-c32 | 739182.100 | 739483.750 | -0.04% | 0.110 | 0.110 |
| msgpack-1kb-c128 | 998676.200 | 1024233.600 | -2.50% | 0.240 | 0.240 |
| msgpack-1kb-c32 | 739334.500 | 729859.450 | +1.30% | 0.110 | 0.110 |
| text-c128 | 1049590.550 | 1041665.000 | +0.76% | 0.230 | 0.230 |
| text-c32 | 720376.700 | 752642.200 | -4.29% | 0.110 | 0.110 |

## Telemetry Digest

| Run | Server CPU (user/sys/wait/idle) | Client CPU (user/sys/wait/idle) | Server Net (rx/tx MB/s, retrans/s) | Client Net (rx/tx MB/s, retrans/s) | Top Perf Hotspot |
|-----|----------------------------------|----------------------------------|------------------------------------|------------------------------------|------------------|
| axum_json-10kb-c128 | 25.1% / 15.3% / 0.0% / 59.5% | 3.2% / 22.3% / 0.0% / 74.5% | 98.0 / 6313.9 MB/s · retrans 0.04/s | 7631.1 / 138.6 MB/s · retrans 0.00/s | - |
| harrow_json-10kb-c128 | 24.8% / 15.3% / 0.0% / 59.9% | 3.1% / 22.5% / 0.0% / 74.4% | 98.7 / 6355.8 MB/s · retrans 0.00/s | 7630.6 / 138.7 MB/s · retrans 0.00/s | - |
| axum_json-10kb-c32 | 16.1% / 8.6% / 0.0% / 75.2% | 2.0% / 14.7% / 0.0% / 83.3% | 62.5 / 4002.2 MB/s · retrans 0.00/s | 4820.6 / 88.2 MB/s · retrans 0.00/s | - |
| harrow_json-10kb-c32 | 16.0% / 8.7% / 0.0% / 75.3% | 2.0% / 14.9% / 0.0% / 83.0% | 62.9 / 4014.9 MB/s · retrans 0.00/s | 4802.0 / 88.1 MB/s · retrans 0.00/s | - |
| axum_json-1kb-c128 | 8.2% / 7.4% / 0.0% / 84.3% | 2.5% / 13.3% / 0.0% / 84.2% | 83.7 / 842.3 MB/s · retrans 0.00/s | 1015.7 / 113.3 MB/s · retrans 0.00/s | - |
| harrow_json-1kb-c128 | 7.4% / 7.1% / 0.0% / 85.5% | 2.6% / 13.8% / 0.0% / 83.6% | 81.3 / 817.8 MB/s · retrans 0.00/s | 991.4 / 110.6 MB/s · retrans 0.00/s | - |
| axum_json-1kb-c32 | 6.3% / 5.6% / 0.0% / 88.1% | 2.1% / 9.9% / 0.0% / 88.0% | 61.3 / 617.0 MB/s · retrans 0.00/s | 743.9 / 83.0 MB/s · retrans 0.00/s | - |
| harrow_json-1kb-c32 | 6.2% / 5.5% / 0.0% / 88.4% | 2.1% / 10.6% / 0.0% / 87.3% | 61.8 / 622.1 MB/s · retrans 0.00/s | 749.8 / 83.6 MB/s · retrans 0.00/s | - |
| axum_msgpack-1kb-c128 | 6.1% / 7.1% / 0.0% / 86.8% | 2.5% / 13.3% / 0.0% / 84.2% | 86.1 / 425.8 MB/s · retrans 0.00/s | 513.4 / 117.5 MB/s · retrans 0.00/s | - |
| harrow_msgpack-1kb-c128 | 5.4% / 7.3% / 0.0% / 87.2% | 2.6% / 13.9% / 0.0% / 83.5% | 83.1 / 410.9 MB/s · retrans 0.00/s | 499.4 / 114.3 MB/s · retrans 0.00/s | - |
| axum_msgpack-1kb-c32 | 4.6% / 5.2% / 0.0% / 90.2% | 2.0% / 10.0% / 0.0% / 88.0% | 61.5 / 304.4 MB/s · retrans 0.00/s | 367.3 / 84.1 MB/s · retrans 0.00/s | - |
| harrow_msgpack-1kb-c32 | 4.3% / 5.3% / 0.0% / 90.5% | 1.9% / 10.1% / 0.0% / 88.0% | 62.3 / 308.0 MB/s · retrans 0.00/s | 370.8 / 84.9 MB/s · retrans 0.00/s | - |
| axum_text-c128 | 5.0% / 7.4% / 0.0% / 87.6% | 2.7% / 13.3% / 0.0% / 84.0% | 84.1 / 125.8 MB/s · retrans 0.00/s | 141.6 / 114.1 MB/s · retrans 0.00/s | - |
| harrow_text-c128 | 4.4% / 7.1% / 0.0% / 88.5% | 2.6% / 13.7% / 0.0% / 83.7% | 82.0 / 122.6 MB/s · retrans 0.00/s | 142.2 / 114.6 MB/s · retrans 0.00/s | - |
| axum_text-c32 | 3.8% / 5.4% / 0.0% / 90.7% | 2.0% / 10.2% / 0.0% / 87.8% | 61.5 / 91.9 MB/s · retrans 0.00/s | 103.0 / 83.0 MB/s · retrans 0.00/s | - |
| harrow_text-c32 | 3.5% / 5.5% / 0.0% / 91.0% | 2.0% / 9.7% / 0.0% / 88.2% | 58.1 / 87.0 MB/s · retrans 0.00/s | 97.9 / 78.9 MB/s · retrans 0.00/s | - |

## Telemetry Charts

### json-10kb-c128

![json-10kb-c128 telemetry](./json-10kb-c128.server.telemetry.svg)

### json-10kb-c32

![json-10kb-c32 telemetry](./json-10kb-c32.server.telemetry.svg)

### json-1kb-c128

![json-1kb-c128 telemetry](./json-1kb-c128.server.telemetry.svg)

### json-1kb-c32

![json-1kb-c32 telemetry](./json-1kb-c32.server.telemetry.svg)

### msgpack-1kb-c128

![msgpack-1kb-c128 telemetry](./msgpack-1kb-c128.server.telemetry.svg)

### msgpack-1kb-c32

![msgpack-1kb-c32 telemetry](./msgpack-1kb-c32.server.telemetry.svg)

### text-c128

![text-c128 telemetry](./text-c128.server.telemetry.svg)

### text-c32

![text-c32 telemetry](./text-c32.server.telemetry.svg)

## Artifacts

| Run | JSON | Perf Report | Perf Script | Perf SVG | Server CPU | Server Net | Client CPU | Client Net |
|-----|------|-------------|-------------|----------|------------|------------|------------|------------|
| axum_json-10kb-c128 | [json](./axum_json-10kb-c128.json) | [perf-report](./axum_json-10kb-c128.server.perf-report.txt) | [perf-script](./axum_json-10kb-c128.server.perf.script) | [perf.svg](./axum_json-10kb-c128.server.perf.svg) | [server cpu](./axum_json-10kb-c128.server.sar-u.txt) | [server net](./axum_json-10kb-c128.server.sar-net.txt) | [client cpu](./axum_json-10kb-c128.client.sar-u.txt) | [client net](./axum_json-10kb-c128.client.sar-net.txt) |
| harrow_json-10kb-c128 | [json](./harrow_json-10kb-c128.json) | [perf-report](./harrow_json-10kb-c128.server.perf-report.txt) | [perf-script](./harrow_json-10kb-c128.server.perf.script) | [perf.svg](./harrow_json-10kb-c128.server.perf.svg) | [server cpu](./harrow_json-10kb-c128.server.sar-u.txt) | [server net](./harrow_json-10kb-c128.server.sar-net.txt) | [client cpu](./harrow_json-10kb-c128.client.sar-u.txt) | [client net](./harrow_json-10kb-c128.client.sar-net.txt) |
| axum_json-10kb-c32 | [json](./axum_json-10kb-c32.json) | [perf-report](./axum_json-10kb-c32.server.perf-report.txt) | [perf-script](./axum_json-10kb-c32.server.perf.script) | [perf.svg](./axum_json-10kb-c32.server.perf.svg) | [server cpu](./axum_json-10kb-c32.server.sar-u.txt) | [server net](./axum_json-10kb-c32.server.sar-net.txt) | [client cpu](./axum_json-10kb-c32.client.sar-u.txt) | [client net](./axum_json-10kb-c32.client.sar-net.txt) |
| harrow_json-10kb-c32 | [json](./harrow_json-10kb-c32.json) | [perf-report](./harrow_json-10kb-c32.server.perf-report.txt) | [perf-script](./harrow_json-10kb-c32.server.perf.script) | [perf.svg](./harrow_json-10kb-c32.server.perf.svg) | [server cpu](./harrow_json-10kb-c32.server.sar-u.txt) | [server net](./harrow_json-10kb-c32.server.sar-net.txt) | [client cpu](./harrow_json-10kb-c32.client.sar-u.txt) | [client net](./harrow_json-10kb-c32.client.sar-net.txt) |
| axum_json-1kb-c128 | [json](./axum_json-1kb-c128.json) | [perf-report](./axum_json-1kb-c128.server.perf-report.txt) | [perf-script](./axum_json-1kb-c128.server.perf.script) | [perf.svg](./axum_json-1kb-c128.server.perf.svg) | [server cpu](./axum_json-1kb-c128.server.sar-u.txt) | [server net](./axum_json-1kb-c128.server.sar-net.txt) | [client cpu](./axum_json-1kb-c128.client.sar-u.txt) | [client net](./axum_json-1kb-c128.client.sar-net.txt) |
| harrow_json-1kb-c128 | [json](./harrow_json-1kb-c128.json) | [perf-report](./harrow_json-1kb-c128.server.perf-report.txt) | [perf-script](./harrow_json-1kb-c128.server.perf.script) | [perf.svg](./harrow_json-1kb-c128.server.perf.svg) | [server cpu](./harrow_json-1kb-c128.server.sar-u.txt) | [server net](./harrow_json-1kb-c128.server.sar-net.txt) | [client cpu](./harrow_json-1kb-c128.client.sar-u.txt) | [client net](./harrow_json-1kb-c128.client.sar-net.txt) |
| axum_json-1kb-c32 | [json](./axum_json-1kb-c32.json) | [perf-report](./axum_json-1kb-c32.server.perf-report.txt) | [perf-script](./axum_json-1kb-c32.server.perf.script) | [perf.svg](./axum_json-1kb-c32.server.perf.svg) | [server cpu](./axum_json-1kb-c32.server.sar-u.txt) | [server net](./axum_json-1kb-c32.server.sar-net.txt) | [client cpu](./axum_json-1kb-c32.client.sar-u.txt) | [client net](./axum_json-1kb-c32.client.sar-net.txt) |
| harrow_json-1kb-c32 | [json](./harrow_json-1kb-c32.json) | [perf-report](./harrow_json-1kb-c32.server.perf-report.txt) | [perf-script](./harrow_json-1kb-c32.server.perf.script) | [perf.svg](./harrow_json-1kb-c32.server.perf.svg) | [server cpu](./harrow_json-1kb-c32.server.sar-u.txt) | [server net](./harrow_json-1kb-c32.server.sar-net.txt) | [client cpu](./harrow_json-1kb-c32.client.sar-u.txt) | [client net](./harrow_json-1kb-c32.client.sar-net.txt) |
| axum_msgpack-1kb-c128 | [json](./axum_msgpack-1kb-c128.json) | [perf-report](./axum_msgpack-1kb-c128.server.perf-report.txt) | [perf-script](./axum_msgpack-1kb-c128.server.perf.script) | [perf.svg](./axum_msgpack-1kb-c128.server.perf.svg) | [server cpu](./axum_msgpack-1kb-c128.server.sar-u.txt) | [server net](./axum_msgpack-1kb-c128.server.sar-net.txt) | [client cpu](./axum_msgpack-1kb-c128.client.sar-u.txt) | [client net](./axum_msgpack-1kb-c128.client.sar-net.txt) |
| harrow_msgpack-1kb-c128 | [json](./harrow_msgpack-1kb-c128.json) | [perf-report](./harrow_msgpack-1kb-c128.server.perf-report.txt) | [perf-script](./harrow_msgpack-1kb-c128.server.perf.script) | [perf.svg](./harrow_msgpack-1kb-c128.server.perf.svg) | [server cpu](./harrow_msgpack-1kb-c128.server.sar-u.txt) | [server net](./harrow_msgpack-1kb-c128.server.sar-net.txt) | [client cpu](./harrow_msgpack-1kb-c128.client.sar-u.txt) | [client net](./harrow_msgpack-1kb-c128.client.sar-net.txt) |
| axum_msgpack-1kb-c32 | [json](./axum_msgpack-1kb-c32.json) | [perf-report](./axum_msgpack-1kb-c32.server.perf-report.txt) | [perf-script](./axum_msgpack-1kb-c32.server.perf.script) | [perf.svg](./axum_msgpack-1kb-c32.server.perf.svg) | [server cpu](./axum_msgpack-1kb-c32.server.sar-u.txt) | [server net](./axum_msgpack-1kb-c32.server.sar-net.txt) | [client cpu](./axum_msgpack-1kb-c32.client.sar-u.txt) | [client net](./axum_msgpack-1kb-c32.client.sar-net.txt) |
| harrow_msgpack-1kb-c32 | [json](./harrow_msgpack-1kb-c32.json) | [perf-report](./harrow_msgpack-1kb-c32.server.perf-report.txt) | [perf-script](./harrow_msgpack-1kb-c32.server.perf.script) | [perf.svg](./harrow_msgpack-1kb-c32.server.perf.svg) | [server cpu](./harrow_msgpack-1kb-c32.server.sar-u.txt) | [server net](./harrow_msgpack-1kb-c32.server.sar-net.txt) | [client cpu](./harrow_msgpack-1kb-c32.client.sar-u.txt) | [client net](./harrow_msgpack-1kb-c32.client.sar-net.txt) |
| axum_text-c128 | [json](./axum_text-c128.json) | [perf-report](./axum_text-c128.server.perf-report.txt) | [perf-script](./axum_text-c128.server.perf.script) | [perf.svg](./axum_text-c128.server.perf.svg) | [server cpu](./axum_text-c128.server.sar-u.txt) | [server net](./axum_text-c128.server.sar-net.txt) | [client cpu](./axum_text-c128.client.sar-u.txt) | [client net](./axum_text-c128.client.sar-net.txt) |
| harrow_text-c128 | [json](./harrow_text-c128.json) | [perf-report](./harrow_text-c128.server.perf-report.txt) | [perf-script](./harrow_text-c128.server.perf.script) | [perf.svg](./harrow_text-c128.server.perf.svg) | [server cpu](./harrow_text-c128.server.sar-u.txt) | [server net](./harrow_text-c128.server.sar-net.txt) | [client cpu](./harrow_text-c128.client.sar-u.txt) | [client net](./harrow_text-c128.client.sar-net.txt) |
| axum_text-c32 | [json](./axum_text-c32.json) | [perf-report](./axum_text-c32.server.perf-report.txt) | [perf-script](./axum_text-c32.server.perf.script) | [perf.svg](./axum_text-c32.server.perf.svg) | [server cpu](./axum_text-c32.server.sar-u.txt) | [server net](./axum_text-c32.server.sar-net.txt) | [client cpu](./axum_text-c32.client.sar-u.txt) | [client net](./axum_text-c32.client.sar-net.txt) |
| harrow_text-c32 | [json](./harrow_text-c32.json) | [perf-report](./harrow_text-c32.server.perf-report.txt) | [perf-script](./harrow_text-c32.server.perf.script) | [perf.svg](./harrow_text-c32.server.perf.svg) | [server cpu](./harrow_text-c32.server.sar-u.txt) | [server net](./harrow_text-c32.server.sar-net.txt) | [client cpu](./harrow_text-c32.client.sar-u.txt) | [client net](./harrow_text-c32.client.sar-net.txt) |

## Flamegraphs

### axum_json-10kb-c128

![axum_json-10kb-c128 flamegraph](./axum_json-10kb-c128.server.perf.svg)

### harrow_json-10kb-c128

![harrow_json-10kb-c128 flamegraph](./harrow_json-10kb-c128.server.perf.svg)

### axum_json-10kb-c32

![axum_json-10kb-c32 flamegraph](./axum_json-10kb-c32.server.perf.svg)

### harrow_json-10kb-c32

![harrow_json-10kb-c32 flamegraph](./harrow_json-10kb-c32.server.perf.svg)

### axum_json-1kb-c128

![axum_json-1kb-c128 flamegraph](./axum_json-1kb-c128.server.perf.svg)

### harrow_json-1kb-c128

![harrow_json-1kb-c128 flamegraph](./harrow_json-1kb-c128.server.perf.svg)

### axum_json-1kb-c32

![axum_json-1kb-c32 flamegraph](./axum_json-1kb-c32.server.perf.svg)

### harrow_json-1kb-c32

![harrow_json-1kb-c32 flamegraph](./harrow_json-1kb-c32.server.perf.svg)

### axum_msgpack-1kb-c128

![axum_msgpack-1kb-c128 flamegraph](./axum_msgpack-1kb-c128.server.perf.svg)

### harrow_msgpack-1kb-c128

![harrow_msgpack-1kb-c128 flamegraph](./harrow_msgpack-1kb-c128.server.perf.svg)

### axum_msgpack-1kb-c32

![axum_msgpack-1kb-c32 flamegraph](./axum_msgpack-1kb-c32.server.perf.svg)

### harrow_msgpack-1kb-c32

![harrow_msgpack-1kb-c32 flamegraph](./harrow_msgpack-1kb-c32.server.perf.svg)

### axum_text-c128

![axum_text-c128 flamegraph](./axum_text-c128.server.perf.svg)

### harrow_text-c128

![harrow_text-c128 flamegraph](./harrow_text-c128.server.perf.svg)

### axum_text-c32

![axum_text-c32 flamegraph](./axum_text-c32.server.perf.svg)

### harrow_text-c32

![harrow_text-c32 flamegraph](./harrow_text-c32.server.perf.svg)

