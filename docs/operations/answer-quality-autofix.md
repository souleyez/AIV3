# Answer Quality Autofix

**Scope:** passive low-quality answer collection and fixed Cloudflare Codex optimization tasks.

`answer_quality_autofix` is not the rolled-back customer-facing quality gate. It must not block normal replies, hide acceptable answers, or loop ReAct in front of the customer. It runs after an answer is produced and records bounded cases for asynchronous diagnosis.

## Signals

DataMax may collect a low-quality case when one or more signals appear:

- direct user dissatisfaction or strong complaint;
- weak "资料不足/无法回答/需要继续检索" answer while evidence, facts, or spreadsheet row analysis exists;
- answer-quality retry exhausted;
- controlled fallback used;
- parse-quality recovery is suggested but no VLM upgrade was attempted;
- private smoke marks an answer as low quality.

Accepted structured answers should not be collected. For example, a table answering attendance absence plus longest/shortest work hours from `spreadsheet_row_analysis` is treated as satisfied.

## Case Package

The case package is bounded and safe:

- assistant run id;
- low-quality signal names;
- user question excerpt;
- customer answer excerpt;
- selected-scope summary counts;
- supply-quality summary;
- answer supply source names;
- compact event trace.

Do not include provider keys, raw provider payloads, server credentials, database URLs, raw stdout/stderr, or full customer documents.

## Fixed Codex Task

Collected system-defect cases can be routed to fixed template `answer_quality_autofix`.

Allowed write scope:

- `crates/platform-api/src/lib.rs`
- `fixtures/document-quality/**`
- `scripts/run-document-quality-smoke.ps1`
- `scripts/run-DataMax-quality-gate-smoke.ps1`
- `docs/validation/**`

The task may diagnose and propose low-risk answer-quality patches without per-task human confirmation. It must include regression tests or smoke updates before behavior changes.

## Human Review

Human review is required when:

- the case is missing source data rather than a system defect;
- the host cannot reproduce the failure;
- a proposed patch touches files outside the allowlist;
- the patch changes public APIs, auth, database schema, third-party contracts, deploy scripts, billing, or static-page product code;
- risk level is medium or high;
- deployment is requested.

## Rollback

Disable task routing by removing `answer_quality_autofix` from:

```text
CODEX_HOST_TASK_ALLOWLIST
CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES
```

Case collection may remain enabled because it is passive and does not change customer-facing answers.
