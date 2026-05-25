# V3 Parsing And Answer Quality Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve V3 document parsing quality, dataset understanding, retrieval supply, and model-authored answer reliability without changing third-party public interfaces or the stable UI shell.

**Architecture:** This is a dedicated quality thread parallel to the feature thread. It owns parser quality detection, PaddleOCR/MiniMax fallback policy, document structure extraction, entity/noun-term extraction, retrieval/ranking evidence supply, and answer-quality guardrails. The feature thread continues database sources, third-party integrations, static-page/report artifacts, observability, deployment, and public docs; both threads communicate only through stable internal supply contracts and tests.

**Tech Stack:** Rust `platform-api`, `assistant-runtime`, `storage`, `ingest-worker`, `retrieval-worker`, `media-worker`, `document-vlm-runtime`; PostgreSQL; PaddleOCR PP-StructureV3 sidecar; MiniMax VLM/media lanes; focused shell smoke scripts; 8-server deployment. Do not deploy or sync 120 server.

---

## Thread Boundary

Quality thread owns:

- Parser status, parse quality score, retry/fallback state, and model-visible parser status supply.
- Low-quality PDF/OCR cases such as one-character extraction, empty chunks, tiny text ratio, garbled text, and missing tables.
- PaddleOCR structured parse as primary structured fallback where available.
- MiniMax VLM fallback for low-confidence visual documents.
- Section, heading, paragraph, table, figure, entity, noun-term, and resume-profile extraction.
- Retrieval/ranking quality, evidence selection, context budget, detail-first behavior, and direct-answer behavior.
- Answer reliability: no fixed "已收到指令..." fallback as normal output, bounded retries, timeout handling, honest partial/failed-state replies.
- Quality smoke cases: third-party doc "邓工是谁", resume company statistics, multi-dimension resume/document ranking, schema-only database understanding as supplied evidence only.

Feature thread continues:

- Third-party/public API shape, docs, observability pages, protected test UI, deployment.
- Database source P0/P1 integration, source registration, sync, schema/profile/aggregate APIs, dataset binding.
- Static-page/report/template artifact generation and render/export quality.
- Requested skills, template skills, output-format selection, and external workspace/conversation-scoped behavior.
- Incremental code decomposition around touched feature areas.

Shared contract:

- No quality-thread change may alter third-party request/response fields, auth, URL paths, or public docs without an explicit operator decision.
- Quality-thread outputs should enter AssistantRun through existing typed supply items: parser status, document detail, retrieval evidence, entity scan, resume profile, database schema/aggregate, media context, and answer policy.
- Feature thread must not bypass these supply contracts with direct host-composed answers.

## Current Baseline

Known implemented pieces:

- Third-party document scope and temporary dataset membership exist.
- Parser status can be supplied to AssistantRun.
- PaddleOCR has been previously connected and is the preferred structured fallback.
- MiniMax VLM fallback exists for document/media slices but needs better quality gating and smoke coverage.
- Entity scan and resume-profile scan exist and already support some company/resume questions.
- Database schema/aggregate context is now supplied through dataset evidence and can drive static-page/report snapshots.
- Provider answer path is model-authored, but timeout/empty/suppressed-output behavior still needs tighter tests and retries.

Known gaps:

- Parse quality is still coarse; a parse can look "successful" while semantically unusable.
- Paragraph segmentation, heading hierarchy, table structure, and figure/table captions are not consistently exposed to retrieval.
- Noun-term extraction is still shallow and not yet general enough for enterprise dataset understanding.
- Resume dimensions need stronger generalization: time, skill, project, age, gender, company, school, city, certification, ranking, and table output.
- Direct answers still need a stronger evidence/no-evidence distinction, especially for third-party minimal-context callers.
- Smoke data and regression fixtures are scattered; quality needs a small repeatable fixture set.

## Implementation Status - 2026-05-25

- Done: the document-quality smoke harness and fixture manifest now exist under `scripts/run-document-quality-smoke.ps1`, `scripts/run-v3-quality-gate-smoke.ps1`, and `fixtures/document-quality/`. The manifest covers one-character PDF, DOC/DOCX "邓工是谁", resume company statistics, multi-dimension resume ranking, table-heavy documents, scanned/visual PDF fallback, attendance date/work-hour formatting, smart-home dissatisfaction, and smart-elevator point-list questions.
- Done: deterministic answer paths exist for several high-frequency cases: `spreadsheet_row_analysis` for attendance absence/work-hour/date questions, resume profile rows for company/ranking tables, selected-document detail for "邓工是谁", and queryable-fact snapshots where enabled.
- Done: answer-quality retry/ReAct/VLM recovery logic has a bounded implementation in `docs/plans/2026-05-23-v3-quality-gate-react-vlm-plan.md`, but it must remain careful: the gate previously blocked too many normal answers, so it should not be re-enabled as a hard customer-facing block until the smoke matrix proves low false positives.
- Current deployment: 8 server is on `c37d9638e`, with third-party group+document union scope deployed and services active.
- Current risk: local tests cover many deterministic cases, but the current deployed build still needs a fresh private 8-server smoke pass for the priority customer cases before the gate/retry policy is tightened again.
- Verification 2026-05-25: local priority smoke passed on HEAD `c37d963` with:
  - `.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case @('one_character_pdf','deng_engineer','resume_company_stats','resume_ranking_table','attendance_final','attendance_frequent','smart_home','smart_elevator')`
  - Report: `target/document-quality-smoke/document-quality-smoke-20260525T113758Z-22828.md`
  - Observed aggregate sources: `dataset_fact_snapshot` for resume company statistics, `dataset_entity_scan` for resume ranking and smart-elevator point lists, and `spreadsheet_row_analysis` for attendance.
  - No local priority case failed.
- Verification 2026-05-25 after scoped fact aggregation (`d125291`): the same local priority smoke passed again.
  - Report: `target/document-quality-smoke/document-quality-smoke-20260525T115211Z-29568.md`
  - Targeted tests also passed for scoped fact snapshot compaction and the existing fact snapshot direct-answer path.
- Blocker: no private `ServerCaseConfigPath` is currently present in the repo/worktree, so real 8-server customer-document smoke still needs the private case config before it can send safe scoped requests.

## Immediate Execution Queue - 2026-05-25

1. Re-run local smoke for the priority quality cases:
   - `one_character_pdf`
   - `deng_engineer`
   - `resume_company_stats`
   - `resume_ranking_table`
   - `attendance_final`
   - `attendance_frequent`
   - `smart_home`
   - `smart_elevator`
2. Re-run private 8-server smoke with the configured private case file and record:
   - final answer text;
   - `answer_supply_sources`;
   - `aggregate_answer_source`;
   - whether ReAct/quality-gate was triggered;
   - whether the answer used deterministic rows/facts instead of generic retrieval.
3. For failures, fix in this order:
   - wrong or missing deterministic supply;
   - bad date/time/table formatting;
   - missing document detail for selected-document questions;
   - fact snapshot not supplied for global cross-document statistics;
   - only then consider quality-gate/ReAct retry behavior.
4. Keep the customer-facing quality gate conservative:
   - do not block normal answers solely because they mention partial evidence;
   - route suspicious low-quality answers to passive collection and `answer_quality_autofix`;
   - only tighten blocking after repeated smoke passes with no normal-answer false positives.

## Phase 0: Quality Fixture And Smoke Harness

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Create or update: `scripts/run-document-quality-smoke.ps1`
- Create or update: `scripts/run-external-direct-reply-smoke.ps1`
- Create or update: `fixtures/document-quality/README.md`
- Create or update: `fixtures/document-quality/*`

**Step 1: Define the first quality fixture list**

Create `fixtures/document-quality/README.md` with at least:

- One-character/low-text PDF case.
- Third-party DOC/DOCX containing "邓工是谁".
- Resume set with company, skills, project time, age, gender, and ranking questions.
- Table-heavy PDF or DOCX.
- Scanned/visual PDF needing OCR/VLM fallback.

**Step 2: Add focused smoke script**

Add `scripts/run-document-quality-smoke.ps1` that can run local or 8-server checks and prints:

- upload/parse status;
- parse lifecycle;
- chunk count;
- section/table/entity counts;
- direct answer text;
- evidence/source refs;
- failure reason if not answerable.

**Step 3: Add regression tests around model-facing status**

Add tests that assert low-quality parse states are supplied to the model as partial/failed/retrying instead of silent success.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api document_parse_status_supply --lib
cargo test -p platform-api external_document_parse --lib
cargo test -p platform-api dataset_entity_scan --lib
```

Expected: existing tests pass; new tests pin exact missing quality behavior before implementation.

## Phase 1: Parse Quality Gate

**Files:**

- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs` only if parse-quality metadata cannot be stored in existing document metadata/parse detail.
- Test near changed parser/status code.

**Step 1: Add parse-quality classifier**

Classify parse output with signals:

- extracted text length;
- non-whitespace character count;
- chunk count;
- table count;
- section count;
- OCR confidence if available;
- visual-page ratio;
- repeated/garbled text ratio;
- one-character or empty extraction flag.

**Step 2: Store quality metadata**

Persist in parse detail metadata:

```json
{
  "parse_quality": {
    "status": "usable | partial | low_quality | failed",
    "score": 0.0,
    "reasons": ["too_few_characters"],
    "recommended_fallback": "paddleocr | minimax_vlm | retry | manual_review"
  }
}
```

**Step 3: Model-visible supply**

Ensure AssistantRun receives parse-quality metadata and instruction:

- If usable: answer with evidence.
- If partial: answer from available evidence and disclose partial parse.
- If low_quality/failed/retrying: explain current parsing state and avoid inventing document content.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api document_parse_status_supply --lib
cargo test -p platform-api external_document_parse --lib
```

Expected: one-character parse cannot appear as clean success to the answer planner.

## Phase 2: PaddleOCR And MiniMax Fallback Policy

**Files:**

- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/document-vlm-runtime/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test parser fallback policy near parser code.

**Step 1: Prefer PaddleOCR for structured visual documents**

When parse quality is low and the file is PDF/image-heavy:

- enqueue PaddleOCR structured parse when configured;
- preserve headings, tables, cells, coordinates, and page spans where available;
- mark lifecycle as `fallback_running`.

**Step 2: Use MiniMax VLM only when needed**

Use MiniMax VLM fallback when:

- PaddleOCR unavailable or failed;
- pages are visual/scanned and text extraction remains low quality;
- document asks for semantic visual understanding, not just OCR text.

**Step 3: Expose fallback state**

Model-visible parser supply must include:

- fallback provider;
- status;
- partial evidence count;
- retry/error reason;
- whether current answer can use partial content.

**Step 4: Verify**

Run targeted unit tests and one smoke script against the known bad PDF.

Expected: low-quality parse transitions to fallback or honest partial state.

## Phase 3: Document Structure And Term Extraction

**Files:**

- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/retrieval-worker/src/main.rs` if index payload needs richer structure.
- Test: entity/term/section scan tests in `platform-api`.

**Step 1: Normalize document blocks**

Emit structured blocks:

- heading;
- paragraph;
- table;
- table_row/table_cell;
- list;
- figure/caption;
- key-value section.

**Step 2: Add paragraph/section hierarchy**

Attach to chunks/evidence:

- section title path;
- heading level;
- page range;
- paragraph index;
- table id if inside table.

**Step 3: Strengthen entity and noun-term extraction**

Generalize beyond resumes:

- organization/company;
- person;
- role/title;
- project/product/system;
- location;
- date/time period;
- skill/technology;
- metric/amount/status;
- policy/risk/action item.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api dataset_entity_scan --lib
cargo test -p platform-api assistant_run_general_entity_scan --lib
```

Expected: document dimension questions use structured rows rather than raw candidate terms.

## Phase 4: Retrieval And Answer Quality

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Modify: `crates/retrieval-worker/src/main.rs` only if ranking payload changes.
- Test near AssistantRun and retrieval supply.

**Step 1: Detail-first retrieval for document questions**

For prompts asking "who/what/which/how many/rank/table":

- prefer document detail evidence;
- include parser status;
- include section/table/entity rows;
- avoid relying only on generic chunk excerpts.

**Step 2: Evidence/no-evidence answer policy**

Provider input must separate:

- supplied citable evidence;
- parser status;
- unavailable/failed/retrying state;
- general model knowledge.

**Step 3: Remove fixed fallback as normal answer**

If model provider times out or returns empty:

- retry within bounded budget;
- if still failed, return a failure/partial message that reflects actual state;
- never normal-output the old "已收到指令..." as if the answer succeeded.

**Step 4: Verify**

Run:

```powershell
cargo test -p platform-api assistant_run_resume --lib
cargo test -p platform-api assistant_run_general_entity_scan --lib
cargo test -p platform-api external_channel --lib
```

Expected: no orchestration-copy direct answer for normal third-party questions.

## Phase 5: Resume And Dataset Dimension Answers

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/assistant-runtime/src/lib.rs`
- Test: resume/entity scan tests.

**Step 1: Expand dimension detectors**

Support dimensions:

- company;
- project;
- skill;
- time range;
- age;
- gender;
- education/school;
- city/location;
- certificate;
- title/role;
- availability/status.

**Step 2: Support sorted table answers**

For "按 X 排序/统计/出表":

- build structured rows;
- include count/rank/order;
- preserve document refs;
- let model author final text/table.

**Step 3: Generalize beyond resumes**

Keep the same dimension/query pattern for enterprise documents, contracts, policies, databases-as-documents, and media transcripts.

**Step 4: Verify**

Run resume and general dataset smoke cases.

Expected: company/statistics/ranking answers are stable and cite visible evidence.

## Parallel Thread Handoff Prompt

Use this prompt to start the dedicated quality thread:

```text
继续 V3 解析质量/回答质量专项。只按 docs/plans/2026-05-22-v3-parsing-answer-quality-plan.md 执行。
不要改第三方公开接口、URL、鉴权、请求/响应字段；如必须改先问。
目标：低质量解析不再当成功，PaddleOCR 优先结构化兜底，MiniMax VLM 兜底，段落/标题/表格/实体/名词术语增强，检索和回答质量提升，第三方普通问题必须模型直接回答且不能用固定“已收到指令...”兜底。
优先 smoke：一字 PDF、doc 问“邓工是谁”、简历公司名统计、多维简历排序出表。
```

## Feature Thread Handoff Prompt

Keep this current thread on non-quality feature work:

```text
继续 V3 功能主线。解析质量/回答质量专项已拆到 docs/plans/2026-05-22-v3-parsing-answer-quality-plan.md，不在本线程展开。
本线程继续数据库源、第三方对接、模板 skill、静态页/报表产物、观测页、部署和文档。不要改第三方公开接口；必要时先问。
```
