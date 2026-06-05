# DataMax Model-Visible Capability Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the loop between DataMax platform capabilities, model-visible capability awareness, Host-controlled execution, customer-facing status, proactive message-channel outreach, and regression smoke.

**Architecture:** DataMax remains the system of record for documents, datasets, permissions, AssistantRun state, workflow state, generated artifacts, third-party contracts, outbound message actions, and audit events. Models may only request product-level capabilities through a bounded `<V3_TOOL_REQUEST>` protocol; DataMax Host parses, validates, routes, records, and returns stable text/task/card/artifact responses. Low-level tools such as retrieval, document detail reads, VLM reparse, Codex Host execution, web search, permission resolution, message sending, and raw connector calls stay Host-owned and are never exposed as direct third-party powers.

**Tech Stack:** Rust `platform-api`, `ingest-worker`, `retrieval-worker`, `codex-host-agent`, `workflow-definitions`; PostgreSQL AssistantRun/workflow/document state; third-party SSE/status cards; Cloudflare Codex/Image2 fixed tasks; docs and 8-server private smoke scripts.

---

## Current Baseline

- Main ReAct already exposes internal actions such as `retrieve_evidence`, `read_document_detail`, `upgrade_parse_vlm`, static-page draft/render actions, report actions, and `codex_host_task`.
- Third-party direct model prompting currently exposes only one product-level capability: `static_page_artifact`.
- The third-party direct model can output `<V3_TOOL_REQUEST>{"tool":"static_page_artifact",...}</V3_TOOL_REQUEST>`, and DataMax intercepts it before customer delivery.
- Data ingestion / building tables / staging plans already has a fixed Codex task, safe card/status response, operator confirmation routes, and sync-status restoration, but it is triggered mostly by deterministic keywords.
- Document parse status, degraded parse detection, auto-reparse metadata, and ReAct `upgrade_parse_vlm` exist. VLM reparse is budget-gated, visible-document-gated, and can queue an upload-ingest reparse workflow when configured.
- `answer_quality_autofix` exists as an asynchronous fixed task. It must remain passive and must not become a customer-facing hard quality gate.
- Background document enrichment and canonical dedup are planned but not fully implemented as a single background capability.
- Proactive message-channel outreach is a desired system capability: DataMax may need to notify a user, ask a follow-up, report task completion, request confirmation, or start a new conversation through an already configured channel. This is not yet closed as a model-visible capability.

## Non-Negotiable Guardrails

- Do not change third-party public URLs, auth, request fields, or response fields. Additive card/status metadata is allowed only when existing clients can ignore it.
- Do not expose low-level tool names, raw provider payloads, raw stdout/stderr, database URLs, credentials, full table dumps, full documents, source cursors, or private filesystem paths to the model or customer.
- Do not let the model expand document, dataset, database-source, user, tenant, or conversation permissions. Scope is calculated and enforced by DataMax only.
- Do not auto-write production data, migrate schema, overwrite stable customer URLs, change auth, or alter public integration contracts through a model request.
- Do not use VLM by default. VLM/deep reparse is premium recovery only, controlled by parse quality, budget, visible-document scope, active-workflow checks, and operator/runtime configuration.
- Do not re-enable a hard customer-facing quality gate. Low-quality answer handling remains retry/controlled-fallback/passive-autofix unless a separate regression-proven gate is approved.
- Do not let a model send proactive messages directly. Outbound messages require a configured channel, allowed recipient scope, idempotency key, rate limit, audit event, and a clear reason. New recipients, cross-channel delivery, sensitive content, or marketing-like outreach require confirmation.

## Capability Policy

### Product-Level Capabilities The Model May See

| Capability | Purpose | Default Action | Confirmation Policy |
| --- | --- | --- | --- |
| `static_page_artifact` | Static page, visualization report, dashboard, mobile report, published page link, style/template revision | Queue or reuse DataMax static-page pipeline and return task/status/artifact link | No confirmation for new generated artifacts; confirmation for overwrite/stable URL replacement/scope expansion |
| `data_ingestion_analysis` | Data ingestion, table creation analysis, field mapping, cleaning, schema/profile/staging plan | Queue existing fixed Codex task and return `data_ingestion_analysis_*` status/card | No production writes; staging plan needs operator confirmation before sync/import |
| `document_processing` | Document parse status, document入库状态, deep parse/reparse request, low-quality parse recovery | Return parse-status summary or queue gated reparse when safe | VLM/reparse requires Host gates; otherwise return review/needs-input status |
| `collection_setup_analysis` | Collection/crawling setup, source采集规划, public-source or configured-source collection request | Produce analysis/confirmation card only in first phase | Any new source, login state, credential, external write, or crawler execution needs confirmation |
| `integration_setup_analysis` | Third-party system, database, user/permission, document library, connector对接 planning | Produce analysis/confirmation card or data-ingestion route when source is selected | Any API/auth/schema/permission change needs confirmation |
| `message_channel_outreach` | Proactively send or start a conversation through an already configured message channel for task completion, follow-up, confirmation, or operator/customer notification | Create an outbound message action or confirmation card; same-conversation task notices may be automatic when channel policy allows | New recipients, cross-channel messages, sensitive/permissioned content, or external publish require confirmation |

### Host-Only Capabilities

These stay invisible as direct third-party model-requested tools:

- `retrieve_evidence`
- `read_document_detail`
- `upgrade_parse_vlm`
- `recall_conversation_memory`
- `web_search`
- `openclaw_memory_recall`
- `openclaw_readonly_execution`
- `codex_host_task`
- raw connector/API calls
- raw message sending / conversation creation
- permission grants, permission expansion, credential operations

### Model Responsibility, Not A Tool

`answer_composition` is the model's job, not a tool. Once DataMax supplies scoped evidence, facts, statuses, and task results, the model should organize a fluent, useful, customer-ready answer. It must not expose internal tool traces, raw JSON, or "system will analyze later" as a final answer.

## Closed-Loop Definition

A capability is closed only when all of these exist:

1. Model-facing capability guidance with clear trigger and non-trigger examples.
2. Strict parser for the internal `<V3_TOOL_REQUEST>` line.
3. Host preflight validation for scope, capability, allowlist, confirmation policy, and runtime readiness.
4. Deterministic dispatch to an existing DataMax workflow, fixed task, status card, or confirmation card.
5. Customer-facing reply shape using existing `reply.text`, `reply.task_status`, `reply.card`, `reply.artifact_links`, `requires_confirmation`, `action_id`, and `confirmation_id`.
6. AssistantRun/workflow events for observability.
7. Unit tests, negative tests, and a small real-prompt regression corpus.
8. 8-server private smoke for the enabled capabilities.

---

### Task 1: Add A Product-Level Capability Catalog And Generic Tool Request Parser

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing parser tests**

Add tests near the existing external-channel static-page tests:

```rust
#[test]
fn external_channel_model_tool_request_parses_product_capabilities() {
    for tool in [
        "static_page_artifact",
        "data_ingestion_analysis",
        "document_processing",
        "collection_setup_analysis",
        "integration_setup_analysis",
        "message_channel_outreach",
    ] {
        let raw = format!(
            r#"<V3_TOOL_REQUEST>{{"tool":"{tool}","intent":"create_or_update","reason":"用户请求平台执行"}}</V3_TOOL_REQUEST>"#
        );
        let request = external_channel_model_tool_request(&raw).expect("request should parse");
        assert_eq!(request.tool.as_str(), tool);
    }
}

#[test]
fn external_channel_model_tool_request_rejects_unknown_or_malformed_tools() {
    assert!(external_channel_model_tool_request(
        r#"<V3_TOOL_REQUEST>{"tool":"codex_host_task","intent":"run"}</V3_TOOL_REQUEST>"#
    ).is_none());
    assert!(external_channel_model_tool_request(
        r#"<V3_TOOL_REQUEST>{"tool":"static_page_artifact"</V3_TOOL_REQUEST>"#
    ).is_none());
}
```

**Step 2: Run the focused failing test**

Run:

```powershell
cargo test -p platform-api external_channel_model_tool_request --lib
```

Expected: fail because only `external_channel_model_static_page_tool_request` exists.

**Step 3: Implement the generic parser**

Replace the static-page-only parser with a product-level parser:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExternalChannelModelToolCapability {
    StaticPageArtifact,
    DataIngestionAnalysis,
    DocumentProcessing,
    CollectionSetupAnalysis,
    IntegrationSetupAnalysis,
    MessageChannelOutreach,
}

impl ExternalChannelModelToolCapability {
    fn from_wire(value: &str) -> Option<Self> {
        match value.trim() {
            "static_page_artifact" | "static-page-artifact" | "static_page" | "report_artifact" | "dashboard_artifact" => Some(Self::StaticPageArtifact),
            "data_ingestion_analysis" | "data-ingestion-analysis" | "data_ingestion" | "database_ingestion_analysis" => Some(Self::DataIngestionAnalysis),
            "document_processing" | "document-processing" | "document_parse" | "document_reparse" => Some(Self::DocumentProcessing),
            "collection_setup_analysis" | "collection-setup-analysis" | "collection_setup" | "crawler_setup_analysis" => Some(Self::CollectionSetupAnalysis),
            "integration_setup_analysis" | "integration-setup-analysis" | "integration_setup" | "connector_setup_analysis" => Some(Self::IntegrationSetupAnalysis),
            "message_channel_outreach" | "message-channel-outreach" | "outbound_message" | "start_conversation" | "proactive_message" => Some(Self::MessageChannelOutreach),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::StaticPageArtifact => "static_page_artifact",
            Self::DataIngestionAnalysis => "data_ingestion_analysis",
            Self::DocumentProcessing => "document_processing",
            Self::CollectionSetupAnalysis => "collection_setup_analysis",
            Self::IntegrationSetupAnalysis => "integration_setup_analysis",
            Self::MessageChannelOutreach => "message_channel_outreach",
        }
    }
}

#[derive(Clone, Debug)]
struct ExternalChannelModelToolRequest {
    tool: ExternalChannelModelToolCapability,
    payload: Value,
}
```

`external_channel_model_tool_request(output_text: &str)` should:

- read only the first `<V3_TOOL_REQUEST>...</V3_TOOL_REQUEST>`;
- parse JSON only;
- require a recognized `tool` / `tool_name` / `toolName`;
- return canonical tool identity plus sanitized payload;
- reject unknown tools instead of falling back to natural execution.

**Step 4: Keep the old static-page behavior through compatibility**

Either remove the old function or make it call the generic parser and filter `StaticPageArtifact`. Do not change public response fields.

**Step 5: Run tests**

Run:

```powershell
cargo test -p platform-api external_channel_model_tool_request --lib
cargo test -p platform-api external_channel_static_page --lib
```

Expected: pass.

**Step 6: Commit**

Commit after this task if executing in isolation:

```powershell
git add crates/platform-api/src/lib.rs
git commit -m "Generalize external model tool requests"
```

---

### Task 2: Centralize Model-Facing Capability Guidance

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing prompt tests**

Add tests proving the third-party direct prompt includes product capabilities and excludes Host-only raw powers:

```rust
#[test]
fn external_channel_provider_input_exposes_product_capability_catalog() {
    let mut message = sample_external_bot_message();
    message.text = Some("帮我把这些资料做成经营报表，也可以后续接数据库".to_string());
    let request = external_bot_message_to_assistant_run_request("generic-chat-main", &message);
    let provider_input = build_assistant_run_provider_input_with_evidence(
        &request,
        Some(&json!({"status":"not_requested"})),
    );

    assert!(provider_input.contains("static_page_artifact"));
    assert!(provider_input.contains("data_ingestion_analysis"));
    assert!(provider_input.contains("document_processing"));
    assert!(provider_input.contains("collection_setup_analysis"));
    assert!(provider_input.contains("integration_setup_analysis"));
    assert!(provider_input.contains("message_channel_outreach"));
    assert!(provider_input.contains("<V3_TOOL_REQUEST>"));
    assert!(!provider_input.contains("\"tool\":\"codex_host_task\""));
    assert!(!provider_input.contains("\"tool\":\"send_message\""));
}
```

**Step 2: Add a helper for capability guidance**

Add `external_channel_model_tool_capability_guidance_lines()` and call it inside `build_assistant_run_provider_input_with_evidence` only when `assistant_run_scope_is_external_channel(selected_scope)` is true.

The guidance must say:

- output exactly one internal tool-request line when a product capability is needed;
- do not mix final answer text with tool request text;
- use natural answer for ordinary Q&A and口径解释;
- `answer_composition` is model-authored natural language, not a tool;
- `document_processing`, `collection_setup_analysis`, `integration_setup_analysis`, and `message_channel_outreach` may return confirmation/status rather than immediate execution;
- proactive messages are requests to DataMax Host, not direct model-sent messages.

**Step 3: Run tests**

Run:

```powershell
cargo test -p platform-api external_channel_provider_input_exposes_product_capability_catalog --lib
cargo test -p platform-api assistant_run_provider_input_enforces_external_channel_direct_reply_contract --lib
```

Expected: pass.

**Step 4: Commit**

```powershell
git add crates/platform-api/src/lib.rs
git commit -m "Expose product capability catalog to external models"
```

---

### Task 3: Dispatch Static Page Requests Through The Generic Router

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write regression tests**

Keep existing static-page model-tool test and add a route-specific test:

```rust
#[test]
fn external_channel_model_tool_request_static_page_maps_to_artifact_mode() {
    let request = external_channel_model_tool_request(
        r#"<V3_TOOL_REQUEST>{"tool":"static_page_artifact","intent":"create_or_update","reason":"用户要求移动端暗黑报表"}</V3_TOOL_REQUEST>"#
    ).expect("tool request");
    assert_eq!(request.tool, ExternalChannelModelToolCapability::StaticPageArtifact);
    assert_eq!(request.payload["intent"], json!("create_or_update"));
}
```

**Step 2: Add a dispatch helper**

Add:

```rust
async fn external_channel_model_tool_request_reply(
    state: &AppState,
    connection_id: &str,
    connection: &ExternalChannelConnectionSummary,
    run_id: AssistantRunId,
    assistant_request: &CreateAssistantRunRequest,
    message: &ExternalBotMessageView,
    tool_request: &ExternalChannelModelToolRequest,
    now: DateTime<Utc>,
) -> std::result::Result<Option<ExternalBotReplyView>, ApiError>
```

For `StaticPageArtifact`, keep the current behavior:

- clone the message;
- set `artifact_type=static_page`;
- set `render_mode=artifact`;
- default `output_format=rich_text`;
- attach `model_tool_request` into selected scope;
- call `enrich_external_channel_database_source_scope`;
- call `maybe_enqueue_external_channel_static_page_pipeline`;
- return the task/status reply.

**Step 3: Replace direct static-page parser use**

In `external_channel_chat_model_or_acceptance_reply`, replace the static-only parser branch with:

```rust
if let Some(tool_request) = external_channel_model_tool_request(&output_text) {
    if let Some(reply) = external_channel_model_tool_request_reply(...).await? {
        return Ok(reply);
    }
}
```

**Step 4: Run tests**

Run:

```powershell
cargo test -p platform-api external_channel_model_tool_request_static_page --lib
cargo test -p platform-api external_channel_static_page --lib
```

Expected: pass, no static-page regression.

**Step 5: Commit**

```powershell
git add crates/platform-api/src/lib.rs
git commit -m "Route static page model tool requests generically"
```

---

### Task 4: Close The Data Ingestion / 建表 Capability Loop

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Write failing dispatch tests**

Add a test that model-requested data ingestion can bypass keyword-only detection while still using existing task/status contracts:

```rust
#[test]
fn external_channel_model_tool_request_data_ingestion_is_recognized() {
    let request = external_channel_model_tool_request(
        r#"<V3_TOOL_REQUEST>{"tool":"data_ingestion_analysis","intent":"create_staging_plan","reason":"用户要把当前资料整理成可查询业务库"}</V3_TOOL_REQUEST>"#
    ).expect("tool request");
    assert_eq!(request.tool, ExternalChannelModelToolCapability::DataIngestionAnalysis);
}
```

If a storage-backed test is practical, add a test that dispatch calls the existing data-ingestion reply path and returns one of:

- `data_ingestion_analysis_source_required`;
- `data_ingestion_analysis_queued`.

**Step 2: Add force mode to data-ingestion enqueue**

Change `maybe_enqueue_external_channel_data_ingestion_analysis` to accept an optional tool request or boolean:

```rust
async fn maybe_enqueue_external_channel_data_ingestion_analysis(
    state: &AppState,
    connection_id: &str,
    connection: &ExternalChannelConnectionSummary,
    run: &AssistantRun,
    assistant_request: &CreateAssistantRunRequest,
    message: &ExternalBotMessageView,
    now: DateTime<Utc>,
    force_tool_request: Option<&Value>,
) -> std::result::Result<Option<ExternalBotReplyView>, ApiError>
```

Trigger when either:

- `external_channel_message_requests_data_ingestion_analysis(&run.user_prompt)` is true; or
- `force_tool_request.is_some()`.

Do not change public fields.

**Step 3: Add dispatch case**

In `external_channel_model_tool_request_reply`, route `DataIngestionAnalysis` by loading the run, attaching `model_tool_request` into selected scope, then calling the updated data-ingestion enqueue.

**Step 4: Add event**

Append `assistant_run.external_channel_model_tool_request_detected` with:

- `tool`;
- `intent`;
- `channel_connection_id`;
- `message_external_id`;
- `source=external_channel_model_reply`.

Do not store raw provider payload beyond the bounded tool JSON.

**Step 5: Run tests**

Run:

```powershell
cargo test -p platform-api external_channel_model_tool_request_data_ingestion --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api data_ingestion_analysis --lib
```

Expected: pass.

**Step 6: Commit**

```powershell
git add crates/platform-api/src/lib.rs
git commit -m "Let external models request data ingestion analysis"
```

---

### Task 5: Close The Document Processing / 深解析 / 重解析 Loop

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: `crates/platform-api/src/react_agent_tools.rs`
- Test: `crates/platform-api/src/lib.rs`
- Test if needed: `crates/platform-api/src/react_agent_tools.rs`

**Step 1: Write tests for model-visible document processing requests**

Add parser tests:

```rust
#[test]
fn external_channel_model_tool_request_document_processing_is_recognized() {
    let request = external_channel_model_tool_request(
        r#"<V3_TOOL_REQUEST>{"tool":"document_processing","intent":"reparse_request","reason":"用户说上传文档无法回答，怀疑解析失败"}</V3_TOOL_REQUEST>"#
    ).expect("tool request");
    assert_eq!(request.tool, ExternalChannelModelToolCapability::DocumentProcessing);
}
```

Add dispatch tests for the safe default response:

- visible parse status exists -> return parse status task/card;
- no visible documents -> `needs_input` or source-required style status;
- VLM/runtime disabled -> no fake success.

**Step 2: Implement safe status response first**

Add a helper:

```rust
async fn external_channel_document_processing_reply_from_tool_request(
    state: &AppState,
    connection_id: &str,
    run: &AssistantRun,
    message: &ExternalBotMessageView,
    tool_request: &ExternalChannelModelToolRequest,
    now: DateTime<Utc>,
) -> std::result::Result<ExternalBotReplyView, ApiError>
```

Default behavior:

- inspect selected visible document parse statuses already present in evidence or load visible selected documents;
- return `reply_type=TaskStatus`;
- set `task_status=document_processing_status`;
- set `card.type=v3_document_processing`;
- include only safe counts, titles, external IDs, parse status, parse quality summary, and whether reparse is already running.

**Step 3: Add gated reparse request support**

For `intent` values like `reparse_request`, `deep_parse`, `upgrade_parse`, or `vlm_reparse`:

- require visible selected document;
- require no active reparse workflow;
- require degraded/failed/low-text parse status or explicit model judge parse-quality reason;
- require runtime flag, for example `EXTERNAL_CHANNEL_DOCUMENT_PROCESSING_REPARSE_ENABLED=true`;
- when not enabled, return `document_processing_review_required`, not failure;
- if enabled, reuse the same upload-ingest reparse path and budget rules already used by ReAct `upgrade_parse_vlm`.

Do not add a third-party public request field.

**Step 4: Add events**

Record:

- `assistant_run.external_channel_document_processing_status_returned`;
- `assistant_run.external_channel_document_reparse_review_required`;
- `assistant_run.external_channel_document_reparse_queued`.

**Step 5: Run tests**

Run:

```powershell
cargo test -p platform-api document_parse_status --lib
cargo test -p platform-api external_channel_model_tool_request_document_processing --lib
cargo test -p platform-api assistant_run_supplies_document_parse_status_for_failed_and_reparsing_documents --lib
```

Expected: pass.

**Step 6: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/platform-api/src/react_agent_tools.rs
git commit -m "Add gated document processing model tool requests"
```

---

### Task 6: Add Collection And Integration Analysis As Confirmation-First Capabilities

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`
- Docs: `docs/integrations/third-party-integration-api.zh-CN.md`
- Docs: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

**Step 1: Write parser and dispatch tests**

Add tests proving both tools are recognized and return confirmation/status rather than execution:

```rust
#[test]
fn external_channel_model_tool_request_collection_and_integration_require_confirmation() {
    for tool in ["collection_setup_analysis", "integration_setup_analysis"] {
        let raw = format!(
            r#"<V3_TOOL_REQUEST>{{"tool":"{tool}","intent":"plan","reason":"用户要求接入外部系统"}}</V3_TOOL_REQUEST>"#
        );
        let request = external_channel_model_tool_request(&raw).expect("request should parse");
        assert!(matches!(
            request.tool,
            ExternalChannelModelToolCapability::CollectionSetupAnalysis
                | ExternalChannelModelToolCapability::IntegrationSetupAnalysis
        ));
    }
}
```

**Step 2: Implement analysis/confirmation reply**

Add a shared helper:

```rust
fn external_channel_capability_confirmation_reply(
    message: &ExternalBotMessageView,
    capability: ExternalChannelModelToolCapability,
    payload: &Value,
) -> ExternalBotReplyView
```

Return:

- `reply_type=RequiresConfirmation` when a concrete external action is implied;
- `task_status=needs_confirmation` or `processing` only if a later workflow actually exists;
- `card.type=v3_collection_setup_analysis` or `v3_integration_setup_analysis`;
- redacted `requested_capability`, `intent`, `reason`, `risk_level`, and `next_actions`.

Do not execute crawling, login, external writes, new credentials, or API changes.

**Step 3: Add action-risk rules**

Use these risk rules:

- public read-only source planning: low;
- configured source status check: low;
- new crawler/source setup: medium, needs confirmation;
- login/cookie/credential request: high, needs human;
- API/auth/request/response field change: high, needs human;
- permission change or external publish: high/critical, needs human.

**Step 4: Run tests**

Run:

```powershell
cargo test -p platform-api external_channel_model_tool_request_collection --lib
cargo test -p platform-api external_channel_model_tool_request_integration --lib
npm run check:pure-third-party-guide-html
```

Expected: pass.

**Step 5: Commit**

```powershell
git add crates/platform-api/src/lib.rs docs/integrations/third-party-integration-api.zh-CN.md docs/integrations/pure-third-party-integration-guide.zh-CN.md
git commit -m "Add confirmation-first collection and integration capability requests"
```

---

### Task 7: Close The Message Channel Outreach / 主动发起对话 Loop

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify if needed: `crates/platform-api/src/external_feishu.rs`
- Modify if needed: `crates/platform-api/src/external_wecom.rs`
- Test: `crates/platform-api/src/lib.rs`
- Docs: `docs/integrations/third-party-integration-api.zh-CN.md`
- Docs: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

**Step 1: Write parser and policy tests**

Add tests proving the capability is recognized but does not directly send:

```rust
#[test]
fn external_channel_model_tool_request_message_outreach_requires_host_routing() {
    let request = external_channel_model_tool_request(
        r#"<V3_TOOL_REQUEST>{"tool":"message_channel_outreach","intent":"notify_task_completed","reason":"报表完成后通知用户查看"}</V3_TOOL_REQUEST>"#
    ).expect("tool request");
    assert_eq!(request.tool, ExternalChannelModelToolCapability::MessageChannelOutreach);
}
```

Add policy tests for:

- same `conversation_external_id` progress/completion notice -> may be low risk if channel policy allows;
- new recipient or `mention_external_user_ids` not already in the allowed context -> requires confirmation;
- cross-channel delivery -> requires confirmation;
- sensitive document/report content -> requires permission review;
- no configured outbound channel -> status/confirmation card, no fake send.

**Step 2: Define outbound message action shape**

Use existing response fields. Do not add a required third-party field.

The reply/card should carry only safe metadata:

```json
{
  "type": "v3_message_channel_outreach",
  "status": "message_outreach_confirmation_required",
  "requested_capability": "message_channel_outreach",
  "intent": "notify_task_completed",
  "risk_level": "medium",
  "target_summary": {
    "channel_connection_id": "generic-chat-main",
    "conversation_external_id": "conv-...",
    "recipient_count": 1
  },
  "idempotency_key": "message-outreach:..."
}
```

When Host can safely enqueue an automatic same-conversation notice, use:

- `reply_type=TaskStatus`;
- `task_status=message_outreach_queued` or `message_outreach_sent`;
- `card.type=v3_message_channel_outreach`;
- `card.status=message_outreach_queued|message_outreach_sent|message_outreach_failed`.

**Step 3: Add routing helper**

Add:

```rust
fn external_channel_message_outreach_reply_from_tool_request(
    message: &ExternalBotMessageView,
    tool_request: &ExternalChannelModelToolRequest,
    now: DateTime<Utc>,
) -> ExternalBotReplyView
```

First implementation can be confirmation-first unless an existing same-conversation outbound API already exists and is safe to call. The helper must:

- not send raw model text as-is;
- generate a bounded action summary;
- preserve source conversation and sender context;
- require idempotency;
- return `requires_confirmation=true` for anything beyond same-conversation operational notices.

**Step 4: Add events**

Record:

- `assistant_run.external_channel_message_outreach_requested`;
- `assistant_run.external_channel_message_outreach_confirmation_required`;
- `assistant_run.external_channel_message_outreach_queued`;
- `assistant_run.external_channel_message_outreach_sent`;
- `assistant_run.external_channel_message_outreach_failed`.

Event payloads must not include raw credentials, private URLs, full customer documents, or unbounded model text.

**Step 5: Add dispatch case**

In `external_channel_model_tool_request_reply`, route `MessageChannelOutreach` to the helper. Unknown or unsafe delivery returns confirmation/status, not a natural-language promise.

**Step 6: Run tests**

Run:

```powershell
cargo test -p platform-api external_channel_model_tool_request_message_outreach --lib
cargo test -p platform-api external_channel --lib
npm run check:pure-third-party-guide-html
```

Expected: pass.

**Step 7: Commit**

```powershell
git add crates/platform-api/src/lib.rs crates/platform-api/src/external_feishu.rs crates/platform-api/src/external_wecom.rs docs/integrations
git commit -m "Add message channel outreach capability routing"
```

---

### Task 8: Update External Docs Without Changing Public Contracts

**Status:** Completed locally on 2026-06-04.

**Files:**
- Modify: `docs/integrations/third-party-integration-api.zh-CN.md`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Modify generated HTML docs with the existing docs generation command or script used in this repo
- Test: docs validation scripts

**Step 1: Add a capability explanation section**

Document that third-party clients do not call internal capabilities. They send ordinary messages, document scopes, dataset scopes, templates, or business source IDs. DataMax may internally classify requests into product capabilities and return existing reply/card/status fields.

**Step 2: Add a non-breaking status table**

Add a table for internal capability outcomes:

- static page -> `processing`, `static_page_published`, `failed`;
- data ingestion -> existing `data_ingestion_analysis_*`;
- document processing -> `document_processing_status`, `document_processing_review_required`, `document_reparse_queued`;
- collection/integration -> `needs_confirmation` / confirmation card;
- message-channel outreach -> `message_outreach_confirmation_required`, `message_outreach_queued`, `message_outreach_sent`, or `message_outreach_failed`.

**Step 3: Regenerate/check HTML**

Run the repo's current docs generation/check commands. At minimum:

```powershell
npm run check:pure-third-party-guide-html
```

Expected: pass.

**Step 4: Commit**

```powershell
git add docs/integrations
git commit -m "Document model-visible capability routing"
```

---

### Task 9: Build A Regression Corpus For Routing Decisions

**Status:** Completed locally on 2026-06-04.

**Files:**
- Create: `fixtures/external-channel-capability-routing/cases.jsonl`
- Modify: `crates/platform-api/src/lib.rs`
- Optional script: `scripts/run-external-capability-routing-smoke.ps1`

**Step 1: Create JSONL cases**

Add cases like:

```jsonl
{"prompt":"我不喜欢这个风格的报表，最好暗黑一点，适合手机端展示","expected_tool":"static_page_artifact"}
{"prompt":"根据这个表帮我建表，后续可以按门店查询","expected_tool":"data_ingestion_analysis"}
{"prompt":"刚上传的 PDF 为什么问不出来，能不能重新解析","expected_tool":"document_processing"}
{"prompt":"帮我采集这个公开网站的政策更新","expected_tool":"collection_setup_analysis","expected_confirmation":true}
{"prompt":"我们要接入客户自己的 OA 权限和文档库","expected_tool":"integration_setup_analysis","expected_confirmation":true}
{"prompt":"报表生成好以后主动发消息给店总，让他打开链接看","expected_tool":"message_channel_outreach","expected_confirmation":true}
{"prompt":"长期卧床老人多长时间翻身一次","expected_tool":null}
{"prompt":"帮我分析这份报表口径有哪些问题","expected_tool":null}
```

**Step 2: Add deterministic classifier tests where possible**

Use existing keyword classifiers for deterministic routes and parser tests for model output. Do not require a live model for unit tests.

**Step 3: Add smoke script for live 8-server runs**

Script should:

- accept `-BaseUrl`, `-Bearer`, `-ConnectionId`;
- generate unique `conversation_external_id` and `message_external_id`;
- send selected cases through `/v1/external/channels/{connection_id}/events/stream`;
- assert no `<V3_TOOL_REQUEST>` leaks to customer output;
- assert expected `task_status`, `card.type`, or `artifact_links`.

**Step 4: Run tests**

Run local unit tests first:

```powershell
cargo test -p platform-api external_channel_model_tool_request --lib
cargo test -p platform-api external_channel_data_ingestion --lib
cargo test -p platform-api external_channel_static_page --lib
```

Then 8-server smoke only with a real private bearer.

**Step 5: Commit**

```powershell
git add fixtures/external-channel-capability-routing scripts crates/platform-api/src/lib.rs
git commit -m "Add external capability routing regression corpus"
```

---

### Task 10: 8-Server Rollout And Smoke

**2026-06-05 follow-up:** The live capability-routing smoke harness now treats focused Xinbai report cases as link-required. If a fixture has `expected_focus`, `/events/stream` must emit an immediate generated-artifact link and that link must carry the expected `?focus=` value; queued progress without a clickable report link no longer passes these high-frequency report cases.

**2026-06-05 strict-smoke fix:** The tightened smoke caught an 8-server regression where `取高` recorded an accepted dataset-overlap template URL internally but public SSE completed without a link because the fixed-task queue card won the response merge. The local fix prefers accepted template baseline links over queue cards and keeps top-level `artifact_links[]` while preserving background refresh status fields.

**Files:**
- Modify only if needed: `docs/validation/**`
- Do not modify public API contracts unless explicitly approved.

**Step 1: Local final checks**

Run:

```powershell
cargo fmt --check
cargo check -p platform-api
cargo test -p platform-api external_channel_model_tool_request --lib
cargo test -p platform-api external_channel_static_page --lib
cargo test -p platform-api external_channel_data_ingestion --lib
npm run check:pure-third-party-guide-html
git diff --check
```

Expected: pass.

**Step 2: Deploy to 8 server**

Use the established release command pattern:

```powershell
ssh 8服务器 "cd /srv/aiv3/repo && git pull --ff-only && CC=clang CXX=clang++ cargo build --release -p platform-api && sudo systemctl restart aiv3-platform-api.service && sleep 3 && systemctl is-active aiv3-platform-api.service && git rev-parse --short HEAD"
```

Expected: service active and HEAD matches GitHub.

**Step 3: Run private smoke**

Required cases:

- third-party ordinary document Q&A: `邓工是谁`;
- dark/mobile report redesign -> static page status/link;
- Xinbai report request with existing accepted template -> reuse `xinbai-functional-modular-template-20260604` and return a focused URL when the prompt mentions risk, take-high opportunity, low activity, category, or brand details;
- data ingestion natural wording -> `data_ingestion_analysis_*`;
- PDF parse/reparse complaint -> `document_processing_*`;
- ordinary care question -> natural answer, no artifact/data-ingestion tool;
- collection/integration request -> confirmation/status, no external execution;
- proactive message request -> confirmation/status or same-conversation operational notice, no direct model-sent message.

**Step 4: Inspect events**

For model-requested tools, verify AssistantRun events include:

- `assistant_run.external_channel_model_tool_request_detected`;
- capability-specific queued/status event;
- no raw credentials, raw provider payload, or full customer document.

**Step 5: Record validation receipt**

Update a validation doc or append a short dated note to this plan with:

- deployed commit;
- service status;
- tested cases;
- pass/fail;
- any rollback instruction.

---

## Expected End State

- The model knows DataMax can handle documents, parsing/reparse, reports/static pages, answer organization, data ingestion, collection planning, integration planning, proactive message-channel outreach, and permission-scoped enterprise work.
- The model only requests product-level capabilities; it does not call low-level Host tools.
- DataMax Host can parse and route product-level capability requests without leaking internal tags to customers.
- Static pages and data ingestion are executable closed loops.
- Document processing is at least a safe status/review loop, with gated reparse where runtime and policy allow it.
- Collection and integration are confirmation-first loops, not hidden external execution.
- Message-channel outreach is a Host-controlled loop with recipient scope, idempotency, confirmation policy, and audit; the model never sends messages directly.
- Public third-party API shape remains backward-compatible.
- Regression tests and 8-server smoke prove the behavior before release.
