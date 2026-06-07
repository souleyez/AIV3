# Video/PPT Deliverable Smoke

Use this checklist for controlled video/PPT extraction validation. It is meant to prove the DataMax deliverable contract, not to bypass video-platform access limits.

## Scope

Supported sources:

- Uploaded video files
- Direct video URLs
- Public pages that expose a direct video asset through supported HTML fields

Out of scope:

- Login-gated pages
- QR login
- Cookies or private hosts
- Browser recording or playback bypass

## Local Deterministic Check

Run these before any jump-host smoke:

```powershell
cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete
npm run test:video-deliverables
git diff --check
```

The media-worker test builds a controlled sample path in a temporary directory. The Node validator checks the same public deliverable contract that a real smoke output must satisfy:

- `video_slides_screenshot_based.pptx`
- `final_deliverables_manifest.json`
- `published_deliverable_manifest.json`
- `published_version_history.json`
- `extraction_artifacts_manifest.json`
- `slide_rectangles_manifest.json`
- `slide_notes.md`
- `video_slides.md`
- `subtitle_page_map.json`
- PPTX ZIP magic and required OOXML entries such as `[Content_Types].xml`, `ppt/presentation.xml`, and `ppt/slides/slide1.xml`
- final manifest status flags and output groups
- published manifest type, immutable version metadata, lifecycle state, and redacted file coverage
- published version history type, package-scoped history status, latest immutable `v1`, published manifest pointer, and redacted version file coverage
- slide rectangle manifest status, detector or full-frame fallback crop boxes, selected keep-list de-duplication status, exact selected-frame duplicate metadata when present, and `review_required=true`
- detector crop application in PPTX slide XML through DrawingML `a:srcRect` when a detector crop exists
- extraction manifest file-kind coverage
- public manifest, JSON, and Markdown redaction for local paths and token-like URLs

## Jump-Host Smoke Check

Real Codex validation belongs on `windows-jump` or the later Mac host. Do not run real `codex exec` on the developer workstation.

First confirm the jump-host validator path itself is healthy:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-jump-host-video-deliverable-smoke.ps1 -SelfTest
```

The self-test sends the local validator to `windows-jump`, creates a tiny temporary fixture there, and validates it on the jump host. It does not require the repository to be checked out on the remote host.

After a jump-host media smoke produces a `video-extraction-<document_id>` session directory, run the validator against that session directory or its `generated_artifacts` child:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-jump-host-video-deliverable-smoke.ps1 `
  -RemoteDeliverablesPath "C:\Users\soulz\codex-host\tasks\<task>\video-extraction-<document_id>"
```

For machine-readable output:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-jump-host-video-deliverable-smoke.ps1 `
  -RemoteDeliverablesPath "C:\Users\soulz\codex-host\tasks\<task>\video-extraction-<document_id>" `
  -Json
```

Expected result:

```text
OK video deliverables: ...
ok pptx video_slides_screenshot_based.pptx ...
ok final_deliverables_manifest final_deliverables_manifest.json ...
ok published_deliverable_manifest published_deliverable_manifest.json ...
ok published_version_history published_version_history.json ...
ok extraction_artifacts_manifest extraction_artifacts_manifest.json ...
ok slide_rectangles_manifest slide_rectangles_manifest.json ...
ok slide_notes slide_notes.md ...
ok video_slides_markdown video_slides.md ...
ok subtitle_page_map subtitle_page_map.json ...
```

Do not commit generated smoke artifacts, local task directories, provider keys, `.storage`, or raw logs. If the validator fails, fix the first concrete contract gap in the worker/API/UI path and rerun the smallest relevant tests.

## PPTX Structure Validation Follow-Up

Follow-up validated commit: `e8e5538`.

The local workstation ran:

```text
npm run test:video-deliverables
cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete
git diff --check
powershell -ExecutionPolicy Bypass -File .\scripts\run-jump-host-video-deliverable-smoke.ps1 -SelfTest
```

The deployment target `8服务器` pulled `e8e5538` and ran:

```text
npm run test:video-deliverables
```

Result: passed.

The validator now rejects PPTX files that only have ZIP magic bytes and rejects ZIP containers that do not expose the required presentation OOXML entries through the ZIP central directory. The jump-host self-test fixture now creates a tiny structured ZIP instead of a 4-byte placeholder.

## Published Version Manifest Follow-Up

The public deliverable contract now also requires `published_deliverable_manifest.json` and `published_version_history.json` for complete video/PPT packages. The published manifest records `manifest_type=v3.video_ppt_published_deliverable.v1`, `lifecycle_state=published_version_ready`, `immutable_version=true`, `version_no=1`, and redacted file entries for the PPTX, final manifest, extraction manifest, slide notes, `video_slides.md`, subtitle map, published manifest, and version history.

`published_version_history.json` records `manifest_type=v3.video_ppt_published_version_history.v1`, `status=history_ready`, `history_scope=generated_artifact_workspace`, latest immutable `v1`, the `published_deliverable_manifest.json` pointer, and redacted file entries for the complete published package. Complete packages are now also promoted into durable storage through `published_video_ppt_packages` and `published_video_ppt_versions`; the stored manifest keeps redacted artifact pointer metadata and does not copy private source media, local paths, provider payloads, cookies, or tokens.

Run the same local and jump-host checks after changing this contract:

```text
npm run test:video-deliverables
cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete
cargo test -p storage auth_migrations_are_registered_in_order
powershell -ExecutionPolicy Bypass -File .\scripts\run-jump-host-video-deliverable-smoke.ps1 -SelfTest
```

### 2026-05-15 Target Durable History Enablement

Deployment target `8服务器` pulled commit `5707747` at `/srv/aiv3/repo` and ran:

```text
cargo test -p storage auth_migrations_are_registered_in_order --lib
cargo test -p media-worker durable_published_version --lib
CC=clang CXX=clang++ cargo build -p platform-api -p media-worker --release
```

After the release build, `aiv3-platform-api.service` and `aiv3-media-worker.service` were restarted and confirmed active. The target database now exposes both durable published-version tables: `published_video_ppt_packages` and `published_video_ppt_versions`.

The target host still needs `CC=clang CXX=clang++` for release builds because its default GCC 10 toolchain hits the known `aws-lc-sys` compiler issue.

## Slide Rectangle Manifest Follow-Up

The public deliverable contract now also requires `slide_rectangles_manifest.json` for complete video/PPT packages. The current promoted mode is intentionally conservative: selected keep-list frames are de-duplicated in selection order, exact duplicate selected frame bytes are removed before PPTX generation, and decodable JPEG/PNG frames also pass through a conservative visual-similarity dedupe to reject near-identical selected frames caused by compression or tiny capture differences. Remaining JPEG/PNG frames may be exported as `border_background_contrast_v2` detector crops when a clear non-background rectangle exists, refined as `foreground_component_v1` crops when a dominant connected slide component should exclude external foreground overlays, exported as `edge_projection_v1` crops when a non-uniform background makes the median-background crop unsafe but strong rectangular edge lines remain, or exported as `bright_canvas_v1` crops when a low-contrast background still contains a large bright slide canvas. The public validator still accepts older `simple_background_contrast_v1` manifests. Ambiguous or undecodable frames remain relative full-frame fallback crops. Every crop still requires `review_required=true`.

### 2026-05-15 Target Foreground Component Crop Enablement

Deployment target `8服务器` pulled commit `afb5a4d` at `/srv/aiv3/repo` and ran:

```text
cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo build -p media-worker --release
```

After the release build, `aiv3-media-worker.service` was restarted and confirmed active. The service log showed a fresh NATS connection and `media` queue polling startup.

Generated PPTX slides apply detector crop boxes through DrawingML `a:srcRect`. Full-frame fallback crops intentionally omit `a:srcRect` and keep the previous whole-frame rendering behavior.

Generated PPTX slides also include redacted native picture alt text. The alt text records slide number, candidate number, source frame file name, timestamp, crop mode, transcript segment count, and `paths redacted`; it must not contain raw local frame paths, source URLs, cookies, tokens, or provider payloads.

Deployment target `8服务器` pulled commit `5a3a96b`, passed the PPTX slide XML metadata regression test, rebuilt `media-worker` with `CC=clang CXX=clang++`, restarted `aiv3-media-worker.service`, and confirmed it active.

This is a data-quality gate and review contract, not a claim that visual slide-boundary detection is complete. A later slice should replace the simple background-contrast detector with a stronger visual detector. Exact/visual duplicate removal is already part of the public review contract: when selected candidates are removed, summaries now surface `selected_slide_duplicates_removed` and a `review_slide_dedupe_manifest` follow-up action.

## Markdown Deck Follow-Up

Complete video/PPT packages now also include `video_slides.md` as a final-output Markdown deck next to the screenshot PPTX. It mirrors the selected slide order, source frame file names, contact-sheet anchors, crop status, relative crop box, and available aligned narration without exposing local paths or source URLs. The validator treats this file as a required redacted Markdown deliverable so third parties can inspect the package without opening PPTX first.

## 2026-06-07 Priority-Lane Local Completion Receipt

Task source: `docs/plans/datamax-active-execution-plan.md` Task 3, "Video Extraction To PPT Deliverable Priority Lane".

Scope proven locally:

- supported source boundary remains uploaded video files, direct video URLs, and public pages that expose a direct video asset;
- login-gated pages, QR login, cookies, private hosts, browser recording, and playback bypass remain out of scope;
- the deterministic deliverable contract requires `video_slides_screenshot_based.pptx`, `video_slides.md`, `slide_notes.md`, `subtitle_page_map.json`, `slide_rectangles_manifest.json`, `extraction_artifacts_manifest.json`, `final_deliverables_manifest.json`, `published_deliverable_manifest.json`, and `published_version_history.json`;
- public JSON/Markdown and assistant-visible payloads must redact local paths, source URLs, cookies, provider payloads, and token-like strings;
- AssistantRun model-completion handoff remains queue-backed through `assistant_run / consume_model_completion_turn`.

Current path audit:

```text
rg -n "video_extraction|VideoExtraction|extract_video_ppt|video_slides|pptx|PublishedVideoPpt|wechat_video|media-worker|assistant_run_model_completion" crates apps scripts docs -g "*.rs" -g "*.js" -g "*.mjs" -g "*.md" -g "*.sh" -g "*.ps1"
```

Result: 828 matching references across media-worker, workflow definitions, storage/domain durable published-version models, platform API/react-agent tools, web artifact manifest rendering, validators, scripts, and validation ledgers.

Local deterministic checks at `26afa6e`:

```text
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p media-worker durable_published_version --lib
CC=clang CXX=clang++ cargo test -p media-worker model_completion --lib
CC=clang CXX=clang++ cargo test -p storage auth_migrations_are_registered_in_order --lib
CC=clang CXX=clang++ cargo test -p domain-model workflow_kind_roundtrips_video_extraction
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
CC=clang CXX=clang++ bash scripts/run-assistant-run-worker-smoke.sh
```

Result:

- `npm run test:video-deliverables` passed, 15 tests.
- `controlled_video_sample_deliverable_contract_is_complete` passed.
- `slide_rectangle` passed, 4 tests.
- `durable_published_version` passed, 2 tests.
- `model_completion` passed.
- `storage auth_migrations_are_registered_in_order` passed.
- `domain-model workflow_kind_roundtrips_video_extraction` passed.
- `platform-api video_extraction` passed, 3 tests.
- `platform-api video_ppt` passed, 4 tests.
- `scripts/run-assistant-run-worker-smoke.sh` passed; report: `target/assistant-run-worker-smoke/assistant-run-worker-smoke-20260607T021243Z.md`.
- The assistant-run-worker smoke DB-backed consumer check was intentionally skipped because it requires an explicitly disposable test database.

Non-local smoke status:

- `powershell` / `pwsh` was not available on this macOS workstation, so `scripts/run-jump-host-video-deliverable-smoke.ps1 -SelfTest` was not run here.
- No real login-gated or private video was substituted for the jump-host self-test.
- A real video/provider smoke remains operator-scoped and requires an approved accessible source.

8-server read-only state:

- Repository: `/srv/aiv3/repo`.
- Head checked after validation sync: `26afa6e2f`.
- Status: `## main...origin/main` plus known untracked `mode`; left untouched.
- `aiv3-media-worker.service`: active.
- `aiv3-assistant-run-worker.service`: active.

Safety result:

- No raw customer media, local task directories, provider keys, cookies, source URLs, private file paths, raw provider payloads, or generated smoke artifacts were committed.

## 2026-06-07 Delivery-Loop UI Completion Receipt

Task source: `docs/plans/datamax-active-execution-plan.md` Task 10, "Video PPT Extraction Delivery Loop".

Scope completed:

- the video extraction summary now exposes prioritized download actions for `video_slides_screenshot_based.pptx`, `video_slides.md`, `final_deliverables_manifest.json`, `published_deliverable_manifest.json`, `published_version_history.json`, and `extraction_artifacts_manifest.json`;
- the same download priority is reused by the generated-project shelf card and the opened HTML artifact toolbar, so operators do not need to inspect raw manifest payloads to find PPT/Markdown/manifest files;
- download hrefs are routed through `/api/v3/html-artifacts/{artifact_id}/files/{index}` with `assistant_run_id` or `local_thread_id` query scope, not through raw local file paths;
- the existing read-only sandbox rendering still shows completion status, user notification, model follow-up, quality warnings, source-resolution audit, redaction boundary, transcript/scenes/OCR evidence, and generated file names without raw paths.

Local checks at `771399b` plus local Task 10 changes:

```text
node --test app/lib/html-artifact-manifest.test.mjs
node --check app/lib/html-artifact-manifest.js
npm run test:video-deliverables
node --check tools/validate-video-deliverables.mjs
npm run build
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p media-worker durable_published_version --lib
CC=clang CXX=clang++ cargo test -p media-worker model_completion --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
bash scripts/run-assistant-run-worker-smoke.sh
```

Result:

- `node --test app/lib/html-artifact-manifest.test.mjs` passed, 12 tests, with the existing Node module-type warning only;
- the new download-link helper test proves PPTX, Markdown, delivery manifest, published manifest, version history, and artifact index links are prioritized and do not include the fixture local path;
- `node --check app/lib/html-artifact-manifest.js` passed;
- `npm run test:video-deliverables` passed, 15 tests;
- `node --check tools/validate-video-deliverables.mjs` passed;
- `npm run build` in `apps/web` passed; Next reported existing middleware-deprecation and NFT tracing warnings only;
- targeted media-worker checks passed: controlled deliverable contract 1 test, slide rectangle 4 tests, durable published version 2 tests, model-completion dispatch 1 test;
- targeted platform-api checks passed: `video_extraction` 3 tests and `video_ppt` 4 tests;
- `scripts/run-assistant-run-worker-smoke.sh` passed; report: `target/assistant-run-worker-smoke/assistant-run-worker-smoke-20260607T033438Z.md`;
- DB-backed assistant-run-worker consumer checks remain intentionally scoped to disposable test databases only.

Operator-only smoke status:

- `pwsh` / `powershell` was unavailable on this macOS workstation, so `scripts/run-jump-host-video-deliverable-smoke.ps1 -SelfTest` was skipped with the literal reason `pwsh/powershell not found`;
- no real login-gated/private video smoke was substituted;
- real login-gated or private video extraction remains operator-scoped and requires an approved accessible source, account boundary, and rollback/cleanup scope.

Safety result:

- no raw media URL, local object path, generated artifact path, provider payload, cookie, bearer token, database URL, credential, full customer document, or raw customer row was recorded;
- no production write, live private-video smoke, service build, service restart, deployment, or 120-server action was run.

## 2026-06-07 Controlled PPT-Playback Video Smoke

Task source: follow-up to Task 3 and Task 10 after the video/PPT lane was promoted ahead of the remaining active-plan items.

Scope clarified:

- this smoke validates videos that already contain PPT/slide playback;
- it does not claim to convert arbitrary ordinary videos into authored PPT decks;
- login-gated/private/video-platform bypass sources remain out of scope;
- the public page path is limited to pages that expose a direct video asset in supported HTML fields.

Code deployed:

- `6a565f0` added automatic stable PPT-page keyframe selection from decoded raw frames, preserving manual keep-list override behavior;
- `442ceec` changed remote video frame extraction to download the approved public video URL into the extraction session before invoking ffmpeg, avoiding ffmpeg network-input instability while keeping URL redaction;
- 8-server `/srv/aiv3/repo` fast-forwarded to `442ceec9c`;
- `CC=clang CXX=clang++ cargo build --release -p media-worker` passed;
- only `aiv3-media-worker.service` was restarted after the second media-worker deployment, and it returned `active`;
- the known untracked `?? mode` entry remained untouched.

Local verification before deployment:

```text
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
git diff --check
```

Result:

- `media-worker --lib` passed, 37 tests;
- `platform-api video_extraction` passed, 3 tests;
- `platform-api video_ppt` passed, 4 tests;
- `npm run test:video-deliverables` passed, 15 tests;
- `git diff --check` passed.

8-server controlled smoke:

- generated a non-customer MP4 that simulates 3 stable PPT pages playing in a video;
- local-file extraction smoke completed with `frame_count=25`;
- auto page selection returned `selection_source=auto_unique_slide_keyframes`, `selected_candidate_indices=[5,13,22]`, and `selected_count=3`;
- final state was `final_pptx_ready`;
- generated PPTX ZIP inspection found `[Content_Types].xml`, `ppt/presentation.xml`, and exactly 3 slide XML files;
- a controlled public HTML page under the existing `v3.elepcloud.com` generated-artifacts alias was reachable and exposed the same MP4 by relative video reference;
- the platform page-candidate resolver regression `public_video_page_extracts_video_sources_from_html` passed on 8-server;
- HTTPS remote-video extraction smoke completed through the new download-first path with `input_kind=remote_video_url`, `input_url_redacted=true`, `input_downloaded=true`, `input_download_bytes=8429`, `frame_count=25`, `selected_candidate_indices=[5,13,22]`, `selected_count=3`, and exactly 3 PPTX slide XML files;
- the frame manifest redaction check confirmed `input_path=[redacted]` and that the manifest did not contain the public source host.

Service health after smoke:

- `aiv3-platform-api.service`: active;
- `aiv3-web.service`: active;
- `aiv3-media-worker.service`: active;
- `aiv3-ingest-worker.service`: active;
- `aiv3-assistant-run-worker.service`: active;
- workflow queue stats endpoint returned valid JSON.

Safety result:

- no customer/private/login-gated video was used;
- no source URL, generated artifact path, local extraction path, database URL, token, cookie, provider payload, raw customer row, or document body was recorded in this receipt;
- the temporary high-port HTTP attempt was stopped and was not used for the passing smoke;
- the passing smoke used the existing HTTPS generated-artifacts route and a non-sensitive synthetic PPT-playback MP4 only.

## 2026-06-07 Video Upload And Special Trigger Clarification

Scope clarified:

- uploaded `.mp4`, `.mov`, `.m4v`, `.webm`, `.mkv`, and `.avi` files can be registered as video materials on the main site or through third-party document parse flows;
- a registered video material is not the same as a completed PPT extraction package;
- normal parsing may record/download/index the video material and any available transcript/OCR evidence, but `extract_video_ppt_transcript` remains the special trigger for generating screenshot-based PPT, Markdown, and video extraction manifests;
- the special trigger should be keyed primarily by explicit requests to extract PPT, slides, or courseware from the video; transcript/subtitle-only requests should not be promoted into PPT extraction unless the user also asks for PPT/slides/courseware;
- the trigger remains scoped to videos that already show PPT, slides, or courseware during playback, not arbitrary-video-to-PPT authoring;
- login-gated/private/video-platform bypass sources remain out of scope; upload the file or provide an anonymous direct video URL instead.

Code/docs follow-up:

- main-site upload classification now recognizes `.m4v` as video material;
- scope planning now treats `.mkv` and `.avi` video names/URLs as video PPT extraction prompts, matching backend video-material recognition;
- third-party integration docs now explain video material registration, normal parse status, and the special PPT extraction trigger boundary.

## 2026-06-07 Main-Site Visible Video/PPT Smoke

Task source: `docs/plans/datamax-active-execution-plan.md` P0-1 and P0-2.

Scope:

- prove the main-site assistant-run-bound path, not only a backend workflow smoke;
- use a public non-customer PPT-playback video URL;
- validate that PPTX, Markdown, and manifests are visible through the main-site HTML artifact file API;
- do not run login-gated/private/video-platform bypass sources.

Input:

- prompt: `请提取这个视频里的 PPT：https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4`
- local thread: `video-ppt-main-visible-20260607-01`
- assistant run id: `46f57e74-85f9-4088-bad0-99f1ae0a6fea`

Observed main-site run:

- create SSE returned one accepted event, 13 delta events, one completed event, and one done event;
- create phase produced assistant run `46f57e74-85f9-4088-bad0-99f1ae0a6fea`;
- the run event log contains `video_extraction.workflow_completed`;
- model completion handoff was requested, enqueued, consumed, and appended a model-owned continue turn;
- the workflow completion follow-up reported `status=final_pptx_ready`;
- ready file kinds included `source_text`, `ppt_outline`, `slide_rectangles_manifest`, `slide_notes`, `video_slides_markdown`, `pptx`, `timestamp_map`, `final_deliverables_manifest`, `published_deliverable_manifest`, `published_version_history`, and `extraction_artifacts_manifest`.

Visible artifact:

- artifact id: `html-artifact-video-extraction-46f57e74-85f9-4088-bad0-99f1ae0a6fea-0c41706c-3e39-4c75-8f35-32d26f98e8b3`;
- source type: `video_extraction`;
- template id: `video_extraction_summary`;
- owner scope: assistant run `46f57e74-85f9-4088-bad0-99f1ae0a6fea`;
- `GET /api/v3/html-artifacts?assistant_run_id=...&local_thread_id=...` returned the full video extraction artifact manifest;
- `deliverable_status.state=final_pptx_ready`;
- `generated_artifacts.status=completed`;
- `frame_count=1281`;
- `file_count=18`;
- `has_pptx=true`;
- `has_video_slides_markdown=true`;
- `has_slide_notes=true`;
- `has_slide_rectangles_manifest=true`;
- `has_subtitle_page_map=false`.

Download checks through `/api/v3/html-artifacts/{artifact_id}/files/{index}`:

| Index | Kind | Result |
| ---: | --- | --- |
| 10 | `video_slides_markdown` | HTTP 200, `video_slides.md`, 10881 bytes |
| 12 | `pptx` | HTTP 200, `video_slides_screenshot_based.pptx`, 268540 bytes |
| 14 | `final_deliverables_manifest` | HTTP 200, `final_deliverables_manifest.json`, 11095 bytes |
| 15 | `published_deliverable_manifest` | HTTP 200, `published_deliverable_manifest.json`, 16036 bytes |
| 16 | `published_version_history` | HTTP 200, `published_version_history.json`, 5163 bytes |
| 17 | `extraction_artifacts_manifest` | HTTP 200, `extraction_artifacts_manifest.json`, 9126 bytes |

Quality review:

- PPTX ZIP inspection found `[Content_Types].xml`, `ppt/presentation.xml`, and `ppt/slides/slide1.xml`;
- PPTX contains 30 slide XML files and 30 picture elements;
- 27 slides include DrawingML `a:srcRect` crop metadata;
- `video_slides.md` contains `# Video Slides`, 30 `### Slide` sections, source frame names, timestamps, crop status, relative crop boxes, and per-slide narration availability notes;
- selected slides manifest reports `selected_count=30`;
- visual-similarity dedupe removed 7 near-duplicate selected candidates before PPTX generation;
- `has_subtitle_page_map=false` is expected for this sample because no aligned transcript evidence is attached.

Warnings interpreted:

- `missing_transcript_alignment`: narration/page mapping is not available for this sample;
- `full_frame_rectangle_fallback`: some slide regions may need review, although most slides had detector crop metadata;
- `selected_slide_duplicates_removed`: duplicate suppression worked and is visible in the manifest;
- `screenshot_based_pptx`: output is screenshot-based, not editable native slide reconstruction;
- `speaker_notes_metadata_only`: notes preserve frame metadata without aligned narration;
- `parse_partial` and `provider_failure`: provider transcript/native understanding is disabled, but screenshot PPTX and Markdown delivery still reached `final_pptx_ready`.

Reusable smoke check drafted locally:

The command below records the reproducible check shape used for this receipt. Treat the script/package wiring as implementation work that should be committed separately with its own validation scope; this receipt only records the main-site evidence and expected gate.

```text
node scripts/smoke/video-ppt-main-visible.mjs --base-url https://v3.elepcloud.com --assistant-run-id 46f57e74-85f9-4088-bad0-99f1ae0a6fea --local-thread-id video-ppt-main-visible-20260607-01 --timeout-ms 60000 --output-dir target/video-ppt-main-visible-script-smoke
```

Result:

- `ok=true`;
- `createdRun=false`;
- `artifactOk=true`;
- `deliverableState=final_pptx_ready`;
- downloaded required file kinds: `video_slides_markdown`, `pptx`, `final_deliverables_manifest`, `published_deliverable_manifest`, `published_version_history`, `extraction_artifacts_manifest`;
- `pptxSlideCount=30`;
- `markdownSlideHeadingCount=30`.

Safety result:

- no customer/private/login-gated video was used;
- no 8-server deployment, build, restart, service mutation, or 120-server action was run;
- no cookie, token, database URL, raw provider payload, raw customer row, full customer document, raw local file path, or generated artifact local path was recorded in this receipt;
- generated smoke downloads were kept under `target/` and were not committed;
- plan-only synchronization should not include script/package changes unless the implementation slice is explicitly in scope.

## 2026-06-07 WeChat Video Login Handoff Slice

Task source: `docs/plans/datamax-active-execution-plan.md` P1-2.

Scope:

- do not auto-parse WeChat Video Channels or other login-gated sources;
- do not ask users for QR login, cookies, account credentials, browser storage, or private playback payloads;
- do not claim DataMax has watched the video or generated PPT when no video file/direct URL was obtained;
- provide an actionable handoff: upload video file, provide an anonymous direct video URL, or request operator-approved capture handling.

Implemented behavior:

- main-site assistant runs can emit a `wechat_video_login_handoff` HTML artifact for video-channel PPT extraction requests;
- the artifact owner scope is the assistant run and its payload uses `failure_reason=login_gated_video_source_not_supported`;
- source URL is not copied into the artifact payload; only a short share code is retained when detectable;
- ReAct `resolve_video_url` returns the same unsupported reason and `upload_video_provide_direct_url_or_request_authorized_capture` next action;
- the web artifact renderer shows the three handoff options while suppressing QR-login handoff text, raw source links, and credential-oriented fields;
- third-party integration docs describe the same three options.

Validation:

```text
cargo fmt --check
CC=clang CXX=clang++ cargo test -p platform-api wechat_video_login_handoff --lib
CC=clang CXX=clang++ cargo test -p platform-api video_url_resolution --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
CC=clang CXX=clang++ cargo test -p ingest-worker --bin ingest-worker video_parse_media_placeholder -- --nocapture
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
npm run check:pure-third-party-guide-html
node --check apps/web/app/lib/html-artifact-manifest.js
```

Result:

- platform-api handoff tests passed: 3 tests;
- platform-api video URL resolution tests passed: 4 tests;
- platform-api video PPT tests passed: 4 tests;
- ingest-worker video placeholder tests passed: 2 tests; the DB-backed fixture branch was intentionally skipped because the current `PLATFORM_DATABASE_URL` points at the shared non-test database;
- web artifact manifest tests passed: 12 tests;
- third-party guide check passed;
- JavaScript syntax check passed.

Safety result:

- no customer/private/login-gated video was fetched;
- no 8-server deployment, build, restart, service mutation, or 120-server action was run;
- no cookie, token, database URL, raw provider payload, raw customer row, full customer document, raw local file path, or generated artifact local path was recorded in this receipt.

## 2026-06-07 Main-Site Video/PPT Release Gate

Task source: `docs/plans/datamax-active-execution-plan.md` P1-3.

Scope:

- turn the previously ad hoc main-site visible video/PPT smoke into a reusable release gate;
- validate assistant-run-bound artifact visibility, not only backend workflow success;
- download final user-facing deliverables through the HTML artifact file API;
- avoid re-running heavy video extraction when an existing completed assistant run can be reused.

Implemented behavior:

- added package script `smoke:video-ppt-main-visible`;
- added `scripts/smoke/video-ppt-main-visible.mjs`;
- documented the smoke in `scripts/README.md`;
- supported both create-run mode and `--assistant-run-id` reuse mode;
- required `video_extraction_summary` with `deliverable_status.state=final_pptx_ready`;
- downloaded `pptx`, `video_slides_markdown`, `final_deliverables_manifest`, `published_deliverable_manifest`, `published_version_history`, and `extraction_artifacts_manifest` through `/api/v3/html-artifacts/{artifact_id}/files/{index}`;
- validated PPTX ZIP central-directory entries `[Content_Types].xml`, `ppt/presentation.xml`, and `ppt/slides/slide1.xml`;
- validated PPTX slide count and Markdown `### Slide` heading count agreement.

Smoke command:

```text
npm run smoke:video-ppt-main-visible -- --base-url https://v3.elepcloud.com --assistant-run-id 46f57e74-85f9-4088-bad0-99f1ae0a6fea --local-thread-id video-ppt-main-visible-20260607-01 --timeout-ms 60000 --output-dir target/video-ppt-main-visible-release-gate-smoke
```

Result:

- `ok=true`;
- `createdRun=false`;
- `artifactOk=true`;
- `deliverableState=final_pptx_ready`;
- artifact id: `html-artifact-video-extraction-46f57e74-85f9-4088-bad0-99f1ae0a6fea-0c41706c-3e39-4c75-8f35-32d26f98e8b3`;
- downloaded required file kinds: `video_slides_markdown`, `pptx`, `final_deliverables_manifest`, `published_deliverable_manifest`, `published_version_history`, `extraction_artifacts_manifest`;
- `pptxSlideCount=30`;
- `markdownSlideHeadingCount=30`;
- report: `target/video-ppt-main-visible-release-gate-smoke/20260607135915/report.json`.

Validation:

```text
node --check scripts/smoke/video-ppt-main-visible.mjs
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
npm run test:video-deliverables
git diff --check
```

Remaining P1-3 work:

- main-site uploaded-video smoke;
- third-party video registration and special-trigger smoke.

Safety result:

- no customer/private/login-gated video was fetched;
- generated downloads stayed under `target/` and were not committed;
- no 8-server deployment, build, restart, service mutation, or 120-server action was run;
- no cookie, token, database URL, raw provider payload, raw customer row, full customer document, raw local file path, or generated artifact local path was recorded in this receipt.

## 2026-06-07 Public Page Video Resolver Slice

Task source: `docs/plans/datamax-active-execution-plan.md` P1-1.

Scope:

- support public pages that expose anonymous direct video assets in static HTML fields;
- keep login-gated, QR-login, private-host, JavaScript/data URL, and player-only pages outside automatic extraction;
- stabilize resolver failure reasons so product and third-party callers can route next steps.

Implemented behavior:

- HTML candidate extraction now covers `<video src>`, `<source src>`, OpenGraph video fields, Twitter video fields, and JSON-LD `contentUrl`/`embedUrl`;
- relative candidates are resolved against the page URL, then filtered through existing direct-video extension and public-host checks;
- JSON-LD player `embedUrl` values are collected for review but rejected unless they are direct video URLs;
- no-video pages return `public_page_no_video_asset`;
- missing source URL returns `direct_video_url_required`;
- login-gated sources remain `login_gated_video_source_not_supported`;
- third-party docs now document public-page fields and `public_page_no_video_asset`.

Validation:

```text
cargo fmt --check
CC=clang CXX=clang++ cargo test -p platform-api public_video_page --lib
CC=clang CXX=clang++ cargo test -p platform-api video_url_resolution --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run check:pure-third-party-guide-html
git diff --check
```

Result:

- public page resolver tests passed: 6 tests;
- video URL resolution tests passed: 5 tests;
- video PPT tests passed: 4 tests;
- third-party guide freshness check passed.

Safety result:

- no external public page was fetched in this slice; fixtures were local unit tests;
- no customer/private/login-gated video was fetched;
- no 8-server deployment, build, restart, service mutation, or 120-server action was run;
- no cookie, token, database URL, raw provider payload, raw customer row, full customer document, raw local file path, or generated artifact local path was recorded in this receipt.

## 2026-06-07 Executable Plan Refresh

Task source: user requested a plan-only consolidation after the video/PPT tests, extraction-quality review, WeChat Video Channels handoff research, and possible authorized recording fallback discussion.

Scope:

- refreshed the active plan as an executable next-stage plan rather than a code implementation slice;
- kept P1-3 as the immediate active development queue, with separate steps for direct URL release gate, main-site uploaded-video smoke, third-party video registration special-trigger smoke, and login-gated/WeChat handoff smoke;
- kept P2-1 as a later authorized-capture fallback MVP, starting with an isolated script and runbook instead of a server integration;
- preserved the boundary that DataMax extracts PPT/slide/courseware frames already present in a video, and does not convert arbitrary ordinary video into authored PPT;
- preserved the boundary that WeChat Video Channels and other login-gated links require upload, anonymous direct video URL, or operator-approved capture handling.

Plan updates:

- `docs/plans/datamax-active-execution-plan.md` now records current head `852d878`, the latest P1-1/P1-2/P1-3 direct URL gate commits, and the remaining P1-3B/P1-3C/P1-3D work;
- P1-3 upload and third-party smoke steps now map to the real main-site upload APIs, external document parse API, external message API, artifact download surface, and required evidence fields;
- P2-1 now has explicit phases for authorization record format, local/controlled-host capture, operator-approved sample smoke, and optional 8-server review;
- the verification matrix and decision gates now separate live main-site writes, third-party bearer requirements, capture approval, and deployment permission.

Safety result:

- no code was changed in this refresh;
- no live upload smoke, third-party smoke, private-video smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, raw media URL, local object path, or generated artifact local path was recorded.

## 2026-06-07 Main-Site Uploaded Video Smoke Script Slice

Task source: `docs/plans/datamax-active-execution-plan.md` P1-3B.

Scope:

- add a reusable main-site uploaded-video smoke entrypoint without running a production upload in this slice;
- keep direct URL release gate unchanged;
- use only public/non-customer video fixtures when the smoke is run;
- make the script follow the real main-site upload path before triggering video/PPT extraction.

Implemented behavior:

- added package script `smoke:video-ppt-upload-main`;
- added `scripts/smoke/video-ppt-upload-main.mjs`;
- documented the entrypoint in `scripts/README.md`;
- the script creates or reuses a local-thread scoped smoke dataset through `/api/v3/datasets`;
- the script uploads a fixture through `/api/v3/local-document-uploads`;
- the script registers and ingests the uploaded video through `/api/v3/documents` and `/api/v3/documents/{document_id}/ingest`;
- the script creates a dataset-scoped assistant run through `/api/v3/assistant-runs/stream` with an explicit PPT/slides/courseware extraction prompt;
- the script waits for `video_extraction_summary`, requires `final_pptx_ready`, downloads PPTX/Markdown/manifests through `/api/v3/html-artifacts/{artifact_id}/files/{index}`, and validates PPTX OOXML entries plus Markdown/PPTX slide-count agreement;
- the report records IDs and redacted summaries, but does not record upload object keys, server local upload paths, cookies, bearer tokens, database URLs, or provider payloads.

Validation:

```text
node --check scripts/smoke/video-ppt-upload-main.mjs
node --check scripts/smoke/video-ppt-main-visible.mjs
npm run smoke:video-ppt-upload-main -- --help
node --test apps/web/app/lib/upload-classifier.test.mjs
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
npm run test:video-deliverables
```

Result:

- uploaded-video smoke script syntax check passed;
- existing direct URL video/PPT smoke syntax check passed;
- package entrypoint help command returned the expected usage and check list;
- upload classifier tests passed, 7 tests, with the existing Node module-type warning only.
- HTML artifact manifest tests passed, 12 tests, with the existing Node module-type warning only;
- video deliverables validator tests passed, 15 tests.

Remaining P1-3B work:

- run a live/controlled main-site upload smoke after explicit approval, because it writes a controlled smoke upload/document/assistant-run record;
- append the live report with `local_thread_id`, dataset id, document id, ingest workflow id, assistant run id, artifact id, `deliverableState`, downloaded file kinds, PPTX slide count, Markdown slide count, and warning interpretation.

Safety result:

- no live upload smoke, third-party smoke, private-video smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no customer/private/login-gated video was fetched;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, upload object key, server local upload path, or generated artifact local path was recorded.

## 2026-06-07 Third-Party Video PPT Smoke Script Slice

Task source: `docs/plans/datamax-active-execution-plan.md` P1-3C.

Scope:

- add a reusable third-party video/PPT smoke entrypoint without running a production third-party event in this slice;
- keep the main-site direct URL and uploaded-video smoke paths unchanged;
- separate video material registration from the special “extract PPT/slides/courseware already shown in this video” trigger;
- keep WeChat Video Channels, login-gated, QR-login, cookie/session, and private-player sources outside automatic extraction.

Implemented behavior:

- added package script `smoke:external-video-ppt`;
- added `scripts/smoke/external-video-ppt.mjs`;
- documented the entrypoint in `scripts/README.md`;
- refreshed the active plan to record P1-3C script completion and the remaining live bearer smoke gate;
- the script registers a public or loopback video fixture through `/v1/external/channels/{connection_id}/documents/parse`;
- the script polls `/v1/external/channels/{connection_id}/documents/{document_external_id}/parse-detail` only to verify the video material is registered or ready for special trigger;
- the script sends a scoped `/v1/external/channels/{connection_id}/events` message with `dataset_external_ids`, `available_document_external_ids`, and an explicit Chinese prompt asking to extract PPT/slides/courseware from the video;
- the script asks for `video_ppt_extraction` and records `extract_video_ppt_transcript` as the expected action;
- the script polls `/v1/external/channels/{connection_id}/assistant-runs/{run_id}/reply`;
- the script validates that the third-party reply exposes a video/PPT extraction surface and is not a login-gated or direct-video-required unsupported response;
- when deliverable downloads are enabled, the script requires `pptx`, `video_slides_markdown`, `final_deliverables_manifest`, `published_deliverable_manifest`, `published_version_history`, and `extraction_artifacts_manifest`, then validates PPTX OOXML entries and Markdown/PPTX slide-count agreement.

Validation:

```text
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:external-video-ppt -- --help
npm run smoke:external-video-ppt -- --self-test
node --check scripts/smoke/external-scoped-document-chat.mjs
node --check scripts/smoke/video-ppt-upload-main.mjs
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
npm run test:video-deliverables
git diff --check
```

Result:

- third-party video/PPT smoke script syntax check passed;
- package entrypoint help command returned the expected usage, checks, and live bearer requirements;
- deterministic offline self-test passed and did not call the network;
- existing third-party scoped document chat smoke syntax check passed;
- existing uploaded-video smoke syntax check passed;
- HTML artifact manifest tests passed, 12 tests, with the existing Node module-type warning only;
- video deliverables validator tests passed, 15 tests.

Remaining P1-3C work:

- run a live/controlled third-party smoke after the user provides or approves the inbound bearer and target connection/source ids;
- append the live report with `connection_id`, `source_id`, `dataset_external_id`, `document_external_id`, `conversation_external_id`, assistant run id, reply status, artifact/download surface, downloaded file kinds, PPTX slide count, Markdown slide count, and warning interpretation;
- if the third-party reply completes extraction but lacks a customer-visible download surface, record it as a third-party artifact visibility gap rather than weakening acceptance.

Safety result:

- no live upload smoke, third-party live smoke, private-video smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no customer/private/login-gated video was fetched;
- no WeChat Video Channels URL was bypassed or recorded as parsed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, upload object key, server local upload path, private object path, or generated artifact local path was recorded.

## 2026-06-07 Video Login-Gated Handoff Smoke Script Slice

Task source: `docs/plans/datamax-active-execution-plan.md` P1-3D.

Scope:

- add a reusable lightweight handoff smoke for WeChat Video Channels and login-gated video PPT requests;
- validate the unsupported-source handoff surface without fetching source pages, downloading video, extracting frames, OCR, or generating PPT;
- cover both main-site `wechat_video_login_handoff` artifact shape and third-party equivalent unsupported-source card shape;
- keep live third-party execution behind the inbound bearer gate.

Implemented behavior:

- added package script `smoke:video-ppt-handoff`;
- added `scripts/smoke/video-ppt-handoff.mjs`;
- documented the entrypoint in `scripts/README.md`;
- refreshed the active plan to record P1-3D script completion and the current live visibility gap;
- default mode is `--self-test` and does not call the network;
- live modes are explicit: `--mode main`, `--mode external`, or `--mode both`;
- main mode creates or reuses a lightweight assistant run, then polls `/api/v3/html-artifacts` for `wechat_video_login_handoff`;
- external mode sends a scoped `/v1/external/channels/{connection_id}/events` message and polls `/assistant-runs/{run_id}/reply`;
- the validator requires `login_gated_video_source_not_supported`;
- the validator requires upload-video-file, anonymous direct-video-URL, and authorized-capture next steps;
- the validator rejects success/download signals such as `final_pptx_ready`, `video_extraction_summary`, `download_exports`, frame extraction, or OCR-complete markers.

Validation:

```text
node --check scripts/smoke/video-ppt-handoff.mjs
node scripts/smoke/video-ppt-handoff.mjs --help
node scripts/smoke/video-ppt-handoff.mjs --self-test
npm run smoke:video-ppt-handoff -- --help
npm run smoke:video-ppt-handoff -- --self-test
npm run smoke:video-ppt-handoff -- --mode main --base-url https://v3.elepcloud.com --local-thread-id video-ppt-handoff-20260607-01 --timeout-ms 60000 --output-dir target/video-ppt-handoff-main-smoke
CC=clang CXX=clang++ cargo test -p platform-api wechat_video_login_handoff --lib
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
node --check scripts/smoke/external-video-ppt.mjs
npm run test:video-deliverables
git diff --check
```

Result:

- handoff smoke script syntax check passed;
- direct help and npm help returned expected usage, checks, and live-mode notes;
- direct self-test and npm self-test passed and did not call the network;
- platform-api WeChat Video login handoff tests passed, 3 tests;
- HTML artifact manifest tests passed, 12 tests, with the existing Node module-type warning only;
- existing third-party video/PPT smoke syntax check passed;
- video deliverables validator tests passed, 15 tests;
- one main-site lightweight live smoke was attempted; it did not fetch the WeChat source, did not download video, did not extract frames, and did not generate PPT, but it also did not find `wechat_video_login_handoff` within 60 seconds.

Current P1-3D live interpretation:

- The script and local contract are ready.
- Current main-site live surface is not yet proven because the handoff artifact was not visible within the smoke timeout.
- Treat this as a main-site handoff visibility/deployment receipt gap, not as a reason to parse or capture the WeChat source.
- Third-party live handoff smoke was not run because it requires an approved inbound bearer and target connection/source ids.

Remaining P1-3D work:

- after confirming the current main-site deployment includes the handoff artifact path, rerun the same `--mode main` smoke and append the pass/fail receipt;
- after the user provides or approves third-party inbound bearer/context, run `--mode external` or `--mode both` and append the equivalent third-party receipt.

Safety result:

- no private/login-gated video page was fetched;
- no WeChat Video Channels URL was treated as a direct media URL;
- no video download, frame extraction, OCR, PPT generation, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, upload object key, server local upload path, private object path, or generated artifact local path was recorded.

## 2026-06-07 Authorized Capture Fallback MVP Slice

Task source: `docs/plans/datamax-active-execution-plan.md` P2-1.

Scope:

- add a runbook and isolated helper for operator-approved capture fallback;
- keep capture outside normal chat, workers, and production services;
- require explicit authorization metadata before dry-run or live capture planning;
- do not run live browser capture in this slice;
- keep the captured MP4 handoff as a normal video upload or third-party video registration follow-up, not a separate PPT-generation path.

Implemented behavior:

- added `docs/operations/video-capture-fallback-runbook.md`;
- added `scripts/capture-authorized-video.mjs`;
- added package script `capture:authorized-video`;
- documented the helper in `scripts/README.md`;
- the runbook defines required approval fields, non-goals, operator flow, command examples, report contract, failure split, 8-server gate, and acceptance criteria;
- the script defaults to `--self-test` / `--dry-run` workflows that do not open a browser and do not run FFmpeg;
- live capture requires `--run-capture --ack-authorized --approval-id ... --approved-by ... --url ... --purpose ...`;
- the script validates source URL scheme/host, duration hard limits, retention limit, capture mode, and handoff mode;
- live capture path uses a temporary browser profile and FFmpeg command plan, deletes the profile by default, and prints a handoff command instead of uploading automatically;
- reports store redacted source summary rather than full source URL.

Validation:

```text
node --check scripts/capture-authorized-video.mjs
node scripts/capture-authorized-video.mjs --help
node scripts/capture-authorized-video.mjs --self-test
node scripts/capture-authorized-video.mjs --dry-run --ack-authorized --approval-id DRYRUN-20260607-002 --approved-by operator-dryrun --url https://example.com/authorized-video-page --purpose dry-run-authorized-capture-plan --duration-seconds 30 --handoff upload-main
npm run capture:authorized-video -- --help
npm run capture:authorized-video -- --self-test
npm run capture:authorized-video -- --dry-run --ack-authorized --approval-id DRYRUN-20260607-003 --approved-by operator-dryrun --url https://example.com/authorized-video-page --purpose dry-run-authorized-capture-plan --duration-seconds 30 --handoff upload-main
node scripts/capture-authorized-video.mjs --dry-run --approval-id DRYRUN-NEGATIVE --approved-by operator-dryrun --url https://example.com/authorized-video-page --purpose missing-ack-negative-test
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker frame_extraction --lib
git diff --check
```

Result:

- capture helper syntax check passed;
- direct help and npm help returned expected usage and safety notes;
- direct self-test and npm self-test passed without browser/FFmpeg execution;
- direct dry-run and npm dry-run passed without browser/FFmpeg execution;
- negative authorization gate passed: missing `--ack-authorized` is rejected before capture planning;
- millisecond/PID run ids prevent self-test and dry-run reports from overwriting each other in parallel.
- video deliverables validator tests passed, 15 tests;
- media-worker frame extraction tests passed, 6 tests.

Remaining P2-1 work:

- run P2-1C only after an operator supplies an approved playable source, approval id, maximum duration, audio policy, retention policy, and handoff target;
- after live capture, manually review the MP4 before running main upload or third-party video registration;
- do not evaluate 8-server capture until the user explicitly approves a separate 8-server deployment/review window.

Safety result:

- no live browser was opened;
- no FFmpeg capture was run;
- no video was downloaded, uploaded, extracted, OCRed, or converted to PPT in this slice;
- no 8-server deployment, build, restart, config mutation, capture dependency install, or 120-server action was run;
- no cookie, token, account credential, QR screenshot, browser storage, HAR, database URL, provider payload, raw customer row, full customer document, private object path, or generated artifact local path was recorded.

## 2026-06-07 Main-Site WeChat Handoff Early Return Fix

Task source: P1-3D live handoff visibility gap found by `smoke:video-ppt-handoff -- --mode main`.

Scope:

- fix current code so WeChat Video Channels / login-gated video PPT requests do not wait for provider/ReAct before returning handoff;
- keep the behavior as an unsupported-source handoff, not a video fetch, capture, OCR, or PPT generation path;
- strengthen test evidence so the main assistant-run endpoint exposes the handoff artifact through the normal HTML artifact list surface.

Implemented behavior:

- moved main-site WeChat video handoff into a deterministic early-return path inside assistant-run creation;
- after the run is created and `assistant_run.started` is recorded, matching prompts now immediately:
  - write a direct-answer runtime manifest with `model=wechat-video-login-handoff-v1`;
  - attach an assistant message and `wechat_video_login_handoff` output artifact to the run;
  - append `assistant_run.wechat_video_login_handoff_required` with the handoff HTML artifact;
  - append `assistant_run.completed`;
  - return the unsupported-source answer without calling ReAct or provider runtime;
- removed the old post-completion handoff append path;
- added a handler-level test that sets provider mode to an unreachable gateway and proves the handoff still succeeds and is listable through `/api/v3/html-artifacts`.

Validation:

```text
cargo fmt --check
CC=clang CXX=clang++ cargo test -p platform-api assistant_run_wechat_video_handoff_short_circuits_provider_and_lists_artifact --lib
CC=clang CXX=clang++ cargo test -p platform-api wechat_video_login_handoff --lib
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
node --check scripts/smoke/video-ppt-handoff.mjs
npm run smoke:video-ppt-handoff -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
git diff --check
```

Result:

- rustfmt check passed;
- new handler-level handoff test passed, 1 test;
- existing WeChat Video login handoff unit tests passed, 3 tests;
- HTML artifact manifest tests passed, 12 tests, with the existing Node module-type warning only.
- video PPT platform-api tests passed, 4 tests;
- video deliverables validator tests passed, 15 tests;
- handoff smoke syntax/self-test passed.

Remaining P1-3D work:

- deploy only after explicit approval;
- rerun `npm run smoke:video-ppt-handoff -- --mode main ...` against the deployed main site and append the live pass/fail receipt;
- run third-party handoff live only after approved inbound bearer/context is available.

Safety result:

- no live smoke was rerun in this slice;
- no WeChat Video Channels URL was fetched;
- no video download, frame extraction, OCR, PPT generation, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, or generated artifact local path was recorded.

## 2026-06-07 Third-Party WeChat Handoff Deterministic Card

Task source: P1-3D follow-up after the main-site handoff early-return fix; third-party `/events` still needed a deterministic unsupported-source surface that did not depend on model text.

Scope:

- make WeChat Video Channels / login-gated video PPT requests through `/v1/external/channels/{connection_id}/events` return the same product handoff as the main site;
- keep the behavior as an unsupported-source card, not a video fetch, capture, OCR, frame extraction, or PPT generation path;
- preserve idempotency and the existing third-party reply lookup endpoint.

Implemented behavior:

- after the external channel run is created and `assistant_run.external_channel_message_received` is recorded, matching prompts now immediately:
  - write a direct-answer runtime manifest with `model=wechat-video-login-handoff-v1`;
  - attach a short assistant message and `wechat_video_login_handoff` output artifact to the run;
  - append `assistant_run.wechat_video_login_handoff_required` with the handoff HTML artifact and external card reply;
  - append `assistant_run.completed`;
  - return a `reply_type=card` response with `task_status=login_gated_video_source_not_supported`;
- external reply reconstruction now checks the handoff event before assistant-message text, so duplicate idempotency and `/assistant-runs/{run_id}/reply` both restore the card instead of degrading to plain text;
- the card exposes the three supported next steps: upload video file, provide anonymous direct video URL, or request authorized capture handling;
- the card does not include `download_exports`, artifact links, raw WeChat URLs, cookies, QR-login handoff instructions, or credential-oriented fields.

Validation:

```text
cargo fmt
CC=clang CXX=clang++ cargo test -p platform-api generic_chat_wechat_video_ppt_handoff_short_circuits_provider --lib
cargo fmt --check
CC=clang CXX=clang++ cargo test -p platform-api wechat_video_login_handoff --lib
CC=clang CXX=clang++ cargo test -p platform-api assistant_run_wechat_video_handoff_short_circuits_provider_and_lists_artifact --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run smoke:video-ppt-handoff -- --self-test
npm run smoke:external-video-ppt -- --self-test
```

Result:

- new third-party endpoint handoff test passed, 1 test;
- existing WeChat Video login handoff unit tests passed, 3 tests;
- main-site handler-level handoff test passed, 1 test;
- video PPT platform-api tests passed, 5 tests, including the new third-party handoff endpoint test;
- handoff smoke self-test passed;
- third-party video/PPT smoke self-test passed;
- rustfmt check passed.

Remaining P1-3D work:

- deploy only after explicit approval;
- rerun `npm run smoke:video-ppt-handoff -- --mode main ...` against the deployed main site and append the live pass/fail receipt;
- run `npm run smoke:video-ppt-handoff -- --mode external ...` only after approved inbound bearer/context is available.

Safety result:

- no live smoke was run in this slice;
- no WeChat Video Channels URL was fetched;
- no video download, upload, frame extraction, OCR, PPT generation, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, or generated artifact local path was recorded.

## 2026-06-07 Slide Quality Report Local Slice

Task source: P2-2A from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- add an optional `slide_quality_report.json` review artifact to video/PPT generated packages;
- keep the report derived from selected slides, slide rectangles, subtitle page map, and dedupe metadata;
- keep legacy packages valid when they do not contain `slide_quality_report.json`;
- expose the report as a front-end download labeled `质量报告`;
- do not change source access policy, WeChat Video Channels handling, capture fallback behavior, upload semantics, or third-party authorization.

Implemented behavior:

- media-worker writes `slide_quality_report.json` next to the screenshot PPTX, Markdown deck, slide notes, rectangle manifest, selected slides manifest, and other generated artifacts;
- the report uses `schema=v3.video_ppt_slide_quality_report.v1`, includes an overall quality score, risk flags, summary counts, per-slide crop/transcript risk, and review actions;
- report risk flags cover full-frame fallback crops, missing transcript alignment, selected duplicate removal, and manual review requirements;
- `video_deliverable_status` exposes `has_slide_quality_report`;
- final, published, and extraction manifests include `slide_quality_report` in review outputs when present;
- `tools/validate-video-deliverables.mjs` validates the report if present, but does not require it for legacy package compatibility;
- Web artifact download priority and InsightPanel labels now surface `slide_quality_report` as `质量报告`.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
node --check apps/web/app/components/InsightPanel.js
node --check apps/web/app/lib/html-artifact-manifest.js
npm --prefix apps/web run build
```

Result:

- rustfmt check passed;
- media-worker selected-slides quality-report path passed, 1 test;
- controlled video sample deliverable contract passed, 1 test;
- slide rectangle tests passed, 4 tests;
- platform-api video extraction tests passed, 3 tests;
- platform-api video PPT tests passed, 5 tests, including the third-party WeChat handoff regression;
- video deliverables validator tests passed, 17 tests, including legacy-without-quality-report and malformed-quality-report cases;
- HTML artifact manifest tests passed, 12 tests, with the existing Node module-type warning only;
- InsightPanel and HTML artifact manifest syntax checks passed;
- Web production build passed with the existing Next.js middleware deprecation and NFT tracing warnings.

Remaining P2-2 work:

- P2-2B: improve slide crop detection and reduce `full_frame_rectangle_fallback`;
- P2-2C: improve stable-interval selection, transition-frame filtering, and duplicate-page review output;
- P2-2D: use an approved subtitle/transcript/OCR sample to improve `subtitle_page_map` and slide notes;
- P2-2E: run the three-sample quality review matrix, including a customer video only after explicit authorization.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, or generated artifact local path was recorded.

## 2026-06-08 Subtitle Page Map Conditional Contract Local Slice

Task source: P0-3 from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- resolve the contract mismatch where no-subtitle video/PPT packages were product-acceptable with `has_subtitle_page_map=false`, but the public validator still treated `subtitle_page_map.json` as always required;
- keep `subtitle_page_map.json` strict when transcript/subtitle evidence exists, or when a manifest/file declares that artifact;
- keep OCR snippets as review evidence only, not as a substitute for transcript/subtitle page mapping;
- update only local contract, validator, tests, and documentation; do not run live smoke or deploy.

Implemented behavior:

- `tools/validate-video-deliverables.mjs` now treats `subtitle_page_map` as a conditional deliverable: required only when the file exists, a deliverable status flag is true, or a final/published/extraction/history manifest declares the artifact;
- no-subtitle packages with complete PPTX, Markdown, slide notes, rectangle manifest, published manifest, version history, extraction manifest, and `has_subtitle_page_map=false` now pass validator;
- malformed, unmapped, unredacted, or inconsistently declared subtitle maps still fail validator;
- `crates/media-worker/src/lib.rs` now computes required file kinds dynamically for deliverable package summary, published manifest, published version history, and durable published-version manifest;
- final/published package readiness no longer depends on `subtitle_page_map` when no transcript/subtitle map exists;
- media-worker still writes and publishes `subtitle_page_map.json` when selected slide transcript mapping is actually available.

Validation:

```text
cargo fmt
cargo fmt --check
node --test tools/validate-video-deliverables.test.mjs
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker final_video_deliverables_do_not_require_subtitle_page_map_without_transcript_alignment --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker durable_published_version --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

Result:

- rustfmt check passed;
- validator tests passed, 19 tests, including the new no-subtitle package acceptance case and the existing malformed subtitle map rejection case;
- media-worker no-subtitle package contract test passed, proving package summary, published manifest, version history, and durable manifest no longer require `subtitle_page_map`;
- controlled video sample deliverable contract still passed, proving mapped subtitle packages remain valid;
- media-worker durable published version tests passed, 2 tests;
- platform-api video extraction tests passed, 3 tests;
- platform-api video PPT tests passed, 5 tests;
- whitespace check passed.

Remaining work:

- P1-3B main-site uploaded video live/controlled smoke still requires explicit user authorization because it writes a main-site smoke record;
- P1-3C third-party video registration live smoke still requires inbound bearer, connection id, source id, and authorization;
- P1-3D live handoff pass still waits for an approved deployment window before running live main/external handoff smoke;
- P2-1C authorized capture sample remains unexecuted until operator approval and source details are supplied;
- P2-2E still needs the three-sample quality review matrix and human review conclusions.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, or generated artifact local path was recorded.

## 2026-06-08 Dark Stable Segment Auto-Selection Guard Local Slice

Task source: P2-2C from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- reduce false-positive auto-selected PPT pages from stable black screens or dark transition frames;
- keep the rule narrow so dark-theme slides with visible text/graphics are not rejected only because they are dark;
- preserve manual keep-list behavior and existing stable PPT page auto-selection.

Implemented behavior:

- auto-selection now computes luma summary from the midpoint frame of each stable visual cluster;
- if the midpoint is both very dark and low-information, the cluster is written to `rejected_clusters` with reason `dark_low_information_stable_segment`;
- rejected dark clusters do not enter `selected_candidate_indices`;
- cluster policy text now documents that short unstable visual changes and dark low-information stable segments are rejected;
- the rejected cluster records selected candidate index, average luma, luma range, and guard thresholds for review.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker auto_select --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
git diff --check
```

Result:

- rustfmt check passed;
- auto-select tests passed, 2 tests, including the new dark stable transition guard fixture;
- selected-slides subtitle/quality-report path passed, 1 test;
- controlled video sample deliverable contract passed, 1 test;
- platform-api video extraction tests passed, 3 tests;
- platform-api video PPT tests passed, 5 tests;
- video deliverables validator tests passed, 18 tests;
- whitespace check passed.

Remaining P2-2 work:

- P2-2C still needs broader transition-frame and speaker-obstruction handling against real公开视频课程 samples;
- P2-2B still needs more crop-quality review across public and customer-authorized samples;
- P2-2D still needs subtitle/transcript/OCR alignment samples;
- P2-2E still needs the three-sample quality review matrix and human review conclusions.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, or generated artifact local path was recorded.

## 2026-06-08 Bright Canvas Slide Rectangle Detector Local Slice

Task source: P2-2B from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- reduce full-frame fallback for a conservative class of slide frames where a bright PPT canvas is embedded in a low-contrast player or page background;
- preserve existing detector priority for stronger detectors such as background contrast, foreground component refinement, and edge projection;
- keep every detector crop marked `review_required=true`;
- update the public validator so generated detector modes are accepted by the deliverable contract.

Implemented behavior:

- media-worker adds `bright_canvas_v1` as a fallback detector after background contrast and edge projection;
- `bright_canvas_v1` only promotes a crop when a large bright rectangular region is clearly inside frame bounds, has a plausible slide aspect ratio, and enough bright-pixel density;
- existing `edge_projection_v1` retains precedence for strong rectangular edge-line scenes;
- aggregate rectangle mode can now surface `bright_canvas_v1`;
- validator now accepts both `foreground_component_v1` and `bright_canvas_v1` as valid slide rectangle modes;
- validator test coverage now proves both modes are accepted.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
git diff --check
```

Result:

- rustfmt check passed;
- slide rectangle tests passed, 5 tests, including the new low-contrast bright canvas fixture;
- controlled video sample deliverable contract passed, 1 test;
- selected-slides subtitle/quality-report path passed, 1 test;
- platform-api video extraction tests passed, 3 tests;
- platform-api video PPT tests passed, 5 tests;
- video deliverables validator tests passed, 18 tests, including foreground component and bright canvas detector-mode acceptance;
- whitespace check passed.

Remaining P2-2 work:

- broader P2-2B crop quality work still needs real sample review against公开视频课程 and customer-authorized video;
- P2-2C still needs stronger stable-interval, transition-frame, and duplicate-page handling;
- P2-2D still needs an approved subtitle/transcript/OCR sample;
- P2-2E still needs the three-sample quality review matrix and human review conclusions.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, or generated artifact local path was recorded.

## 2026-06-08 Selected Slide OCR Evidence Local Slice

Task source: P2-2D from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- carry time-windowed keyframe OCR snippets from parsed video evidence into selected slide artifacts;
- expose OCR evidence in `slide_notes.md` and `video_slides.md` for per-slide review;
- keep `subtitle_page_map.json` transcript-only, so OCR is not misrepresented as subtitles;
- do not change source access, capture, WeChat Video Channels, upload, or third-party authorization behavior.

Implemented behavior:

- selected slide manifest now records `ocr_alignment_status` and `ocr_snippets` for snippets whose timestamp falls inside the slide transcript window;
- `slide_notes.md` now renders `Aligned OCR snippets` when selected slides have OCR evidence;
- `video_slides.md` now renders an `OCR evidence` section per slide when OCR snippets are available;
- when transcript is missing but OCR exists, notes still say no transcript is aligned and show OCR separately;
- subtitle page map generation remains based on transcript segments only.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
git diff --check
```

Result:

- rustfmt check passed;
- selected-slides transcript/OCR path passed, 1 test, including selected manifest, slide notes, and Markdown deck assertions;
- controlled video sample deliverable contract passed, 1 test;
- platform-api video extraction tests passed, 3 tests;
- platform-api video PPT tests passed, 5 tests;
- video deliverables validator tests passed, 18 tests;
- whitespace check passed.

Remaining P2-2 work:

- P2-2D still needs a real approved sample with usable subtitles/transcript/OCR to verify alignment quality beyond deterministic fixture coverage;
- `subtitle_page_map.json` still requires transcript/subtitle evidence and is not filled by OCR-only snippets;
- P2-2E still needs the three-sample quality review matrix and human review conclusions.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, or generated artifact local path was recorded.

## 2026-06-08 Slide Quality Report OCR Coverage Local Slice

Task source: P2-2D from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- make `slide_quality_report.json` summarize OCR coverage without requiring operators to inspect selected slide manifests first;
- keep OCR as review evidence, not as a replacement for transcript/subtitle page mapping;
- preserve legacy package compatibility and redaction checks.

Implemented behavior:

- `slide_quality_report.json` summary now includes `ocr_mapped_count` and `ocr_missing_count`;
- each report slide row now includes `ocr_alignment_status`, `ocr_snippet_count`, and `ocr_risk`;
- OCR risk is `low` when a slide has OCR snippets, `medium` for explicitly unmatched OCR evidence, and `high` when OCR is missing;
- validator now requires OCR summary fields and per-slide OCR risk/count fields when a quality report is present;
- validator fixture and media-worker selected-slides test cover the new OCR quality-report fields.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
git diff --check
```

Result:

- rustfmt check passed;
- selected-slides transcript/OCR/quality-report path passed, 1 test;
- controlled video sample deliverable contract passed, 1 test;
- platform-api video extraction tests passed, 3 tests;
- platform-api video PPT tests passed, 5 tests;
- video deliverables validator tests passed, 18 tests;
- whitespace check passed.

Remaining P2-2 work:

- P2-2D still needs a real approved sample with usable subtitles/transcript/OCR to verify alignment quality beyond deterministic fixture coverage;
- `subtitle_page_map.json` still requires transcript/subtitle evidence and is not filled by OCR-only snippets;
- P2-2E still needs the three-sample quality review matrix and human review conclusions.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, or generated artifact local path was recorded.

## 2026-06-08 Complete Executable Plan Refresh

Task source: user requested the previous plan-only consolidation to be completed, including original plan scope, video/PPT extraction tests, extraction-quality review, WeChat Video Channels / login-gated source handling, and possible authorized recording fallback.

Scope:

- plan-only update; no worker/API/web/script implementation changes;
- keep `docs/plans/datamax-active-execution-plan.md` as the single active plan;
- make the next-stage order explicit: main-site uploaded-video smoke, third-party video registration special-trigger smoke, video-channel/login-gated handoff live pass after deployment approval, authorized capture sample after operator approval, and quality-review follow-up;
- add a no-live-authorization local fallback slice: P2-2F sharpness/readability quality signals for `slide_quality_report.json`;
- preserve the boundary that DataMax extracts slides/courseware already shown in a video, and does not convert arbitrary ordinary video into authored PPT;
- preserve the boundary that WeChat Video Channels and login-gated sources are not auto-fetched; accepted next actions remain uploaded video file, anonymous direct video URL, or operator-approved capture handling.

Plan updates:

- active plan now starts with a concise execution overview, current proven capability baseline, next priority table, local-development fallback, and authorization gates;
- P2-2 now includes P2-2F `sharpness/readability` as the next unblocked local quality slice;
- runbook section 5.8 now contains P2-2F target files, implementation steps, validation commands, and acceptance criteria;
- validation matrix, next-execution suggestions, and milestone table now include P2-2F separately from the three-sample P2-2E review matrix;
- 8-server actions remain gated behind explicit approval and are not implied by GitHub sync.

Safety result:

- no live upload smoke, third-party live smoke, private-video smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Slide Sharpness Quality Signal Local Slice

Task source: P2-2F from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- add optional sharpness/readability evidence to `slide_quality_report.json`;
- keep the signal derived from selected slide frame image bytes only, without provider text or source URL dependence;
- keep legacy packages valid when the quality report does not contain sharpness fields;
- use low sharpness as a review signal, not as a hard failure for screenshot-based PPTX delivery;
- do not change source access policy, WeChat Video Channels handling, capture fallback behavior, upload semantics, or third-party authorization.

Implemented behavior:

- media-worker measures selected slide frame sharpness when the internal frame can be decoded;
- per-slide quality rows now include `sharpness_status`, `sharpness_score`, and `sharpness_risk`;
- `sharpness_status=measured` uses a 0-100 score and `low`/`medium`/`high` risk;
- undecodable or unavailable frames use `sharpness_status=unavailable`, `sharpness_score=null`, and `sharpness_risk=unknown`;
- quality report summary now includes `sharpness_low_count`, `sharpness_medium_count`, `sharpness_high_count`, and `sharpness_unknown_count`;
- reports add `frame_sharpness_review_required` when high/unknown sharpness pages need review;
- validator keeps old reports compatible, but validates sharpness row fields, summary counts, score ranges, enum values, and row/summary consistency when sharpness fields are present;
- direct sharpness fixture tests distinguish a crisp text-like slide from a uniform low-information frame.

Validation:

```text
cargo fmt
cargo fmt --check
node --check tools/validate-video-deliverables.mjs
node --test tools/validate-video-deliverables.test.mjs
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker measures_slide_frame_sharpness_for_quality_report --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

Result:

- Rust formatting and format check passed;
- validator syntax check passed;
- validator tests passed, 19 tests;
- `npm run test:video-deliverables` passed, 19 tests;
- media-worker sharpness helper test passed, 1 test;
- media-worker selected-slides test passed, 1 test;
- controlled video sample deliverable contract passed, 1 test;
- slide rectangle tests passed, 5 tests;
- platform-api video extraction tests passed, 3 tests;
- platform-api video PPT tests passed, 5 tests;
- whitespace check passed.

Remaining P2-2 work:

- P2-2B/P2-2C still need broader real-sample quality review for crop fallback, transition frames, speaker obstruction, and dark-theme slides;
- P2-2D still needs a real approved subtitle/transcript/OCR sample;
- P2-2E still needs the three-sample quality review matrix and human review conclusions.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Low-Information Stable Segment Guard

Task source: P2-2C follow-up from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- reduce false-positive auto-selected PPT pages from stable black screens, white screens, flat gray loading screens, bright blanks, or low-information transition frames;
- keep the existing dark low-information stable-segment guard intact while extending it to bright and flat low-information segments;
- preserve ordinary stable PPT page auto-selection and selected-slide deliverable contracts;
- avoid live extraction, network fetches, uploads, browser recording, or deployment.

Implemented behavior:

- auto-selection now rejects dark, bright, and flat low-information stable segments;
- low-information rejection uses the midpoint frame luma summary from the existing visual-signature grid;
- rejected bright segments are recorded in `slide_image_candidates_manifest.auto_selection.rejected_clusters` with `reason=bright_low_information_stable_segment`;
- rejected flat gray/loading segments are recorded with `reason=flat_low_information_stable_segment`;
- `ordinary_video_guard` policy text now states that short unstable visual changes and dark, bright, or flat low-information stable segments are rejected;
- selected slides, PPTX generation, rectangle manifests, and deliverable package output remain unchanged for valid stable PPT pages.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker auto_selects_slides_without_bright_stable_transition_segments --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects_slides_without_dark_stable_transition_segments --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects_slides_without_flat_stable_loading_segments --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
npm run test:video-deliverables
node --test tools/validate-video-deliverables.test.mjs
git diff --check
```

Result:

- formatting and format check passed;
- bright low-information guard test passed;
- dark low-information guard test passed;
- flat low-information guard test passed;
- `auto_selects` passed, 4 tests;
- `selected_slides` filtered regression passed;
- controlled video sample deliverable contract passed;
- slide rectangle regression passed, 5 tests;
- video deliverable validator passed, 19 tests;
- whitespace check passed.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated temp frames stayed under test temp directories and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Video PPT Quality Matrix Local Deliverables Input

Task source: P2-2E-1 from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- extend the P2-2E quality matrix smoke beyond pure self-test so it can review a local video/PPT `generated_artifacts/` package;
- reuse the public `validate-video-deliverables` contract instead of duplicating deliverable checks;
- keep public course video and customer-authorized video cases pending until real approved inputs exist;
- avoid live extraction, network fetches, uploads, browser recording, or deployment.

Implemented behavior:

- `npm run smoke:video-ppt-quality-matrix` now accepts `--synthetic-deliverables <path>`;
- `VIDEO_PPT_QUALITY_MATRIX_SYNTHETIC_DELIVERABLES` can provide the same path through the environment;
- `--self-test` and `--synthetic-deliverables` are mutually exclusive;
- local deliverables mode writes `status=partial_local_deliverables_reviewed`, `self_test=false`, `input_mode=synthetic_deliverables`, and `matrix_complete=false`;
- the synthetic case records only validator status, error/warning codes, checked file kinds, slide counts, quality score, risk flags, and summary counts;
- report redaction keeps `source_urls_included=false`, `object_paths_included=false`, `credentials_included=false`, `provider_payloads_included=false`, and `deliverables_input_redacted=true`;
- public course and customer-authorized cases remain `pending_accessible_sample` and `pending_authorization`.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --help
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty
node tools/validate-video-deliverables.mjs target/video-ppt-quality-matrix-fixture/generated_artifacts
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables target/video-ppt-quality-matrix-fixture/generated_artifacts --pretty --output-dir target/video-ppt-quality-matrix-smoke
npm run smoke:video-ppt-quality-matrix -- --self-test --synthetic-deliverables target/video-ppt-main-visible-release-gate-smoke/20260607135915/downloads
```

Result:

- syntax check passed;
- help output returned both `--self-test` and `--synthetic-deliverables` usage;
- self-test passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`;
- temporary fixture validator passed for PPTX, final/published/version/extraction manifests, slide rectangle manifest, slide notes, Markdown deck, and `slide_quality_report.json`;
- fixture has no `subtitle_page_map.json`, and validator accepted that as the existing conditional subtitle-map contract;
- local deliverables matrix passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`, `matrix_complete=false`;
- generated local-deliverables matrix report: `target/video-ppt-quality-matrix-smoke/20260607T173025-54508-synthetic-deliverables.json`;
- mutual-exclusion check failed as expected when both `--self-test` and `--synthetic-deliverables` were provided.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated fixture and smoke reports stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Video PPT Quality Matrix Self-Test Scaffold

Task source: P2-2E from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- add a deterministic self-test entrypoint for the P2-2E three-sample quality review matrix;
- cover the required categories: synthetic PPT playback, public course video, and customer-authorized video;
- keep public course and customer samples pending until real approved inputs exist;
- do not run live extraction, download videos, upload files, record browser sessions, or deploy services.

Implemented behavior:

- added package script `smoke:video-ppt-quality-matrix`;
- added `scripts/smoke/video-ppt-quality-matrix.mjs`;
- documented the entrypoint in `scripts/README.md`;
- self-test report schema is `v3.video_ppt_quality_matrix_smoke.v1`;
- self-test sets `matrix_complete=false` and `status=partial_local_self_test_ready`;
- synthetic PPT playback case is locally classified as `deliverable`;
- public course video case is classified as `pending_accessible_sample`;
- customer-authorized video case is classified as `pending_authorization`;
- safety gates record `live_smoke_run=false`, `production_write_allowed=false`, and generated reports as non-committable.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --help
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty
```

Result:

- syntax check passed;
- help output returned expected usage and safety notes;
- self-test passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`;
- generated report: `target/video-ppt-quality-matrix-smoke/20260607T171254-6254-self-test.json`;
- report redaction flags showed no source URLs, object paths, credentials, or provider payloads included.

Remaining P2-2E work:

- attach a real synthetic PPT playback extraction report to the matrix;
- run a public course video sample only after an anonymous direct video URL or uploaded fixture is approved;
- run a customer-authorized sample only after explicit customer/operator authorization;
- keep generated videos, PPTX files, reports, and matrix outputs under `target/` and out of Git.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.
