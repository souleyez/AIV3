# Data Ingestion Analysis Smoke

This smoke validates the fixed Cloudflare Codex `data_ingestion_analysis` path without writing databases or changing public interfaces.

The contract is:

- Customer requests for data接入, 入库, 建表, 字段映射, 清洗, schema, ETL, 导入, or database analysis are detected from existing chat text.
- DataMax packages only selected datasets, documents, files, tables, or configured database-source previews.
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

For a reviewed real third-party data-ingestion smoke, provide both a bearer and a private source scope:

```powershell
$env:V3_EXTERNAL_CHANNEL_BEARER_TOKEN = "<private token>"
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 `
  -BaseUrl https://v3.elepcloud.com `
  -AllowServerMutation `
  -Case data-ingestion-analysis `
  -ServerCaseConfigPath .\private\data-ingestion-smoke.case.json
```

The private config must include at least one external-channel source selector: `dataset_external_id`, `dataset_external_ids`, or `available_document_external_ids`. It may also include `connection_id`, `available_document_source_id`, `data_ingestion_text`, and `bearer_token_env`. Without a bearer or without a source selector, the script fails before mutation with `mutation_attempted=false`.

## 2026-05-25 Local Evidence

- Environment: local Windows workspace, plan-only, no server writes
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis -Json`
- Result: passed
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T040306Z.json`
- Markdown summary: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260525T040306Z.md`
- Source type: DataMax-selected datasets/documents/files/database-source previews only
- Workflow id: not created in local plan-only smoke
- Risk decision: unsafe credential/schema/API/production-write cases route to `needs_human`
- Rollback: set `CODEX_HOST_TASK_ENABLED=false` or remove `data_ingestion_analysis` from `CODEX_HOST_TASK_ALLOWLIST`

Read-only deployment readiness:

- Environment: `8服务器` public DataMax endpoint, plan-only, no mutation
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -PlanOnly -Case data-ingestion-analysis -Json`
- Result: passed read-only readiness checks; mutation skipped by guard
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T021620Z.json`
- Checked public guide: `https://v3.elepcloud.com/external-integrations/pure-third-party-integration-guide.zh-CN.html`
- Checked external events auth guard: no-token `POST /v1/external/channels/generic-chat-main/events` returned `external_channel_auth_failed`
- Checked workflow queue diagnostics: `GET /v1/workflow-tasks/queue-stats` returned JSON
- Expected output statuses after reviewed mutation smoke: `analysis_ready`, `staging_spec_ready`, `needs_human`, or `failed`
- Production writes allowed: false

Guarded mutation dry runs:

- No bearer command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -AllowServerMutation -Case data-ingestion-analysis -Json`
- Result: failed by design before mutation; `mutation_attempted=false`, `bearer_configured=false`
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T043600Z.json`
- Source-scope guard command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -AllowServerMutation -Case data-ingestion-analysis -BearerToken dummy-for-source-guard -Json`
- Result: failed by design before mutation; `mutation_attempted=false`, `bearer_configured=true`, `source_configured=false`
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T043901Z.json`

Reviewed real mutation smoke:

- Environment: `8服务器` public DataMax endpoint, bearer loaded from the active `local-dev` external-channel connection without printing it
- Config: private ignored case file with one `dataset_external_ids` entry and no credentials
- Before config fix: the first run reached DataMax and was rejected by preflight with `codex_host_task_not_allowlisted`
- Config fix: added `data_ingestion_analysis` to `CODEX_HOST_TASK_ALLOWLIST` and `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES`, then restarted `aiv3-platform-api.service` and `aiv3-codex-host-agent.service`
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -AllowServerMutation -Case data-ingestion-analysis -ServerCaseConfigPath target\private-smoke\data-ingestion-smoke.case.json -ServerPollTimeoutSec 90 -ServerPollIntervalSec 10 -Json`
- Result: passed as accepted processing; initial status `data_ingestion_analysis_queued`, later status `data_ingestion_analysis_running` / `data_ingestion_analysis_retrying`, no terminal failure/source-required state
- Assistant run: `eac46e93-7011-42ce-bd50-829de60240d6`
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T045815Z.json`
- Follow-up: continue polling this run until `data_ingestion_analysis_completed`, `data_ingestion_analysis_needs_human`, or `data_ingestion_analysis_failed` to validate the terminal result quality.

Post-fix terminal smoke:

- Fix: host-agent now preserves the Cloudflare orchestrator `task_id` across retry payload updates, so one DataMax workflow polls one remote Codex task instead of submitting a new task on every retry.
- Fix: host-agent now records external data-ingestion terminal events directly from fixed-task output, so `/assistant-runs/{id}/reply` can return `data_ingestion_analysis_completed`, `data_ingestion_analysis_needs_human`, or `data_ingestion_analysis_failed`.
- Command: `.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -BaseUrl https://v3.elepcloud.com -AllowServerMutation -Case data-ingestion-analysis -ServerCaseConfigPath target\private-smoke\data-ingestion-smoke.case.json -ServerPollTimeoutSec 240 -ServerPollIntervalSec 10 -Json`
- Result: passed terminal smoke with `data_ingestion_analysis_needs_human` after six polls; no terminal failure.
- Assistant run: `b72db616-4561-4810-af51-6d2b69c5f6d9`
- Workflow task: `c73ebb0a-870a-4c5c-b50e-9421b462c397`
- Remote Cloudflare task: `task_6761204b-3d17-4dbf-a94d-91daee4dfaad`; all three `codex_host_task.poll_retry` events used this same task id.
- JSON report: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260531T055420Z.json`

Do not record credentials, database URLs, raw customer data dumps, SSH details, or unrestricted filesystem paths in this validation note.
