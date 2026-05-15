# AssistantRun Worker Smoke

## Purpose

This smoke validates the deployment shape for the background AssistantRun model-completion worker. It is intentionally non-destructive by default: it does not load `/etc/aiv3/aiv3.env`, does not reset a database, and does not require live provider credentials.

The smoke checks that:

- `assistant-run-worker` compiles on the target host;
- the worker's default queue is `assistant_run`;
- the worker's default task key is `consume_model_completion_turn`;
- `assistant_run_model_completion_workflow` is registered in `workflow-definitions`;
- the video completion dispatch request remains redacted and idempotent;
- optional DB-backed consumer coverage is only run against an explicitly supplied disposable test database.

## Runner

```bash
bash scripts/run-assistant-run-worker-smoke.sh
```

Default report output:

```text
target/assistant-run-worker-smoke/<timestamp>.json
target/assistant-run-worker-smoke/<timestamp>.md
```

## Optional DB-Backed Check

Only enable this against a disposable test database:

```bash
export PLATFORM_DATABASE_URL='postgres://.../ai_data_platform_v3_test'
export ASSISTANT_RUN_WORKER_SMOKE_DATABASE_TEST=true
bash scripts/run-assistant-run-worker-smoke.sh
```

The script refuses to run the DB-backed check against a non-test `ai_data_platform_v3` database.

## Production Service Shape

After code deployment, the process manager should run either the built binary or:

```bash
cargo run -p assistant-run-worker
```

Expected service environment:

```text
PLATFORM_DATABASE_URL=<platform postgres url>
PLATFORM_NATS_URL=<optional nats url>
ASSISTANT_RUN_QUEUE=assistant_run
ASSISTANT_RUN_TASK_KEY=consume_model_completion_turn
ASSISTANT_RUN_POLL_INTERVAL_MS=1000
```

The worker keeps database polling as the durable fallback and uses NATS only as an acceleration wake-up path.
