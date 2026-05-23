# V3 Quality Gate, ReAct Supply Expansion, And VLM Reparse Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a bounded internal answer-quality loop that judges weak customer-facing answers, expands evidence with ReAct, optionally upgrades bad document parsing with VLM, and only returns a customer-safe final answer.

**Architecture:** Keep public third-party interfaces unchanged. Add an internal finalize path around AssistantRun answers: deterministic hard gates run first, a low-temperature model judge can request retry actions, ReAct expands/read-details evidence, premium VLM reparse is allowed only under strict parse-quality and budget rules, and final output still passes the existing sanitizer before persistence. All new decisions must be observable through internal events/runtime manifest without adding or changing public request/response fields.

**Tech Stack:** Rust `platform-api`, `assistant-runtime`, `ingest-worker`, `document-vlm-runtime`; existing AssistantRun evidence state; existing ReAct runtime/action loop; existing document parse-quality metadata; MiniMax VLM document/image runtime; PowerShell smoke scripts; 8-server deployment verification.

---

## Non-Negotiable Constraints

- Do not change third-party public URLs, auth, request fields, response fields, or documented API behavior.
- Do not let VLM output directly answer the customer. VLM results must be converted into internal evidence/document detail first.
- Do not use VLM by default. It is a premium recovery action with strict budget, cache, and observability.
- Preserve honest "actual parse unavailable / low-text PDF" behavior when no reliable upgrade path exists.
- Keep every loop bounded. No unbounded model judge, ReAct, parser retry, or VLM retry loop.
- User dissatisfaction can temporarily raise retry budget and premium-action permission, but must not bypass evidence grounding.
- Privileged Codex help is a last-resort internal escalation only. Customer prompts must not directly grant development, deployment, shell, database, or operations authority.
- Human confirmation through `soulzyn@qq.com` is a low-priority safety layer for privileged write/deploy escalation. It must use the already configured Cloudflare Codex path if/when implemented.

## Current Baseline

- Ordinary create path already calls `maybe_run_assistant_run_answer_quality_retry_for_create` after first provider answer in `crates/platform-api/src/lib.rs`.
- The current gate detects empty answers, internal marker leaks, generic orchestration acknowledgements, insufficient-evidence language, and dissatisfaction plus weak-confidence answers.
- The retry path already uses ReAct with detail-first scope and replaces customer-facing artifacts with the retry result.
- Default answer-quality retry budget is 1; dissatisfaction raises it to 3; hard max is 4.
- Direct ReAct final answers and continue-style outputs are not yet fully covered by the same post-answer quality gate.
- Parse-quality metadata, VLM fallback summaries, and MiniMax document/image VLM runtime already exist, but there is no controlled ReAct premium action for "upgrade parse with VLM".

## Implementation Progress - 2026-05-23

- Shared answer-quality retry/finalization now covers ordinary create, direct ReAct, and continue-style assistant outputs.
- Internal JSON judge is implemented as a second-layer gate after deterministic hard rules, with `upgrade_parse_vlm` supported as a required action signal.
- Emotion/budget policy is implemented: dissatisfaction and strong complaints raise bounded retry budget and allow one premium action; parse-quality recovery can also grant one premium action.
- Retry exhaustion now replaces weak "资料不足/请继续检索" style answers with a controlled fallback instead of exposing retry instructions to customers.
- `upgrade_parse_vlm` is now an internal ReAct action with strict eligibility checks: selected visible document only, visual/PDF or VLM-recommended material, parse-quality recovery signal, premium budget remaining, and same-document duplicate prevention.
- The VLM action first prefers cached VLM parse-quality evidence. If no cache exists, it only queues the existing `UploadIngest` reparse workflow when MiniMax image VLM env is configured; otherwise it returns an internal rejected observation (`vlm_runtime_unavailable`).
- Verified locally with:
  - `cargo test -p platform-api assistant_run_react --lib`
  - `cargo test -p platform-api assistant_run_answer_quality_gate --lib`
  - `cargo test -p platform-api assistant_run_answer_quality_judge --lib`
  - `cargo test -p platform-api assistant_run_document_parse_quality --lib`
  - `cargo test -p platform-api upgrade_parse_vlm --lib`
  - `cargo test -p document-vlm-runtime --lib`
- Quality smoke harness was extended with V3 answer-quality observability fields and wrapper aliases:
  - `scripts/run-document-quality-smoke.ps1`
  - `scripts/run-v3-quality-gate-smoke.ps1`
  - `fixtures/document-quality/smoke-cases.json`
  - `fixtures/document-quality/README.md`
- Full local quality smoke passed:
  - `.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case all`
  - Covered one-character PDF, DOC/DOCX "邓工是谁", resume company statistics, multi-dimensional resume ranking table, table-heavy documents, attendance date/work-hour formatting, scanned visual PDF fallback, smart-home dissatisfied-customer answers, and smart-elevator point-list table answers.

## Desired Loop

```text
candidate answer
  -> hard deterministic gate
  -> internal model judge when needed
  -> accept if grounded/customer-safe
  -> otherwise build retry plan
  -> ReAct cheap actions: retrieve_evidence / read_document_detail / scan rows
  -> optional premium action: VLM reparse of selected material/page(s)
  -> rebuild evidence
  -> regenerate final answer
  -> judge again
  -> optional internal privileged-Codex escalation for system/ops defects
  -> accept or controlled exhausted fallback
  -> final sanitizer
```

## Task 1: Pin The Current Gate Behavior With Tests

**Files:**

- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Add failing tests for missing coverage**

Add unit tests near the existing answer-quality gate tests:

- direct ReAct final answer with "资料不足/需要继续检索" must be rejected by shared finalization.
- continue/follow-up answer with weak "请继续/建议先检索" language must be rejected by shared finalization.
- retry budget exhaustion must not expose a weak "资料不足/继续检索" final answer when supplied evidence exists.
- dissatisfaction prompt should permit higher retry budget and one premium action flag.

Use existing test helpers and existing `assistant_run_answer_quality_retry_reason` style assertions first.

**Step 2: Run tests and verify they fail**

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate --lib
```

Expected: new tests fail because shared finalization and premium-action state do not exist yet.

**Step 3: Commit after implementation, not after failing tests**

Do not commit failing tests alone unless the implementation session explicitly wants a checkpoint branch.

## Task 2: Extract A Shared Internal Finalization Function

**Files:**

- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Introduce an internal finalization struct**

Create private structs/enums close to the current gate code:

```rust
struct AssistantRunAnswerFinalizationInput<'a> {
    request: &'a CreateAssistantRunRequest,
    run_id: Uuid,
    selected_scope: &'a Value,
    evidence_state: &'a Value,
    output_artifacts: Vec<AssistantRunOutputArtifact>,
    local_thread_id: Option<&'a str>,
    active_secret_binding_ids: &'a [Uuid],
    current_user_id: Uuid,
}

struct AssistantRunAnswerFinalizationOutcome {
    evidence_state: Value,
    runtime_manifest: Option<Value>,
    retry_trail_steps: Vec<AssistantRunReasoningStep>,
    output_artifacts: Vec<AssistantRunOutputArtifact>,
    retry_events: Vec<AssistantRunLifecycleEventInput>,
}
```

Keep these private to `platform-api`; do not expose them in API contracts.

**Step 2: Move existing retry orchestration behind the shared finalizer**

Refactor `maybe_run_assistant_run_answer_quality_retry_for_create` into a shared internal function that can be called from:

- ordinary provider create path;
- direct ReAct create path after ReAct returns final artifacts;
- continue/follow-up output path if it produces customer-facing assistant artifacts.

**Step 3: Preserve current sanitizer order**

Final flow must still call:

```rust
assistant_run_sanitize_customer_facing_output_artifacts(...)
```

after quality retry/finalization and before attach/persist.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p platform-api assistant_run_react --lib
```

Expected: existing tests remain green; direct ReAct/continue coverage starts using the same gate.

## Task 3: Add Internal Model Judge As A Second-Layer Gate

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify only if needed: `crates/assistant-runtime/src/lib.rs`

**Step 1: Keep hard rules first**

Hard rules remain deterministic and do not need the model:

- empty answer;
- raw parse/internal marker leak;
- generic orchestration acknowledgement;
- obvious insufficient-evidence wording;
- public-interface/security-sensitive leak;
- known actual-parse-unavailable honest answer.

**Step 2: Add judge request/response types**

Create private internal JSON schema:

```json
{
  "verdict": "accept | retry | controlled_fallback",
  "reason": "ok | insufficient_evidence | ungrounded | incomplete_task | parse_quality_insufficient | low_customer_confidence | unsafe_internal_leak",
  "confidence": 0.0,
  "customer_safe": true,
  "required_actions": ["retrieve_evidence", "read_document_detail"],
  "premium_action_allowed": false
}
```

The judge must be internal only. Do not attach this exact JSON to public response artifacts.

**Step 3: Trigger judge only for high-risk cases**

Call the judge when any of these are true:

- request expresses dissatisfaction;
- answer has uncertainty/hedging language but no hard-rule hit;
- prompt asks for table/statistics/ranking/entity lookup;
- supply quality indicates fallback chunks, low-text parse, or degraded parse;
- answer is very short relative to a structured request.

**Step 4: Use a strict prompt**

Judge prompt rules:

- output JSON only;
- judge answer quality, do not answer the user;
- compare user task, evidence summary, parse status, and candidate answer;
- if evidence exists but answer refuses, return `retry`;
- if parse quality blocks answer and VLM is a valid recovery, return `retry` with `upgrade_parse_vlm`;
- if truly unanswerable after attempts, return `controlled_fallback`.

**Step 5: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate --lib
```

Expected: hard-rule tests do not require model calls; judge parsing tests can use static JSON parser fixtures.

## Task 4: Add Budget And Emotion Policy

**Files:**

- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Replace single retry count with a budget policy**

Represent internal budget as:

```rust
struct AssistantRunQualityBudget {
    answer_retry_budget: usize,
    react_step_budget: usize,
    premium_action_budget: usize,
    reason: &'static str,
}
```

Suggested policy:

- normal: `answer_retry_budget=1`, `premium_action_budget=0`.
- parse degraded plus document-dependent prompt: `answer_retry_budget=2`, `premium_action_budget=1` only if VLM is configured and cheap actions fail first.
- dissatisfied: `answer_retry_budget=3`, `premium_action_budget=1`.
- strong complaint: `answer_retry_budget=4`, `premium_action_budget=1`, broader document scan allowed.

Keep hard cap from env, but do not let env create unbounded loops.

**Step 2: Extend dissatisfaction detection carefully**

Use existing `assistant_run_request_expresses_dissatisfaction` and add stronger categories:

- weak dissatisfaction: "不满意", "观感差", "客户有些不满意";
- strong complaint: "投诉", "客户很不满意", "严重错误", "反复答错", "线上事故".

**Step 3: Emit internal budget event**

Add internal lifecycle event:

```text
assistant_run.answer_quality_gate.budget_selected
```

Payload includes reason, retry budget, premium budget, and dissatisfaction category. Do not expose new public response fields.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate_increases_budget_for_dissatisfaction --lib
```

Expected: existing dissatisfaction test still passes; new strong-complaint test gets larger bounded budget.

## Task 5: Add Controlled Premium ReAct Action For VLM Reparse

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/document-vlm-runtime/src/lib.rs` only if the existing runtime lacks the required request shape.
- Modify: `crates/ingest-worker/src/main.rs` only if upgraded parse persistence cannot be requested from platform-api with existing paths.

**Step 1: Define action eligibility**

`upgrade_parse_vlm` is eligible only when all are true:

- selected material is PDF/image/visual document or parse-quality metadata recommends VLM;
- current evidence is low-text, degraded, missing table structure, or semantically insufficient;
- user question depends on document content;
- cheap actions have already run or judge explicitly requests parse upgrade;
- premium budget remains;
- same document has not already been VLM-upgraded in this answer-quality loop.

**Step 2: Add action to ReAct action catalog**

Add an internal ReAct action name such as:

```text
upgrade_parse_vlm
```

Allowed parameters should be internal and minimal:

```json
{
  "document_id": "...",
  "reason": "low_text_coverage | missing_table_structure | visual_document_needed",
  "page_hint": [1, 2],
  "question_focus": "..."
}
```

Do not expose this action in public API schemas.

**Step 3: Implement action executor**

In `execute_assistant_run_react_action`, implement:

- validate eligibility and premium budget;
- prefer cached VLM parse/summary if available;
- select relevant pages when page hints exist;
- call existing document VLM runtime or enqueue existing parse fallback path;
- persist/attach upgraded evidence through existing document detail/evidence state contracts;
- return an observation summary, not a customer answer.

**Step 4: Add guardrails**

The action must refuse with an internal observation when:

- document is too large and no page focus exists;
- VLM runtime is unavailable;
- premium budget exhausted;
- document already upgraded in this loop;
- parse quality is already usable and the issue is answer synthesis, not parsing.

**Step 5: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_react --lib
cargo test -p platform-api assistant_run_document_parse_quality --lib
cargo test -p document-vlm-runtime --lib
```

Expected: ReAct can request VLM upgrade only under controlled conditions; VLM result enters evidence before answer generation.

## Task 6: Improve Retry Exhaustion Behavior

**Files:**

- Modify: `crates/platform-api/src/lib.rs`

**Step 1: Track best candidate answer**

During retries, store:

- last accepted-by-hard-rules answer;
- judge verdict and confidence;
- evidence count and parse status at that attempt;
- whether answer contains customer-hostile uncertainty language.

**Step 2: Add controlled fallback generator**

If budget is exhausted and final candidate is still weak:

- if enough evidence exists, generate a concise grounded partial answer using available evidence;
- if parse remains truly unavailable, return controlled parse-state explanation;
- never output "需要先检索/请继续/资料不足" as the main customer answer when internal retry was possible.

**Step 3: Add event**

Emit:

```text
assistant_run.answer_quality_gate.exhausted_controlled_fallback
```

Payload includes reason, attempts, premium actions used, and final fallback class.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate --lib
```

Expected: exhaustion tests do not expose weak retry language.

## Task 7: Add Observability And Smoke Assertions

**Files:**

- Modify or create: `scripts/run-document-quality-smoke.ps1`
- Create if clearer: `scripts/run-v3-quality-gate-smoke.ps1`
- Modify: `fixtures/document-quality/README.md`

**Step 1: Extend smoke output**

Smoke scripts should print:

- original parse status and parse-quality status;
- quality gate triggered reason;
- retry attempts;
- ReAct actions used;
- premium action used or skipped reason;
- final sanitizer leak check;
- final answer excerpt.

**Step 2: Add smoke labels**

Required smoke cases:

- one-character PDF;
- DOC/DOCX asks "邓工是谁";
- resume company-name statistics;
- multi-dimensional resume ranking table;
- attendance missing/longest/shortest work-hour query with normalized dates;
- smart home / smart elevator recent customer-feedback documents.

**Step 3: Add failure markers**

Smoke should fail when final answer contains:

- raw internal parse identifiers;
- "需要先检索", "请继续", "无法回答" when supplied evidence exists;
- date formats outside the expected normalized style for attendance output;
- missing table output for ranking/table prompts.

**Step 4: Verify locally and on 8 server**

Run local targeted tests first, then 8-server smoke after deployment:

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p platform-api assistant_run_react --lib
.\scripts\run-v3-quality-gate-smoke.ps1 -BaseUrl <8-server-url> -Case attendance_final
.\scripts\run-v3-quality-gate-smoke.ps1 -BaseUrl <8-server-url> -Case low_text_pdf_final
.\scripts\run-v3-quality-gate-smoke.ps1 -BaseUrl <8-server-url> -Case resume_ranking_table
```

Expected: final answers are customer-safe, evidence-grounded, and no leak markers appear.

## Task 8: Deployment Sequence

**Files:**

- No code files beyond prior tasks.
- Update deployment notes only if the existing repo convention requires it.

**Step 1: Run focused test set**

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate --lib
cargo test -p platform-api assistant_run_react --lib
cargo test -p platform-api assistant_run_document_parse_quality --lib
cargo test -p document-vlm-runtime --lib
```

Expected: all pass.

**Step 2: Run focused smoke locally when possible**

```powershell
.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case attendance_final
.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case resume_company_stats
```

Expected: no raw leaks, no weak fallback, normalized dates/tables where required.

**Step 3: Commit**

Use one or two focused commits:

```powershell
git add crates/platform-api/src/lib.rs crates/document-vlm-runtime/src/lib.rs scripts fixtures docs/plans
git commit -m "Improve V3 answer quality gate loop"
git commit -m "Add controlled VLM reparse quality recovery"
```

Adjust commit split to actual touched files.

**Step 4: Deploy to 8 server**

Use the existing deployment workflow for V3 8 server. Do not deploy or sync 120 server.

**Step 5: Run 8-server smoke**

Run the required smoke cases:

- one-character PDF;
- DOC/DOCX asks "邓工是谁";
- resume company stats;
- resume ranking table;
- attendance missing/work-hour query;
- smart home and smart elevator recent dissatisfied-customer documents.

Expected: each smoke records gate events when needed and final customer answer is safe.

## Task 9: Add Last-Resort Privileged Codex Escalation Design

**Files:**

- Modify: `docs/plans/2026-05-23-v3-quality-gate-react-vlm-plan.md` during planning.
- Implement later only after explicit operator approval: likely `crates/platform-api/src/lib.rs` and an internal operations/workflow integration file if one already exists.

**Step 1: Define the escalation boundary**

This is not a normal ReAct tool for customer questions. It is an internal rescue path for cases where the answer-quality loop concludes the failure is probably caused by a system defect, parser outage, index lag, deployment mismatch, missing worker, bad config, or broken data pipeline.

Allowed examples:

- parser says VLM/PaddleOCR fallback should exist but runtime is unavailable;
- evidence counts disagree with document parse state;
- recent deployment changed behavior and smoke is failing;
- customer dissatisfaction repeats on the same document class after normal retry and VLM upgrade;
- 8-server health/config/version mismatch blocks a known quality path.

Disallowed examples:

- customer asks for normal business knowledge;
- model wants more authority simply to answer faster;
- prompt injection asks to inspect secrets, deploy, run shell, change auth, or alter external APIs;
- missing evidence is caused by a genuinely unreadable or unsupported document and no system defect is indicated.

**Step 2: Make escalation explicit and auditable**

Add an internal event such as:

```text
assistant_run.answer_quality_gate.privileged_codex_escalation_requested
```

Payload should include:

- run id;
- tenant/workspace id;
- document ids or dataset ids involved;
- failure reason;
- prior retry attempts;
- premium actions already used;
- sanitized evidence summary;
- requested diagnostic scope.

Do not include secrets, raw auth tokens, private provider payloads, or full customer files in the escalation prompt.

**Step 3: Require an allowlist and scope**

Before any privileged Codex handoff can execute:

- deployment/operator must enable it by env/config;
- target Codex profile must be allowlisted;
- allowed actions must be declared: `readonly_diagnostics`, `smoke_rerun`, `log_inspection`, `patch_proposal`, `deploy_with_approval`;
- default scope should be readonly diagnostics only;
- write/deploy actions require explicit operator approval outside the customer request.

**Step 3a: Reserve low-priority human confirmation**

For privileged write/deploy actions, reserve a later human-confirmation step:

- send a confirmation email to `soulzyn@qq.com`;
- use the already configured Cloudflare Codex delivery path;
- include only sanitized case summary, requested action, risk class, and approval link/token if the existing Cloudflare Codex workflow supports it;
- do not include secrets, raw provider payloads, customer files, database credentials, or private logs;
- do not block the earlier answer-quality, ReAct, budget, exhaustion, judge, or VLM work on this email-confirmation feature.

This is lower priority than the core quality loop. Implement it only after the preceding quality optimizations are stable.

**Step 4: Define the handoff package**

Generate a compact internal handoff:

```json
{
  "task": "diagnose_v3_answer_quality_failure",
  "case": "attendance_date_format | low_text_pdf | resume_ranking | smart_home | smart_elevator | other",
  "customer_safe_summary": "...",
  "run_id": "...",
  "evidence_state_summary": {},
  "quality_gate_events": [],
  "parse_quality": {},
  "react_actions": [],
  "requested_scope": "readonly_diagnostics"
}
```

The receiving Codex should answer with one of:

- `diagnosis_only`;
- `patch_plan`;
- `operator_action_required`;
- `known_unanswerable`.

It must not directly change public interfaces or deploy unless the configured scope and operator approval allow it.

**Step 5: Add controlled fallback when escalation is pending**

Customer-facing output should not say "I asked a privileged Codex". It should return a concise controlled response:

- answer the grounded part if possible;
- say the system is still processing/validating the material only when true;
- avoid exposing internal operations or implementation details.

**Step 6: Verify**

Add tests that assert:

- customer prompt cannot directly trigger privileged actions;
- escalation is skipped when allowlist/config is absent;
- escalation payload is sanitized;
- repeated dissatisfied quality failures can request readonly escalation;
- public response fields remain unchanged.

Run:

```powershell
cargo test -p platform-api assistant_run_answer_quality_gate --lib
```

Expected: escalation is internal, bounded, sanitized, and not customer-controlled.

## Suggested Implementation Order

1. Shared finalization coverage for create/direct-ReAct/continue.
2. Budget and emotion policy.
3. Retry exhaustion controlled fallback.
4. Internal model judge JSON gate.
5. Premium VLM reparse ReAct action.
6. Last-resort privileged Codex escalation design and tests.
7. Smoke script assertions and 8-server validation.
8. Low-priority human confirmation email via configured Cloudflare Codex for privileged write/deploy escalation.

This order reduces customer-visible bad answers before adding the expensive VLM path.

## Handoff Prompt

```text
继续 V3 质量门禁专项。按 docs/plans/2026-05-23-v3-quality-gate-react-vlm-plan.md 执行。
不要改第三方公开接口、URL、鉴权、请求/响应字段；如必须改先问。
目标：把答案后置质量门禁升级成统一 finalize 流程，覆盖普通 create、直接 ReAct、continue；加入内部模型 judge；ReAct 可按预算扩供料；在解析明显低质且客户问题依赖材料内容时，允许一次受控 VLM 升级解析；客户表达不满时临时提升预算；重试耗尽时不要把“资料不足/请继续检索”直接输出给客户。
最后一级可设计“向具备开发权限和本系统运维权限的 Codex 求助”的内部升级通道，但只能在系统/运维/解析链路疑似故障时触发，必须 allowlist、只读优先、审计、脱敏，写入/部署需要明确 operator approval，客户提示词不能直接获得该权限。
低优先级补充：高权限写入/部署升级可后续加入人类确认邮件，收件人 `soulzyn@qq.com`，使用已配置的 Cloudflare Codex 通道；不要阻塞前面的质量门禁、ReAct、VLM 和 smoke 优化。
优先 smoke：一字 PDF、doc 问“邓工是谁”、简历公司名统计、多维简历排序出表、考勤缺勤/工时长短日期格式、智能家居/智能梯控近期不满意材料。
```
