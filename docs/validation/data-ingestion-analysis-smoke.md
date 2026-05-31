# Data Ingestion Analysis Smoke

This smoke validates the fixed Cloudflare Codex `data_ingestion_analysis` path without writing databases or changing public interfaces.

The contract is:

- Customer requests for data接入, 入库, 建表, 字段映射, 清洗, schema, ETL, 导入, or database analysis are detected from existing chat text.
- V3 packages only selected datasets, documents, files, tables, or configured database-source previews.
- Local plan-only execution validates the fixed template contract, host preflight, platform output validation, and audit validation path.
- Analysis outputs require source summary, data-quality report, validation checks, and recommended next actions.
- Staging-spec outputs must include a bounded staging specification.
- Credential requests, database URLs, production writes, schema migrations, public API changes, auth changes, URL changes, or third-party request/response field changes become `needs_human`.

## Local Command

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis
```

Expected:

- no 8-server writes;
- no real database writes;
- no credentials or raw database URLs in package/output;
- fixed task package validates;
- structured output validation accepts `analysis_ready` and rejects unsafe content;
- unsafe production/schema/public API cases become `needs_human`.

## Remote Readiness

Read-only deployment readiness may be checked with:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case data-ingestion-analysis
```

Actual remote mutation/execution requires explicit operator review plus `-AllowServerMutation`. The approved output must remain read-only analysis or staging-spec proposal; production writes and schema changes still require human confirmation.

## 2026-05-25 Local Evidence

- Environment: local Windows workspace, plan-only, no server writes
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis -Json`
- Result: passed
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T040306Z.json`
- Markdown summary: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T040306Z.md`
- Source type: V3-selected datasets/documents/files/database-source previews only
- Workflow id: not created in local plan-only smoke
- Risk decision: unsafe credential/schema/API/production-write cases route to `needs_human`
- Rollback: set `CODEX_HOST_TASK_ENABLED=false` or remove `data_ingestion_analysis` from `CODEX_HOST_TASK_ALLOWLIST`

Read-only deployment readiness:

- Environment: `8服务器` public V3 endpoint, plan-only, no mutation
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case data-ingestion-analysis -Json`
- Result: passed read-only readiness checks; mutation skipped by guard
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T021620Z.json`
- Checked public guide: `https://v3.elepcloud.com/external-integrations/pure-third-party-integration-guide.zh-CN.html`
- Checked external events auth guard: no-token `POST /v1/external/channels/generic-chat-main/events` returned `external_channel_auth_failed`
- Checked workflow queue diagnostics: `GET /v1/workflow-tasks/queue-stats` returned JSON
- Expected output statuses after reviewed mutation smoke: `analysis_ready`, `staging_spec_ready`, `needs_human`, or `failed`
- Production writes allowed: false

Do not record credentials, database URLs, raw customer data dumps, SSH details, or unrestricted filesystem paths in this validation note.
