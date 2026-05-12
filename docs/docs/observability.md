---
title: Observability
description: Prometheus metrics and structured logging for Alchemist.
---

Alchemist exposes operational data in two complementary forms:

- A **Prometheus `/metrics` endpoint** for scraping into Grafana, VictoriaMetrics, or any Prometheus-compatible TSDB.
- **Structured JSON logs** for ingestion into Loki, Elasticsearch, or Datadog.

Both are off by default — a default install behaves exactly as it always has.

## Prometheus `/metrics`

Enable scraping by setting `metrics_enabled = true` under `[system]` in your `config.toml`:

```toml
[system]
metrics_enabled = true
```

The endpoint then responds on the main Alchemist HTTP port (`3000` by default):

```bash
curl http://localhost:3000/metrics
```

### LAN-only access

`/metrics` is **not** authenticated, by design — Prometheus scrape configurations
expect either an unauthenticated endpoint or bearer auth, and the simpler option
matches typical homelab deployments. To compensate, Alchemist refuses requests
from anything outside the local network with `403 METRICS_LAN_ONLY`.

If Prometheus runs behind a reverse proxy, list the proxy IP in
`[system].trusted_proxies` so the resolved `X-Forwarded-For` address — not the
proxy itself — is what the LAN check evaluates.

### Exposed metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `alchemist_jobs_total` | Gauge | `status` | Non-archived jobs grouped by status (`queued`, `active`, `completed`, `failed`, `cancelled`, `skipped`). |
| `alchemist_completed_jobs_total` | Gauge | — | Snapshot of completed, non-archived jobs. |
| `alchemist_bytes_saved_total` | Gauge | — | Cumulative bytes saved across all completed encodes. |
| `alchemist_encodes_completed_total` | Counter | `codec` | Successful encodes since process start, broken down by output codec. |
| `alchemist_encode_duration_seconds` | Histogram | `codec` | Encode wall-time distribution. Buckets: 10s → 4h. |
| `alchemist_pipeline_errors_total` | Counter | `code` | Job failures since process start, labelled by structured failure code. |

Counters and histograms reset on restart; gauges reflect the live database
state.

### Example `scrape_configs`

```yaml
scrape_configs:
  - job_name: alchemist
    metrics_path: /metrics
    scrape_interval: 30s
    static_configs:
      - targets: ['alchemist.lan:3000']
```

## Structured JSON logging

For log ingestion pipelines, run Alchemist with one structured log line per
event:

```bash
ALCHEMIST_LOG_FORMAT=json alchemist
```

Each line is a self-contained JSON object — `timestamp`, `level`, `message`,
`target`, and any spans / fields attached via `tracing` — making it safe to
pipe directly into Loki, Vector, Filebeat, or Fluent Bit.

Alternatively, set it permanently in your `config.toml`:

```toml
[system]
log_format = "json"
```

The environment variable wins when both are set.

### Sample Loki query

To filter for failures:

```logql
{job="alchemist"} | json | level="ERROR"
```

To watch a specific job:

```logql
{job="alchemist"} | json | job_id="1234"
```
