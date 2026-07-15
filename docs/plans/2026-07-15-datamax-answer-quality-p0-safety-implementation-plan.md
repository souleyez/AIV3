# DataMax Answer Quality P0 Safety Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stop the three proven P0 answer-quality failure modes: ordinary questions triggering artifact workflows, identifier or garbage database fields becoming aggregates, and the live QA harness reporting false green results.

**Architecture:** Preserve the unchanged user question and model behavior. Add one explicit user-action gate before every artifact side-effect entry point, fail-closed table and metric eligibility before database evidence supply with a second connector guard, and receipt-based evaluator and side-effect checks in the live harness. DataMax continues to organize evidence only; it does not plan the answer.

**Tech Stack:** Rust/Axum, existing MySQL source connector contracts, Node.js ESM smoke harnesses, Cargo tests, npm smoke scripts.

---

**Status:** LOCAL P0 SAFETY CANDIDATE GREEN — REAL LIVE QUALITY, DISPOSABLE DB, AND FIELD SEMANTIC CONTRACT PENDING

**Design:** `docs/plans/2026-07-15-datamax-answer-quality-p0-safety-design.md`

**Baseline:** `a8171c468d4e339bd10cbb03765f1038301f9d88`

**Local gate date:** 2026-07-15

The three proven P0 failure classes now have local fail-closed guards and green deterministic regression coverage. This status is deliberately not `COMPLETE`: no real provider/live answer run or disposable PostgreSQL integration run was executed, and case 012 cannot recover a sales-gap aggregate until a persisted field semantic contract authorizes the relevant aggregation.

## Task 1: Freeze the live failures as regression cases

**Files:**

- Modify: `crates/platform-api/src/external_channel_action_prompt_support.rs`
- Modify: `crates/platform-api/src/static_page_prompt_intent_support.rs`
- Modify: `crates/platform-api/src/assistant_run_xinbai_report_link_support.rs`
- Modify: `crates/platform-api/src/wechat_video_login_handoff_support.rs`
- Modify: `crates/platform-api/src/assistant_run_answer_quality_case_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/assistant_run_database_field_support.rs`
- Modify: `crates/platform-api/src/assistant_run_database_prompt_support.rs`
- Modify: `crates/platform-api/src/assistant_run_database_summary_support.rs`
- Modify: `scripts/smoke/newbai-customer-answer-live-capture.mjs`

**Step 1: Add failing action-route cases**

Add the exact mixed question and adjacent report-reference prompts. Assert that they do not request an artifact even when current capability fields or a model tool proposal are present. Keep explicit create/update prompts as positive controls.

**Step 2: Add failing database cases**

Assert that `parentcode` and identifier-like fields are not metrics, `s.a` is not a reliable analysis mapping, a meaningful mapping wins even when `s` is first, and an unrelated question returns no automatic aggregate mapping.

**Step 3: Add failing harness cases**

Create an offline response fixture where the evaluator process exits zero but the inner report is false, and where the captured response contains an artifact/report side effect. Assert that the outer report fails.

**Step 4: Run the focused tests and confirm expected failure**

```powershell
cargo test -p platform-api external_channel_action_prompt_support::tests --lib
cargo test -p platform-api database_field_support --lib
cargo test -p platform-api database_prompt_support --lib
npm run smoke:newbai-customer-answer-live-capture -- --self-test
```

Expected: every newly added guard fails against baseline behavior for the documented reason.

## Task 2: Require explicit artifact mutation intent

**Files:**

- Modify: `crates/platform-api/src/external_channel_action_prompt_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `fixtures/external-channel-capability-routing/cases.jsonl`
- Modify: `scripts/smoke/external-report-focus.mjs`
- Modify: `fixtures/newbai-customer-answer/cases.jsonl`

**Step 1: Implement a pure action-authorization helper**

Require a same-phrase creation/update/render/export/publish action and artifact target. Treat read/explain/quote/compare/summarize wording as ordinary questions. Keep negation highest priority.

**Step 2: Apply the helper to every side-effect path**

Gate broad prompt report heuristics, current `artifact_type` and render/skill capability paths, and model-proposed static-page tool dispatch. Capability metadata cannot replace authorization in the unchanged user prompt.

**Step 3: Reconcile old tests and smoke fixtures**

Turn noun-only and view requests into ordinary QA controls. Retain explicit natural-language action requests as positive cases. Add `forbid_report_generation` to NewBai case 012.

**Step 4: Verify focused routing**

```powershell
cargo test -p platform-api external_channel_static_page_artifact_ -- --nocapture
cargo test -p platform-api external_channel_capability_routing_fixture_checks_deterministic_routes -- --nocapture
cargo test -p platform-api external_channel_action_prompt_support::tests -- --nocapture
npm run smoke:external-report-focus -- --self-test --pretty
```

Expected: mixed 012 and adjacent questions stay side-effect free; explicit action controls pass.

## Task 3: Make automatic database mapping fail closed

**Files:**

- Modify: `crates/platform-api/src/assistant_run_database_field_support.rs`
- Modify: `crates/platform-api/src/assistant_run_database_prompt_support.rs`
- Modify: `crates/platform-api/src/assistant_run_database_summary_support.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Replace substring identifier matching**

Tokenize ASCII identifiers and match complete table and column tokens. Apply configured identifier, title/time/version, and unknown-short-field exclusions before positive metric recognition.

**Step 2: Remove first-table fallback**

Score only positive table or field matches. Return `None` when no reliable mapping exists. Keep low-information tables available for sync and schema inspection, but not automatic analytical supply.

**Step 3: Remove unverified business semantics**

Do not state that `quekou` or `xuzengxiaoshou` has a known unit, direction, or high-score meaning without a persisted contract. Keep evidence descriptive and attributable.

**Step 4: Verify planner behavior**

```powershell
cargo test -p platform-api database_field_support --lib
cargo test -p platform-api database_prompt_support --lib
cargo test -p platform-api database_aggregate_heuristics --lib
```

Expected: no `parentcode` metric, no `/s` fallback, and no aggregate for unrelated DCF questions.

## Task 4: Add connector-level aggregate defense

**Files:**

- Modify: `crates/external-source-connectors/src/mysql.rs`

**Step 1: Add failing query-builder tests**

Cover configured identifiers, code/id/key-like fields, unknown one-character fields, and rate/ratio fields. Confirm `count(*)` remains supported and real numeric aggregate controls still work.

**Step 2: Validate metric role before SQL construction**

Reject non-count aggregation for configured identifiers and non-metric name classes. Reject sum for ratio/rate/percent/score/index-like fields without an explicit contract.

**Step 3: Verify generated SQL**

```powershell
cargo test -p external-source-connectors aggregate_query --lib
```

Expected: generated SQL never contains numeric casts or SUM expressions for identifier-like fields.

## Task 5: Make the live evaluator fail closed

**Files:**

- Modify: `scripts/smoke/newbai-customer-answer-live-capture.mjs`
- Modify if fixture behavior requires it: `scripts/smoke/newbai-customer-answer.mjs`

**Step 1: Parse the unique inner evaluator receipt**

Use an evaluator directory unique to the run. Require process success, `inner.ok=true`, `diagnostic_match=true`, and `decision_eligible=false`. Missing, duplicate, or malformed reports fail.

**Step 2: Derive captured side effects from response rows**

Check `report_triggered`, artifact counts/lists, and all boolean side-effect flags. Add counts and a `noCapturedTemplateOrReportSideEffects` check to the outer summary.

**Step 3: Keep request intent separate from observed behavior**

Retain the statement that the harness did not request publication, but never use that statement as proof that no artifact was produced.

**Step 4: Verify offline harness behavior**

```powershell
node --check scripts/smoke/newbai-customer-answer.mjs
node --check scripts/smoke/newbai-customer-answer-live-capture.mjs
npm run smoke:newbai-customer-answer -- --self-test --pretty
npm run smoke:newbai-customer-answer-live-capture -- --self-test
```

Expected: normal fixtures pass; an inner false report or captured side effect makes the outer report false while remaining diagnostic-only.

## Task 6: Run the P0 regression gate

**Files:**

- Modify: `docs/validation/datamax-main-gap-closure.md`
- Modify: `docs/plans/2026-07-15-datamax-answer-quality-p0-safety-implementation-plan.md`

**Step 1: Run every focused test from Tasks 2 through 5**

Record exact pass and fail results.

**Step 2: Run the established offline QA suites**

```powershell
bash scripts/run-assistant-chat-contract-smoke.sh
bash scripts/run-external-direct-reply-smoke.sh
bash scripts/run-retrieval-quality-smoke.sh --baseline
bash scripts/run-newbai-customer-answer-smoke.sh
```

**Step 3: Run formatting and repository checks**

```powershell
cargo fmt --all -- --check
git diff --check
git status --short
```

**Step 4: Record an honest receipt**

Append the baseline, failing-test-first evidence, exact commands, pass/fail counts, known limitations, and side-effect statement to the validation ledger. Mark this plan complete only if every acceptance criterion is proven locally.

**Step 5: Stop before release actions**

Do not commit, push, deploy, change flags, restart services, call a provider, or run a live write without a subsequent explicit release request.

### 2026-07-15 local execution receipt

- Focused action gate: 7/7 passed.
- Database prompt gate: 5/5 passed.
- Connector aggregate query gate: 11/11 passed.
- Platform API full library suite: 2898 passed, 0 failed, 2 ignored, 2900 discovered.
- The full suite included 182 PostgreSQL-backed tests that returned early because the configured database was not disposable, plus 2 mock-gateway tests that returned early because their mock URLs were unset. They are not live integration evidence.
- Live-capture self-test passed with receipt `target/newbai-customer-answer-live-capture/20260715110346706-18496.json`; no network/provider call was made and the receipt remains diagnostic-only and non-decision-eligible.
- Official offline wrappers passed: assistant chat contract, external direct reply, retrieval quality baseline, and NewBai customer answer.
- `external-report-focus` self-test passed 7 report controls and 17 ordinary guards, but it is only a JavaScript fixture/oracle self-test and is not production Rust route or end-to-end evidence.
- No provider call, live write, disposable PostgreSQL run, commit, push, deployment, flag change, service restart, or server access was performed.

Open gates remain the disposable PostgreSQL integration run, a controlled real-provider answer-quality run, persisted per-column type/unit/additivity/allowed-aggregation contracts, and broader claim-level traceability. The direct image structured-extraction path remains a read-only evidence preprocessing path outside this mutating artifact-action P0; this receipt does not claim that every provider/model preprocessing route is disabled.

## Deferred P1: Persist per-column semantic contracts

After P0 is green, independently design a backward-compatible profile contract that persists data type, semantic role, unit status, additivity, allowed aggregations, confidence, and source for each mapped column. Re-profiled sources may then safely recover aggregates that P0 conservatively omits. This is not required to stop the current three proven failures.
