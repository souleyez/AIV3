# External Bot And Third-Party Smoke

Use this smoke check when validating the V3 external bot, third-party knowledge, artifact, and action path before a release or deployment checkpoint.

## Scope

This smoke covers:

- external artifact action policy for status, publish, and revoke;
- external business action confirmation policy;
- external ACL filtering for the same question from different users;
- generic third-party source sync workflow creation;
- Feishu/Lark encrypted callback normalization;
- WeCom encrypted callback normalization;
- high-risk external channel action confirmation flow;
- external integrations observability panel normalization and no direct home navigation.

It is a contract smoke, not a live customer-system test. It uses deterministic unit tests and local PostgreSQL fixture tests already present in the repo.

## Local Or Jump-Host Run

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-bot-third-party-smoke.ps1
```

For a fast deterministic pass without local PostgreSQL:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-bot-third-party-smoke.ps1 -SkipDatabase
```

For backend-only policy checks:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-external-bot-third-party-smoke.ps1 -SkipWeb
```

The same script can run on `windows-jump` after the repository is checked out there. Prefer the jump host for environment-sensitive Codex or provider validation; keep generated logs and task artifacts out of git.

## PostgreSQL Fixture

Database-backed steps use the repo's standard local fixture:

```text
postgres://ai_platform:ai_platform@127.0.0.1:5432/ai_data_platform_v3
```

If the fixture database is not available, the relevant Rust tests print a `skipping ... failed to connect local postgres fixture` message and exit successfully. That is acceptable for a lightweight local pass, but a release smoke should run with PostgreSQL available so the ACL, source sync, adapter, and confirmation paths actually execute.

## Expected Coverage

The smoke should prove these product rules:

- the same external question is filtered by external principal and source ACL before evidence enters AssistantRun context;
- Feishu/Lark and WeCom adapters stay thin and route normalized events into the shared channel ingress;
- low-risk artifact publish is allowed without confirmation;
- artifact revoke and cross-system business actions require confirmation;
- high-risk channel actions pause and return a confirmation-required reply;
- external integration observability includes safe management, drift, artifact, and audit summaries;
- raw secrets, raw provider payloads, raw prompt text, hidden document content, and raw artifact URLs stay out of public summaries.

## Follow-Up When It Fails

Fix the first failing contract boundary before broadening the run:

- Policy or risk classification failures usually belong in `crates/assistant-runtime`.
- Channel ingress, adapter, ACL, action, or audit failures usually belong in `crates/platform-api`.
- Panel normalization failures usually belong in `apps/web/app/lib/external-integrations.js` or `apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`.
- Documentation drift belongs in `docs/integrations/third-party-integration-api.md` and `docs/integrations/third-party-integration-api.zh-CN.md`.

Do not commit generated smoke artifacts, raw logs, provider keys, local database dumps, or task directories.
