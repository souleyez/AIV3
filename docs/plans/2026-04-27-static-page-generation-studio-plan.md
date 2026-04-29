# Static Page Generation Studio Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**2026-04-29 consolidation note:** Use `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md` as the active next-thread execution entry. This document remains the detailed source record for static-page, assistant shell, parsing, visibility, AssistantRun, and renderer decisions.

**Goal:** Build static page generation, report creation, and knowledge-augmented chat inside the V3 intelligent assistant, matching the original assistant UI 1:1 on desktop and mobile, while allowing users to drive planning, layout, data binding, style choice, image preview, final page generation, and report outputs mainly through natural language.

**Architecture:** The assistant is chat-first and dataset-enhanced. If no dataset is selected, chat behaves like normal model chat with a concise platform/database briefing. A lightweight scope planner can preselect relevant visible datasets and conversation memory as supply candidates, then the host retrieves evidence/detail and feeds the model without locally composing the final answer. Static page and report work is embedded in the same assistant page: the main chat area becomes the active report/static-page workspace, while the right panel always remains the finished-output and draft shelf. Draft state is schema-first, model-operated, and later persisted through V3 Rust APIs and queued image/static-page workers.

**Tech Stack:** Next.js 16 / React 19 in `apps/web`, existing `/api/v3/*` proxy, Rust `platform-api`, `contracts`, `domain-model`, `storage`, PostgreSQL 17.9 as the target server version for fresh/production-like environments, future static-page runtime/worker/renderer crates, future media parse worker using authorization-free OSS tools plus configured MiniMax capability probes where verified, original MiniMax VLM document fallback for image/PDF/presentation visual parsing, `react-grid-layout` for desktop planning canvas, `@dnd-kit/core` and `@dnd-kit/sortable` for mobile vertical ordering, `@puckeditor/core` as a future component-render adapter, `recharts` for first static charts, optional Apache ECharts for advanced charts, Cloudflare/Codex image queue integration.

---

## Current Plan Audit

I checked `docs/plans/2026-04-27-static-page-generation-studio-plan.md` on 2026-04-27.

Finding:

- The file was still untracked in git, so there is no committed baseline to diff against.
- Its timestamp was `2026-04-27 14:06:04`, after the first draft work.
- The content was not the latest intended direction: it still said `Isolated Popup Studio Route`, `Hard-Coded Style Presets`, and `clean single-purpose generation studio`.
- It did not include the latest decisions: original assistant UI 1:1, mobile parity, model-operated global editing, `react-grid-layout`, `dnd-kit`, Puck capability, commercial-risk notes, and replacing hard-coded styles with three business style directions.

Conclusion:

- Treat the previous file content as an outdated draft, whether caused by another thread or by an earlier unsynced write.
- This document is now the authoritative merged plan.

## Implementation Progress

2026-04-27 frontend/backend slice completed:

- Added frontend startup briefing utilities so the assistant can summarize visible datasets, document counts, report counts, current scope, recent activity, parse status, default public categories, and available capabilities before answering.
- Added frontend Scope Planner utilities that keep a user-selected dataset as highest priority, preselect visible datasets from natural-language intent, and include local conversation memory only when the prompt refers to prior context.
- Updated the assistant home UI so no dataset is selected by default. The user can chat in ordinary mode, manually select a dataset, or let the Scope Planner preselect one.
- Moved dataset creation above the dataset list and added an explicit `普通聊天` rail item.
- Added a small model-scope strip in the chat area showing current supply mode, visible asset counts, preselected datasets, and whether local conversation memory is relevant.
- Kept only one visible static-page generation action in the composer area and added the upload button placeholder in the bottom composer.
- Added browser-local ordinary chat cache for the current terminal/browser.
- Added `POST /v1/assistant-runs` as a lightweight no-dataset AssistantRun endpoint. This endpoint does not fake a dataset and does not write to `chat_sessions`; it can call an OpenAI-compatible provider through `ASSISTANT_RUN_RUNTIME_*` env vars, with placeholder fallback.
- Wired the no-dataset frontend chat path to `/api/v3/assistant-runs`, with local fallback if the backend endpoint is unavailable.

2026-04-27 upload/classification slice completed:

- Added frontend upload classification utilities for `订单`, `客服`, `企业问答`, `网页采集`, and `未分类`.
- The bottom composer upload button now opens a file picker, classifies selected files, prefers the selected dataset, then matching visible/default datasets, then `未分类`.
- If the default target dataset is missing, the web client creates the default public dataset before registering the document.
- Uploaded files are registered through `POST /v1/documents`, then the existing `upload_ingest_workflow` is created and started through `POST /v1/documents/{document_id}/ingest` plus workflow start.
- Upload/classification activity is cached in browser-local activity history and is included in the assistant startup briefing as recent activity.
- The UI shows upload-classified datasets in the scope strip and left rail preselection state.

2026-04-27 real ingest parser rebaseline started:

- Compared the original `ai-data-platform` document pipeline and confirmed the mature path was `saveMultipartFiles -> ingestDocumentFiles -> parseDocument(quick) -> detailed parse queue -> knowledge/vector sync`.
- Identified the original parser capabilities: `pdf-parse`/`pypdf`/OCR fallback, `mammoth` for DOCX, `xlsx` sheet reader with table summaries, PPT/PPTX extraction, Tesseract image OCR, MarkItDown canonical markdown, VLM fallback, structured profile, evidence chunks, and library grouping.
- Added a local V3 upload route that saves browser files into ignored `storage/uploads` and returns a worker-readable `object_key`.
- Updated the web upload flow so documents now point at saved local files instead of metadata-only placeholder keys.
- Updated `ingest-worker` from pure placeholder output to local parsing first: direct text extraction for text/markdown/csv/json/html/xml, MarkItDown command fallback for richer file types, chunk splitting, parse method metadata, and explicit placeholder fallback only when extraction fails.
- This is the first migration slice, not parity with the original parser yet. Remaining parity work is old binary Office support, high-fidelity table/document structure, MiniMax VLM fallback, detailed parse queue semantics, structured profiles, and vector/memory sync.

2026-04-27 Office OOXML parser slice completed:

- Added direct zip-based OOXML extraction in `ingest-worker` for DOCX, XLSX/XLSM, and PPTX/PPTM before the MarkItDown fallback.
- DOCX parsing now reads `word/document.xml`, strips XML markup, decodes common XML entities, and preserves paragraph/table-cell boundaries as readable text.
- XLSX parsing now reads `xl/sharedStrings.xml` and worksheet XML, resolves shared-string references, extracts inline/numeric values, and preserves tab-separated row/column structure for downstream chunking.
- PPTX parsing now reads slide XML files and extracts text per slide in slide order.
- Added parser metadata coverage so ingestion records the concrete parse method such as `docx-ooxml`, `xlsx-ooxml`, `pptx-ooxml`, `markitdown`, or `placeholder`.
- Added unit tests that build minimal in-memory DOCX/XLSX/PPTX zip files and verify the local ingestion processor extracts real content.
- This slice reduces dependence on external tools for common Office uploads, but old binary XLS/PPT, high-fidelity tables, MiniMax VLM enhancement, and deep parse/vector sync still require the next parity slices.

2026-04-27 PDF parser slice completed:

- Added a PDF extraction path before generic MarkItDown fallback.
- PDF parsing first tries `PDFTOTEXT_BIN` or `pdftotext` when available, then tries Python `pypdf`/`PyPDF2` through `PYTHON_BIN`, then falls back to a lightweight PDF literal-string extractor.
- Added PDF parse method metadata: `pdf-pdftotext`, `pdf-python`, or `pdf-literal`.
- Added a deterministic unit test with a minimal PDF text stream so local CI can verify PDF extraction without requiring external PDF tools.
- This is still a baseline text extraction path. OCR provider fallback, table recovery, layout-aware parsing, VLM enhancement, and detailed parse queue behavior remain follow-up work.

2026-04-27 OCR fallback slice completed:

- Added image extension detection and image content-type inference for browser uploads.
- Image uploads now produce `image-ocr` when Tesseract extracts text, or `image-ocr-empty` with explicit metadata when OCR tools are missing or extraction fails.
- Image uploads no longer fall through to fake placeholder chunks when no OCR text is available.
- PDF parsing now tries OCRmyPDF after normal text extraction and before lightweight PDF literal fallback.
- PDF parsing also has a best-effort Python render plus Tesseract fallback when `pdf2image` and Tesseract are installed.
- Added shared command candidate helpers for Python, MarkItDown, Tesseract, OCRmyPDF, and temporary OCR working directories.
- Added tests covering image OCR-empty behavior without placeholder fallback.
- MiniMax VLM is intentionally still pending. It must be ported from the original project as the visual enhancement layer for images, scanned PDFs, and rendered presentations because the user wants the original MiniMax VLM path preserved.
- MiniMax can also participate in audio/video parsing only through explicit capability probes. The product must not assume MiniMax TTS, voice clone, video generation, or file-management APIs are equivalent to transcription or video understanding.

2026-04-27 MiniMax document VLM runtime slice completed:

- Added `document-vlm-runtime` as a Rust crate for MiniMax-backed image document understanding.
- Ported the original strict JSON contract at the runtime boundary: summary, document kind, layout type, topic tags, visual summary, evidence blocks, field candidates, entities, claims, chart/table signals, and transcribed text.
- Added MiniMax OpenAI-compatible chat-completions request building with data-URL image payloads, controlled by `DOCUMENT_IMAGE_PARSE_MODE`, `DOCUMENT_IMAGE_VLM_PROVIDER`, `DOCUMENT_IMAGE_VLM_MODEL`, `MINIMAX_API_KEY`, `MINIMAX_BASE_URL`, size limit, and timeout env vars.
- Integrated direct image uploads into `ingest-worker`: OCR still runs first, then MiniMax VLM can enrich image text as `image-ocr+vlm` or rescue OCR-empty images as `image-vlm` when configured and successful.
- Added a scanned-PDF VLM fallback path: when MiniMax image VLM is configured and normal PDF text/OCR paths fail, the worker can render bounded PDF pages with Python `pdf2image`, send page images to MiniMax, and store `pdf-vlm` text.
- Added a presentation VLM fallback path: when MiniMax image VLM and LibreOffice/soffice are available, PPTX/PPTM can be converted to PDF, rendered into bounded slide images, sent to MiniMax, and appended as `pptx-ooxml+presentation-vlm`.
- Added ingest metadata marker `cloud_structured_provider=minimax` when a VLM parse method is used.
- Added `parse_metadata` persistence for document metadata and chunk metadata, so VLM provider/model/payload/page evidence can be inspected and reused by later retrieval/report/static-page layers instead of existing only as flattened text.
- Remaining MiniMax parity work is deeper structured profile projection into first-class fields, evidence/entity/claim persistence beyond metadata/text chunks, recoverable provider error metadata, and exact media capability probing.

2026-04-27 media upload first-slice completed:

- Added conservative audio/video extension and content-type recognition to `ingest-worker`.
- Audio/video uploads now become first-class extracted materials instead of placeholder chunks.
- Added `media-partial` parse output when no configured transcription command produces text. This explicitly states that transcript was not extracted and does not invent subtitles.
- Added `ffprobe` probing support when available, storing format/stream metadata under `parse_metadata.media.probe`.
- Added optional `MEDIA_TRANSCRIBE_BIN` adapter. If configured and it returns text on stdout, ingest stores `media-transcript` with transcript source metadata.
- Added frontend upload media-kind detection for audio/video files.
- This is not full media understanding yet. Remaining work is MiniMax media capability probing, native/video-keyframe provider routing, richer transcript segment timestamps, scene/keyframe extraction, and a dedicated media detail API.

2026-04-27 foreground/background processing boundary completed:

- Locked the product rule that foreground upload work may only save files, preclassify, register documents, and enqueue workflows.
- Added document registration metadata for `initial_classification`, `processing_policy`, and non-blocking `parse_state`.
- Extended `RegisterDocumentRequest` and `NewDocument` to accept metadata while preserving secret-binding metadata.
- Frontend upload now records the selected/auto-classified dataset, confidence, reason, media kind, and background-required capabilities at registration time.
- Heavy work remains in background workflows: content parsing, VLM enrichment, media transcription, indexing, and report/static-page supply.

2026-04-27 dataset visibility/security foundation slice completed:

- Added first-class `DatasetVisibility` with `public` and `private` states in the domain model and dataset API contracts.
- Dataset metadata now defaults to `visibility=public` and stores `default_secret_binding_ids` for future local-key/private-scope enforcement.
- Dataset summaries now expose visibility, secret-binding ids, and a public-scope warning when a dataset is created without a private secret.
- `GET /v1/datasets` now ensures the default public datasets exist: `订单`, `客服`, `企业问答`, `网页采集`, and `未分类`.
- Platform API now accepts `X-AI-Data-Platform-Secret-Binding-Ids` as the active local secret-binding scope and filters private datasets server-side.
- Main dataset-bound surfaces now use the same visibility gate: dataset list, memory directories, retrieval evidences, retrieval search, dataset outputs, chat sessions, documents, document chunks/detail/evidence, compare, ingest start, report planning, report continuation, render, publish, render-output listing, and published-report reads/listing.
- The web API proxy now forwards the active secret-binding header, and the browser client has local-storage plumbing for future local-key UI.
- This is the security foundation only. Remaining work is user-facing key creation/verification, real secret binding/grant CRUD, startup briefing filtering from backend data, and enforcing the same visibility snapshot inside future AssistantRun/static-page persistent records.

2026-04-27 local secret binding first slice completed:

- Added dataset-level secret binding creation in storage using the existing `secret_bindings` table. The server stores provider label, fingerprint, and binding relationship, not the raw local key.
- Added `POST /v1/dataset-secret-bindings` so an existing visible dataset can be converted to private and bound to a local secret-binding id.
- Extended `POST /v1/datasets` to accept an optional local secret fingerprint and create a private dataset in the same backend flow, avoiding a visible public dataset gap between create and bind.
- The web dataset create form now has an optional local-secret field. When filled, the browser computes a SHA-256 fingerprint locally, sends only the fingerprint, stores the raw key and binding id in browser-local storage, and then sends the binding id on later API calls.
- Upload registration now includes the current local secret-binding ids so documents created by the current terminal inherit the active local key.
- This is still the simple local-key model. Remaining work is a proper key management surface, unlock/switch/revoke flows, device fingerprint grants, and migration from binding-id headers toward verified secret-grant headers.

2026-04-27 local secret unlock/switch slice completed:

- Added fingerprint lookup for dataset-level secret bindings so a user can enter the same local key on another browser/terminal and unlock matching private datasets.
- Added `POST /v1/dataset-secret-bindings/resolve`, returning matching binding ids and private dataset summaries only after the browser sends the locally computed fingerprint.
- Added a compact left-rail local secret panel for unlocking and clearing the current browser's local key state.
- Added a left-rail action to bind the currently selected visible dataset to the entered local key, so existing public datasets can be converted to private without recreating them.
- New private dataset creation now merges the newly created binding with existing browser-active bindings and immediately updates the active binding count.
- Clearing the local key only removes browser-local secret state and active binding ids; it does not delete datasets or server-side bindings.
- Unlocking stores the raw key only in browser-local storage, stores binding ids for later request headers, refreshes the visible dataset list, and prefers the first unlocked dataset as the active supply scope.
- Remaining work is audited key lifecycle, revoke/rotate UX, multi-key display, better private-dataset warnings during upload, and replacing raw local storage with a stronger browser-side protection model if the product later needs it.

2026-04-27 AssistantRun persistence first slice completed:

- Added `AssistantRunId` and `AssistantRunEventId` plus domain structs for assistant runs and run events.
- Added API contracts for persistent assistant run views, run detail, event append, selected scope, evidence state, execution trail, output artifacts, and required confirmations.
- Added PostgreSQL system-of-record tables `assistant_runs` and `assistant_run_events`, including tenant/local-thread indexes and ordered event storage.
- Added storage repository methods to create a run, read a run, append/list events, update selected scope, update evidence state, and attach output artifacts.
- `POST /v1/assistant-runs` now persists each ordinary assistant run after the runtime returns, stores startup briefing, selected scope, scope candidates, context policy, runtime manifest, execution trail, output artifacts, and a completion event.
- Added `GET /v1/assistant-runs/{run_id}` and `POST /v1/assistant-runs/{run_id}/events` for durable run inspection and lightweight continuous-execution progress notes.
- The browser now sends a stable browser-local `local_thread_id` with ordinary no-dataset AssistantRun requests, preserving the principle that each terminal/browser has independent conversation records.
- This is the persistence shell only. Remaining work is hidden conversation memory, provider-backed scope planner persistence, selected-scope RAG/detail supply, run continuation, and report/static-page capability calls attached to the same run.

2026-04-27 hidden conversation memory first slice completed:

- Added `ConversationMemoryItemId` and a hidden conversation memory domain model for local thread id, role, item kind, summary, source message refs, artifact refs, metadata, and timestamps.
- Added PostgreSQL table `conversation_memory_items` with tenant/local-thread indexing.
- Added storage methods to create conversation memory items and list candidates by browser-local thread id with optional summary query filtering.
- Added `POST /v1/conversation-memory-items` and `GET /v1/conversation-memory-items` for browser-local memory summary sync.
- Added `GET /v1/assistant-runs/{run_id}/conversation-memory-candidates`, which resolves the run's local thread and returns memory candidates without requiring the browser to expose that thread id again.
- The web client now best-effort stores only user statements from ordinary local chat as hidden memory summaries. It does not upload assistant output artifacts, and it does not block chat if memory sync fails.
- This is still candidate storage, not automatic context injection. Remaining work is model/host intent selection, summarization quality, dedupe/compaction, retention policy, and explicit artifact-summary handling.

2026-04-27 backend scope planner first slice completed:

- Added `assistant-runtime` as the first backend runtime crate for assistant orchestration.
- Implemented a deterministic scope planner matching the current product principle: user-selected dataset wins, visible dataset name/business-topic hits become candidates, and hidden conversation memory is only offered when the prompt references prior context.
- `POST /v1/assistant-runs` now runs the backend scope planner against server-filtered visible datasets and the browser-local conversation-memory availability signal.
- Frontend-supplied `scope_candidates` are no longer trusted as the persisted candidate source; the backend recomputes candidates after dataset visibility filtering.
- AssistantRun now stores backend-planned `scope_candidates`, `selected_scope`, and a short execution-trail step for candidate planning.
- The web client now consumes backend `scope_candidates` and `selected_scope` from ordinary AssistantRun responses, refreshing the catalog when the backend preselects a dataset.
- Added regression coverage that an untrusted frontend candidate cannot leak a private dataset unless the matching local secret-binding header is present.
- This is still a deterministic planner. Remaining work is provider-backed scope planning, richer document-level candidates, evidence retrieval after candidate selection, and controlled continuous execution based on the same run context.

2026-04-27 AssistantRun selected-scope evidence supply first slice completed:

- `POST /v1/assistant-runs` now treats backend-planned `selected_scope` as authoritative instead of persisting the frontend-selected scope verbatim.
- When the backend-selected scope contains visible datasets, AssistantRun retrieves latest retrieval evidences from those datasets, ranks them against the user prompt, and stores the result in `evidence_state`.
- Provider-backed AssistantRun input now includes the supplied evidence state so the model receives host-provided material while still owning the final answer.
- Placeholder AssistantRun output now surfaces only a short supply status such as `已检索 2 条证据`, avoiding fake local answer composition.
- The run execution trail now includes `检索供料证据` with evidence status and supplied count.
- Added regression coverage that selected-scope evidence is ranked, persisted, returned in run detail, and reflected in the run trail.
- This is the first evidence supply slice. Remaining work is richer document-level scope selection, hidden conversation-memory retrieval injection, multi-step continuation actions, provider-backed planning, and static-page/report capability calls attached to the same run.

2026-04-27 AssistantRun hidden conversation-memory injection slice completed:

- Scope planner now preserves hidden conversation memory in `selected_scope` even when a user-selected or preselected dataset is also present.
- When `selected_scope.conversation_memory` contains `local-thread`, `POST /v1/assistant-runs` retrieves browser-local hidden memory for the run's `local_thread_id`.
- Hidden memory supply is appended to `evidence_state.supplied_items` as `conversation_memory_item`, alongside normal retrieval evidence.
- AssistantRun only injects user-role memory items by default and excludes assistant/artifact output-like item kinds, keeping the first slice aligned with the product rule that conversation memory is mainly "用户说过什么".
- Placeholder output now reports supplied "供料项" instead of only "证据", because a run may include dataset evidence and hidden conversation memory at the same time.
- Added regression coverage that history-referencing runs supply hidden memory, persist it in run detail, and still support dataset plus memory scope planning.
- Remaining work is higher-quality memory summarization/dedupe, provider-backed memory intent classification, memory compaction/retention policy, and richer artifact-summary handling.

2026-04-27 AssistantRun continue API first slice completed:

- Added `ContinueAssistantRunRequest` and `ContinueAssistantRunResponse` contracts.
- Added `POST /v1/assistant-runs/{run_id}/continue` as the bounded continuous-execution entry point.
- Continue requests reuse the existing run's selected scope, evidence state, local thread, output artifacts, and runtime configuration.
- The endpoint clamps `max_steps` to 1-5, appends a `继续执行` trail step, appends a new assistant-message output artifact, and records an `assistant_run.continued` event.
- Placeholder mode returns a short continuation acknowledgement instead of pretending to execute report/static-page actions.
- Provider mode gets a model input containing run id, original prompt, continue instruction, max step budget, selected scope, supplied evidence, current artifact, and recent messages.
- Storage now supports updating `execution_trail` as a first-class AssistantRun JSON field.
- Remaining work is attaching concrete platform actions to continue loops: retrieval/detail refresh, report/static-page draft creation, operation application, image queue submission, confirmation gates, and model-requested tool calls.

2026-04-27 AssistantRun frontend continue wiring first slice completed:

- Browser ordinary chat now caches the latest `assistant_run_id` per local terminal/browser.
- If the user sends a follow-up that looks like `继续`, `刚才`, `下一步`, `修改`, or similar, the web client first calls `POST /v1/assistant-runs/{run_id}/continue`.
- Continue calls include recent local messages, the active static-page draft if any, and a conservative `max_steps=3`.
- If continue fails because the old run is gone or the backend is unavailable, the browser clears the cached run id and falls back to creating a fresh AssistantRun.
- Starting a new conversation clears the cached AssistantRun id, matching the product rule that each browser-local conversation is independent.
- This is still ordinary no-dataset chat wiring only. Selected-dataset chat sessions and future report/static-page workspaces still need to migrate toward AssistantRun-backed continuation.

2026-04-27 AssistantRun static page draft backend/API slice completed:

- Added first-class `StaticPageDraftId`, draft status, and `StaticPageDraft` domain model rooted under `assistant_run_id`.
- Added PostgreSQL `static_page_drafts` storage with selected scope snapshot, visibility snapshot, source refs, and schema-first draft payload.
- Added `POST /v1/assistant-runs/{run_id}/static-page-drafts` and `GET /v1/static-page-drafts/{draft_id}` for AssistantRun-rooted draft creation and loading.
- Added `PATCH /v1/static-page-drafts/{draft_id}` for draft payload/status/title updates.
- Added `POST /v1/static-page-drafts/{draft_id}/operations` so model/client operations can be appended and persisted against the durable draft.
- Added `POST /v1/static-page-drafts/{draft_id}/intent` with deterministic Chinese intent fallback for style direction, risk emphasis, text compression, module ordering, and chart changes.
- Static page draft creation, update, operation append, and intent application now append AssistantRun events so continuous execution has a visible trail.
- The web client now creates an immediate local draft, then syncs it to the backend when an AssistantRun id exists. Later module operations and natural-language edits sync to the backend when a durable draft exists, with local fallback if the backend is unavailable.
- Remaining work is provider-backed static-page intent runtime, operation schema hardening, image queue/job persistence, preview confirmation, final renderer output persistence, right-shelf durable draft listing, and selected-dataset chat migration toward AssistantRun-backed static-page creation.

2026-04-27 static page intent runtime slice completed:

- Added `static-page-runtime` as the dedicated static-page intent interpretation crate.
- The runtime accepts prompt, draft payload, AssistantRun id, startup briefing, selected scope, supplied evidence state, conversation memory refs, and recent messages.
- Deterministic fallback now covers Chinese prompts for executive style, client delivery style, data-dashboard style, risk emphasis, text compression, selected-scope data binding, conversation-memory binding, and chart changes.
- Provider-backed operation generation is available behind `STATIC_PAGE_INTENT_RUNTIME_MODE=provider`, `STATIC_PAGE_INTENT_RUNTIME_PROVIDER`, `STATIC_PAGE_INTENT_RUNTIME_MODEL`, and the shared OpenAI-compatible `STATIC_PAGE_INTENT_RUNTIME_*` env wiring.
- Provider output must be strict JSON with `summary` and `operations`. Invalid provider output falls back to deterministic interpretation rather than applying unsafe model JSON.
- Static page operations now pass through a runtime white-list sanitizer before being applied. Unknown operation types and unsafe JSON keys such as `__proto__`, `constructor`, and `prototype` are rejected.
- `POST /v1/static-page-drafts/{draft_id}/intent` now calls `static-page-runtime` and stores runtime metadata in the AssistantRun event payload.
- `POST /v1/static-page-drafts/{draft_id}/operations` now reuses the same sanitizer, so client/model operations share one safety gate.
- Remaining work is stronger typed operation structs, richer provider prompt templates, model-requested tool-call loops, image job persistence, queue polling, preview confirmation, and final renderer output persistence.

2026-04-27 static page image/render persistence skeleton completed:

- Added `StaticPageImageJobId`, `StaticPageRenderOutputId`, image job status, and render output status to the domain model.
- Added PostgreSQL `static_page_image_jobs` and `static_page_render_outputs` tables plus storage repositories.
- Added `POST /v1/static-page-drafts/{draft_id}/image-jobs` to create a durable effect-image queue record from the confirmed draft modules, style direction, selected scope, visibility snapshot, and data bindings.
- Added `GET /v1/static-page-image-jobs/{job_id}` for queue polling.
- Added `POST /v1/static-page-image-jobs/{job_id}/confirm` to persist a confirmed preview asset key and update the draft to `confirmed`.
- Added `POST /v1/static-page-drafts/{draft_id}/renders` to create a skeleton final HTML render output after preview confirmation.
- Final render is blocked until a confirmed preview exists, preserving the product gate that customers must confirm the effect preview before static-page generation.
- The render skeleton includes module titles, content, data labels, chart placeholders, style direction class, and confirmed preview metadata.
- Image job creation, preview confirmation, and final render creation append AssistantRun events so the chat can show concise continuous-execution steps later.
- Remaining work is remote queue-position fidelity, browser display of real image artifacts, richer chart rendering, workerized final render/export packaging, and frontend polling/display hardening.

2026-04-27 static page frontend image/render API wiring completed:

- The web static-page flow now calls the durable backend image job API when the user clicks `生成效果图` or one-click output enters the queue.
- Local draft state is updated with the backend image job id and queue position, while still falling back to local queue state if the backend is unavailable.
- `确认效果图` now confirms the backend image job when a durable draft exists and stores the confirmed preview asset key in the draft payload.
- `按效果制作静态页` now calls `POST /v1/static-page-drafts/{draft_id}/renders`, passing the confirmed image job id and storing the returned render output id, asset manifest, and HTML in the frontend draft state.
- The final static-page preview can display backend-rendered HTML in an isolated iframe, while retaining the existing local mock render when no backend render exists.
- One-click static-page creation now submits a backend image job after the AssistantRun-backed draft is created, avoiding a queued-looking local draft without a durable image job row.
- Remaining work is true queue polling, remote Cloudflare/Codex image artifact pickup, worker status transitions from `queued` to `preview_ready`, right-shelf durable draft/render listing, and replacing skeleton HTML with a dedicated renderer.

2026-04-27 static page durable shelf slice completed:

- Added `GET /v1/static-page-drafts` so the browser can list current-terminal static page drafts by `local_thread_id`, or list drafts under a specific AssistantRun.
- Added `GET /v1/static-page-drafts/{draft_id}/image-jobs` and `GET /v1/static-page-drafts/{draft_id}/renders` for right-shelf hydration and future polling.
- Added storage support for listing static page drafts through the AssistantRun local-thread relationship, plus indexes for draft/job/render lookup.
- The web client now loads the current terminal's static page draft shelf on startup and polls it periodically.
- The right panel now has a `静态页成品` shelf showing durable草稿/成品 status, module count, update time, and click-to-open behavior.
- Reloaded rendered drafts can hydrate the latest backend render output and display backend HTML preview again instead of losing the generated page to local state.
- Remaining work is moving the active desktop planning workspace fully into the main chat area, replacing skeleton HTML with a dedicated renderer, and connecting real queue polling to remote image generation status.

2026-04-28 static page main workspace alignment completed:

- The desktop assistant main chat area now becomes the active static-page planning/render workspace when a draft or finished static page is open.
- The bottom composer remains visible while the static-page workspace is open, so the user can continue asking for changes in natural language.
- The right panel no longer renders the active planning canvas; it only keeps the durable `静态页成品` shelf alongside existing sessions/reports/outputs.
- Mobile keeps its dedicated vertical static-page builder instead of showing the desktop planning canvas inside the mobile chat surface.
- A `返回聊天记录` action clears the active static-page workspace without deleting the shelf item.
- Remaining work is real queue polling, Cloudflare/Codex image worker integration, dedicated renderer output, and richer chart rendering from bound data.

2026-04-28 static page renderer extraction completed:

- Added `static-page-renderer` as a dedicated Rust crate for static-page HTML and asset-manifest generation.
- `platform-api` now calls the renderer crate from `POST /v1/static-page-drafts/{draft_id}/renders` instead of owning ad-hoc HTML assembly.
- The renderer input includes draft id, AssistantRun id, draft payload, selected scope snapshot, visibility snapshot, confirmed preview asset key, and image job id.
- The renderer output includes escaped standalone HTML and an asset manifest with renderer id, module count, modules, selected scope, visibility snapshot, preview asset, and image job reference.
- Added renderer tests for HTML/manifest output and HTML escaping.
- Remaining work is real data-bound chart rendering, richer layout-aware HTML/CSS, asset export packaging, and worker-side render execution instead of synchronous API rendering.

2026-04-28 static page Codex image worker first slice completed:

- Added `static_page_image_generation_workflow` to the workflow catalog with queue `static_page` and task key `generate_static_page_image`.
- Added `static-page-worker` as the first real background worker for static-page effect previews.
- The worker reads `CODEX_ORCHESTRATOR_ACCESS_KEY` only from the server environment, calls the canonical Codex Orchestrator v1 task API, submits `kind=static-page-visual`, and requests an `image-artifact` from the Cloudflare runtime.
- The worker stores orchestrator task state inside the image job payload, polls until completion, extracts the first returned image artifact URL/data URL, marks the job `preview_ready`, updates the draft payload, and appends an AssistantRun event.
- `POST /v1/static-page-drafts/{draft_id}/image-jobs` now creates and starts the workflow automatically, so the browser still has one simple "generate effect image" action while backend queue execution is durable.
- Remaining work is deployment wiring for the provided server env file, queue-position fidelity against the remote queue, browser display of real bitmap artifacts instead of the current preview card, retry/cancel UX, and final workerized static-page render/export packaging.

2026-04-28 static page real preview display slice completed:

- The active static-page workspace now polls its backend draft more aggressively while the image job is `queued` or `running`, instead of waiting only for the slower right-shelf refresh.
- Backend image job hydration now maps `preview_ready` into the local draft state, carries the real `preview_asset_key`, and uses a customer-facing "effect image generated" queue message.
- The effect-preview component now renders real remote image artifacts when the asset key is an HTTP URL, data image, blob URL, or same-origin API path.
- Backend image jobs no longer expose the local "view mock effect image" shortcut while they are still running, preventing accidental confirmation of a simulated preview when a real Codex image is pending.
- The static-page worker now normalizes relative codex-web artifact paths such as `/api/codex/artifacts/...` against `CODEX_ORCHESTRATOR_BASE_URL` before storing `preview_asset_key`, so the browser can load images from the orchestrator host instead of the V3 API host.
- Failed image jobs now return the draft to an editable planning state, show the failure reason in the effect-preview panel, and allow the user to submit a fresh image job from the same draft.
- Re-generating an effect image clears stale preview/final-render state so a new job cannot accidentally reuse an old confirmed image or rendered page.
- A live Cloudflare Codex Orchestrator smoke test against `https://souleye.cc` completed with task `task_524ec187-c6df-4e37-9c01-bb21bc481a0b`, `artifactStatus=available`, one PNG image artifact, and a locally downloaded verification copy under ignored `.storage/static-page-smoke-tests/`.
- Remaining work is wiring the deployed V3 worker process to the provided production env file, cancel UX, queue-position fidelity against remote runtime guardrails, and final workerized static-page render/export packaging.

2026-04-28 static page DesignSpec / visual-contract slice completed:

- Reframed the system contract so the effect image is a `visual contract`, while `StaticPageDraft` / `DesignSpec` remains the source of truth for final HTML/CSS/SVG output.
- Frontend static-page drafts now carry `visualSpec`, `renderSpec`, `dataSnapshot`, and `previewContract` alongside modules, layout, data binding, visualization type, style direction, mobile order, image job, preview, and final render state.
- `visualSpec` defines renderer-safe palette, typography, surface, decoration, and density presets for `decision-brief`, `client-delivery`, and `data-command`.
- `renderSpec` declares the renderer-safe contract: 12-column desktop grid, mobile single-column order, DOM text, SVG/chart components, supported editable fields, and generation guardrails that prevent the image model from inventing visuals the renderer cannot reproduce.
- `dataSnapshot` records module-level bindings and evidence/source references so image generation and final rendering can describe the same data contract.
- `previewContract` stores status, image job id, preview asset key, confirmation state, and a design fingerprint. Editing module content, layout, data binding, visualization, mobile order, or style after preview confirmation marks the preview contract stale and clears the stale preview/final render.
- Backend draft creation now seeds the same contract fields, and backend image prompt payloads now include `visual_spec`, `render_spec`, `data_snapshot`, `preview_contract`, and a short `design_contract` explaining that the image is not the final source code.
- Backend queue, preview-ready, confirm, and failed states update `previewContract` / `preview_contract` so the durable draft can tell whether the current bitmap preview still matches the current design.
- `static-page-renderer` now reads `visualSpec`, `renderSpec`, `dataSnapshot`, and `previewContract`, exposes them in the asset manifest, and applies visual-spec colors/radius as CSS variables in the standalone HTML.
- Remaining work is making the renderer fully layout-aware, rendering real chart components from `dataSnapshot`, adding screenshot/VLM visual-diff scoring against the confirmed effect image, and moving final render/export packaging into a background worker.

2026-04-28 static page layout-aware renderer first slice completed:

- The dedicated `static-page-renderer` now renders modules inside a 12-column CSS grid instead of a single-column card stack.
- Each module consumes its `layout` `{ x, y, w, h }` and emits grid column/span, row span, minimum height, module id, and a mobile order variable.
- Mobile rendering degrades to a single-column ordered stack using the existing `mobileOrder`, keeping mobile static-page generation usable without desktop drag precision.
- Chart placeholders have been replaced with first-pass renderer-owned visualizations for `headline`, `kpi-cards`, `bar-chart`, `line-chart`, `donut-chart`, `table`, `timeline`, `risk-matrix`, and `text-insight`.
- Chart data is read from module visualization data, module-level data, or matching `dataSnapshot.moduleBindings`; when no data is present, the renderer uses deterministic fallback points rather than empty broken charts.
- The generated HTML keeps text, metrics, and chart marks as DOM/SVG structures, preserving the rule that final static pages should not be bitmap-only copies of the effect image.
- Renderer tests now cover grid layout output, mobile ordering metadata, visual-spec CSS variables, data-snapshot chart extraction, and multiple chart variants.
- Remaining work is replacing deterministic fallback points with real RAG/data-query snapshots, adding richer data-bound chart semantics, exporting packaged assets, and adding screenshot/VLM visual-diff scoring against the confirmed effect image.

2026-04-28 static page module adjustment UX slice completed:

- Desktop module cards now include a collapsed `微调模块` editor, keeping the default planning canvas clean while still allowing direct adjustment when needed.
- Users can directly adjust module title, module body copy, data binding label, and visualization type from the module card.
- Mobile static-page builder now reuses the same module card editor inside the sortable module list, so mobile users can reorder vertically and still adjust module content without a desktop-only dependency.
- Module adjustments are emitted as a single `update_module` operation, preserving the existing backend operation persistence path and avoiding multiple stale updates from one UI edit.
- A module edit after preview confirmation marks the preview contract stale and clears stale preview/final render state, forcing a fresh effect image before final static page generation.
- The desktop and mobile hints now explicitly tell users they can expand module micro-adjustment for title, content, data, and chart changes.
- Remaining work is model-assisted field suggestions inside the module editor, richer data-source selection from actual dataset fields, and bulk operations for "apply this wording style to all modules".

Verification:

- `node --test app/lib/static-page-draft.test.mjs` passed after module adjustment operation coverage.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-renderer"` passed after layout-aware renderer wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check"` passed after layout-aware renderer wiring.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after DesignSpec / visual-contract wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-renderer -p static-page-worker"` passed after DesignSpec / visual-contract wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p platform-api"` passed after DesignSpec / visual-contract wiring.
- `npm run build` in `apps/web` passed.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p platform-api"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p ingest-worker"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p ingest-worker"` passed, including text, PDF, image OCR-empty, DOCX, XLSX, PPTX, Unicode chunking, and placeholder fallback tests.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p document-vlm-runtime -p ingest-worker && cargo test -p document-vlm-runtime -p ingest-worker"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p document-vlm-runtime -p ingest-worker"` passed, including VLM metadata persistence coverage.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p platform-api -p ingest-worker -p document-vlm-runtime && cargo test -p document-vlm-runtime -p ingest-worker"` passed, including media partial parsing coverage.
- `node --test apps/web/app/lib/upload-classifier.test.mjs` passed, including audio/video detection coverage.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p platform-api -p storage -p ingest-worker -p document-vlm-runtime && cargo test -p document-vlm-runtime -p ingest-worker"` passed after adding registration metadata.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage dataset_metadata"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api dataset_visibility"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p contracts -p storage -p platform-api"` passed.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed.
- `npm run build` in `apps/web` passed after adding dataset visibility header plumbing.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api create_dataset_secret_binding_marks_dataset_private"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api create_dataset_with_secret_fingerprint_returns_private_dataset"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api resolve_dataset_secret_bindings_returns_matching_private_dataset"` passed.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p contracts -p storage -p platform-api"` passed after local secret unlock support.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after local secret unlock support.
- `npm run build` in `apps/web` passed after local secret unlock support.
- `npm run build` in `apps/web` passed after adding selected-dataset local secret binding in the left rail.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p storage assistant_run && cargo test -p platform-api assistant_run"` passed after AssistantRun persistence.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after browser-local AssistantRun thread ids.
- `npm run build` in `apps/web` passed after browser-local AssistantRun thread ids.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p storage conversation_memory && cargo test -p platform-api conversation_memory"` passed after hidden conversation memory storage/API.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p contracts -p storage -p platform-api"` passed after hidden conversation memory storage/API.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after hidden conversation memory frontend sync.
- `npm run build` in `apps/web` passed after hidden conversation memory frontend sync.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p assistant-runtime"` passed after backend scope planner runtime.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_scope_planner_only_uses_visible_datasets && cargo test -p platform-api assistant_run"` passed after AssistantRun scope planner integration.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p assistant-runtime -p contracts -p storage -p platform-api"` passed after AssistantRun scope planner integration.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after consuming backend scope candidates.
- `npm run build` in `apps/web` passed after consuming backend scope candidates.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"` passed after AssistantRun selected-scope evidence supply.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p assistant-runtime -p contracts -p storage -p platform-api"` passed after AssistantRun selected-scope evidence supply.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after AssistantRun selected-scope evidence supply.
- `npm run build` in `apps/web` passed after AssistantRun selected-scope evidence supply.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p assistant-runtime"` passed after AssistantRun hidden conversation-memory injection.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"` passed after AssistantRun hidden conversation-memory injection.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p assistant-runtime -p contracts -p storage -p platform-api"` passed after AssistantRun hidden conversation-memory injection.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after AssistantRun hidden conversation-memory injection.
- `npm run build` in `apps/web` passed after AssistantRun hidden conversation-memory injection.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run_continue_appends_event_trail_and_output"` passed after AssistantRun continue API.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"` passed after AssistantRun continue API.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p assistant-runtime -p contracts -p storage -p platform-api"` passed after AssistantRun continue API.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after AssistantRun continue API.
- `npm run build` in `apps/web` passed after AssistantRun continue API.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p platform-api assistant_run"` passed after AssistantRun frontend continue wiring.
- `node --test app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after AssistantRun frontend continue wiring.
- `npm run build` in `apps/web` passed after AssistantRun frontend continue wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page"` passed after static page draft API creation/update/operation/intent support.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo check -p assistant-runtime -p contracts -p storage -p platform-api && cargo test -p platform-api assistant_run"` passed after static page draft API creation/update/operation/intent support.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after static page backend sync wiring.
- `npm run build` in `apps/web` passed after static page backend sync wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p static-page-runtime && cargo test -p platform-api static_page"` passed after static page intent runtime extraction.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p static-page-runtime -p assistant-runtime -p contracts -p storage -p platform-api && cargo test -p platform-api assistant_run"` passed after static page intent runtime extraction.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after static page intent runtime extraction.
- `npm run build` in `apps/web` passed after static page intent runtime extraction.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page"` passed after static page image/render persistence skeleton.
- `node --test app/lib/static-page-draft.test.mjs` in `apps/web` passed after frontend image/render API wiring.
- `npm run build` in `apps/web` passed after frontend image/render API wiring.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after frontend image/render API wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p platform-api static_page"` passed after frontend image/render API wiring.
- `npm run build` in `apps/web` passed after durable static page shelf wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p platform-api static_page"` passed after durable static page shelf wiring.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after durable static page shelf wiring.
- `npm run build` in `apps/web` passed after moving the active desktop static-page workspace into the main chat area.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after moving the active desktop static-page workspace into the main chat area.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p platform-api static_page"` passed after moving the active desktop static-page workspace into the main chat area.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p static-page-runtime -p assistant-runtime -p contracts -p storage -p platform-api"` passed after moving the active desktop static-page workspace into the main chat area.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p static-page-renderer && cargo test -p platform-api static_page"` passed after extracting the static-page renderer crate.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p static-page-renderer -p static-page-runtime -p assistant-runtime -p contracts -p storage -p platform-api"` passed after extracting the static-page renderer crate.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p static-page-worker && cargo test -p workflow-definitions"` passed after adding the Codex static-page image worker.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page"` passed after automatically starting the static-page image generation workflow.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check -p static-page-worker -p static-page-renderer -p static-page-runtime -p assistant-runtime -p contracts -p storage -p platform-api"` passed after static-page worker integration.
- `npm run build` in `apps/web` passed after static-page worker integration.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after real preview display wiring.
- `npm run build` in `apps/web` passed after real preview display wiring.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p static-page-worker"` passed after artifact URL normalization.
- `node --test app/lib/static-page-draft.test.mjs app/lib/assistant-startup-briefing.test.mjs app/lib/scope-planner.test.mjs app/lib/upload-classifier.test.mjs` in `apps/web` passed after image-job failure/retry UX hardening.
- `npm run build` in `apps/web` passed after image-job failure/retry UX hardening.
- `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --check && cargo test -p static-page-worker"` passed after image-job failure/retry UX hardening.
- Live Cloudflare Codex Orchestrator smoke test passed with a completed `static-page-visual` task and downloadable PNG artifact after reading the runtime key from ignored local storage.

## Locked Product Decisions

- UI shell must visually match the original `ai-data-platform` intelligent assistant, including desktop and mobile.
- The home page right panel remains the report/output shelf. It shows finished reports, published outputs, static-page previews, and saved drafts; it must not become the active planning/editor surface.
- Report and static-page creation flow starts from the main chat interface or model-initiated capability call. The user should not be pushed into a separate report form just to create a report.
- When a report/static-page draft or finished artifact is opened, it occupies the main chat area. The bottom composer remains visible so the user can keep asking for changes in natural language.
- Do not make the static page studio look like a separate SaaS editor, CMS, or admin form.
- Do not resurrect the old static page visual workbench.
- Do not make copied form filling the main interaction.
- Users should describe changes in natural language; the model translates them into draft operations.
- All system capabilities that would otherwise require user form filling must first be understandable and operable by the model. The simple UI exists for review, override, confirmation, and light correction.
- Direct manipulation exists only where it is natural: desktop module drag/resize, mobile vertical module ordering, and light inline corrections.
- Desktop planning and static page construction live inside the assistant page's main chat/workspace area. Finished outputs and saved drafts land in the right-side shelf.
- Mobile must also build the static page inside the assistant page. It is not read-only and not just a status card.
- Mobile uses an in-page `静态页构建` mode: vertical module composition, module order changes, content/data/chart summaries, natural-language edits, queue state, preview confirmation, and final render status.
- The earlier "single clean popup page" idea is superseded by the newer 1:1 UI requirement. If a separate window is later needed, it must still use the same original assistant shell language and not introduce a new product skin.
- The "write fixed styles" wording is removed. The product offers three curated style directions that can be model-selected or user-confirmed.
- If no dataset is selected, the assistant behaves like normal model chat.
- Dataset locking means current selection, not a hard security pin. A dataset selected by the user and a dataset selected during conversation have the same effect: it becomes the preferred supply scope.
- When a dataset is selected, all material supply and full-text retrieval decisions should prefer that selected scope. If the model needs retrieval, it should search within the already selected range unless the user asks to change scope or the selected scope is clearly insufficient.
- The assistant must follow the original product principle: do not over-compose locally. The host supplies directory facts, evidence, details, and execution results; the model answers and chooses the next necessary system capability.
- The system needs controlled continuous execution. The model can ask the host to run multiple steps, while the chat shows a short user-facing step trail such as `正在定位资料 -> 正在读取详情 -> 正在生成报表草稿`.
- Since the UI has an explicit report/static-page output entry, the old mandatory `2选1` report gate is removed. The model may initiate report/static-page capability calls when the user's intent is clear, while destructive/publish/share actions still require confirmation.

## Original Product Capability Memory

These original project decisions are treated as product memory for V3:

- The home workbench is the main interaction surface for normal cloud answers and knowledge-grounded answers.
- The March 2026 original chat design correctly identified the product direction: cloud/model-led answers with local knowledge supply. Its weakness was a coarse `intent/output/references` protocol and too much keyword/special-case behavior over time.
- The April 2026 model-facing capability spec is the stronger reference: memory knows what exists, skills define service behavior, and host execution returns trusted results.
- The old static page workbench remains a technical reference only. Its separate workbench/product-editor shape should not be copied.
- The original system definition is `筛文件、找证据、喂模型`: scope documents, retrieve evidence, and feed the model. It is not a heavy local answer orchestrator.
- Local conversation memory is terminal/browser scoped and is useful for recent upload, collection, grouping, and document-summary feedback.
- Document center owns document operation: upload, grouping, quick parse, deep parse status, document details, and manual grouping corrections.
- Data source workbench owns ongoing collection: web public, web login, discovery, database, ERP, public upload, run records, and target knowledge-library binding.
- Report center owns output templates and finished reports. The original project placed natural-language report adjustment in the home right-side current report workspace; V3 updates this pattern so the active draft opens in the main workspace and the right side stays a draft/output shelf.
- Template output uses shared template assets and a structured envelope: fixed structure, variable zones, output hints, and later normalization.
- Model-facing architecture follows the historical rule: memory knows what exists, skills define task behavior and evidence requirements, host execution performs real actions and returns trusted results.

## Model-Facing Assistant Principles

The model must understand the platform before it answers or operates:

- Product truth: this is a knowledge/data workbench, not a generic chat box.
- Startup briefing: every chat request should let the model know what the system is for and what visible database assets exist at directory level: visible dataset count, document count, estimated word/chunk count, recent uploads/collections, parse/index status, and available capabilities.
- Default lane without selected dataset: ordinary model chat with the platform/database briefing only.
- Enhanced lane: `Material Service`, covering lookup, explanation, comparison, summarization, and evidence-grounded judgment whenever scope planner finds relevant visible datasets, documents, or conversation memory.
- Output lane: `Report Service`, opened by explicit report/static-page UI entry, clear user output intent, or a model-initiated report/static-page capability call.
- Selected dataset semantics: current selection is the preferred supply scope. It can come from the left rail, a conversational selection, or an upload/classification result.
- Context policy: conversation history is local by default and only enters the model request when the current intent needs it.
- Supply policy: host does not write the final answer with rules. Host returns directory state, retrieval evidence, detail facts, comparison facts, draft state, and execution results.
- Execution policy: model may request the next platform capability, but the host must execute the action and return a durable result before the model claims completion.
- Continuous execution policy: one user request may run a bounded loop of 3 to 5 host actions. If evidence or execution remains insufficient, degrade honestly instead of pretending success.
- UI policy: every capability exposed through forms must also be available as a model-operated action. Manual controls are fallbacks and confirmation surfaces.
- Quality policy: token saving is not the primary constraint. If relevant documents are hit and performance/memory focus allow it, prefer RAG/detail supply for current answer quality.

## Assistant Startup Briefing

The model should not begin from a blank generic chat context. Each chat turn receives a compact, model-facing platform briefing:

- What the system does: knowledge/data workbench, document ingestion, retrieval, report/static-page output, data source collection, and controlled platform actions.
- Visible database summary: public datasets plus private datasets unlocked by the current local secret grant.
- Directory metrics: dataset count, document count, estimated word/chunk count, parse/index completion, latest uploads, latest data source collections, and default public categories.
- Current UI state: selected scope, open draft/artifact, active composer mode, visible right-shelf drafts/outputs.
- Available capabilities: ordinary chat, scope planning, retrieval, document detail, media transcript/summary, compare, upload/classify, report planning, static page planning, final render, and controlled actions.

This briefing is directory-level context, not evidence. It lets the model know what exists without pretending it has read full documents.

## Selected Dataset And Retrieval Semantics

Do not treat selected dataset as a rigid lock.

- `selectedDatasetId` means preferred current supply scope.
- User-selected dataset and model-selected dataset are equivalent once active.
- Scope planner may preselect datasets during chat. The left rail must show this selected state and let the user unselect it.
- The UI may show small, low-emphasis scope hints such as `已按订单数据集供料` or `可能相关：客服、网页采集`. The final answer body remains model-authored.
- If a dataset is selected, new chat turns, static page drafts, report creation, upload classification, and retrieval should prefer it.
- If no dataset is selected, the model may use directory awareness and lightweight intent matching to recommend or select a dataset.
- If multiple datasets are plausible, the assistant should either ask a short clarification or run directory-level comparison before full retrieval.
- Full-text/detail retrieval should be more likely when a dataset is selected, because the scope is already narrowed enough to justify deeper reads.
- The UI should show the current selected scope in simple language, not as a technical lock: `当前供料范围：销售简报知识集`.

## Scope Planner And Supply Policy

Add a distinct scope-planning layer before evidence supply.

- Scope planner decides which visible datasets, hidden conversation memory, and candidate documents may be relevant.
- Scope planner does not compose the final answer and does not force a report flow.
- The host executes retrieval/detail/compare based on the planned scope.
- The model receives the supplied evidence and decides the final answer.
- If relevant documents continue to be hit across turns, keep retrieving deeper details within the selected scope.
- Treat conversation history as a hidden dataset. Index user statements, choices, uploads, confirmations, rejected suggestions, draft summaries, and artifact summaries.
- Do not inject entire generated artifacts into context by default. First retrieve what the user said and the minimal summaries/decisions needed.
- Conversation memory follows the same intent logic as normal datasets: it enters supply only when the current turn likely depends on prior conversation.

## Dataset Visibility And Secret Model

Dataset visibility must be enforced by host/backend, not only by UI.

- Public datasets are visible and usable by all users.
- Private datasets are visible and usable only when the current local secret grant matches.
- Datasets with non-matching secrets are invisible, unavailable to retrieval, absent from model startup briefing, and absent from scope planner candidates.
- Creating a dataset without a secret should warn: `该数据集是公开数据集，所有用户可见`.
- Uploading without a private secret should also warn that the document will go into public/default unclassified scope unless the user selects a private dataset.
- Provide default public datasets for common use: `订单`, `客服`, `企业问答`, `网页采集`, and `未分类`.
- Upload classification should prefer an explicitly selected dataset, then matching public/private dataset, then `未分类`.

## Home Report And Static Page Relationship

The home page has three connected surfaces:

- Chat center: starts and controls normal Q&A, report creation, static page creation, and continuous execution.
- Left rail: shows datasets, create dataset at the top, and current supply scope.
- Right panel: always shows finished outputs and saved drafts. It is a shelf, not the active editor.

Report creation behavior:

- User starts report creation from chat or the single output action.
- The chat message explains the short creation steps.
- The active report draft opens in the main chat/workspace area.
- Finished report/static-page artifacts return to the right panel output list.
- Report center pages remain for management/history, not for the main natural-language creation loop.

Static page behavior:

- Static page planning uses the same main workspace pattern as report creation.
- Planning, style selection, image queue, preview confirmation, and final render are all represented as one active workspace in the main area.
- Final static page render becomes a report/output artifact and appears with finished reports.
- Clicking a right-shelf draft or finished artifact opens it in the main area and keeps only the composer visible for follow-up change requests.

## PostgreSQL Runtime Standard

Target server version:

- Fresh and production-like V3 environments should use PostgreSQL `17.9`.
- Pin container images to `postgres:17.9` instead of floating `postgres:17`.
- Current local V3 and Java client containers were observed on PostgreSQL `16.13`; do not reuse a PostgreSQL 16 data volume directly with a PostgreSQL 17 container.
- Upgrade existing local data with an explicit backup/restore or `pg_upgrade` path, not by swapping the image over the same volume.
- Keep Java client compatibility wording aligned: local can remain 16 only for old reusable verification volumes, while target compatibility is PostgreSQL 17.9.
- Update README/compose only when the migration path is ready, or create a separate `compose.postgres17.yml` for fresh verification first.

Architecture note:

- PostgreSQL 17.9 does not change the assistant architecture, but it should influence test matrix and migration discipline.
- Any JSONB/search/index features used for conversation memory, scope planner candidates, and document manifests should be tested on 17.9 before being treated as baseline behavior.

## Architecture Recommendation Update

The previous recommendation needs one important adjustment: do not make Report Service gating the center of the architecture. The stronger center is an assistant run contract with optional supply.

Updated recommendation:

- Build `AssistantRun` as the canonical execution unit above dataset chat sessions.
- `AssistantRun` accepts ordinary chat with no selected dataset.
- `AssistantRun` always includes startup briefing and visible capability metadata.
- `AssistantRun` may call scope planner to select visible datasets and hidden conversation memory.
- `AssistantRun` then executes supply actions such as retrieval/detail/compare when useful.
- `AssistantRun` can open report/static-page capabilities directly when the UI entry or user intent is clear.
- Dataset chat sessions can remain as a persisted specialized view, but they should not be the only way to talk to the assistant.
- The model-facing protocol should expose service capabilities: `ordinary_chat`, `scope_plan`, `retrieve`, `read_detail`, `compare`, `upload_classify`, `report_plan`, `static_page_plan`, `render`, and `controlled_action`.
- The host must enforce visibility and secret grants before startup briefing, scope planning, retrieval, or report generation.
- The UI should be a state viewer and override surface, not a decision-maker. Decisions about supply and next capability belong in the model/host protocol.

This updates the earlier plan in three ways:

- Replace `2选1` as the main report gateway with explicit output entry and model-initiated report capability calls.
- Replace right-panel active editing with main-workspace editing and right-side draft/output shelf.
- Replace dataset-required chat with ordinary chat plus optional scope-planned supply.

## Overall Architecture Rebaseline

V3 should be organized around five layers. This section supersedes older assumptions that static-page/report APIs can hang directly from dataset chat sessions.

### Layer 1: Assistant Shell

Responsibility:

- Owns the visible user experience: left dataset rail, center chat/workspace, right output shelf, bottom composer.
- Stores terminal-local conversation cache and lightweight UI state.
- Shows selected/preselected datasets, small scope hints, execution trail, draft/artifact workspace, and output shelf.
- Does not decide final answer content and does not independently choose business flow by keyword.

### Layer 2: AssistantRun Contract

Responsibility:

- Canonical execution unit for every user turn.
- Accepts ordinary model chat even when no dataset is selected.
- Injects startup briefing, visible capability list, selected scope, hidden conversation memory candidates, and current draft/artifact state.
- Runs scope planner when useful.
- Orchestrates bounded host actions such as retrieval, detail read, compare, upload classify, report plan, static page plan, render, and controlled action.
- Produces model-authored answer plus durable metadata: selected scope updates, evidence state, execution trail, output artifacts, confirmations, and errors.

Why this layer exists:

- Current V3 `chat_session` is dataset-bound. That is too narrow for ordinary chat and for model-selected supply.
- Static page/report generation needs a request-level context that may contain no dataset, one selected dataset, multiple candidate datasets, and hidden conversation memory.
- The model should see platform capabilities and evidence state consistently across Q&A, reports, static pages, and uploads.

### Layer 3: Visibility And Supply

Responsibility:

- Enforce public/private dataset visibility before anything reaches the model.
- Build startup briefing only from visible datasets and visible summaries.
- Build scope planner candidates from visible datasets plus hidden conversation memory.
- Execute RAG/detail/compare supply inside selected/preselected scope.
- Prefer answer quality over token thrift when relevant documents are hit and memory/performance focus allows deeper supply.

Key rule:

- Non-matching private datasets must be invisible to UI, startup briefing, scope planner, retrieval, report generation, and static page generation.

### Layer 4: Output Workspace

Responsibility:

- Report/static-page drafts, visual previews, confirmed renders, and output artifacts live here.
- Active work opens in the main chat/workspace area.
- Right panel is only a shelf for finished outputs and saved drafts.
- Drafts are model-operated through structured operations, with manual controls as review/override surfaces.

Source of truth:

- Final factual content comes from evidence, data bindings, draft modules, and render manifests.
- Generated effect images are visual confirmation artifacts, not factual source of truth.

### Layer 5: Durable Runtime And Providers

Responsibility:

- PostgreSQL stores assistant runs, datasets, visibility grants, documents, retrieval evidence, conversation memory summaries, static page/report drafts, image jobs, render outputs, and published artifacts.
- NATS/workflow tasks accelerate long-running work but are not the source of truth.
- Cloudflare/Codex image provider handles visual generation secrets and returns image artifacts.
- PostgreSQL 17.9 is the fresh/production-like target; 16.13 local volumes require explicit migration.

## Superseded Assumptions

The following earlier assumptions are now outdated:

- Static page draft creation is primarily under `POST /v1/chat-sessions/{session_id}/static-page-drafts`.
- Static page draft creation is primarily under `POST /v1/datasets/{dataset_id}/static-page-drafts`.
- A selected dataset is required before chat or static page planning.
- Report Service entry requires the old mandatory `2选1` gate.
- Active static page planning lives in the right panel.
- Conversation history is only local UI state and not a retrievable hidden data source.
- Dataset visibility can be handled by frontend filtering alone.

Replacement assumptions:

- `AssistantRun` is the primary request context.
- Dataset/session are optional references inside selected scope and source context.
- Report/static-page APIs attach to `assistant_run_id` or accept an explicit assistant context payload.
- Right panel is a shelf; main workspace is the active editor/viewer.
- Conversation memory is a hidden dataset-like supply source.
- Visibility is enforced server-side before model briefing, planner, retrieval, and output generation.

## Visual Design Contract

The V3 assistant should borrow the quieter parts of Codex's own interface language while preserving the original assistant identity:

- Remove visible border-heavy card styling. Lines should be absent or nearly invisible.
- Separate modules by background color, surface tone, spacing, and subtle elevation instead of outlines.
- Use smaller typography than the current V3 shell where density is too high.
- Empty states must be compact. Placeholder pages should take as little vertical space as possible.
- Prefer calm dark surfaces, muted color blocks, and low-contrast separators.
- Avoid large framed forms. Inline controls should feel like lightweight chips, pills, or compact actions.
- Keep the original assistant brand/topbar rhythm, but reduce visual noise in cards and report/static-page modules.
- Mobile should stay dense and direct: one active surface, compact topbar, compact composer, minimal empty-state copy.

## Original Assistant UI Contract

Use the original project at `C:\Users\soulzyn\Desktop\codex\ai-data-platform` as the visual source of truth.

Reference assets already captured:

- `docs/prototypes/original-assistant-desktop.png`
- `docs/prototypes/original-assistant-mobile.png`
- `docs/prototypes/static-page-studio-original-shell-desktop.png`
- `docs/prototypes/static-page-studio-original-shell-mobile.png`

Desktop contract:

- Keep the dark top toolbar gradient and brand block.
- Keep the AI square logo, `智能助手` identity, navigation pills, and status pills.
- Keep the dark left dataset rail.
- Keep the center chat panel as the primary natural-language control surface.
- Keep the right result panel sparse, dark, and task-oriented.
- Static page planning should feel like a new result mode inside the original assistant, not a modal editor.

Mobile contract:

- Keep the fixed full-screen dark assistant shell.
- Keep the compact topbar identity.
- Keep chat as the default single visible surface.
- Keep bottom composer behavior.
- Keep dataset/result surfaces as drawer or switched panels.
- Static page construction appears as an in-page mobile build panel, entered from an assistant message or a top/bottom action.
- The mobile build panel shows a live static page structure preview, vertical module list, style direction, generation state, and final render state.
- Mobile only supports up/down reordering for modules; no freeform grid dragging or arbitrary resizing on phone.
- Mobile users can still complete the whole build flow: plan, adjust, confirm style, queue image, confirm effect image, and generate the final static page.

## User Flow

Desktop:

1. User chats normally in the intelligent assistant. If no dataset is selected, this behaves like ordinary model chat with a compact system/database briefing.
2. Scope planner watches the turn for relevant visible datasets, hidden conversation memory, and candidate documents.
3. If scope planner finds likely matches, the left rail shows selected/preselected datasets and the chat UI shows a small low-emphasis scope hint.
4. Host retrieves evidence/detail when useful and feeds the model; the model decides the final answer body.
5. User asks for a static page/report, the model initiates the capability, or the user clicks the single output action in the composer area.
6. Assistant creates a `StaticPageDraft` or report draft from the current selected supply scope, selected session, hidden conversation memory, and necessary evidence.
7. The active draft opens in the main chat/workspace area, replacing the normal message stream above the composer.
8. The right panel continues to show finished outputs and saved drafts.
9. The planning workspace shows modules already placed on a 12-column grid.
10. Each module card directly shows title, intended content, data source, and visualization type.
11. User can drag/resize modules on desktop.
12. User can ask natural-language changes in the composer, such as `把风险放大一点`, `减少文字`, `换成趋势图`, `整体更像给董事会看的`.
13. Model converts the request into structured operations and refreshes the draft.
14. User confirms one of three style directions.
15. Assistant builds an image prompt payload and submits the queued effect-image job.
16. While waiting, the main workspace shows `资源正在排队，可以联系商务开通高级用户跳过等待。`
17. A generated effect image appears for confirmation.
18. User confirms the effect image.
19. Renderer produces a final static page from confirmed modules, chart/data bindings, and style direction.
20. Final static page returns to the right-side report/output shelf.

Mobile:

1. User remains in the original mobile assistant shell.
2. If no dataset is selected, chat behaves like ordinary model chat with compact platform/database briefing.
3. Scope planner may preselect visible datasets and show them in the mobile scope drawer.
4. Static page draft starts from chat and opens the in-page `静态页构建` mode in the main area.
5. The build mode shows current planning status, live structure preview, style direction, and module list.
6. User can drag modules only vertically to reorder them.
7. User changes content mainly by sending natural-language messages through the composer.
8. The model updates the draft and refreshes the mobile build mode.
9. Queue, preview confirmation, and final static page generation all happen in the same mobile build mode.
10. User can return to chat or open right-shelf drafts without losing the draft.

## Data Model Draft

Frontend shape:

```js
{
  id: 'draft-local-...',
  datasetId: '...',
  sessionId: '...',
  source: {
    conversationSummary: '...',
    selectedMessageIds: [],
    evidenceIds: []
  },
  startupBriefing: {
    visibleDatasetCount: 4,
    visibleDocumentCount: 128,
    estimatedWordCount: 860000,
    latestActivity: '最近上传/采集摘要',
    parseStateSummary: 'quick/deep parse and index state'
  },
  selectedScope: {
    datasetId: '...',
    source: 'user_selected|model_selected|scope_planner|upload_classified',
    label: '当前供料范围'
  },
  scopePlanner: {
    candidates: [
      { type: 'dataset', id: '...', label: '订单', confidence: 'high', reason: '用户询问订单趋势' },
      { type: 'conversation_memory', id: 'local-thread', label: '本轮对话历史', confidence: 'medium', reason: '用户引用了刚才的要求' }
    ],
    hint: '可能相关：订单、对话历史'
  },
  executionTrail: [
    { label: '定位资料', status: 'completed' },
    { label: '生成规划', status: 'running' }
  ],
  status: 'planning',
  objective: '给客户展示当前数据结论，并生成可交付静态页',
  audience: '客户决策层',
  styleDirection: 'decision-brief',
  modelSummary: '模型对当前页面结构的理解',
  mobileOrder: ['hero', 'kpi', 'trend', 'risk', 'next-steps'],
  modules: [
    {
      id: 'hero',
      role: 'hero',
      title: '核心判断',
      content: '先给出一句客户能直接带走的主结论。',
      dataBinding: {
        type: 'conversation_summary',
        label: '来自当前会话摘要',
        sourceId: 'session'
      },
      visualization: {
        type: 'headline',
        label: '大标题 + 关键结论'
      },
      layout: {
        x: 0,
        y: 0,
        w: 12,
        h: 3
      }
    }
  ],
  operations: [],
  imageJob: {
    id: null,
    status: 'idle',
    queuePosition: null,
    queueMessage: ''
  },
  previewImage: null,
  finalPage: null
}
```

Model operation shape:

```js
{
  id: 'op-...',
  type: 'update_module',
  targetModuleId: 'risk',
  reason: '用户要求突出风险',
  patch: {
    title: '主要风险与优先级',
    visualization: { type: 'risk-matrix' },
    layout: { x: 7, y: 3, w: 5, h: 4 }
  }
}
```

Required operation types:

- `update_module`
- `add_module`
- `remove_module`
- `move_module`
- `resize_module`
- `reorder_modules`
- `change_visualization`
- `change_data_binding`
- `change_style_direction`
- `refresh_summary`
- `queue_image_job`
- `confirm_preview`
- `request_final_render`

## Style Directions

Use these as product directions, not hard-coded templates:

- `decision-brief`: 高层决策简报。Dark executive shell, strong conclusion first, KPI and risk emphasis, suited for老板/董事会/甲方决策层.
- `client-delivery`: 客户交付报告。Clean consulting deliverable, more explanation, balanced charts and notes, suited for售前/项目交付/周报月报.
- `data-command`: 数据运营看板。Dense but readable data cockpit, trend/comparison/composition first, suited for运营复盘/经营看板/指标追踪.

Default selection:

- If the user says `给老板`, `决策`, `汇报`, default to `decision-brief`.
- If the user says `给客户`, `交付`, `方案`, default to `client-delivery`.
- If the user says `运营`, `数据`, `指标`, `看板`, default to `data-command`.
- If unclear, model picks one and explains why in one short assistant sentence.

## Visualization Types

First version:

- `headline`: conclusion or hero statement.
- `kpi-cards`: 2-4 key metrics.
- `bar-chart`: category comparison.
- `line-chart`: trend over time.
- `donut-chart`: composition/share.
- `table`: compact evidence table.
- `timeline`: phased roadmap or sequence.
- `risk-matrix`: risk/priority grid.
- `text-insight`: narrative insight block.

Chart layer:

- Use `recharts` for first version because it is React-native, SVG-based, and enough for KPI/bar/line/donut.
- Add Apache ECharts later only for complex dashboards, rich interactions, large-data charts, or advanced composition.

## Open Source Capability Decisions

Dependencies to add in the first implementation slice:

```powershell
pnpm --filter @ai-data-platform-v3/web add react-grid-layout @dnd-kit/core @dnd-kit/sortable @puckeditor/core recharts
```

Commercial and license notes:

- `react-grid-layout`: MIT license. Good direct fit because layout data is `{ x, y, w, h }`, matching `StaticPageDraft.modules[].layout`. No paid requirement for current use.
- `@dnd-kit/core` and `@dnd-kit/sortable`: MIT license. Use for mobile vertical module reordering and accessible drag behavior. No paid requirement.
- `@puckeditor/core`: MIT license. Use carefully as a future page-component config/render adapter. Do not depend on paid Puck AI or hosted services for first version.
- `recharts`: MIT license. Good first chart layer. No paid requirement.
- `echarts`: Apache-2.0 license. Optional later. No paid requirement, but configuration and bundle complexity are higher.

Media parsing commercial and license notes:

- Default first-wave media parsing must work without user account authorization, gated model access, commercial API signup, or paid service terms by using local/open tools. The configured MiniMax key is the one approved exception for this product and may be used when the exact endpoint capability is verified.
- `ffmpeg`: allowed as an external process for audio extraction, metadata probing, transcoding, and keyframe extraction. Use an LGPL-safe distribution path where possible, preserve notices, and avoid bundling/linking GPL or nonfree builds into proprietary product code.
- `faster-whisper`: MIT license. Good default speech-to-text path for server-side audio/video transcription, with CPU/GPU deployment choices.
- `whisper.cpp`: MIT license. Good fallback/edge path when a local C/C++ command runner is easier than Python.
- `PySceneDetect`: BSD-3-Clause license. Good default scene/keyframe segmentation tool for videos.
- `PaddleOCR`: Apache-2.0 license. Good default OCR path for keyframes, screen recordings, subtitles, slides, and document-like video frames.
- MiniMax is approved as a configured provider for this project because the original system already uses MiniMax VLM and the deployment has a validated key. Use it first for the original document-image VLM fallback, then capability-probe audio/video parsing separately.
- MiniMax speech/video generation, voice clone, file upload, or generated-file retrieval endpoints do not by themselves prove audio transcription or video understanding support. Only mark `minimax-audio-transcript`, `minimax-video-understanding`, or `minimax-keyframe-vlm` when the exact request succeeds and returns parseable evidence.
- Do not include `WhisperX` in the default implementation, because its practical speaker diarization path depends on pyannote models or other model terms that require explicit user acceptance.
- Do not include `pyannote.audio` diarization models in the default implementation, because common model repositories require access tokens and terms acceptance even when the code/model license is permissive.
- Do not include video VLMs such as Qwen/InternVL-style models in the default implementation unless the exact model size, weight license, redistribution terms, and commercial-use terms have been checked for the deployment target.
- This video/media VLM exclusion does not remove the original product's MiniMax-backed document image VLM fallback. Keep MiniMax VLM for image documents, scanned PDFs, and rendered presentation pages because the project already has this provider path and a configured key.
- If a customer later supplies explicit non-MiniMax tokens and accepts the relevant model terms, diarization/VLM providers can be added behind an optional provider flag. They must never become required for ordinary upload parsing.

Puck positioning:

- Do not expose Puck as a generic page builder UI in version one.
- Keep our own simple assistant UI.
- Borrow Puck's useful pattern: component config + editable data + independent renderer.
- Add a thin adapter only when the static page component schema stabilizes.

## Implementation Sequence

### Task 0: Protect The Current Direction

**Files:**

- Modify: `docs/plans/2026-04-27-static-page-generation-studio-plan.md`
- Keep reference: `docs/plans/2026-04-25-static-page-visual-workbench-v3-plan.md`
- Keep reference: `docs/prototypes/static-page-studio-original-assistant-shell.html`

**Steps:**

1. Keep this document as the active plan.
2. Keep the old visual workbench plan marked deferred.
3. Do not continue implementing `static-page-studio` as a separate clean popup route unless the user explicitly reverses the 1:1 assistant-shell decision.
4. Use prototype screenshots only as visual references, not as production source.
5. Commit after this plan is accepted.

### Task 1: Match The Original Assistant Shell

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/Sidebar.js`
- Create: `apps/web/app/components/HomeWorkspaceToolbar.js`
- Create: `apps/web/app/components/HomeMobileShell.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Compare V3 shell with original `C:\Users\soulzyn\Desktop\codex\ai-data-platform\apps\web\app\HomePageClient.js`.
2. Add a top workspace toolbar matching the original desktop toolbar.
3. Keep original nav labels: `智能会话`, `数据集`, `采集源`, `静态页`, `成员`, `审计`.
4. Keep original status pill style for system/data/model status.
5. Move V3 current dataset/status summary into the original toolbar visual language.
6. Add `HomeMobileShell` behavior matching the original mobile assistant: topbar, single active panel, bottom composer, drawer-style dataset/results access.
7. Move dataset creation above the dataset list in the left rail.
8. Preserve current V3 data fetching and report-service behavior.
9. Apply the visual design contract: borderless surfaces, color-block separation, smaller typography, compact empty states, and low-contrast separators.
10. Run `pnpm --filter @ai-data-platform-v3/web build`.
11. Manual check desktop at `http://localhost:3100`.
12. Manual check mobile width around `390px`.
13. Commit message: `feat(web): align assistant shell with original UI`.

### Task 1A: Add Home Assistant Product Contract

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Modify: `apps/web/app/components/InsightPanel.js`
- Modify: `apps/web/app/components/Sidebar.js`
- Create: `apps/web/app/lib/home-conversation-cache.js`
- Create: `apps/web/app/lib/assistant-context-policy.js`
- Create: `apps/web/app/lib/assistant-startup-briefing.js`

**Steps:**

1. Allow the chat composer to send without a selected dataset.
2. Store browser/terminal-local conversation state in `localStorage`.
3. Add context-policy decisions: `current_turn_only`, `include_recent_history`, `include_session_summary`, and `include_selected_artifact`.
4. Treat `selectedDatasetId` as preferred supply scope, not a hard lock.
5. Let user-selected and model-selected datasets update the same selected-scope state.
6. When selected scope exists, prefer it for retrieval, full-text detail reads, report creation, and static page drafts.
7. When no selected scope exists, let the model-facing request include directory facts and candidate datasets.
8. Add startup briefing construction: visible dataset count, document count, estimated words/chunks, latest upload/collection, parse/index summary, and available capabilities.
9. Keep the right panel default as finished reports and saved drafts.
10. Open active report/static-page workspaces in the main chat area, not the right panel.
11. Keep only one visible static-page/report output action in the composer area.
12. Add an upload-file button to the bottom composer area.
13. Keep the composer visible when a draft/artifact is open so follow-up changes remain conversational.
14. Run `pnpm --filter @ai-data-platform-v3/web build`.
15. Commit message: `feat(web): add assistant home contract`.

### Task 1C: Add Scope Planner And Hidden Conversation Dataset

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/Sidebar.js`
- Create: `apps/web/app/lib/scope-planner.js`
- Create: `apps/web/app/lib/scope-planner.test.mjs`
- Create: `apps/web/app/lib/conversation-memory-dataset.js`
- Create: `apps/web/app/components/ScopeHint.js`

**Steps:**

1. Add deterministic first-slice scope planner for local development.
2. Scope planner returns candidate visible datasets, candidate hidden conversation memory, confidence, and short reason.
3. Scope planner must not produce final answer text or report structure.
4. Show preselected datasets in the left rail and let the user unselect them.
5. Show a small low-emphasis scope hint in chat when the planner selects supply candidates.
6. Treat conversation history as a hidden dataset with indexed user statements, choices, uploads, confirmations, draft summaries, and artifact summaries.
7. Do not inject full generated artifacts by default; retrieve summaries/decisions first.
8. Add tests for ordinary chat with no dataset, dataset preselection, user unselecting, conversation-memory retrieval, and artifact-summary exclusion.
9. Run `node --test apps/web/app/lib/scope-planner.test.mjs`.
10. Run `pnpm --filter @ai-data-platform-v3/web build`.
11. Commit message: `feat(web): add assistant scope planner`.

### Task 1B: Add Model-Facing Capability Policy

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/chat-session-worker/src/lib.rs`
- Modify: `apps/web/app/lib/platform-api.js`

**Steps:**

1. Add a model-facing platform truth block to assistant/chat requests.
2. Include available service capabilities rather than low-level route names.
3. Add `selected_scope` to requests and responses.
4. Add `startup_briefing`, `scope_planner_candidates`, `evidence_state`, and `context_policy` to model-facing request metadata.
5. Add `execution_trail` items that can be shown as short chat step updates.
6. Support bounded continuous execution for 3 to 5 host actions.
7. Keep ordinary model chat available when no dataset is selected and no scope is hit.
8. Keep Material Service as the default enhanced lane when scope is hit.
9. Remove the mandatory `2选1` report gate; enter Report Service after explicit UI output entry, clear user output intent, or model-initiated report/static-page capability call.
10. Keep confirmation for publish/share/destructive controlled actions.
11. Ensure the host supplies evidence and action results, while the model produces the final answer.
12. Add tests that the worker does not claim completion before a host action result exists.
13. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p contracts -p platform-api -p chat-session-worker"`.
14. Commit message: `feat(runtime): add model-facing assistant policy`.

### Task 1D: Add Dataset Visibility And Default Public Libraries

**Files:**

- Modify: `apps/web/app/components/Sidebar.js`
- Modify: `apps/web/app/HomePageClient.js`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`

**Steps:**

1. Add visibility states: `public` and `private`.
2. Ensure public datasets are visible to all users.
3. Ensure private datasets are visible only when current secret grant matches.
4. Exclude non-matching private datasets from list APIs, scope planner candidates, startup briefing, and retrieval.
5. Add default public datasets: `订单`, `客服`, `企业问答`, `网页采集`, and `未分类`.
6. Warn when creating a dataset without a secret: `该数据集是公开数据集，所有用户可见`.
7. Warn when uploading without a private selected dataset/secret that the file will enter public/default unclassified scope.
8. Add API tests for visibility filtering and scope planner exclusion.
9. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api dataset_visibility"`.
10. Commit message: `feat(datasets): add public private visibility`.

### Task 1E: Add PostgreSQL 17.9 Fresh Verification Path

**Files:**

- Modify: `README.md`
- Modify: `infra/compose/docker-compose.local.yml`
- Create: `infra/compose/docker-compose.postgres17.yml`
- Modify: `docs/plans/2026-04-27-static-page-generation-studio-plan.md`

**Steps:**

1. Keep current local PostgreSQL 16 volume untouched.
2. Add a fresh PostgreSQL 17.9 compose override or explicit `POSTGRES_IMAGE=postgres:17.9` path.
3. Pin production-like documentation to PostgreSQL `17.9`.
4. Document that existing PostgreSQL 16 volumes require backup/restore or `pg_upgrade`, not image swapping.
5. Run migrations against a fresh PostgreSQL 17.9 database.
6. Run `cargo test -p storage` against PostgreSQL 17.9.
7. Run platform API smoke tests against PostgreSQL 17.9.
8. Commit message: `chore(infra): add postgres 17 verification path`.

### Task 1F: Port Original Document MiniMax VLM Fallback

**Files:**

- Create: `crates/document-vlm-runtime/Cargo.toml`
- Create: `crates/document-vlm-runtime/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/ingest-worker/src/lib.rs`
- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `docs/plans/2026-04-27-static-page-generation-studio-plan.md`

**Steps:**

1. Treat the original `document-image-vlm-provider.ts`, `document-image-vlm-capability.ts`, and cloud enrichment visual collection modules as the source reference.
2. Add a provider config layer for `DOCUMENT_IMAGE_PARSE_MODE`, `DOCUMENT_IMAGE_VLM_PROVIDER`, `DOCUMENT_IMAGE_VLM_MODEL`, `MINIMAX_API_KEY`, `MINIMAX_BASE_URL`, and provider timeout/size limits.
3. Keep OCR as the first local baseline. VLM enriches or replaces weak OCR only when configured and available.
4. Support image documents directly by sending the uploaded image to MiniMax VLM.
5. Support scanned PDFs by rendering only a bounded number of pages, then sending page images to MiniMax VLM.
6. Support presentations by rendering only a bounded number of slides/pages, then sending page images to MiniMax VLM.
7. Reuse the original strict JSON intent: visual summary, transcribed text, evidence blocks, field candidates, entities, claims, table/chart detection, and confidence metadata.
8. Convert VLM payloads into canonical text chunks and structured metadata with parse methods such as `image-ocr+vlm`, `pdf-vlm`, and `presentation-vlm`.
9. Preserve dataset visibility and secret filtering before any VLM request is queued or executed.
10. Never claim VLM success if the provider call fails. Keep OCR/parse status and record a recoverable enrichment error.
11. Add deterministic tests with fake VLM provider responses for image, PDF page, presentation page, unavailable provider, and malformed JSON.
12. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p document-vlm-runtime -p ingest-worker document_vlm"`.
13. Commit message: `feat(ingest): add minimax document vlm fallback`.

### Task 1G: Add Media Parse Pipeline With Local And MiniMax Providers

**Files:**

- Create: `crates/media-parse-worker/Cargo.toml`
- Create: `crates/media-parse-worker/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/upload-classifier.js`
- Modify: `apps/web/app/HomePageClient.js`

**Steps:**

1. Extend upload classification to recognize audio/video files: `mp3`, `wav`, `m4a`, `aac`, `flac`, `ogg`, `mp4`, `mov`, `mkv`, `webm`, and `avi`.
2. Register audio/video uploads as normal documents/media assets so they follow the same dataset visibility, secret, selected-scope, and workflow rules as text documents.
3. Add a `media_parse_workflow` that runs after local file storage and before vector/memory sync.
4. Use `ffmpeg`/`ffprobe` only as external commands for metadata, audio extraction, normalization, and optional keyframe extraction. Do not link FFmpeg libraries into Rust product code.
5. Add provider config for `MEDIA_PARSE_MODE`, `MEDIA_PARSE_PROVIDER`, `MEDIA_PARSE_PREFER_MINIMAX`, `MINIMAX_API_KEY`, `MINIMAX_BASE_URL`, `MINIMAX_MEDIA_MODEL`, `MINIMAX_MEDIA_TRANSCRIBE_ENDPOINT`, `MINIMAX_MEDIA_VIDEO_ENDPOINT`, provider timeout, and per-file size/page/frame limits.
6. Add a MiniMax media capability probe before any production request. Probe audio transcript, native video understanding, and image/keyframe VLM separately, and persist the capability matrix for observability.
7. If MiniMax audio transcription is verified, use it as an optional configured provider for audio and extracted video audio. Store parse method `minimax-audio-transcript`.
8. If MiniMax native video understanding is verified, use it only for bounded videos and store parse method `minimax-video-understanding`.
9. If MiniMax native video understanding is not verified but MiniMax image VLM is available, optionally send bounded extracted keyframes as images and store parse method `minimax-keyframe-vlm`.
10. Add pluggable local speech-to-text command adapters in this order when MiniMax is disabled/unavailable or fails: configured `FASTER_WHISPER_BIN`, Python `faster-whisper` runner, configured `WHISPER_CPP_BIN`, then no-transcript partial parse.
11. Store transcript segments with start/end timestamps, detected language when available, confidence-like metadata when available, source parser name, and chunk ids.
12. For videos, optionally run PySceneDetect when installed to produce scene boundaries and representative frame timestamps.
13. For videos, optionally run PaddleOCR when installed on extracted keyframes to capture slide text, subtitles, screen text, and document-like visuals.
14. Build media evidence chunks from MiniMax evidence, transcript segments, scene summaries, OCR snippets, file metadata, and parser status. The host supplies these chunks; the model writes the final summary.
15. Add model-facing media capabilities to startup briefing: media transcript, media summary, scene outline, keyframe OCR, MiniMax media provider status, and report/static-page reuse.
16. Add a media detail API so chat, reports, and static-page drafts can retrieve transcript windows, scene windows, OCR snippets, and provider evidence by timestamp.
17. If media tools and MiniMax parse capabilities are missing, mark parse status as partial instead of falling back to fake transcript text.
18. Exclude WhisperX, pyannote diarization, gated HuggingFace models, non-MiniMax commercial transcription APIs, and non-MiniMax video VLMs from the default path.
19. Keep the original MiniMax-backed document image VLM fallback separate from the media provider path. It remains available for image documents, scanned PDFs, and rendered presentation pages when the configured provider/key is present.
20. Add provider flags for future optional diarization/VLM, but keep non-MiniMax providers disabled unless explicit customer authorization and license acceptance are recorded.
21. Add deterministic tests using fake command adapters and fake MiniMax responses for metadata, local transcript, MiniMax transcript success, MiniMax video unavailable fallback, keyframe VLM success, scenes, OCR, missing-tool partial parse, and no private-data leakage.
22. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p media-parse-worker -p platform-api media_parse"`.
23. Run `node --test apps/web/app/lib/upload-classifier.test.mjs`.
24. Commit message: `feat(media): add local and minimax media parsing`.

### Task 2: Add Static Page Draft Model

**Files:**

- Create: `apps/web/app/lib/static-page-draft.js`
- Create: `apps/web/app/lib/static-page-draft.test.mjs`

**Steps:**

1. Add style direction constants.
2. Add visualization type constants.
3. Add `buildInitialStaticPageDraft({ datasetId, sessionId, conversationSummary, evidenceIds })`.
4. Add `applyStaticPageOperation(draft, operation)`.
5. Add `applyStaticPageOperations(draft, operations)`.
6. Add `buildStaticPageImagePayload(draft, { oneClick })`.
7. Add `buildStaticPageFinalRenderPayload(draft)`.
8. Add bounds validation for desktop grid layout.
9. Add mobile order validation.
10. Add tests for initial draft, module update, desktop resize, mobile reorder, style direction change, and image payload.
11. Run `node --test apps/web/app/lib/static-page-draft.test.mjs`.
12. Commit message: `feat(web): add static page draft model`.

### Task 3: Add Assistant Static Page State

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Create: `apps/web/app/components/static-page/StaticPageAssistantNotice.js`

**Steps:**

1. Add `staticPageDrafts` state keyed by dataset/session.
2. Add `activeStaticPageDraftId` state.
3. Add `handleStartStaticPageDraft({ oneClick })`.
4. Add `handleApplyStaticPagePrompt(prompt)` for natural-language updates.
5. Add a single compact output action near the chat controls, matching original button style.
6. When user asks for a static page in chat, create a draft without requiring a form.
7. Reuse the same action entry for report/static-page output when the model detects explicit output intent.
8. Show a compact assistant notice when a draft is created or refreshed.
9. Keep report-service entry behavior unchanged.
10. Run `pnpm --filter @ai-data-platform-v3/web build`.
11. Commit message: `feat(web): add static page assistant state`.

### Task 4: Add Desktop Planning Main Workspace

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Modify: `apps/web/app/components/InsightPanel.js`
- Create: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Create: `apps/web/app/components/static-page/StaticPagePlanningCanvas.js`
- Create: `apps/web/app/components/static-page/StaticPageModuleCard.js`
- Create: `apps/web/app/components/OutputShelf.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Add a `静态页规划` active workspace inside the main chat/workspace area.
2. Keep the right panel as a finished-output and saved-draft shelf at all times.
3. Use `react-grid-layout` for the desktop planning canvas.
4. Configure a 12-column desktop layout.
5. Render module cards from `draft.modules`.
6. Each card must show title, content summary, data binding, visualization type, and model note.
7. On drag/resize, emit `move_module` and `resize_module` operations.
8. Keep visual styling aligned with original dark result panel.
9. Use color blocks and surface tone instead of visible borders.
10. Keep the bottom composer visible below the active workspace for natural-language changes.
11. Clicking a right-shelf draft or finished artifact opens it in the main workspace.
12. Keep editing controls light; avoid a large form-first editor.
13. Run `pnpm --filter @ai-data-platform-v3/web build`.
14. Commit message: `feat(web): add desktop static page planning workspace`.

### Task 5: Add Mobile Static Page Build Mode

**Files:**

- Modify: `apps/web/app/components/HomeMobileShell.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Create: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Create: `apps/web/app/components/static-page/StaticPageMobileModuleList.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Add a mobile `静态页构建` panel inside the existing assistant page, not a popup and not a separate product skin.
2. Let the user enter the panel from an assistant message, the single output action, or the mobile result switcher.
3. Show status, style direction, live structure preview, image preview area, and module list.
4. Use `@dnd-kit/core` and `@dnd-kit/sortable`.
5. Use `verticalListSortingStrategy`.
6. Only allow vertical reordering on mobile.
7. Convert reorder results into `reorder_modules` operations.
8. Show each module's title, content summary, data source, and visualization type.
9. Do not show desktop grid handles on mobile.
10. Keep the original mobile topbar and navigation behavior.
11. Keep natural-language editing available from the mobile composer or an inline prompt bar.
12. Support the complete mobile build flow: planning, style confirmation, queue, effect preview confirmation, and final static page render status.
13. The mobile panel may use a full-height in-page view, but it must preserve a clear return path to chat.
14. Run `pnpm --filter @ai-data-platform-v3/web build`.
15. Manually test touch-like ordering and build-flow progression in browser mobile emulation.
16. Commit message: `feat(web): add mobile static page build mode`.

### Task 6: Add Model-Operated Intent Handling

**Files:**

- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Modify: `apps/web/app/HomePageClient.js`
- Create: `apps/web/app/components/static-page/StaticPageIntentSummary.js`

**Steps:**

1. Add `interpretStaticPagePromptLocally(draft, prompt)` as a deterministic first slice.
2. Return structured operations, not free text patches.
3. Support Chinese prompts for changing tone, adding data, reducing text, highlighting risk, switching chart type, reordering modules, and changing style direction.
4. Show `模型理解` summary in the static page planning workspace and mobile build panel.
5. Let users override by sending another natural-language message.
6. Tests cover at least six prompt examples.
7. Run `node --test apps/web/app/lib/static-page-draft.test.mjs`.
8. Run `pnpm --filter @ai-data-platform-v3/web build`.
9. Commit message: `feat(web): interpret static page edit prompts`.

### Task 7: Add Style Direction Confirmation

**Files:**

- Create: `apps/web/app/components/static-page/StaticPageStyleDirectionPicker.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Show three style directions: `高层决策简报`, `客户交付报告`, `数据运营看板`.
2. Let the model preselect one direction from the user's wording.
3. Let the user confirm or switch direction.
4. Keep the picker compact and visual, not a template marketplace.
5. Persist style choice as `change_style_direction`.
6. Run `pnpm --filter @ai-data-platform-v3/web build`.
7. Commit message: `feat(web): add static page style directions`.

### Task 8: Add Queue And Effect Preview Mock

**Files:**

- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Add image job states: `idle`, `queued`, `running`, `preview_ready`, `failed`.
2. Add queue copy: `资源正在排队，可以联系商务开通高级用户跳过等待。`
3. Add a deterministic mock preview before real image bytes are connected.
4. Add `确认效果图`, `重新生成`, and `继续修改规划`.
5. Confirmed preview moves draft status to `effect_confirmed`.
6. Run `pnpm --filter @ai-data-platform-v3/web build`.
7. Commit message: `feat(web): add static page image queue mock`.

### Task 9: Add Final Static Page Mock Render

**Files:**

- Create: `apps/web/app/components/static-page/StaticPageFinalRender.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Render a final static page mock from confirmed modules in the main workspace.
2. Use `recharts` for first real chart rendering where data exists.
3. Use placeholders only when data binding has no numeric data yet.
4. Keep final render visually close to the selected style direction.
5. Show clear status that backend renderer is not connected yet.
6. Run `pnpm --filter @ai-data-platform-v3/web build`.
7. Commit message: `feat(web): add static page final render mock`.

### Superseded Backend Tasks 10-15

The earlier backend Tasks 10-15 are superseded by the `AssistantRun` architecture above.

Do not implement these old assumptions directly:

- `POST /v1/chat-sessions/{session_id}/static-page-drafts` as the primary static page entry.
- `POST /v1/datasets/{dataset_id}/static-page-drafts` as the primary static page entry.
- Runtime functions that accept only `dataset context + conversation evidence`.
- Static page drafts that require `dataset_id` or `chat_session_id` as the root owner.

Those shapes may remain as compatibility helpers later, but they must call into the assistant-run based APIs.

### Task 10A: Add AssistantRun Domain, Contracts, And Storage

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add `AssistantRunId`.
2. Add `AssistantRun` domain struct with tenant, local thread id, user prompt, startup briefing, selected scope, scope planner candidates, context policy, evidence state, service lane, execution trail, output artifact refs, and timestamps.
3. Add selected-scope contracts that can reference dataset ids, document ids, conversation memory ids, and output artifact ids.
4. Add startup briefing contract with visible dataset count, visible document count, estimated words/chunks, latest upload/collection summary, parse/index summary, and capability list.
5. Add scope planner candidate contract with type, id, label, confidence, and reason.
6. Add execution trail contract with short user-facing labels and durable status.
7. Add PostgreSQL tables for assistant runs and assistant run events.
8. Add repository methods to create a run, append events, update selected scope, update evidence state, and attach output artifacts.
9. Add tests for ordinary no-dataset run creation, selected-scope run creation, event append order, and output artifact attachment.
10. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p domain-model -p contracts -p storage assistant_run"`.
11. Commit message: `feat(assistant): add assistant run domain`.

### Task 10B: Add Visibility And Secret Filtering To Assistant Context

**Files:**

- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `apps/web/app/lib/platform-api.js`

**Steps:**

1. Add dataset visibility fields: `public` and `private`.
2. Add secret-grant request metadata to assistant context endpoints.
3. Filter dataset lists server-side by public visibility plus matching secret grants.
4. Ensure non-matching private datasets are excluded from startup briefing.
5. Ensure non-matching private datasets are excluded from scope planner candidates.
6. Ensure non-matching private datasets are excluded from retrieval/detail/report/static-page APIs.
7. Add default public datasets: `订单`, `客服`, `企业问答`, `网页采集`, and `未分类`.
8. Add create-dataset response warning when no secret grant is active.
9. Add upload-target warning when no private selected dataset or matching secret exists.
10. Add tests for private dataset invisibility in list, briefing, planner, and retrieval.
11. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api dataset_visibility"`.
12. Commit message: `feat(datasets): enforce visibility in assistant context`.

### Task 10C: Add Hidden Conversation Memory Dataset

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/home-conversation-cache.js`

**Steps:**

1. Add `ConversationMemoryItem` domain struct for local thread id, role, item kind, summary, source message ids, artifact refs, and timestamps.
2. Add item kinds: `user_statement`, `user_choice`, `upload_event`, `classification_event`, `draft_summary`, `artifact_summary`, and `rejected_suggestion`.
3. Add storage for conversation memory summaries without storing large artifact payloads by default.
4. Add API to submit local conversation memory summaries from the browser.
5. Add API to retrieve conversation memory candidates for an assistant run.
6. Ensure full generated artifacts are not injected unless specifically selected or summarized.
7. Add tests for user statement retrieval, draft summary retrieval, and artifact payload exclusion.
8. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage -p platform-api conversation_memory"`.
9. Commit message: `feat(assistant): add hidden conversation memory`.

### Task 10D: Add Scope Planner Runtime

**Files:**

- Create: `crates/assistant-runtime/Cargo.toml`
- Create: `crates/assistant-runtime/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/chat-session-worker/src/lib.rs`

**Steps:**

1. Add deterministic scope planner for tests and local fallback.
2. Add provider-backed scope planner behind config.
3. Scope planner input includes user prompt, startup briefing, selected scope, visible datasets, recent conversation memory candidates, and open artifact state.
4. Scope planner output includes visible dataset candidates, document candidates, conversation memory candidates, confidence, and reason.
5. Scope planner must not return final answer text.
6. Persist planner output on the assistant run.
7. Update selected/preselected scope when confidence is sufficient.
8. Add tests for ordinary chat with no scope hit, dataset candidate hit, multiple candidates, hidden conversation memory hit, and no private leakage.
9. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p assistant-runtime"`.
10. Commit message: `feat(assistant): add scope planner runtime`.

### Task 11: Add AssistantRun API

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/platform-api.js`
- Modify: `apps/web/app/HomePageClient.js`

**Steps:**

1. Add `POST /v1/assistant-runs`.
2. Request accepts prompt, local thread id, selected scope, current draft/artifact id, context policy hint, and secret grants.
3. Response returns assistant run id, answer message, selected scope update, scope planner candidates, evidence state, execution trail, output artifacts, and required confirmations.
4. Add `GET /v1/assistant-runs/{run_id}`.
5. Add `POST /v1/assistant-runs/{run_id}/events`.
6. Add `POST /v1/assistant-runs/{run_id}/continue` for bounded continuous execution.
7. Ensure ordinary no-dataset chat works.
8. Ensure selected-scope RAG/detail supply works.
9. Ensure Report Service can be entered by explicit output intent or model-initiated capability call.
10. Add platform-api tests for no-dataset chat, selected dataset supply, model-selected scope, and report capability entry.
11. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"`.
12. Run `pnpm --filter @ai-data-platform-v3/web build`.
13. Commit message: `feat(assistant): add assistant run API`.

### Task 12: Add AssistantRun-Based Static Page Draft Domain

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add `StaticPageDraftId`, `StaticPageImageJobId`, and `StaticPageRenderOutputId`.
2. Add domain structs for draft, module, operation, image job, and final render output.
3. Root static page drafts under `assistant_run_id`.
4. Store optional source refs: dataset ids, document ids, chat session id, conversation memory ids, and report plan id.
5. Store selected scope snapshot and visibility snapshot used at draft creation.
6. Add statuses matching frontend draft states.
7. Add contract request/response views.
8. Add storage tables and repository methods.
9. Add tests for draft creation from no-dataset assistant run, selected dataset assistant run, operation append, image job transition, and final render output persistence.
10. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p domain-model -p contracts -p storage static_page"`.
11. Commit message: `feat(static-page): add assistant run draft domain`.

### Task 13: Add AssistantRun-Based Static Page API

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/platform-api.js`
- Modify: `apps/web/app/HomePageClient.js`

**Steps:**

1. Add `POST /v1/assistant-runs/{run_id}/static-page-drafts`.
2. Add `GET /v1/static-page-drafts/{draft_id}`.
3. Add `PATCH /v1/static-page-drafts/{draft_id}`.
4. Add `POST /v1/static-page-drafts/{draft_id}/operations`.
5. Add `POST /v1/static-page-drafts/{draft_id}/intent`.
6. Keep dataset/chat-session specific endpoints out of the primary path.
7. If compatibility endpoints are later required, make them create an assistant run internally.
8. Wire frontend to use API when available and local model only as development fallback.
9. Add platform-api tests.
10. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page"`.
11. Run `pnpm --filter @ai-data-platform-v3/web build`.
12. Commit message: `feat(static-page): add assistant run draft API`.

### Task 14: Add Provider-Backed Intent Interpretation

**Files:**

- Create: `crates/static-page-runtime/Cargo.toml`
- Create: `crates/static-page-runtime/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add runtime function accepting draft, prompt, assistant run context, startup briefing, selected scope, supplied evidence, and hidden conversation memory refs.
2. Return `StaticPageDraftOperation[]`.
3. Keep deterministic fallback for tests.
4. Put provider-backed model call behind config.
5. Reject unsafe or unrecognized operations instead of applying freeform JSON blindly.
6. Store model reasoning summary and operation list.
7. Add tests for Chinese prompts, selected-scope changes, conversation-memory references, and invalid operation rejection.
8. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-runtime"`.
9. Commit message: `feat(static-page): add model intent runtime`.

### Task 15: Add Image Queue, Preview Confirmation, And Final Renderer

**Files:**

- Create: `crates/static-page-worker/Cargo.toml`
- Create: `crates/static-page-worker/src/main.rs`
- Create: `crates/static-page-renderer/Cargo.toml`
- Create: `crates/static-page-renderer/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/workflow-definitions/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/platform-api.js`
- Modify: `apps/web/app/components/static-page/StaticPageFinalRender.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`

**Steps:**

1. Add workflow kind or task payload for `static_page_image_generation`.
2. Add `POST /v1/static-page-drafts/{draft_id}/image-jobs`.
3. Add `GET /v1/static-page-image-jobs/{job_id}`.
4. Build image prompt payload from confirmed modules, style direction, selected scope, supplied evidence summaries, and data bindings.
5. Use the configured Cloudflare/Codex image endpoint.
6. Treat text-only completion with no image artifact as a failed job.
7. Persist queue status, queue position if available, preview asset key, and failure reason.
8. Add `POST /v1/static-page-image-jobs/{job_id}/confirm`.
9. Persist confirmed preview asset key.
10. Block final render until preview is confirmed.
11. Allow `重新生成效果图` to submit a new job from the same draft.
12. Add renderer consuming confirmed draft modules, data bindings, style direction, selected scope snapshot, and confirmed preview metadata.
13. Produce final HTML and asset manifest.
14. Render charts from bound data.
15. Add `POST /v1/static-page-drafts/{draft_id}/renders`.
16. Add frontend polling for image job and render output.
17. Add tests that final HTML includes all module titles, data labels, chart placeholders or chart data, style direction class, and no private data outside visibility snapshot.
18. Commit message: `feat(static-page): generate and render static pages`.

## First Recommended Execution Slice

Implement Tasks 1, 1A, 1C, 2-9 first. Start Task 1B and 1D once the frontend contract is visually stable, because they touch durable chat/runtime and dataset visibility semantics. Run Task 1E as a separate infrastructure verification slice before treating PostgreSQL 17.9 as the local default.

Reason:

- It validates the full customer-visible flow before backend queue and renderer complexity.
- It locks the 1:1 original assistant shell before adding more product logic.
- It fixes the home contract first: ordinary chat without selected dataset, startup briefing, scope planner, selected scope semantics, one output action, upload button, right-side output shelf, and compact Codex-style visual density.
- It validates that active drafts open in the main workspace while the right side remains saved drafts and finished outputs.
- It gives backend work a concrete draft schema and operation contract.
- It avoids wasting time on a separate popup/editor surface the product direction no longer wants.
- It keeps model-facing continuous execution explicit without blocking the first customer-visible shell improvements.

Do not start Tasks 10-15 until desktop and mobile static page/report creation both feel correct in the assistant shell.

## Verification Commands

Frontend:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs
pnpm --filter @ai-data-platform-v3/web build
```

Rust later:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check --workspace"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test"
```

Manual acceptance:

- Desktop visually matches the original intelligent assistant shell.
- Mobile visually matches the original mobile assistant shell.
- Right panel always behaves as finished-output and saved-draft shelf.
- Active report/static-page creation opens in the main chat/workspace area.
- Opening a right-shelf draft or finished artifact fills the main area and keeps the composer visible.
- Chat can send without a selected dataset and behaves like ordinary model chat when no scope is hit.
- Startup briefing gives the model compact platform/database awareness without full evidence injection.
- Scope planner can preselect likely visible datasets and hidden conversation memory.
- Scope planner hints are shown in small low-emphasis UI, while the final answer remains model-authored.
- Selected dataset is shown as current supply scope, not as a hard lock.
- User-selected and model-selected datasets behave the same after selection.
- The user can cancel/unselect model-preselected datasets from the left rail.
- When a dataset is selected, retrieval/detail/report/static-page actions prefer that selected scope.
- Relevant document hits lead to RAG/detail supply when performance and memory focus allow it.
- Audio/video uploads are treated as first-class knowledge materials after local media parsing or verified MiniMax media parsing.
- Default media parsing can produce transcript segments, scene boundaries, keyframe OCR snippets, MiniMax provider evidence when verified, and model-authored summaries.
- Media parsing must degrade to partial parse status when local tools are missing and MiniMax cannot provide the exact parse capability; it must not invent transcripts.
- Media parsing must not require WhisperX, pyannote, gated HuggingFace models, non-MiniMax commercial transcription APIs, or non-MiniMax video VLM authorization in the default product path.
- Conversation history remains browser-local by default and only enters model context when policy says it is needed.
- Conversation history is treated as a hidden dataset and does not inject full generated artifacts by default.
- The bottom composer has an upload-file button.
- There is only one visible static-page/report output action.
- Dataset creation is above the dataset list in the left rail.
- Default public datasets exist for `订单`, `客服`, `企业问答`, `网页采集`, and `未分类`.
- Creating a dataset without a secret warns that it is public and visible to all users.
- Users only see public datasets and private datasets unlocked by matching local secret grants.
- Non-matching private datasets are absent from UI, startup briefing, scope planner, and retrieval.
- Host behavior follows `筛文件、找证据、喂模型`; local code does not over-compose the final answer.
- Controlled continuous execution shows short user-facing step progress in chat.
- Visible card borders are removed or nearly invisible; modules are separated by color/surface tone.
- Typography and empty states are compact.
- Static page generation can start from natural-language chat.
- Desktop main workspace shows planned modules on a grid.
- Desktop modules can be moved and resized.
- Module cards show title, content, data source, and visualization type without opening a form.
- Mobile shows an in-page static page build mode, not just a status card.
- Mobile modules can be reordered vertically.
- Mobile can complete the static page build flow through final render status.
- Natural-language changes update the whole draft through model operations.
- Three style directions are available and model-selectable.
- Queue state shows the business upgrade copy.
- Effect preview must be confirmed before final static page render.
- Final static page is rendered from confirmed modules/data, not from image pixels alone.
