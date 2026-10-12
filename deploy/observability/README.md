# Observability

The [metrics contract](../../docs/spec/12-metrics.md) defines observation boundaries
and labels. These files contain the Grafana dashboard and initial alert policy.

## Collection

Enable `GROVE_METRICS_BIND=0.0.0.0:9091` on a metrics-enabled server image. A private
ClusterIP service exposes port `metrics`, with a ServiceMonitor using a 30-second
interval and a five-second scrape timeout. The public HTTP service stays on 8080.
The metrics service publishes not-ready Pod addresses, independently of the API
readiness gate, so database outages remain observable.
Network policy limits the metrics port to Prometheus. The listener has no account
authentication and never exposes credentials or object identifiers.

`rules.yaml` selects namespace `grove-storage`, service `grove-storage-metrics`
and endpoint `metrics`. Its PrometheusRule uses the release label selected by the
local Prometheus Operator. `grafana.json` uses datasource UID `prometheus` and a
cluster selector for aggregated Thanos data. Without external cluster labels,
select All or adapt the selector when importing the dashboard.

## Alerts

Initial alerts are warnings, not availability SLOs:

| Condition | Threshold |
|---|---|
| Missing discovery or failed target | 5 minutes |
| S3 server errors | At least 3 in 10 minutes and over 5%, sustained 2 minutes |
| Spool admission rejection | At least 3 in 10 minutes, sustained 2 minutes |
| Stale snapshot | Over 3 minutes, sustained 2 minutes, with startup grace |
| Shared object job or retention stream | No success for 20 minutes, sustained 5 minutes |
| Per-Pod spool cleanup | No success for 10 minutes, sustained 5 minutes |
| Recovery queue age | Over 30 minutes, sustained 10 minutes, with fresh DB state |
| Unknown recovery age | Present for 30 minutes, with fresh DB state |

For scrape failure, check the listener, service, monitor selector and network
policy. For a stale snapshot, check database/disk errors before interpreting
inventory or queue panels. For transfer rejection, compare reservation budget,
slots, temporary files and node filesystem space. For stalled maintenance, inspect
job errors, provider failures and retention stream failures in logs. Provider 404
and HTTP 4xx are not automatically service outages. Unsupported S3 operations'
expected 501 responses are excluded from the server-error alert. Latency panels are per
operation; large uploads have no universal p95 alarm threshold.

## Verification

```sh
python3 -B scripts/check-observability.py --promtool /path/to/promtool
python3 -B scripts/e2e-metrics.py
```

The second command uses the existing disposable PostgreSQL/MinIO fixture,
`boto3` and `prometheus-client`. It verifies presigned transfers, private labels,
scrape independence and DB snapshot failure. CI runs both checks. Activating a
GitOps monitor is a separate step after a verified metrics-enabled image release;
an older server returning no metrics is not a successful deployment.
