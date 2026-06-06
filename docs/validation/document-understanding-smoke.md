# Document Understanding Smoke

This smoke validates the current P0 document-understanding contract for third-party document ranges, parser quality, entity extraction, and model-visible parse status.

## Command

```bash
bash scripts/run-document-understanding-smoke.sh
```

The script writes JSON and Markdown reports under:

```text
target/document-understanding-smoke/
```

## Current Coverage

- PaddleOCR parser contract and one-character PDF low-quality detection.
- MiniMax VLM rescue selection for weak unstructured PDF text when configured.
- Structure-aware chunking, parser section preservation, paragraph samples, and candidate noun terms.
- External parse request/detail compatibility for Java/camelCase callers.
- Third-party `documentExternalId` source inference and unresolved-document model visibility.
- Conversation-scoped temporary document ranges that do not move canonical documents.
- Selected external DOC/DOCX evidence supply for short-name questions such as “邓工是谁”.
- Parse status supply for failed, reparsing, degraded, and selected-range documents.
- Resume/company and general typed entity scan behavior.
- Background enrichment coverage for table structures, elderly-care procedure steps and time thresholds, entity terms, resume profiles, and attendance/spreadsheet metrics.

## Answer-Quality Autofix Escalation

Low-quality answer collection and fixed Codex patch proposal are covered by the local fixed-template smoke:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary
```

This smoke validates only the bounded task package, output validation, human-exception audit, and runtime summary. It does not re-enable a blocking answer gate, deploy changes, or broaden document/data permissions.

Latest local receipt on 2026-06-06:

- Fixed-task JSON: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.json`
- Fixed-task Markdown: `target/cloudflare-codex-fixed-task-smoke/cloudflare-codex-fixed-task-smoke-20260606T011735Z.md`
- Quality-gate JSON: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.json`
- Quality-gate Markdown: `target/document-quality-smoke/document-quality-smoke-20260606T011549Z-23164.md`

## 2026-06-06 Local Background Enrichment Evidence

- Command: `powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local`
- Result: passed
- JSON report: `target/document-quality-smoke/document-quality-smoke-20260606T005235Z-6760.json`
- Markdown summary: `target/document-quality-smoke/document-quality-smoke-20260606T005235Z-6760.md`
- Added coverage includes:
  - elderly-care procedure and threshold facts;
  - table-structure enrichment;
  - resume-profile enrichment;
  - attendance/spreadsheet metric enrichment.

## 2026-06-06 Local Aggregate-First Supply Evidence

- Result: passed locally.
- Full local document-quality smoke receipt:
  - JSON: `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.json`
  - Markdown summary: `target/document-quality-smoke/document-quality-smoke-20260606T010841Z-27544.md`
- Scope:
  - deterministic aggregate intent detection for resume company statistics, multi-resume project ranking, attendance absence/work-hour/date questions, and Xinbai-style store/brand/take-high/risk/low-active metrics;
  - `dataset_fact_snapshot` model context and compact retry guidance;
  - selected-document and external temporary scope `document_facts_scoped_aggregate`;
  - database aggregate heuristics for take-high, operating health, low-active risk, rent-sales ratio, and traffic decline prompts;
  - ReAct planning catalog aggregate-supply guidance;
  - answer-quality judge keeps satisfied spreadsheet answers unblocked while still retrying deferred/weak retrieval answers.
- Commands:

```powershell
cargo fmt --check -p platform-api
cargo test -p platform-api dataset_fact_snapshot --lib
cargo test -p platform-api scoped_fact --lib
cargo test -p platform-api database_aggregate_heuristics --lib
cargo test -p platform-api assistant_run_general_entity_scan_prompts_request_dataset_scan --lib
cargo test -p platform-api assistant_run_deterministic_aggregate_intent_covers_customer_smoke_domains --lib
cargo test -p platform-api planning_catalog_prefers_aggregate_fact_supply_without_fact_rows --lib
cargo test -p platform-api assistant_run_answer_quality_judge_runs_for_structured_short_answer --lib
cargo test -p platform-api assistant_run_answer_quality_judge_skips_satisfied_spreadsheet_table --lib
cargo test -p platform-api assistant_run_answer_quality_gate_retries_deferred_retrieval_language --lib
cargo check -p platform-api
```

- Pending: 8-server private aggregate smoke for live resume, attendance, Xinbai, and elderly-care datasets after deployment and private bearer/cookie configuration are available.

## 2026-06-06 8-Server Third-Party Scoped Document Chat Smoke

- Commit deployed: `4aa607319693`.
- Command shape:

```bash
EXTERNAL_SCOPED_DOCUMENT_SMOKE_BEARER=<server-loaded token> \
npm run smoke:external-scoped-document-chat -- \
  --base-url https://v3.elepcloud.com \
  --connection-id generic-chat-main \
  --source-id third-party-source-main \
  --timeout-ms 180000 \
  --parse-timeout-ms 240000 \
  --output-dir target/external-scoped-document-chat-smoke-current-head-passed
```

- Receipt:
  - JSON: `/srv/aiv3/repo/target/external-scoped-document-chat-smoke-current-head-passed/20260606095323.json`
  - Markdown: `/srv/aiv3/repo/target/external-scoped-document-chat-smoke-current-head-passed/20260606095323.md`
- Result: passed, 4/4 cases.
- Fixture readiness:
  - `alpha`, `beta`, `extra`, and `attachment` parse status `parsed`;
  - each fixture had `chunk_count=1` and `retrieval_evidence_count=1`;
  - model status was `ready`.
- Cases covered:
  - `dataset_external_ids` authorized a stable group and `available_document_external_ids` unioned an explicit document outside that group;
  - same `conversation_external_id` follow-up restored the previous scoped document range without repeating scope fields;
  - changed `conversation_external_id` did not expose the prior scoped answer token;
  - `attachment_refs.filename` matched the parsed attachment document and supplied its content to the answer.
- Regressions found and fixed before pass:
  - cross-conversation external user memory could previously supply answer excerpts derived from conversation-scoped document authorization; fixed by not persisting conversation-bound scoped answers to external user memory and by filtering older memory via source run `selected_scope`;
  - attachment title scope could match sibling smoke documents too broadly and fail to supply selected document chunks; fixed by tightening ASCII title token matching and by allowing selected document ids to be loaded directly for fallback chunk supply.
- Local verification for the fixes:

```powershell
node --check scripts\smoke\external-scoped-document-chat.mjs
cargo test -p platform-api external_channel_attachment_title_hints_match_resume_document_titles --lib
cargo test -p platform-api external_user_memory_is_intent_gated_and_supplied --lib
cargo test -p platform-api assistant_run_external_temporary_scope_retrieval_limits_to_selected_documents --lib
cargo test -p platform-api external_channel_dataset_scope_merges_explicit_attachment_title_document --lib
cargo fmt --check -p platform-api
cargo check -p platform-api
git diff --check
```

- Safety:
  - no third-party public URL, auth method, required request field, or existing response field changed;
  - no bearer token, database URL, customer document body, provider payload, or customer row was recorded in this validation note.

## 2026-05-20 Deployment Target Evidence

- Host: `8服务器`
- Repository: `/srv/aiv3/repo`
- HEAD: `d3bf97a`
- Command: `bash scripts/run-document-understanding-smoke.sh`
- Result: passed
- JSON report: `/srv/aiv3/repo/target/document-understanding-smoke/document-understanding-smoke-20260520T081746Z.json`
- Markdown summary: `/srv/aiv3/repo/target/document-understanding-smoke/document-understanding-smoke-20260520T081746Z.md`
