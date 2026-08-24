# Operations

## Configuration and secrets

The initial release is always paper mode. Set `DATABASE_URL` to PostgreSQL and
`ADMIN_API_TOKEN` for the authenticated kill switch. Inject secrets through the
service environment or a secret manager; do not commit an environment file.
Webull keys, tokens, signatures, account IDs, and credentials must never appear
in logs.

## Local startup

```sh
docker compose up --build
```

The engine applies embedded PostgreSQL migrations before processing the
synthetic startup event when `DATABASE_URL` is present. Without it, the
executable uses an in-memory test adapter.

## VM service

Install the ARM64 or AMD64 release binary at
`/opt/trading-engine/trading-engine`, create an unprivileged `trading` user,
write a root-readable `/etc/trading-engine/environment`, and install
`deploy/trading-engine.service`. Validate with:

```sh
systemd-analyze security trading-engine.service
systemctl enable --now trading-engine.service
```

The API handles `SIGINT` gracefully. Durable event IDs, client order IDs, and
database uniqueness constraints protect restart recovery from duplicate
submission.

## Metrics and alerts

The production telemetry adapter must export:

- market-data age and out-of-order rejection count;
- HTTP, MQTT, and gRPC broker connectivity;
- order rejection and risk-block counts by non-sensitive reason;
- reconciliation differences for orders, positions, and each cash currency;
- kill-switch state.

Structured logs are JSON. Never use symbols, strategy IDs, or broker account
identifiers as unbounded metric labels.

## Backup and recovery

1. Run encrypted daily `pg_dump --format=custom` backups and encrypt before
   transfer to versioned object storage.
2. Keep database credentials and encryption keys in separate secret stores.
3. Test restore quarterly into an isolated PostgreSQL instance.
4. Apply migrations to the restored database.
5. Start with the kill switch active and broker transport disabled.
6. Reconcile orders, fills, positions, HKD/USD cash, and FX rates against the
   broker before resuming market-data ingestion.
7. Replay only events whose idempotency keys are absent.

Record recovery-point and recovery-time results after every drill. A backup is
not considered valid until a restore and reconciliation succeed.
