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

## 2026-06-08 Plan-Only Final Executable Plan Closeout

Task source: user confirmed this is a plan-only step and requested completion of the previous full executable plan. This closeout makes the final execution entry explicit and separates pushed GitHub baseline, local candidate script changes, live/customer/deployment gates, and 8-server restrictions.

Scope:

- plan-only update to `docs/plans/datamax-active-execution-plan.md`;
- validation-ledger note only in this file;
- desktop plan copy synchronization to `/Users/manslive01/Desktop/datamax-active-execution-plan.md`;
- no business-code implementation in this step;
- no main-site live upload smoke;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Plan updates:

- added section `0.7.20 2026-06-08 plan-only 最终可执行方案` as the current execution entry;
- corrected the top-level current baseline: pushed GitHub HEAD is `85222eb`, while M6AV customer authorization argument gate remains a local candidate until separately staged, verified, committed, and pushed;
- kept the product boundary that video PPT means extracting slides/courseware already shown in a video, not creating an authored PPT from ordinary video;
- kept WeChat Video Channels/login-gated/private playback sources on handoff unless an uploaded video file, anonymous direct video URL, or operator-approved capture exists;
- made the execution split explicit: P0 plan-only, P1 no-live baseline, P2 main-site upload live, P3 third-party live, P4 handoff, P5 authorized capture, P6 customer quality matrix, P7 8-server deployment, P8 full closeout;
- recorded that GitHub doc-only sync is allowed, but it must not stage scripts, apps, crates, generated artifacts, videos, frames, PPTX files, customer files, or private paths;
- recorded that M6AV candidate scripts, if synchronized later, must run their own no-live syntax/self-test/redaction/`target` gates and still do not replace customer-authorized sample execution.

Validation:

```text
git status --short --branch
cp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
cmp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
git diff --check
git diff --name-only
git ls-files target | wc -l
```

Result:

- desktop plan copy matches the repository plan;
- `git diff --check` passed;
- changed files are limited to plan/validation docs plus the already-isolated M6AV candidate scripts;
- `git ls-files target | wc -l` returned `0`;
- M6AV scripts remain local candidate code unless the user explicitly asks to collect that code slice.

Remaining work:

- P2 main-site upload live controlled smoke still needs explicit approval to write one non-customer smoke record and a safe video input;
- P3 third-party live smoke still needs bearer, `connection_id`, `source_id`, and safe input;
- P4 WeChat/login-gated handoff live pass still needs an approved 8-server deployment window;
- P5 authorized capture live sample still needs a complete approval record and playable source;
- P6 customer quality matrix still needs a customer/operator authorized sample and retention policy;
- P7 8-server deployment remains blocked until the user explicitly approves a deployment window.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Customer Authorization Argument Gate GitHub Sync

Task source: continue developing against the active execution plan after the plan-only closeout. The M6AV candidate script slice was isolated earlier; this entry records the actual no-live validation and GitHub sync.

Scope:

- code slice only in `scripts/smoke/video-ppt-quality-matrix.mjs` and `scripts/smoke/video-ppt-no-live-rollup.mjs`;
- no main-site live upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- customer deliverables argument validation is centralized in `validateInputModeArgs()`;
- quality matrix self-test rejects `--customer-deliverables` without `--customer-approval-id`;
- quality matrix self-test rejects isolated `--customer-approval-id` without `--customer-deliverables`;
- quality matrix self-test rejects mixing `--self-test` with customer deliverables flags;
- the valid customer argument shape is allowed through argument validation without reading a file;
- self-test exposes `customer_authorization_argument_gate_supported=true`;
- no-live rollup copies and validates that support bit from the quality matrix child report.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir target/video-ppt-quality-matrix-customer-args-m6av
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-customer-args-m6av
node -e "<redacted latest quality-matrix gate readback>"
node -e "<redacted latest no-live rollup status readback>"
rg -n "<local-path-url-token-raw-approval-patterns>" target/video-ppt-quality-matrix-customer-args-m6av target/video-ppt-no-live-rollup-customer-args-m6av
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- latest quality matrix report exposed `customer_authorization_argument_gate_supported=true`;
- latest quality matrix report retained `deliverables_mode_failure_class_gate_defaults_supported=true`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- latest no-live rollup report retained `full_acceptance_ready=false` with `gate_count=8`;
- latest no-live rollup report copied `customer_authorization_argument_gate_supported=true` in the quality matrix child evidence;
- sensitive-shape scan found no local absolute paths, raw URLs, bearer values, token query strings, provider keys, password-like values, raw approval ids, raw operator names, or generated-artifacts paths;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`;
- GitHub commit pushed: `21a0296 Gate customer quality matrix authorization args`.

Remaining work:

- this gate does not execute or replace a real customer-authorized quality matrix;
- P6 customer quality matrix still requires customer/operator authorization, input source, approval reference, and retention policy;
- P2/P3/P4/P5/P7/P8 remain pending their live/customer/deployment gates.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, raw approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Customer Retention Policy Argument Gate

Task source: P6 customer quality matrix still requires a customer/operator authorized sample and retention policy. M6AV required a customer approval id; this slice also requires an explicit customer retention policy reference before any customer deliverables package can be reviewed.

Scope:

- code slice only in `scripts/smoke/video-ppt-quality-matrix.mjs` and `scripts/smoke/video-ppt-no-live-rollup.mjs`;
- plan and validation-ledger update for the new required argument;
- no main-site live upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- quality matrix accepts `--customer-retention-policy` and `VIDEO_PPT_QUALITY_MATRIX_CUSTOMER_RETENTION_POLICY`;
- `--customer-deliverables` now requires both `--customer-approval-id` and `--customer-retention-policy`;
- isolated `--customer-retention-policy` without `--customer-deliverables` is rejected;
- self-test with customer deliverables flags remains rejected;
- valid customer argument shape now includes both approval id and retention policy references;
- reports record only `customer_retention_policy_reference_present=true` and `customer_retention_policy_reference_redacted=true`;
- self-test exposes `customer_retention_policy_argument_gate_supported=true`;
- no-live rollup copies and validates that support bit in the quality matrix child evidence.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir target/video-ppt-quality-matrix-retention-policy-m6aw
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-retention-policy-m6aw
node scripts/smoke/video-ppt-quality-matrix.mjs --customer-deliverables customer-deliverables-redacted --customer-approval-id customer-approval-redacted
node scripts/smoke/video-ppt-quality-matrix.mjs --customer-retention-policy customer-retention-redacted
node -e "<redacted latest quality-matrix retention gate readback>"
rg -n "customer_retention_policy_argument_gate_supported|customer_authorization_argument_gate_supported" <latest-no-live-rollup-report>
rg -n "<local-path-url-token-raw-approval-retention-patterns>" target/video-ppt-quality-matrix-retention-policy-m6aw target/video-ppt-no-live-rollup-retention-policy-m6aw
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- direct CLI negative check for customer deliverables without retention policy failed with `--customer-retention-policy is required with --customer-deliverables`;
- direct CLI negative check for isolated retention policy failed with `--customer-retention-policy requires --customer-deliverables`;
- latest quality matrix report exposed `customer_authorization_argument_gate_supported=true`;
- latest quality matrix report exposed `customer_retention_policy_argument_gate_supported=true`;
- latest no-live rollup report copied both support bits in the quality matrix child evidence;
- sensitive-shape scan found no local absolute paths, raw URLs, bearer values, token query strings, provider keys, password-like values, raw approval ids, raw retention policy values, raw operator names, or generated-artifacts paths;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this gate does not execute or replace a real customer-authorized quality matrix;
- P6 customer quality matrix still requires customer/operator authorization, input source, approval reference, retention policy, and retention cleanup expectations;
- P2/P3/P4/P5/P7/P8 remain pending their live/customer/deployment gates.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, raw approval ids, raw retention policy values, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Acceptance Evidence Summary Customer Gate Bits

Task source: M6AX no-live follow-up. M6AV/M6AW added customer authorization and retention policy argument gates to quality matrix evidence; this slice copies both support bits into `acceptance_status.no_live_evidence_summary` so P6 customer-sample prerequisites are visible in the top-level acceptance status.

Scope:

- local no-live rollup evidence-summary code slice only;
- no main-site live upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- `acceptance_status.no_live_evidence_summary.quality_customer_authorization_argument_gate_supported` now mirrors quality matrix child evidence;
- `acceptance_status.no_live_evidence_summary.quality_customer_retention_policy_argument_gate_supported` now mirrors quality matrix child evidence;
- no-live rollup self-validation fails if either support bit is missing or false after a passing quality matrix self-test.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-retention-summary-m6ax
node -e "<redacted latest acceptance evidence summary readback>"
rg -n "<local-path-url-token-raw-approval-retention-patterns>" target/video-ppt-no-live-rollup-retention-summary-m6ax
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- acceptance evidence summary exposed `quality_customer_authorization_argument_gate_supported=true`;
- acceptance evidence summary exposed `quality_customer_retention_policy_argument_gate_supported=true`;
- sensitive-shape scan found no local absolute paths, raw URLs, bearer values, token query strings, provider keys, password-like values, raw approval ids, raw retention policy values, raw operator names, or generated-artifacts paths;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this top-level evidence summary does not execute or replace a real customer-authorized quality matrix;
- P6 customer quality matrix still requires customer/operator authorization, input source, approval reference, retention policy, and retention cleanup expectations;
- P2/P3/P4/P5/P7/P8 remain pending their live/customer/deployment gates.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, raw approval ids, raw retention policy values, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Customer Quality Matrix Readiness Summary

Task source: M6AY no-live follow-up. M6AX made the customer authorization and retention policy gates visible in no-live evidence; this slice adds the matching P6 readiness fields to `acceptance_status.live_gate_readiness_summary`.

Scope:

- local no-live rollup readiness-summary code slice only;
- no main-site live upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- `acceptance_status.live_gate_readiness_summary.customer_quality_matrix_argument_gates_ready=true` when both customer approval and retention policy gates are present in quality matrix evidence;
- `acceptance_status.live_gate_readiness_summary.customer_quality_matrix_retention_policy_required=true`;
- `acceptance_status.live_gate_readiness_summary.customer_quality_matrix_customer_sample_pending=true`;
- no-live rollup self-validation fails if these readiness fields are missing or false after a passing quality matrix self-test.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-customer-readiness-m6ay
node -e "<redacted latest live gate readiness summary readback>"
rg -n "<local-path-url-token-raw-approval-retention-patterns>" target/video-ppt-no-live-rollup-customer-readiness-m6ay
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- readiness summary exposed `customer_quality_matrix_argument_gates_ready=true`;
- readiness summary exposed `customer_quality_matrix_retention_policy_required=true`;
- readiness summary exposed `customer_quality_matrix_customer_sample_pending=true`;
- acceptance status stayed `full_acceptance_ready=false`;
- sensitive-shape scan found no local absolute paths, raw URLs, bearer values, token query strings, provider keys, password-like values, raw approval ids, raw retention policy values, raw operator names, or generated-artifacts paths;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this readiness summary does not execute or replace a real customer-authorized quality matrix;
- P6 customer quality matrix still requires customer/operator authorization, input source, approval reference, retention policy, and retention cleanup expectations;
- P2/P3/P4/P5/P7/P8 remain pending their live/customer/deployment gates.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, raw approval ids, raw retention policy values, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

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

## 2026-06-08 Single-Slide Public Course Review Risk

Task source: EP6 local quality narrow fix from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- surface weak one-page public-course outputs as a quality-review risk;
- keep screenshot PPTX delivery valid when the package is otherwise complete;
- avoid treating a single selected page as a clean full-course extraction result;
- validate against the existing `Nix in Space` anonymous public slides probe;
- avoid live smoke, uploads, third-party events, browser recording, or deployment.

Implemented behavior:

- `slide_quality_report.json` now adds `summary.single_slide_output=true` when `slide_count == 1`;
- risk flags now include `single_slide_output_review_required` with `review_action=confirm_video_contains_only_one_ppt_or_reprocess_with_more_coverage`;
- the risk is advisory and does not change `deliverable_status.state=final_pptx_ready`;
- added `flags_single_slide_output_for_quality_review_without_blocking_delivery` to prove the review risk does not block PPTX delivery.

Public sample evidence:

- reran the weak public-course probe `Nix in Space` from the existing local public fixture;
- offline smoke completed with `deliverable_state=final_pptx_ready`;
- `frame_count=20`;
- `selected_count=1`;
- PPTX, `video_slides.md`, slide notes, slide rectangles, selected slides, quality report, final manifest, published manifest, version history, and extraction manifest were generated;
- `subtitle_page_map.json` remained absent, which is valid for this no-transcript sample;
- `slide_quality_report.json` records `slide_count=1`, `quality_score=50`, `summary.single_slide_output=true`, and `risk_count=4`;
- risk flags are `full_frame_rectangle_fallback`, `missing_transcript_alignment`, `single_slide_output_review_required`, and `manual_review_required`;
- `validate-video-deliverables` passed;
- public-course quality matrix passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, `not_deliverable_count=0`, and `pending_count=1`.

Validation:

```text
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker flags_single_slide_output_for_quality_review_without_blocking_delivery --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slide --lib
npm run test:video-deliverables
cargo check -p media-worker --bin video_ppt_offline_smoke
cargo run -p media-worker --bin video_ppt_offline_smoke -- <Nix in Space local public fixture>
node tools/validate-video-deliverables.mjs <Nix in Space generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <Nix in Space generated_artifacts> --pretty
git diff --check
```

Result:

- formatting check passed;
- single-slide review-risk regression passed;
- controlled video sample deliverable contract passed;
- selected-slide regression passed, 5 tests;
- video deliverable validator tests passed, 19 tests;
- offline smoke binary check passed;
- `Nix in Space` offline smoke passed with `final_pptx_ready` and one selected page;
- generated public deliverables passed validator;
- public-course quality matrix still returns `needs_manual_review`, not a clean deliverable conclusion.

Remaining work:

- this does not complete P1-3B main-site upload live smoke;
- this does not complete P1-3C third-party live smoke;
- this does not complete P1-3D live handoff because 8-server deployment remains unapproved;
- this does not complete P2-2E customer authorized sample quality matrix.

Safety result:

- no live main-site smoke was run;
- no third-party event was sent;
- no WeChat Video Channels source was fetched, captured, or bypassed;
- no customer file, private URL, cookie, token, provider payload, database URL, private object path, or raw frame path was recorded;
- generated videos, frames, PPTX, Markdown, manifests, matrix reports, and offline smoke outputs remained under `target/` and were not committed;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run.

## 2026-06-08 Customer Deliverables And Combined Quality Matrix Input

Task source: EP5/P2-2E quality matrix readiness from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- add a quality-matrix input mode for explicitly authorized customer/operator video PPT deliverables;
- allow synthetic, public-course, and customer-authorized deliverables inputs to be combined into one report;
- keep `--self-test` mutually exclusive with all deliverables inputs;
- allow `matrix_complete=true` only when all required deliverables categories are represented by reviewed inputs;
- use only local non-customer fixtures to prove script behavior;
- avoid live smoke, uploads, third-party events, browser recording, or deployment.

Implemented behavior:

- `scripts/smoke/video-ppt-quality-matrix.mjs` now accepts `--customer-deliverables <generated_artifacts>`;
- `VIDEO_PPT_QUALITY_MATRIX_CUSTOMER_DELIVERABLES` is supported as the environment fallback;
- customer deliverables now require `--customer-approval-id` or `VIDEO_PPT_QUALITY_MATRIX_CUSTOMER_APPROVAL_ID`;
- the raw approval id is not written into the report;
- customer mode classifies the provided deliverables as `customer_authorized_video`;
- customer mode sets `customer_authorized_deliverables_reviewed=true` and `customer_authorization_required=false`;
- reports still include all required categories so missing public/customer evidence cannot be hidden;
- combined mode accepts synthetic, public, and customer deliverables together and writes `matrix_complete=true` only when all three are present;
- quality evaluation now treats `summary.single_slide_output=true` as a review risk in addition to low score, crop fallback, and sharpness risks;
- `scripts/README.md` documents that deliverables flags can be combined, while `--customer-deliverables` is only for explicitly approved customer/operator inputs.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --help
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables <local-fixture-generated_artifacts> --pretty
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <single-slide-public-generated_artifacts> --pretty
npm run smoke:video-ppt-quality-matrix -- --customer-deliverables <local-fixture-generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --customer-approval-id <test-approval-id>
npm run smoke:video-ppt-quality-matrix -- --customer-deliverables <local-fixture-generated_artifacts> --customer-approval-id <test-approval-id> --pretty
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables <local-fixture-generated_artifacts> --public-course-deliverables <single-slide-public-generated_artifacts> --customer-deliverables <local-fixture-generated_artifacts> --customer-approval-id <test-approval-id> --pretty
npm run smoke:video-ppt-quality-matrix -- --self-test --customer-deliverables <local-fixture-generated_artifacts> --customer-approval-id <test-approval-id>
git diff --check
```

Result:

- syntax check passed;
- help output includes `--customer-deliverables`;
- self-test passed with `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- synthetic deliverables mode passed with `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- public-course deliverables mode passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, and `pending_count=1`;
- customer deliverables without `--customer-approval-id` failed as expected;
- `--customer-approval-id` without `--customer-deliverables` failed as expected;
- customer deliverables mode with a test approval id passed with `status=partial_customer_authorized_deliverables_reviewed`, `input_mode=customer_deliverables`, `matrix_complete=false`, `deliverable_count=2`, and `pending_count=1`;
- combined three-input mode passed with `status=complete_deliverables_matrix_reviewed`, `input_mode=combined_deliverables`, `matrix_complete=true`, `deliverable_count=2`, `needs_manual_review_count=1`, and `pending_count=0`;
- customer report redaction flags showed no source URLs, object paths, credentials, or provider payloads included;
- combined report records `customer_approval_reference_present=true` and `customer_approval_reference_redacted=true`;
- combined report did not include the test approval id value;
- combined report redaction flags showed no source URLs, object paths, credentials, or provider payloads included;
- negative self-test+customer invocation failed as expected.

Remaining work:

- this adds the customer-authorized input gate and combined three-input matrix gate but does not supply a real customer/operator authorized sample;
- P2-2E remains incomplete until a real authorized customer sample is run and manually reviewed;
- P1-3B/P1-3C/P1-3D live gates remain unchanged.

Safety result:

- no live main-site smoke was run;
- no third-party event was sent;
- no WeChat Video Channels source was fetched, captured, or bypassed;
- no customer file, private URL, cookie, token, provider payload, database URL, private object path, or raw frame path was recorded;
- generated matrix reports remained under `target/` and were not committed;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run.

## 2026-06-08 Slide Quality Risk Flag Schema Gate

Task source: P2-2E/EP6 validation hardening from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- strengthen `slide_quality_report.json` validation so malformed risk flags cannot pass the deliverable contract;
- cover the new `single_slide_output_review_required` risk introduced for weak one-page public-course outputs;
- keep legacy packages without `slide_quality_report.json` valid;
- avoid live smoke, uploads, third-party events, browser recording, or deployment.

Implemented behavior:

- `tools/validate-video-deliverables.mjs` now validates each quality `risk_flags` entry;
- each risk flag must include a non-empty `code`, `severity` in `low|medium|high`, integer `count >= 1`, and non-empty `review_action`;
- if `summary.single_slide_output=true`, the report must have `slide_count=1` and include `single_slide_output_review_required`;
- if `single_slide_output_review_required` appears, `summary.single_slide_output` must be true;
- validator keeps compatibility with reports that do not yet include `summary.single_slide_output`, unless the single-slide risk is present.

Validation:

```text
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node tools/validate-video-deliverables.mjs <current-single-slide-public-generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables <current-public-generated_artifacts> --public-course-deliverables <current-public-generated_artifacts> --customer-deliverables <current-public-generated_artifacts> --customer-approval-id <test-approval-id> --pretty
git diff --check
```

Result:

- validator syntax check passed;
- video deliverable validator tests passed, 19 tests;
- malformed slide quality report test now covers invalid risk flag severity/count/action and single-slide consistency;
- current `Nix in Space` single-slide public deliverables still pass validator;
- combined quality matrix using current generated deliverables passed with `case_count=3`, `needs_manual_review_count=3`, `pending_count=0`, and `matrix_complete=true`;
- an older local target fixture with pre-schema risk flags is correctly treated as contract-invalid under the stricter validator and is not used as current positive evidence.

Remaining work:

- this does not complete live main-site upload, third-party live smoke, handoff deployment, authorized capture, or real customer sample validation;
- customer-authorized matrix completion still requires a real authorized input and approval id.

Safety result:

- no live main-site smoke was run;
- no third-party event was sent;
- no WeChat Video Channels source was fetched, captured, or bypassed;
- no customer file, private URL, cookie, token, provider payload, database URL, private object path, approval id value, or raw frame path was recorded;
- generated validator and matrix outputs remained under `target/` and were not committed;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run.

## 2026-06-08 Visual Shape Duplicate Dedupe And Public Candidate Rerun

Task source: P2-2E-2B-Next from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- add a conservative contrast-normalized visual-shape duplicate path for selected video/PPT frames;
- keep exact frame duplicate and visual near-duplicate behavior unchanged;
- propagate shape duplicate counts through selected slides, slide rectangles, and slide quality report summaries;
- protect sparse text/build-state pages from over-dedupe;
- rerun the public media.ccc / NixCon slides candidate without live writes or deployment.

Implemented behavior:

- selected-slide dedupe can now reject `visual_shape_duplicate` candidates when the visual signature shape is high-confidence duplicate;
- shape matching uses the 32x32 visual signature, a Jaccard threshold, a strict containment fallback, a signal-sample minimum, and a conservative signal-balance threshold;
- `visual_duplicate_count` now includes both `visual_near_duplicate` and `visual_shape_duplicate`;
- `visual_shape_duplicate_count` is written to `selected_slides_manifest.json`, `slide_rectangles_manifest.json`, and `slide_quality_report.json`;
- rectangle manifest dedupe policy records the shape duplicate thresholds;
- the deterministic fixture `dedupes_selected_slide_manifest_by_visual_shape_similarity` verifies selected manifest, rectangle manifest, and quality report summary counts;
- an initial looser containment threshold over-deduped a sparse text/build fixture, so the signal-balance threshold was tightened and `auto_selects_sparse_text_build_states_in_public_course_style_slides` was kept as the guard.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker dedupes_selected_slide_manifest_by_visual_shape_similarity --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slide --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
cargo check -p media-worker --bin video_ppt_offline_smoke
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input <target-public-candidate-mp4> --output-root target/video-ppt-public-candidate-extraction-shape-dedupe --ffmpeg-bin /opt/homebrew/bin/ffmpeg --interval-seconds 15 --title "Public slides sample: How to teach Nix in 5 minutes" --json-output target/video-ppt-public-candidate-extraction-shape-dedupe/offline-smoke-summary.json
node tools/validate-video-deliverables.mjs <shape-dedupe-public-candidate-generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <shape-dedupe-public-candidate-generated_artifacts> --pretty --output-dir target/video-ppt-public-candidate-quality-matrix-shape-dedupe-final
git diff --check
```

Result:

- formatting and format check passed;
- visual shape duplicate fixture passed;
- selected-slide regression passed, 5 tests;
- auto-selection regression passed, 7 tests, including sparse text/build-state preservation;
- controlled video sample deliverable contract passed;
- video deliverable validator passed, 19 tests;
- offline smoke helper compiled;
- public candidate offline smoke completed with `deliverable_state=final_pptx_ready`;
- public candidate `frame_count=96`;
- public candidate requested/final selected counts: 9/7;
- public candidate requested selected indices: `[2, 16, 25, 31, 39, 55, 66, 85, 93]`;
- public candidate final selected indices: `[2, 16, 25, 31, 55, 85, 93]`;
- public candidate duplicate counts: `deduped_candidate_count=2`, `exact_duplicate_count=0`, `visual_duplicate_count=2`, `visual_shape_duplicate_count=0`;
- duplicate removals remained the two existing visual near-duplicates: candidate 39 matched 31, and candidate 66 matched 55;
- rectangle manifest and quality report summary matched the selected manifest duplicate counts;
- public candidate validator passed for PPTX, Markdown, slide notes, rectangle manifest, quality report, and final/published/version/extraction manifests;
- public-course quality matrix passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, `not_deliverable_count=0`, and `pending_count=1`;
- public candidate quality score remained 61;
- risk flags remain `full_frame_rectangle_fallback`, `missing_transcript_alignment`, `selected_slide_duplicates_removed`, `frame_sharpness_review_required`, and `manual_review_required`.

Page-level review conclusion:

- the final selected frames are real courseware slides, not ordinary non-PPT video content;
- the public sample still contains a weak title fade state and quality risks, so it should remain classified as `needs_manual_review`;
- the conservative shape duplicate rule did not remove any additional public-candidate page in this rerun, which is acceptable because the sparse text/build regression proved that looser containment can over-dedupe meaningful build states;
- this slice improves duplicate accounting and gives a guarded shape-dedupe path, but it does not complete full P2-2E because customer-authorized video remains pending.

Safety result:

- no customer/private/login-gated video was used;
- no WeChat Video Channels page was fetched;
- no main-site upload, third-party event, live smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media, frames, generated PPTX, manifests, quality matrix output, and contact-sheet review image stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full public candidate media path was recorded.

## 2026-06-08 Second Public Course Probe And Bright Template Build Guard

Task source: no-auth follow-up from `docs/plans/datamax-active-execution-plan.md` after P2-2E-2B-Next.

Scope:

- try additional anonymous public-course slides videos to avoid relying on a single media.ccc sample;
- keep all downloads, frames, PPTX, Markdown, and matrix reports under `target/`;
- verify that visual-shape duplicate does not remove meaningful white-background slide build states;
- avoid main-site writes, third-party events, browser recording, service builds, service restarts, and 8-server deployment.

Additional public probes:

- `Nix in Space` slides MP4 was anonymously accessible and downloaded to `target/`; ffprobe reported about 296.8 seconds and about 9.9 MB.
- `Nix in Space` offline smoke completed with `final_pptx_ready`, `frame_count=20`, but selected only 1 final page. Page-level review showed a browser/Google Slides screen with a single visible title-like slide. This candidate is useful as a contract smoke but downgraded as a weak public-course quality sample.
- `Layered Nix Stores` slides MP4 was anonymously accessible and downloaded to `target/`; ffprobe reported about 332.4 seconds and about 15 MB.
- Before the guard, `Layered Nix Stores` exposed an over-dedupe bug: requested candidates `[4, 8, 10, 16]` were reduced to `[4]` by `visual_shape_duplicate_count=3`, even though candidate 8 showed bullet content that candidate 4 did not.

Implemented guard:

- visual-shape duplicate now refuses bright/white-template signatures with average luma above the conservative threshold;
- rectangle manifest records `visual_shape_duplicate_max_avg_luma`;
- new fixture `keeps_bright_template_build_states_out_of_visual_shape_dedupe` creates a bright slide template with increasing bullet build states and verifies all requested pages are preserved with `visual_shape_duplicate_count=0`;
- dark-theme shape duplicate coverage remains in `dedupes_selected_slide_manifest_by_visual_shape_similarity`.

Validation:

```text
cargo fmt
CC=clang CXX=clang++ cargo test -p media-worker keeps_bright_template_build_states_out_of_visual_shape_dedupe --lib
CC=clang CXX=clang++ cargo test -p media-worker dedupes_selected_slide_manifest_by_visual_shape_similarity --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slide --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input <target-layered-nix-stores-slides-mp4> --output-root target/video-ppt-public-candidate-extraction-third-fixed --ffmpeg-bin /opt/homebrew/bin/ffmpeg --interval-seconds 15 --title "Public slides sample 3: Layered Nix Stores" --json-output target/video-ppt-public-candidate-extraction-third-fixed/offline-smoke-summary.json
node tools/validate-video-deliverables.mjs <third-public-candidate-fixed-generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <third-public-candidate-fixed-generated_artifacts> --pretty --output-dir target/video-ppt-public-candidate-quality-matrix-third-fixed
```

Result:

- bright-template build-state guard fixture passed;
- visual shape duplicate fixture still passed;
- selected-slide regression passed, 5 tests;
- auto-selection regression passed, 7 tests;
- fixed `Layered Nix Stores` public smoke completed with `deliverable_state=final_pptx_ready`;
- fixed `Layered Nix Stores` `frame_count=22`;
- fixed `Layered Nix Stores` requested/final selected indices changed from over-deduped `[4]` to `[4, 8, 16]`;
- fixed duplicate counts: `deduped_candidate_count=1`, `visual_duplicate_count=1`, `visual_shape_duplicate_count=0`;
- remaining rejected candidate 10 was a visual near-duplicate of candidate 8;
- fixed quality score: 68;
- fixed rectangle status/mode: detector crop via `edge_projection_v1`, no full-frame fallback;
- fixed risk flags: `missing_transcript_alignment`, `selected_slide_duplicates_removed`, and `manual_review_required`;
- fixed public candidate validator passed for PPTX, Markdown, slide notes, rectangle manifest, quality report, and final/published/version/extraction manifests;
- fixed public-course quality matrix passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, `not_deliverable_count=0`, and `pending_count=1`.

Current conclusion:

- second and third public probes confirm the pipeline can handle additional anonymous public slides videos, but both remain `needs_manual_review`;
- `Nix in Space` is downgraded because it selected a single browser/Slides page;
- `Layered Nix Stores` is a useful regression sample because it caught bright-template build over-dedupe and now preserves meaningful build content;
- full P2-2E remains incomplete until a customer-authorized sample is available.

Safety result:

- no customer/private/login-gated video was used;
- no WeChat Video Channels page was fetched;
- no main-site upload, third-party event, live smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- public probe media, frames, generated PPTX, manifests, and quality matrix outputs stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full public candidate media path was recorded.

## 2026-06-08 Public Candidate Sparse Text Build Auto-Selection

Task source: P2-2E-2B / P2-2C follow-up from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- improve automatic selection for course videos where slide content changes through sparse text/build states on a mostly static dark background;
- keep short animation and low-information guards intact;
- keep the public candidate classified as `needs_manual_review`, not force a clean deliverable conclusion;
- avoid live upload, third-party events, browser recording, capture fallback, or deployment.

Implemented behavior:

- increased the visual signature grid from 16x16 to 32x32 so sparse slide text has a better chance of being sampled;
- tightened the same-segment average luma diff threshold from 5.0 to 2.0;
- added a changed-sample guard with `same_segment_changed_sample_min_diff=24` and `same_segment_max_changed_sample_ratio=0.012`;
- added `auto_selects_sparse_text_build_states_in_public_course_style_slides`, a deterministic dark-background sparse-text build fixture that now selects `[2, 5, 8]`;
- cluster policy in `auto_selection` now records the visual signature grid and changed-sample guard thresholds.

Public candidate rerun result:

- `deliverable_state=final_pptx_ready`;
- `frame_count=96`;
- `requested_selected_count=9`;
- `selected_count=7`;
- requested candidates: `[3, 14, 25, 31, 39, 55, 66, 85, 93]`;
- final selected candidates: `[3, 14, 25, 31, 55, 85, 93]`;
- `deduped_candidate_count=2`;
- `visual_duplicate_count=2`;
- `has_pptx=true`;
- `has_video_slides_markdown=true`;
- `has_slide_quality_report=true`;
- `has_subtitle_page_map=false`;
- `quality_score=60`;
- public-course quality matrix conclusion remains `needs_manual_review`.

Manual visual review note:

- the rerun covers more build content than the earlier 5-page package, including `Know your Audience` and a later multi-line content state;
- two weak fade/title states are still selected, so this is not a clean deliverable;
- one selected page still uses `full_frame_fallback`;
- the next quality slice should decide whether to keep this as a `needs_manual_review` public sample or add a targeted fade/crop/sharpness fix.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker auto_selects_sparse_text_build_states_in_public_course_style_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slide --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
cargo check -p media-worker --bin video_ppt_offline_smoke
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input target/video-ppt-public-course-probe/nix-teach-5min-slides.mp4 --output-root target/video-ppt-public-candidate-extraction-grid32-buildsplit --ffmpeg-bin /opt/homebrew/bin/ffmpeg --interval-seconds 15 --title "Public slides sample: How to teach Nix in 5 minutes" --json-output target/video-ppt-public-candidate-extraction-grid32-buildsplit/offline-smoke-summary.json
node tools/validate-video-deliverables.mjs target/video-ppt-public-candidate-extraction-grid32-buildsplit/<session>/generated_artifacts
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables target/video-ppt-public-candidate-extraction-grid32-buildsplit/<session>/generated_artifacts --pretty --output-dir target/video-ppt-public-candidate-quality-matrix-grid32-buildsplit
git diff --check
```

Result:

- formatting and format check passed;
- sparse-text build fixture passed;
- `auto_selects` passed, 7 tests;
- selected-slide regression passed, 4 tests;
- controlled video sample deliverable contract passed;
- video deliverable validator test suite passed, 19 tests;
- offline smoke helper compiled;
- public candidate offline smoke completed locally with `final_pptx_ready`;
- public candidate deliverables validator passed;
- public-course quality matrix passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, `not_deliverable_count=0`, and `pending_count=1`;
- whitespace check passed.

Remaining P2-2E work:

- page-level human review must decide whether the 7-page public candidate is an acceptable `needs_manual_review` sample;
- targeted fade filtering, crop fallback reduction, or sharpness/readability tuning may still be needed before any clean public-course deliverable claim;
- the full three-sample quality matrix remains incomplete until a customer/operator-authorized sample is supplied.

Safety result:

- no main-site upload or live assistant run was created;
- no third-party event was sent;
- no WeChat Video Channels extraction, login bypass, cookie capture, browser recording, or authorized capture was performed;
- no service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media, frames, generated PPTX, manifests, and quality matrix output stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full public candidate media path was recorded.

## 2026-06-08 Public Candidate Best-Sharpness Representative Selection

Task source: P2-2E-2B / P2-2F follow-up from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- reduce weak fade/low-readability representatives when a stable visual segment has clearer frames available;
- keep midpoint selection as a deterministic tie-break when sharpness scores are equal;
- preserve existing short-transition, low-information, sparse-text build, dedupe, and deliverable contract behavior;
- keep the public candidate as `needs_manual_review` until page-level review or further fade/crop tuning proves otherwise.

Implemented behavior:

- each auto-selection frame now carries a local `sharpness_score`;
- stable cluster representative selection now chooses the highest sharpness score, then the frame closest to midpoint, then the lowest candidate index;
- `auto_selection.selected_clusters[]` records `selected_sharpness_score`;
- `selection_rule` remains `middle_frame_of_stable_visual_segment` when the midpoint wins, otherwise records `highest_sharpness_frame_of_stable_visual_segment`;
- added `selects_highest_sharpness_frame_from_stable_auto_slide_cluster` to lock the selection rule and midpoint tie-break.

Public candidate rerun result:

- `deliverable_state=final_pptx_ready`;
- `frame_count=96`;
- `requested_selected_count=9`;
- `selected_count=7`;
- requested candidates: `[2, 16, 25, 31, 39, 55, 66, 85, 93]`;
- final selected candidates: `[2, 16, 25, 31, 55, 85, 93]`;
- `deduped_candidate_count=2`;
- `visual_duplicate_count=2`;
- `has_pptx=true`;
- `has_video_slides_markdown=true`;
- `has_slide_quality_report=true`;
- `has_subtitle_page_map=false`;
- `quality_score=61`;
- `sharpness_high_count=3`;
- public-course quality matrix conclusion remains `needs_manual_review`.

Manual visual review note:

- best-sharpness improves the public candidate slightly: quality score increased from 60 to 61 and high-sharpness-risk pages dropped from 4 to 3 in the latest public rerun;
- the package still contains weak fade/title states and 1 full-frame fallback, so it is not a clean public-course deliverable;
- next quality work should either add a targeted fade/low-contrast duplicate filter, improve crop fallback for the remaining page, or keep this public candidate formally classified as `needs_manual_review`.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker selects_highest_sharpness_frame_from_stable_auto_slide_cluster --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slide --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
cargo check -p media-worker --bin video_ppt_offline_smoke
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input target/video-ppt-public-course-probe/nix-teach-5min-slides.mp4 --output-root target/video-ppt-public-candidate-extraction-bestsharp --ffmpeg-bin /opt/homebrew/bin/ffmpeg --interval-seconds 15 --title "Public slides sample: How to teach Nix in 5 minutes" --json-output target/video-ppt-public-candidate-extraction-bestsharp/offline-smoke-summary.json
node tools/validate-video-deliverables.mjs target/video-ppt-public-candidate-extraction-bestsharp/<session>/generated_artifacts
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables target/video-ppt-public-candidate-extraction-bestsharp/<session>/generated_artifacts --pretty --output-dir target/video-ppt-public-candidate-quality-matrix-bestsharp
git diff --check
```

Result:

- formatting and format check passed;
- best-sharpness rule test passed;
- `auto_selects` passed, 7 tests;
- selected-slide regression passed, 4 tests;
- controlled video sample deliverable contract passed;
- video deliverable validator test suite passed, 19 tests;
- offline smoke helper compiled;
- public candidate offline smoke completed locally with `final_pptx_ready`;
- public candidate deliverables validator passed;
- public-course quality matrix passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, `not_deliverable_count=0`, and `pending_count=1`;
- whitespace check passed.

Remaining P2-2E work:

- decide whether to keep this public candidate as the accepted `needs_manual_review` public sample or continue with a targeted fade/low-contrast duplicate filter;
- full P2-2E remains incomplete until a customer/operator-authorized sample is supplied;
- this slice does not satisfy upload live, third-party live, handoff live, or authorized capture gates.

Safety result:

- no main-site upload or live assistant run was created;
- no third-party event was sent;
- no WeChat Video Channels extraction, login bypass, cookie capture, browser recording, or authorized capture was performed;
- no service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media, frames, generated PPTX, manifests, and quality matrix output stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full public candidate media path was recorded.

## 2026-06-08 Public Candidate Selected Indices Semantics

Task source: P2-2E-2B from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- clarify the selected slides manifest after exact/visual dedupe;
- make final `selected_candidate_indices` match the pages that actually enter PPTX/Markdown;
- preserve the pre-dedupe request for audit without confusing it with final selected pages;
- keep the public candidate result in `needs_manual_review`, not force it to `deliverable`;
- avoid live upload, third-party events, browser recording, or deployment.

Implemented behavior:

- `selected_slides_manifest.json` now writes `requested_selected_candidate_indices` for the requested/pre-dedupe candidate list;
- `selected_candidate_indices` now contains only candidates that survive exact/visual dedupe and become final selected pages;
- `requested_selected_count` continues to count requested candidates, while `selected_count` continues to count final selected slides;
- selected-slide tests now assert both requested and final indices for manual, exact-dedupe, and visual-dedupe cases;
- public candidate offline smoke was rerun locally under `target/`, producing a new `final_pptx_ready` package.

Public candidate rerun result:

- `deliverable_state=final_pptx_ready`;
- `frame_count=96`;
- `selected_count=5`;
- `has_pptx=true`;
- `has_video_slides_markdown=true`;
- `has_slide_quality_report=true`;
- `has_subtitle_page_map=false`;
- requested candidates: `[3, 14, 33, 55, 66, 85, 93]`;
- final selected candidates: `[3, 14, 33, 85, 93]`;
- `deduped_candidate_count=2`;
- `visual_duplicate_count=2`.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker selected_slide --lib
CC=clang CXX=clang++ cargo test -p media-worker dedupes_selected_slide_manifest --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects_stable_ppt_pages_from_decodable_raw_frames_without_keep_list --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
cargo check -p media-worker --bin video_ppt_offline_smoke
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input target/video-ppt-public-course-probe/nix-teach-5min-slides.mp4 --output-root target/video-ppt-public-candidate-extraction-selected-indices --ffmpeg-bin /opt/homebrew/bin/ffmpeg --interval-seconds 15 --title "Public slides sample: How to teach Nix in 5 minutes" --json-output target/video-ppt-public-candidate-extraction-selected-indices/offline-smoke-summary.json
node tools/validate-video-deliverables.mjs target/video-ppt-public-candidate-extraction-selected-indices/<session>/generated_artifacts
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables target/video-ppt-public-candidate-extraction-selected-indices/<session>/generated_artifacts --pretty --output-dir target/video-ppt-public-candidate-quality-matrix-selected-indices
git diff --check
```

Result:

- formatting and format check passed;
- selected-slide regression passed, 4 tests;
- selected-slide exact/visual dedupe regression passed, 2 tests;
- stable PPT page auto-selection regression passed;
- controlled video sample deliverable contract passed;
- video deliverable validator test suite passed, 19 tests;
- offline smoke helper compiled;
- public candidate offline smoke completed locally with `final_pptx_ready`;
- public candidate deliverables validator passed;
- public-course quality matrix passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, `not_deliverable_count=0`, and `pending_count=1`;
- whitespace check passed.

Remaining P2-2E work:

- page-level human review still needs to decide whether the 5 final pages are acceptable as a `needs_manual_review` public sample or whether a follow-up selection/crop/sharpness fix is needed;
- the full three-sample quality matrix remains incomplete until a customer/operator-authorized sample is supplied;
- this slice does not satisfy P1-3B upload live, P1-3C third-party live, P1-3D handoff live, or P2-1C authorized capture gates.

Safety result:

- no main-site upload or live assistant run was created;
- no third-party event was sent;
- no WeChat Video Channels extraction, login bypass, cookie capture, browser recording, or authorized capture was performed;
- no service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media, frames, generated PPTX, manifests, and quality matrix output stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full public candidate media path was recorded.

## 2026-06-08 Public Slides Video Candidate Access Probe

Task source: S1 / P2-2E-2 from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- find a public, non-customer, anonymously downloadable video candidate whose content is slide/presentation screen recording;
- verify only source access and sampled visual suitability before attempting full DataMax extraction;
- keep downloaded videos, byte-range probes, HTML pages, and frame images under `target/`;
- avoid main-site writes, third-party events, browser recording, service builds, service restarts, and 8-server deployment.

Probe method:

- searched for public pages that expose direct MP4 slide/presentation videos;
- used `curl -fsSI -L` and `curl -fsSL -H 'Range: bytes=0-1048575'` for HEAD/range checks;
- used macOS AVFoundation through a one-off Swift command to export sampled JPEG frames from downloaded public candidates;
- visually inspected sampled frames locally; no generated frames or raw videos are committed.

Rejected or deferred candidates:

- GCU edShare `ALC EEE2` candidates: page was publicly viewable and direct video URLs returned `video/mp4` with `Accept-Ranges: bytes`, but sampled frames at 0/60/120/180/240 seconds were lecturer-camera video rather than visible slide playback, so this is not a good P2-2E public slides sample.
- media.ccc `Nix in Space` slides feed: direct slides MP4 returned `video/mp4`, supported byte ranges, and decoded as a slide feed, but sampled frames showed a mostly static single slide/website screen; keep only as a fallback accessibility probe, not a quality-matrix sample.
- one MIT mirror candidate returned 404 and was excluded.

Selected candidate for next extraction step:

- Host category: public media.ccc CDN / mirror-hosted NixCon 2023 presentation.
- Event title: `How to teach Nix in 5 minutes!`.
- Public event page status: HTTP 200.
- Page metadata: title present, `duration=PT1444S`, and page HTML includes a `source` entry labeled `slides eng 1080p` with `type=video/mp4`.
- Download surface: page contains a `Slides` section with `Download mp4`.
- Direct slides media probe: initial CDN URL redirects to a public mirror, final response is `video/mp4`, `content-length=48967286`, `accept-ranges=bytes`.
- Range probe: `206 Partial Content`, `content-range=bytes 0-1048575/48967286`.
- Full-file local probe: downloaded only under `target/`; SHA-256 prefix `b6ef1adf0290`.
- Visual suitability probe: sampled frames at 30/60/180 seconds show the title slide; 300 and 420 seconds show `Knowing where you are and why you are there`; 360 seconds shows `Know your Audience`. This proves the candidate has visible slide content and at least multiple distinct slide states.

Current status:

- `candidate_accessible=true`
- `candidate_visual_slide_content=true`
- `candidate_has_multiple_slide_states=true`
- `full_extraction_run=false`
- `quality_matrix_run=false`
- `matrix_complete=false`

Next step:

- Use this candidate as the first P2-2E public slide/presentation sample for an offline or controlled extraction run.
- After `generated_artifacts/` exists, run:

```text
node tools/validate-video-deliverables.mjs <generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables <generated_artifacts> --pretty --output-dir target/video-ppt-quality-matrix-smoke
```

Safety result:

- no customer/private/login-gated video was used;
- no WeChat Video Channels page was fetched;
- no main-site upload, third-party event, live smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media and sampled frames stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Public Candidate Exploratory Extraction Blocked By Manifest Redaction

Task source: S2A / P2-2E-2 from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- run a local exploratory extraction against the selected public media.ccc / NixCon slides candidate;
- prove whether the candidate can reach screenshot-based PPTX generation;
- reuse the deliverables validator and quality matrix as acceptance gates;
- avoid main-site writes, third-party events, browser recording, service builds, service restarts, and 8-server deployment.

Extraction result:

- `deliverable_state=final_pptx_ready`
- `frame_extraction_status=completed`
- `frame_count=96`
- PPTX generated: yes
- `video_slides.md` generated: yes
- `slide_notes.md` generated: yes
- `selected_slides_manifest.json` generated: yes
- `slide_rectangles_manifest.json` generated: yes
- `slide_quality_report.json` generated: yes
- `subtitle_page_map.json` generated: no, because there was no transcript/subtitle evidence for this candidate
- selected slides observed from the selected-slides manifest: 5
- PPTX slide count observed by quality matrix: 5
- Markdown slide count observed by quality matrix: 5

Quality report summary:

- `quality_score=56`
- risk flags included `full_frame_rectangle_fallback`, `missing_transcript_alignment`, `selected_slide_duplicates_removed`, `frame_sharpness_review_required`, and `manual_review_required`
- summary counted 1 full-frame fallback, 4 detector crops, 5 missing transcript alignments, 5 missing OCR alignments, and 4 high sharpness/readability review risks

Acceptance gate result:

```text
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts>
```

Result: failed.

Failure class:

- `unredacted_local_path_or_token` in `final_deliverables_manifest.json`
- `unredacted_local_path_or_token` in `extraction_artifacts_manifest.json`
- `unredacted_local_path_or_token` in `published_deliverable_manifest.json`
- `unredacted_local_path_or_token` in `published_version_history.json`

Quality matrix result:

```text
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables <public-candidate-generated_artifacts> --pretty --output-dir target/video-ppt-public-candidate-quality-matrix
```

Result:

- `matrix_complete=false`
- `deliverable_count=0`
- `not_deliverable_count=1`
- `pending_accessible_sample_count=1`
- `pending_authorization_count=1`
- the local candidate package was classified as `deliverable_contract_invalid`, so the sample is not accepted yet

Current conclusion:

- The selected public candidate is suitable for continuing P2-2E because it is public, anonymously downloadable, has multiple slide states, and can reach screenshot-based PPTX generation.
- The candidate is not accepted as a public-course quality sample until public manifests pass redaction validation.
- The next development slice is to fix nested public-manifest redaction without relaxing `tools/validate-video-deliverables.mjs`, then rerun this same candidate.
- If the validator passes after the fix, the remaining quality decision may still be `需人工复核` because this candidate has missing transcript alignment, duplicate removal, full-frame fallback, and sharpness/readability review risks.

Safety result:

- no customer/private/login-gated video was used;
- no WeChat Video Channels page was fetched;
- no main-site upload, third-party event, live smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media, frames, generated PPTX, manifests, and quality matrix output stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full validator-offending value was recorded.

## 2026-06-08 Public Candidate Manifest Redaction Fix And Quality Matrix Pass

Task source: P2-2E-2A from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- fix public artifact manifest redaction without relaxing `tools/validate-video-deliverables.mjs`;
- keep the offline smoke helper local-safe and non-production;
- rerun the same public media.ccc / NixCon slides candidate;
- classify the result through the existing quality matrix.

Implemented behavior:

- public artifact entries now pass through recursive public evidence redaction before being written into final/published/version/extraction manifests;
- non-download public artifact `uri` fields such as internal `artifact://...` pointers are redacted and marked with `uri_redacted=true`, avoiding false public URL/path exposure and validator pattern matches;
- artifact `path` remains `[redacted]` with `path_redacted=true`;
- file names remain visible through `file_name`;
- the offline smoke helper now reads `selected_count` from the generated `selected_slides_manifest` artifact entry instead of a missing summary path.

Validation:

```text
cargo fmt
cargo fmt --check
cargo check -p media-worker --bin video_ppt_offline_smoke
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input <target-public-candidate-mp4> --output-root target/video-ppt-public-candidate-extraction --ffmpeg-bin /opt/homebrew/bin/ffmpeg --interval-seconds 15 --title "Public slides sample: How to teach Nix in 5 minutes" --json-output target/video-ppt-public-candidate-extraction/offline-smoke-summary.json
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables <public-candidate-generated_artifacts> --pretty --output-dir target/video-ppt-public-candidate-quality-matrix
```

Result:

- formatting and format check passed;
- offline smoke helper compiled;
- media-worker controlled deliverable contract test passed;
- video deliverables validator test suite passed, 19 tests;
- public candidate offline smoke completed with `deliverable_state=final_pptx_ready`;
- `frame_count=96`;
- `selected_count=5`;
- PPTX generated: yes;
- `video_slides.md` generated: yes;
- `slide_notes.md` generated: yes;
- `slide_quality_report.json` generated: yes;
- `subtitle_page_map.json` generated: no, because there was no transcript/subtitle evidence for this candidate;
- public candidate deliverables validator passed;
- public manifests no longer contained `artifact://`, token-like query markers, or absolute local path patterns.

Quality matrix result:

- `matrix_complete=false`;
- `deliverable_count=0`;
- `needs_manual_review_count=1`;
- `not_deliverable_count=0`;
- `pending_accessible_sample_count=1`;
- `pending_authorization_count=1`;
- local candidate package state: `final_pptx_ready`;
- validator status: passed;
- PPTX slide count: 5;
- Markdown slide count: 5;
- review conclusion: `needs_manual_review`.

Quality risks:

- `quality_score=56`;
- risk flags: `full_frame_rectangle_fallback`, `missing_transcript_alignment`, `selected_slide_duplicates_removed`, `frame_sharpness_review_required`, `manual_review_required`;
- summary: 1 full-frame fallback, 4 detector crops, 5 missing transcript alignments, 5 missing OCR alignments, 2 visual duplicates removed, and 4 high sharpness/readability review risks.

Current conclusion:

- P2-2E-2A is complete: the public candidate now passes the deliverable contract.
- The public candidate is not marked as cleanly deliverable; its accepted quality conclusion is `needs_manual_review`.
- Full P2-2E remains incomplete because customer-authorized video is still pending, and the public sample still needs human quality review or a follow-up quality slice.

Safety result:

- no customer/private/login-gated video was used;
- no WeChat Video Channels page was fetched;
- no main-site upload, third-party event, live smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media, frames, generated PPTX, manifests, and quality matrix output stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full public candidate media path was recorded.

## 2026-06-08 Public Course Quality Matrix Category Input

Task source: P2-2E-2B from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- stop classifying the real public-course candidate through the synthetic deliverables input;
- add a quality-matrix input that maps local `generated_artifacts/` into the `public_course_video` case;
- keep customer-authorized video pending until explicit approval/input exists;
- avoid main-site writes, third-party events, browser recording, service builds, service restarts, and 8-server deployment.

Implemented behavior:

- `scripts/smoke/video-ppt-quality-matrix.mjs` accepts `--public-course-deliverables <path>`;
- environment fallback `VIDEO_PPT_QUALITY_MATRIX_PUBLIC_COURSE_DELIVERABLES` is supported;
- `--public-course-deliverables` validates the path through `validateVideoDeliverables(path)`;
- the report case id is `public-course-video-deliverables` and category is `public_course_video`;
- `public_course_sample_required=false` and `public_course_deliverables_reviewed=true` when the public deliverables input is used;
- `--self-test` remains mutually exclusive with deliverables inputs;
- only one deliverables input flag may be used per report.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --help
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir target/video-ppt-quality-matrix-smoke
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir target/video-ppt-public-candidate-quality-matrix
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables <public-candidate-generated_artifacts> --pretty --output-dir target/video-ppt-public-candidate-quality-matrix-synthetic-regression
npm run smoke:video-ppt-quality-matrix -- --self-test --public-course-deliverables <public-candidate-generated_artifacts>
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts>
```

Result:

- Node syntax check passed;
- help output includes `--public-course-deliverables`;
- self-test still passed with `deliverable=1`, `pending=2`;
- public-course deliverables report passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=1`, `not_deliverable_count=0`, `pending_count=1`;
- public-course report status: `partial_public_course_deliverables_reviewed`;
- public-course report input mode: `public_course_deliverables`;
- public case state: `final_pptx_ready`;
- public case validator status: passed;
- public case selected/PPTX/Markdown count: 5/5/5;
- public case review conclusion: `needs_manual_review`;
- customer-authorized case remains `pending_authorization`;
- negative mutual-exclusion check failed as expected when `--self-test` and `--public-course-deliverables` were used together;
- old `--synthetic-deliverables` path remained compatible.

Quality matrix summary:

- synthetic case: `deliverable`;
- public-course case: `needs_manual_review`;
- customer-authorized case: `pending_authorization`;
- `matrix_complete=false`;
- `live_smoke_run=false`;
- `production_write_allowed=false`;
- `generated_artifacts_committable=false`.

Current conclusion:

- P2-2E public-course category coverage is now stronger: the public candidate is no longer represented as a synthetic case or a pending sample.
- Full P2-2E remains incomplete because the customer-authorized sample is still pending and the public sample still needs manual quality review or follow-up quality improvement.

Safety result:

- no customer/private/login-gated video was used;
- no WeChat Video Channels page was fetched;
- no main-site upload, third-party event, live smoke, browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- public candidate media, frames, generated PPTX, manifests, and quality matrix output stayed under `target/` and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, or full public candidate media path was recorded.

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

## 2026-06-08 Contentful Dark-Theme Slide Guard

Task source: P2-2B/P2-2C local quality fixture follow-up from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- prove the dark/flat low-information guard does not reject stable dark-theme slides that contain visible courseware content;
- keep the existing black-screen, white-screen, gray/loading-screen rejection behavior intact;
- preserve selected-slide deliverable contracts and avoid live extraction, network fetches, uploads, browser recording, or deployment.

Implemented behavior:

- added `auto_selects_contentful_dark_theme_slides_without_low_information_rejection`;
- the fixture creates a stable dark-background slide segment with visible blue courseware content;
- auto-selection keeps the midpoint candidate in `selected_candidate_indices`;
- the contentful dark-theme cluster is recorded as `stable_ppt_page_segment`;
- `auto_selection.rejected_clusters` remains empty for that contentful dark-theme segment;
- selected slides manifest remains `selected_count=1` with the same selected candidate index.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker auto_selects_contentful_dark_theme_slides_without_low_information_rejection --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
npm run test:video-deliverables
git diff --check
```

Result:

- formatting and format check passed;
- new contentful dark-theme fixture passed;
- `auto_selects` passed, 5 tests, covering stable PPT pages, dark low-information rejection, bright low-information rejection, flat low-information rejection, and contentful dark-theme positive selection;
- selected-slides regression passed;
- controlled video sample deliverable contract passed;
- slide rectangle regression passed, 5 tests;
- video deliverable validator passed, 19 tests;
- whitespace check passed.

Remaining P2-2 work:

- broader real-sample or complex animation-build review still needs coverage beyond this local fixture;
- P2-2D still needs a real approved sample with usable subtitles/transcript/OCR to verify alignment quality beyond deterministic fixture coverage;
- P2-2E still needs public-course and customer-authorized real samples before the full three-sample quality matrix can be marked complete.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated temp frames stayed under test temp directories and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Speaker-Window Foreground Crop Fixture

Task source: P2-2B/P2-2C local quality fixture follow-up from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- prove a PPT frame with an external foreground obstruction can keep the slide rectangle instead of falling back to full-frame output;
- verify the crop signal travels through rectangle manifest, selected slides manifest, quality report, and PPTX XML;
- keep detector crops review-required and avoid claiming real-sample quality completion;
- avoid live extraction, network fetches, uploads, browser recording, or deployment.

Implemented behavior:

- added `writes_foreground_component_crop_for_speaker_window_obstruction`;
- the fixture uses an existing frame generator with a bright PPT canvas plus an external foreground strip representing a speaker-window/player overlay;
- `slide_rectangles_manifest` records `rectangle_extraction_mode=foreground_component_v1`, `rectangle_source=raw_frame_foreground_component`, and `promoted_rectangle_count=1`;
- `selected_slides_manifest` records the same foreground-component rectangle mode;
- `slide_quality_report.json` records `detector_crop_count=1`, `full_frame_fallback_count=0`, and per-slide `crop_risk=medium`;
- PPTX `slide1.xml` contains the expected DrawingML `a:srcRect` crop and redacted alt text `crop promoted_detector_crop/foreground_component_v1`;
- public artifacts do not include the raw frame directory path.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker writes_foreground_component_crop_for_speaker_window_obstruction --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
git diff --check
```

Result:

- formatting and format check passed;
- speaker-window/foreground-component end-to-end fixture passed;
- slide rectangle detector regression passed, 5 tests;
- selected-slides regression passed;
- controlled video sample deliverable contract passed;
- video deliverable validator passed, 19 tests;
- whitespace check passed.

Remaining P2-2 work:

- broader real-sample or complex animation-build review still needs coverage beyond this local fixture;
- P2-2D still needs a real approved sample with usable subtitles/transcript/OCR to verify alignment quality beyond deterministic fixture coverage;
- P2-2E still needs public-course and customer-authorized real samples before the full three-sample quality matrix can be marked complete.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated temp frames stayed under test temp directories and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Low-Contrast Text Quality Report Fixture

Task source: P2-2F/P2-2B local quality fixture follow-up from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- prove low-contrast slide text is surfaced as a quality-review risk in `slide_quality_report.json`;
- keep the existing screenshot-based PPTX deliverable path intact;
- keep the signal as a review warning, not a hard delivery failure;
- avoid live extraction, network fetches, uploads, browser recording, or deployment.

Implemented behavior:

- added a deterministic low-contrast text PNG fixture;
- added `flags_low_contrast_slide_text_in_quality_report`;
- the fixture runs through `write_video_extraction_text_artifacts`, selected slides, slide rectangles, PPTX generation, and quality report generation;
- `slide_quality_report.json` records `sharpness_status=measured`, `sharpness_risk=high`, and a score below the medium-risk threshold;
- report summary records `sharpness_high_count=1` and `sharpness_unknown_count=0`;
- risk flags include `frame_sharpness_review_required` with `high_count=1` and `unknown_count=0`;
- the public quality report does not include the raw frame directory path.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker flags_low_contrast_slide_text_in_quality_report --lib
CC=clang CXX=clang++ cargo test -p media-worker measures_slide_frame_sharpness_for_quality_report --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
npm run test:video-deliverables
git diff --check
```

Result:

- formatting and format check passed;
- low-contrast text end-to-end quality report fixture passed;
- sharpness helper regression passed;
- selected-slides regression passed;
- controlled video sample deliverable contract passed;
- slide rectangle regression passed, 5 tests;
- video deliverable validator passed, 19 tests;
- whitespace check passed.

Remaining P2-2 work:

- broader real-sample or complex animation-build review still needs coverage beyond this local fixture;
- P2-2D still needs a real approved sample with usable subtitles/transcript/OCR to verify alignment quality beyond deterministic fixture coverage;
- P2-2E still needs public-course and customer-authorized real samples before the full three-sample quality matrix can be marked complete.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated temp frames stayed under test temp directories and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Short Animation Transition Auto-Selection Fixture

Task source: P2-2C local quality fixture follow-up from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- prove short non-low-information animation transition frames between two stable PPT pages are not selected as final slide pages;
- keep stable PPT pages selected before and after the animation transition;
- keep the fixture local and deterministic, without claiming full real-sample animation-build quality completion;
- avoid live extraction, network fetches, uploads, browser recording, or deployment.

Implemented behavior:

- added `auto_selects_stable_slides_without_short_animation_transition_frames`;
- the fixture creates two stable PPT page segments with two visually distinct one-frame animation transition frames in between;
- auto-selection keeps only the stable midpoint candidates `[2, 7]`;
- the two animation frames are recorded in `auto_selection.rejected_clusters` with `reason=unstable_short_segment`;
- selected slides manifest keeps `selected_count=2` and `selected_candidate_indices=[2, 7]`.

Validation:

```text
cargo fmt
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker auto_selects_stable_slides_without_short_animation_transition_frames --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
npm run test:video-deliverables
git diff --check
```

Result:

- formatting and format check passed;
- short animation transition fixture passed;
- `auto_selects` passed, 6 tests, covering stable PPT pages, dark/bright/flat low-information rejection, contentful dark-theme positive selection, and short animation transition rejection;
- selected-slides regression passed;
- controlled video sample deliverable contract passed;
- slide rectangle regression passed, 5 tests;
- video deliverable validator passed, 19 tests;
- whitespace check passed.

Remaining P2-2 work:

- broader real-sample or complex animation-build review still needs coverage beyond this local fixture;
- P2-2D still needs a real approved sample with usable subtitles/transcript/OCR to verify alignment quality beyond deterministic fixture coverage;
- P2-2E still needs public-course and customer-authorized real samples before the full three-sample quality matrix can be marked complete.

Safety result:

- no live smoke was run in this slice;
- no video was downloaded, uploaded, fetched from WeChat Video Channels, captured, OCRed, or converted through a live workflow;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated temp frames stayed under test temp directories and were not committed;
- no cookie, token, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded.

## 2026-06-08 Main-Site Upload Smoke Self-Test Gate

Task source: EP2/P1-3B readiness from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- add an offline self-test gate for `smoke:video-ppt-upload-main`;
- let the main-site uploaded-video smoke contract be checked before the user approves a live main-site write;
- verify artifact/download contract shape, minimal PPTX/Markdown download validation, video PPT extraction scope, supported video extensions, and redaction gates without network access;
- avoid live upload, dataset creation, assistant-run creation, third-party events, browser capture, service build/restart, 8-server deployment, or 120-server action.

Implemented behavior:

- `scripts/smoke/video-ppt-upload-main.mjs` now accepts `--self-test`;
- `VIDEO_PPT_UPLOAD_MAIN_SMOKE_SELF_TEST=true` is supported as the environment fallback;
- self-test builds a deterministic assistant-run-bound `video_extraction_summary` fixture with `deliverable_state=final_pptx_ready`;
- self-test verifies required file kinds: `pptx`, `video_slides_markdown`, `final_deliverables_manifest`, `published_deliverable_manifest`, `published_version_history`, and `extraction_artifacts_manifest`;
- self-test verifies selected scope and scope candidate keep `intent=video_ppt_extraction`;
- self-test verifies `.mp4`, `.mov`, `.m4v`, `.webm`, `.mkv`, and `.avi` are classified as video uploads;
- self-test writes a minimal local PPTX ZIP fixture and Markdown deck under `target/`, then runs the same `validateDownloads` path used by live upload smoke;
- self-test verifies the minimal PPTX central directory contains `[Content_Types].xml`, `ppt/presentation.xml`, and `ppt/slides/slide1.xml`;
- self-test verifies PPTX slide count and Markdown `### Slide` heading count are both `1`;
- self-test report records `networkCallsRun=false`, `productionWriteAllowed=false`, `fixtureDownloaded=false`, `uploadAttempted=false`, and `assistantRunCreated=false`;
- `scripts/README.md` now documents the upload self-test and clarifies that live mode writes a controlled smoke upload/document/run record.

Validation:

```text
node --check scripts/smoke/video-ppt-upload-main.mjs
npm run smoke:video-ppt-upload-main -- --help
npm run smoke:video-ppt-upload-main -- --self-test --output-dir target/video-ppt-upload-main-self-test-current
npm run smoke:external-video-ppt -- --self-test --output-dir target/external-video-ppt-self-test-current
git diff --check
```

Result:

- upload smoke syntax check passed;
- help output includes `--self-test` and states that self-test does not call the network, upload files, create datasets, or create assistant runs;
- upload smoke self-test passed with `ok=true`;
- self-test report summary showed `networkCallsRun=false`, `productionWriteAllowed=false`, `fixtureDownloaded=false`, `uploadAttempted=false`, and `assistantRunCreated=false`;
- self-test contract showed `deliverableState=final_pptx_ready` and all six required deliverable kinds;
- self-test download validation passed with `pptxSlideCount=1`, `markdownSlideHeadingCount=1`, and all three required PPTX entries present;
- self-test contract showed supported extensions `.mp4`, `.mov`, `.m4v`, `.webm`, `.mkv`, and `.avi`;
- redaction audit found no raw URL, token marker, bearer marker, or snake-case upload object key in the self-test report;
- external video PPT self-test still passed as a control check;
- whitespace check passed.

Remaining work:

- this improves P1-3B readiness but does not run the live main-site upload smoke;
- P1-3B still needs explicit user approval because live mode writes a non-customer smoke upload/document/run record;
- P1-3C still needs inbound bearer, `connection_id`, `source_id`, and authorized input;
- P1-3D still needs an approved 8-server deployment window before live handoff pass;
- P2-1C and P2-2E customer gates still need operator/customer authorization.

Safety result:

- no live main-site upload smoke was run;
- no fixture video was downloaded;
- no dataset, document, upload object, assistant run, or HTML artifact was created;
- no third-party event was sent;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated self-test reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded in this shared receipt.

## 2026-06-08 Third-Party Video PPT Smoke Self-Test Download Gate

Task source: EP3/P1-3C readiness from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- strengthen `smoke:external-video-ppt -- --self-test` so it validates both the third-party reply surface and the minimal deliverable download contract;
- keep live third-party execution gated on an approved inbound bearer, `connection_id`, `source_id`, and safe input;
- avoid network calls, fixture registration, `/events` delivery, DataMax uploads, browser capture, service build/restart, 8-server deployment, and 120-server action.

Implemented behavior:

- `scripts/smoke/external-video-ppt.mjs` self-test now writes a minimal local PPTX ZIP fixture and Markdown deck under `target/`;
- self-test runs the same `validateDownloads` path used by live third-party deliverable download checks;
- self-test verifies required export kinds: `pptx`, `video_slides_markdown`, `final_deliverables_manifest`, `published_deliverable_manifest`, `published_version_history`, and `extraction_artifacts_manifest`;
- self-test verifies the minimal PPTX central directory contains `[Content_Types].xml`, `ppt/presentation.xml`, and `ppt/slides/slide1.xml`;
- self-test verifies PPTX slide count and Markdown `### Slide` heading count are both `1`;
- self-test report records `networkCallsRun=false`, `fixtureRegistered=false`, `eventSent=false`, and `deliverablesDownloadedFromNetwork=false`;
- `scripts/README.md` now documents that external self-test covers reply surface plus minimal PPTX/Markdown validation.

Validation:

```text
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:external-video-ppt -- --self-test --output-dir target/external-video-ppt-self-test-download-validation
git diff --check
```

Result:

- external video PPT smoke syntax check passed;
- external video PPT self-test passed with `ok=true`;
- self-test reply surface showed `status=video_extraction_summary`, `cardType=video_extraction_summary`, `exportCount=6`, and no missing export kinds;
- self-test summary showed `networkCallsRun=false`, `fixtureRegistered=false`, `eventSent=false`, and `deliverablesDownloadedFromNetwork=false`;
- self-test download validation passed with `pptxSlideCount=1`, `markdownSlideHeadingCount=1`, and all three required PPTX entries present;
- redaction audit found no raw URL, token marker, bearer marker, or snake-case upload object key in the self-test report;
- whitespace check passed.

Remaining work:

- this improves P1-3C readiness but does not run the live third-party registration smoke;
- P1-3C still needs approved inbound bearer, `connection_id`, `source_id`, and an authorized public/uploaded video input;
- P1-3B still needs explicit user approval for a live main-site upload smoke record;
- P1-3D still needs an approved 8-server deployment window before live handoff pass;
- P2-1C and P2-2E customer gates still need operator/customer authorization.

Safety result:

- no live third-party smoke was run;
- no fixture video was downloaded or registered;
- no `/events` request was sent;
- no DataMax upload, dataset, document, assistant run, or HTML artifact was created;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated self-test reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded in this shared receipt.

## 2026-06-08 Video Channel Handoff Negative Self-Test Gate

Task source: P1-3D readiness from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- strengthen `smoke:video-ppt-handoff -- --self-test` so the login-gated/WeChat Video Channels handoff surface has explicit negative proof;
- keep the behavior as unsupported-source handoff, not video fetch, frame extraction, OCR, PPT generation, or browser capture;
- reject false-positive surfaces that look like successful video PPT extraction or expose downloadable artifacts;
- avoid network calls, provider/ReAct calls, DataMax uploads, fixture downloads, service build/restart, 8-server deployment, and 120-server action.

Implemented behavior:

- `assertHandoffSurface` now treats exposed artifact links as an unsafe handoff signal, same as `download_exports` and `final_pptx_ready`;
- self-test report records `networkCallsRun=false`, `providerCalled=false`, `reactToolchainCalled=false`, `videoFetchAttempted=false`, `videoDownloaded=false`, `framesExtracted=false`, `ocrRun=false`, and `pptGenerated=false`;
- self-test report records that no `final_pptx_ready`, artifact links, or download exports were exposed on the accepted main/external handoff fixtures;
- self-test adds five negative fixtures and requires all five to be rejected:
  - missing `login_gated_video_source_not_supported` reason;
  - false success/download surface;
  - artifact-link exposure;
  - raw WeChat source URL leak;
  - credential/cookie/login-state request.

Validation:

```text
node --check scripts/smoke/video-ppt-handoff.mjs
npm run smoke:video-ppt-handoff -- --help
npm run smoke:video-ppt-handoff -- --self-test --output-dir target/video-ppt-handoff-self-test-negative-gate-final
rg -n "weixin\.qq\.com/sph|channels\.weixin\.qq\.com/sph|token=|Bearer |cookie|password|upload_object_key|generated_artifacts|/Users/" target/video-ppt-handoff-self-test-negative-gate-final || true
```

Result:

- handoff smoke syntax check passed;
- help output still documents that the smoke does not fetch the WeChat URL, download video, extract frames, OCR, or generate PPT;
- handoff self-test passed with `ok=true`;
- self-test summary showed `networkCallsRun=false`, `providerCalled=false`, `reactToolchainCalled=false`, `videoFetchAttempted=false`, `videoDownloaded=false`, `framesExtracted=false`, `ocrRun=false`, and `pptGenerated=false`;
- accepted main/external handoff fixtures showed `artifactLinkCount=0`, `downloadExportCount=0`, `unsafeSuccessSignal=false`, and `unsafeArtifactLinkSignal=false`;
- negative fixture gate showed `negativeFixtureCount=5` and `negativeFixturesRejected=5`;
- redaction audit found no raw WeChat source URL, token marker, bearer marker, cookie/password marker, upload object key, generated-artifacts path, or local absolute path in the self-test report.

Remaining work:

- this improves P1-3D offline readiness but does not prove the current production main-site handoff surface;
- P1-3D live pass still needs an approved 8-server deployment window before rerunning the main-site handoff smoke;
- third-party live handoff still needs approved inbound bearer/context;
- no video was extracted from a WeChat/login-gated source in this slice, and the expected product behavior remains handoff only.

Safety result:

- no live handoff smoke was run;
- no source page was fetched;
- no video was downloaded, uploaded, captured, OCRed, frame-extracted, or converted to PPT;
- no provider/ReAct/model branch was called;
- no DataMax upload, dataset, document, assistant run, third-party event, or HTML artifact was created;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated self-test reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, or generated artifact local path was recorded in this shared receipt.

## 2026-06-08 Authorized Capture Helper Self-Test Gate

Task source: P2-1C readiness from `docs/plans/datamax-active-execution-plan.md`.

Scope:

- strengthen `capture:authorized-video -- --self-test` before any operator-approved live capture is attempted;
- verify authorization and policy failures locally without opening a browser, running FFmpeg, downloading video, uploading MP4, or touching production services;
- add a redacted `sharedReceipt` object that can be copied into validation logs without carrying local paths, raw source URLs, approval values, credentials, or provider payloads.

Implemented behavior:

- self-test now exercises six negative authorization/policy fixtures:
  - missing `--ack-authorized`;
  - missing `--approval-id`;
  - invalid non-http/non-https source URL;
  - duration greater than the hard 300-second limit;
  - live capture without `--browser-bin`;
  - `--capture-audio` requested outside live capture mode;
- local reports now include `sharedReceipt` with approval and operator references represented by presence flags and hash prefixes, not raw values;
- `sharedReceipt` includes only source host/path summary, duration, retention, capture mode, file name, handoff mode, cleanup policy, and explicit redaction flags;
- live reports add file size and hash prefix to `sharedReceipt.captureFile` only after an MP4 exists.

Validation:

```text
node --check scripts/capture-authorized-video.mjs
npm run capture:authorized-video -- --self-test --output-dir target/authorized-capture-self-test-gates-final
node <sharedReceipt redaction scan>
git diff --check
```

Result:

- capture helper syntax check passed;
- capture helper self-test passed with `ok=true`;
- self-test summary showed `authorizationGateNegativeCaseCount=6` and `authorizationGateNegativeCasesRejected=6`;
- rejected negative cases covered missing authorization ack, missing approval id, invalid URL scheme, overlong duration, live capture missing browser binary, and dry-run audio request;
- `sharedReceipt.redactionFlags` showed `rawSourceUrlIncluded=false`, `localPathsIncluded=false`, `credentialsIncluded=false`, `providerPayloadsIncluded=false`, and `approvalValuesIncluded=false`;
- redaction scan over `sharedReceipt` found no raw URL, local absolute path, cookie/token/password marker, or raw self-test approval/operator value.

Remaining work:

- this improves P2-1C offline readiness but does not run live capture;
- P2-1C still needs a real operator approval record, playable authorized source, maximum duration, audio policy, retention policy, and handoff target;
- any captured MP4 still must be reviewed manually and then fed into the normal main-site upload or third-party video registration path.

Safety result:

- no browser was opened;
- no FFmpeg command was run;
- no video page was fetched;
- no MP4 was captured, downloaded, uploaded, OCRed, frame-extracted, or converted to PPT;
- no DataMax upload, dataset, document, assistant run, third-party event, or HTML artifact was created;
- no browser capture, service build, service restart, 8-server deployment, or 120-server action was run;
- generated self-test reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, raw approval id, or raw approved-by value was recorded in the shared receipt.

## 2026-06-08 No-Auth Video PPT Local Acceptance Rollup

Task source: after the P1-3B/P1-3C/P1-3D/P2-1C/P2-2E offline gates were strengthened, run a single no-authorization local acceptance sweep before asking for live approvals.

Scope:

- run only self-tests and local unit validation that do not require live credentials, uploads, browser capture, deployment, or customer inputs;
- prove the current local contracts still compose after the upload, third-party, handoff, capture, and quality-matrix hardening slices;
- keep generated reports under `target/` and copy only redacted summaries into this ledger.

Validation:

```text
npm run smoke:video-ppt-upload-main -- --self-test --output-dir target/video-ppt-upload-main-self-test-rollup-final
npm run smoke:external-video-ppt -- --self-test --output-dir target/external-video-ppt-self-test-rollup-final
npm run smoke:video-ppt-handoff -- --self-test --output-dir target/video-ppt-handoff-self-test-rollup-final
npm run capture:authorized-video -- --self-test --output-dir target/authorized-capture-self-test-rollup-final
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir target/video-ppt-quality-matrix-rollup-final
npm run test:video-deliverables
node <rollup shared-summary redaction scan>
```

Result:

- main-site upload self-test passed with `ok=true`, `networkCallsRun=false`, `productionWriteAllowed=false`, `uploadAttempted=false`, and `assistantRunCreated=false`;
- main-site upload self-test download validation passed with minimal PPTX/Markdown counts of `1` slide each and required PPTX entries present;
- third-party video PPT self-test passed with `ok=true`, `networkCallsRun=false`, `fixtureRegistered=false`, `eventSent=false`, and `deliverablesDownloadedFromNetwork=false`;
- third-party self-test download validation passed with `pptxSlideCount=1` and `markdownSlideHeadingCount=1`;
- video-channel/login-gated handoff self-test passed with `ok=true`, `networkCallsRun=false`, `providerCalled=false`, `videoFetchAttempted=false`, `pptGenerated=false`, `negativeFixtureCount=5`, and `negativeFixturesRejected=5`;
- authorized capture helper self-test passed with `authorizationGateNegativeCaseCount=6` and `authorizationGateNegativeCasesRejected=6`; copied evidence used only `sharedReceipt`;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- video deliverables validator passed all 19 node test cases;
- rollup shared-summary redaction scan passed with no raw WeChat source URL, token marker, bearer marker, password marker, upload object key, raw self-test approval/operator value, or local absolute path.

Remaining work:

- this rollup does not run live main-site upload, live third-party registration, deployed handoff smoke, live authorized capture, or real customer-authorized quality review;
- P1-3B still needs explicit approval to write a non-customer main-site smoke upload/document/run record;
- P1-3C still needs approved inbound bearer, `connection_id`, `source_id`, and safe video input;
- P1-3D still needs an approved 8-server deployment window before live handoff smoke;
- P2-1C/P2-2E still need operator/customer authorization and source inputs.

Safety result:

- no live smoke was run;
- no network source was fetched by these self-tests;
- no file was uploaded or registered;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no DataMax upload, dataset, document, assistant run, third-party event, or HTML artifact was created;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated rollup reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, raw approval id, or raw approved-by value was recorded in this shared rollup receipt.

## 2026-06-08 Main-Site Upload Smoke Preflight Gate

Task source: P1-3B live upload smoke still needs approval to write a non-customer main-site smoke record; add a no-network preflight so the approved run has a checked fixture/scope before live execution.

Scope:

- add `smoke:video-ppt-upload-main -- --preflight`;
- validate fixture/source shape, supported video extension, inferred video media kind, video-PPT trigger wording, live write scope, and report redaction;
- avoid fixture download, file upload, dataset creation, document registration, ingest enqueue, assistant-run creation, artifact polling, and artifact downloads.

Implemented behavior:

- `--preflight` accepts the same fixture URL/file arguments as live mode but does not download URL fixtures or upload local files;
- local `--fixture-file` preflight uses `stat` only and stores basename/size with `localPathRedacted=true`;
- URL fixture preflight stores only scheme/host/extension, not the raw URL, path, or query;
- preflight requires one of the supported video extensions `.mp4`, `.mov`, `.m4v`, `.webm`, `.mkv`, or `.avi`;
- preflight no longer treats an unknown extension as `video/mp4` just because the live fallback content type defaults to MP4;
- preflight report records `liveWriteApprovalRequired=true` and the planned live write steps;
- preflight emits a redacted command template without raw base URL, raw fixture URL, local path, token, object key, or bearer value.

Validation:

```text
node --check scripts/smoke/video-ppt-upload-main.mjs
npm run smoke:video-ppt-upload-main -- --help
npm run smoke:video-ppt-upload-main -- --preflight --output-dir target/video-ppt-upload-main-preflight-final
npm run smoke:video-ppt-upload-main -- --preflight --fixture-url https://example.com/not-video.txt --output-dir target/video-ppt-upload-main-preflight-negative-final
npm run smoke:video-ppt-upload-main -- --self-test --output-dir target/video-ppt-upload-main-self-test-preflight-final
rg -n "https?://|token=|object_key|Bearer |/Users/|v3\.elepcloud\.com/generated-artifacts" target/video-ppt-upload-main-preflight-final target/video-ppt-upload-main-preflight-negative-final || true
```

Result:

- upload smoke syntax check passed;
- help output documents `--preflight`;
- positive preflight passed with `ok=true`, `networkCallsRun=false`, `productionWriteAllowed=false`, `fixtureDownloaded=false`, `uploadAttempted=false`, `datasetCreated=false`, `documentRegistered=false`, and `assistantRunCreated=false`;
- positive preflight classified the default `.mp4` fixture as `mediaKind=video` with `supportedExtension=true`;
- negative `.txt` fixture preflight failed as expected with `ok=false`, `mediaKind=""`, and `supportedExtension=false`;
- upload self-test still passed after the preflight change;
- preflight redaction scan found no raw URL, token marker, object key marker, bearer marker, local absolute path, or raw sample URL.

Remaining work:

- this improves P1-3B readiness but does not run the live main-site upload smoke;
- P1-3B still needs explicit user approval because live mode writes a non-customer smoke upload/document/run record;
- after approval, run preflight with the exact approved fixture first, then run the live smoke with the same fixture.

Safety result:

- no live upload smoke was run;
- no fixture video was downloaded;
- no file was uploaded or registered;
- no dataset, document, ingest job, assistant run, third-party event, or HTML artifact was created;
- no browser capture, FFmpeg command, service build, service restart, 8-server deployment, or 120-server action was run;
- generated preflight/self-test reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, raw approval id, or raw approved-by value was recorded in this shared receipt.

## 2026-06-08 Third-Party Video PPT Smoke Preflight Gate

Task source: P1-3C live third-party smoke still needs approved inbound bearer/context; add a no-network preflight so the approved run has checked third-party context, fixture/scope, and trigger payload before live execution.

Scope:

- add `smoke:external-video-ppt -- --preflight`;
- validate connection/source presence, bearer gate, fixture/source shape, supported video extension, inferred video media kind, scoped video-PPT trigger payload, live write scope, and report redaction;
- avoid fixture download, loopback fixture server, `/documents/parse`, parse-detail polling, `/events`, reply polling, and deliverable downloads.

Implemented behavior:

- `--preflight` accepts the same connection/source/fixture arguments as live mode but does not call the network;
- preflight no longer hard-fails before reporting when bearer is absent; it reports `missing_bearer_for_live_external_smoke` unless a real bearer is supplied or `--allow-missing-bearer` is used for local shape checks;
- `--allow-missing-bearer` lets local no-auth preflight validate fixture/context/trigger shape while still reporting `liveCredentialReady=false`;
- local `--fixture-file` preflight uses `stat` only and stores basename/size with `localPathRedacted=true`;
- URL fixture preflight stores only scheme/host/extension, not the raw URL, path, or query;
- preflight requires one of the supported video extensions `.mp4`, `.mov`, `.m4v`, `.webm`, `.mkv`, or `.avi`;
- preflight validates that the event text asks to extract PPT/slides/courseware from the video and that `requested_skills` contains `video_ppt_extraction` with `expected_action=extract_video_ppt_transcript`;
- preflight emits a redacted command template without raw base URL, raw fixture URL, local path, token, object key, or bearer value.

Validation:

```text
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:external-video-ppt -- --help
npm run smoke:external-video-ppt -- --preflight --output-dir target/external-video-ppt-preflight-missing-bearer-final
npm run smoke:external-video-ppt -- --preflight --allow-missing-bearer --output-dir target/external-video-ppt-preflight-final
npm run smoke:external-video-ppt -- --preflight --allow-missing-bearer --fixture-url https://example.com/not-video.txt --output-dir target/external-video-ppt-preflight-negative-final
npm run smoke:external-video-ppt -- --self-test --output-dir target/external-video-ppt-self-test-preflight-final
rg -n "https?://|token=|object_key|Bearer |/Users/|v3\.elepcloud\.com/generated-artifacts" target/external-video-ppt-preflight-final target/external-video-ppt-preflight-missing-bearer-final target/external-video-ppt-preflight-negative-final || true
```

Result:

- external video PPT smoke syntax check passed;
- help output documents `--preflight --allow-missing-bearer`;
- missing-bearer preflight failed as expected with `ok=false`, `credentialGateSatisfied=false`, and `failures=["missing_bearer_for_live_external_smoke"]`;
- local shape preflight with `--allow-missing-bearer` passed with `ok=true`, `networkCallsRun=false`, `fixtureRegistered=false`, `eventSent=false`, `replyPolled=false`, and `deliverablesDownloadedFromNetwork=false`;
- local shape preflight reported `liveCredentialReady=false`, making clear it is not live-ready without an inbound bearer;
- positive preflight classified the default `.mp4` fixture as `mediaKind=video` with `supportedExtension=true`;
- positive preflight validated `requestedSkillIds=["video_ppt_extraction"]` and `expectedAction=extract_video_ppt_transcript`;
- negative `.txt` fixture preflight failed as expected with `ok=false`, `mediaKind=""`, `supportedExtension=false`, and failures for non-video classification plus unsupported extension;
- external video PPT self-test still passed after the preflight change;
- preflight redaction scan found no raw URL, token marker, object key marker, bearer marker, local absolute path, or raw sample URL.

Remaining work:

- this improves P1-3C readiness but does not run the live third-party registration/event smoke;
- P1-3C still needs approved inbound bearer, `connection_id`, `source_id`, and an authorized public/uploaded video input;
- after approval, run preflight with the exact approved context/fixture first, then run the live smoke with the same context/fixture.

Safety result:

- no live third-party smoke was run;
- no fixture video was downloaded;
- no loopback fixture server was opened;
- no `/documents/parse`, parse-detail poll, `/events`, reply poll, or deliverable download was performed;
- no DataMax upload, dataset, document, ingest job, assistant run, third-party event, or HTML artifact was created;
- no browser capture, FFmpeg command, service build, service restart, 8-server deployment, or 120-server action was run;
- generated preflight/self-test reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, raw approval id, or raw approved-by value was recorded in this shared receipt.

## 2026-06-08 Video Channel Handoff Live Preflight Gate

Task source: P1-3D live handoff pass still needs an approved 8-server deployment window and, for external mode, inbound bearer/context; add a no-network preflight before any live handoff smoke is attempted.

Scope:

- add `smoke:video-ppt-handoff -- --preflight`;
- validate main/external target mode shape, login-gated video prompt summary, external bearer/context gate, deployment approval requirement, expected handoff surface, live write scope, and report redaction;
- avoid source-page fetch, video download, frame extraction, OCR, PPT generation, provider calls, assistant-run creation, `/events`, reply polling, and artifact polling.

Implemented behavior:

- `--preflight` defaults to `mode=both` unless `--mode main|external|both` is supplied;
- missing external bearer now produces a preflight report with `ok=false` and `missing_bearer_for_external_handoff_smoke` instead of hard-failing before evidence is written;
- `--allow-missing-bearer` lets local no-auth preflight validate shape while still reporting `liveCredentialReady=false`;
- preflight records `deploymentApprovalRequired=true`, because the live main/external handoff pass still requires current code deployed to 8 server;
- preflight records no source fetch, no video download, no frames/OCR/PPT, and no provider call;
- preflight expected surface is fixed to `login_gated_video_source_not_supported` plus the three next steps: upload video file, provide anonymous direct video URL, and request authorized capture;
- preflight rejects success/download expectations by recording `final_pptx_ready`, `video_extraction_summary`, `download_exports`, and `artifact_links` as disallowed signals;
- preflight emits a redacted command template without raw base URL, raw source URL, token, object key, bearer value, or local path.

Validation:

```text
node --check scripts/smoke/video-ppt-handoff.mjs
npm run smoke:video-ppt-handoff -- --help
npm run smoke:video-ppt-handoff -- --preflight --output-dir target/video-ppt-handoff-preflight-missing-bearer-final
npm run smoke:video-ppt-handoff -- --preflight --allow-missing-bearer --output-dir target/video-ppt-handoff-preflight-final
npm run smoke:video-ppt-handoff -- --self-test --output-dir target/video-ppt-handoff-self-test-preflight-final
rg -n "weixin\.qq\.com/sph|channels\.weixin\.qq\.com/sph|https?://|token=|object_key|Bearer |/Users/|cookie=|password=" target/video-ppt-handoff-preflight-final target/video-ppt-handoff-preflight-missing-bearer-final || true
```

Result:

- handoff smoke syntax check passed;
- help output documents `--preflight --allow-missing-bearer`;
- missing-bearer preflight failed as expected with `ok=false`, `credentialGateSatisfied=false`, and `failures=["missing_bearer_for_external_handoff_smoke"]`;
- local shape preflight with `--allow-missing-bearer` passed with `ok=true`, `targetModes=["main","external"]`, `networkCallsRun=false`, `sourcePageFetched=false`, `videoDownloaded=false`, `framesExtracted=false`, `ocrRun=false`, `pptGenerated=false`, and `providerCalled=false`;
- local shape preflight reported `liveCredentialReady=false` and `deploymentApprovalRequired=true`, making clear it is not live-ready without bearer/deployment approval;
- expected handoff surface showed `failureReason=login_gated_video_source_not_supported` and required next steps `upload_video_file`, `provide_direct_video_url`, and `request_authorized_capture`;
- handoff self-test still passed after the preflight change;
- preflight redaction scan found no raw WeChat source URL, raw URL, token marker, object key marker, bearer marker, local absolute path, cookie value marker, or password value marker.

Remaining work:

- this improves P1-3D readiness but does not run live main/external handoff smoke;
- P1-3D main live still needs an approved 8-server deployment window;
- P1-3D external live still needs approved inbound bearer/context in addition to deployment;
- after approval, run preflight with the exact approved mode/context first, then run the live handoff smoke.

Safety result:

- no live handoff smoke was run;
- no WeChat/login-gated source page was fetched;
- no video was downloaded, uploaded, captured, OCRed, frame-extracted, or converted to PPT;
- no provider/ReAct/model branch was called;
- no assistant run, third-party event, reply poll, artifact poll, or HTML artifact was created;
- no browser capture, FFmpeg command, service build, service restart, 8-server deployment, or 120-server action was run;
- generated preflight/self-test reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, raw approval id, or raw approved-by value was recorded in this shared receipt.

## 2026-06-08 No-Auth Live-Preflight Acceptance Rollup

Task source: after adding preflight gates for P1-3B main-site upload, P1-3C third-party video registration, and P1-3D video-channel/login-gated handoff, rerun the no-authorization local acceptance sweep with all live preflight gates included.

Scope:

- run only no-network preflights, offline self-tests, and local unit validation;
- verify the three live-gate preflights compose with the existing upload, third-party, handoff, capture, quality-matrix, and deliverables-validator evidence;
- keep generated reports under `target/` and copy only redacted summaries into this ledger.

Validation:

```text
npm run smoke:video-ppt-upload-main -- --preflight --output-dir target/video-ppt-upload-main-preflight-rollup-2
npm run smoke:video-ppt-upload-main -- --self-test --output-dir target/video-ppt-upload-main-self-test-rollup-2
npm run smoke:external-video-ppt -- --preflight --allow-missing-bearer --output-dir target/external-video-ppt-preflight-rollup-2
npm run smoke:external-video-ppt -- --self-test --output-dir target/external-video-ppt-self-test-rollup-2
npm run smoke:video-ppt-handoff -- --preflight --allow-missing-bearer --output-dir target/video-ppt-handoff-preflight-rollup-2
npm run smoke:video-ppt-handoff -- --self-test --output-dir target/video-ppt-handoff-self-test-rollup-2
npm run capture:authorized-video -- --self-test --output-dir target/authorized-capture-self-test-rollup-2
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir target/video-ppt-quality-matrix-rollup-2
npm run test:video-deliverables
node <rollup shared-summary redaction scan>
```

Result:

- main-site upload preflight passed with `ok=true`, `fixtureDownloaded=false`, `uploadAttempted=false`, `assistantRunCreated=false`, `mediaKind=video`, and `supportedExtension=true`;
- main-site upload self-test passed with `ok=true`; minimal PPTX/Markdown download validation passed with one slide each;
- third-party video PPT preflight passed with `ok=true`, `fixtureRegistered=false`, `eventSent=false`, `liveCredentialReady=false`, and `expectedAction=extract_video_ppt_transcript`;
- third-party video PPT self-test passed with `ok=true`; minimal PPTX/Markdown download validation passed with one slide each;
- video-channel/login-gated handoff preflight passed with `ok=true`, `liveCredentialReady=false`, `deploymentApprovalRequired=true`, `providerCalled=false`, `pptGenerated=false`, and `failureReason=login_gated_video_source_not_supported`;
- video-channel/login-gated handoff self-test passed with `negativeFixtureCount=5` and `negativeFixturesRejected=5`;
- authorized capture helper self-test passed with `authorizationGateNegativeCaseCount=6` and `authorizationGateNegativeCasesRejected=6`; copied evidence used only `sharedReceipt`;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- video deliverables validator passed all 19 node test cases;
- rollup shared-summary redaction scan passed with no raw WeChat source URL, raw URL, token marker, object key marker, bearer marker, cookie/password value marker, raw self-test approval/operator value, or local absolute path.

Remaining work:

- this rollup does not run live main-site upload, live third-party registration/event, deployed handoff smoke, live authorized capture, or real customer-authorized quality review;
- P1-3B still needs explicit approval to write a non-customer main-site smoke upload/document/run record;
- P1-3C still needs approved inbound bearer, `connection_id`, `source_id`, and safe video input;
- P1-3D still needs an approved 8-server deployment window before live handoff smoke, and external mode still needs bearer/context;
- P2-1C/P2-2E still need operator/customer authorization and source inputs.

Safety result:

- no live smoke was run;
- no network source was fetched by these preflights/self-tests;
- no file was uploaded or registered;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no DataMax upload, dataset, document, assistant run, third-party event, reply poll, artifact poll, or HTML artifact was created;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated rollup reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, raw approval id, or raw approved-by value was recorded in this shared rollup receipt.

## 2026-06-08 Executable Plan Completion Audit

Task source: plan-only follow-up to finish the consolidated executable plan after the upload, third-party, handoff, capture, and quality-matrix preflight gates were added.

Scope:

- update only the active plan and synchronized desktop copy;
- keep `docs/plans/datamax-active-execution-plan.md` as the single active plan under `docs/plans/`;
- add exact preflight-before-live sequencing for P1-3B main-site upload, P1-3C third-party video registration, and P1-3D login-gated handoff;
- add a current-head audit table that separates proven no-auth readiness from pending live/customer/deployment gates.

Plan result:

- EP2 now requires a same-fixture no-network preflight before any live main-site upload controlled smoke;
- EP3 now separates missing-bearer shape checks from live-ready bearer/context/input preflight and live execution;
- EP4 now requires same-mode/context preflight before main or external handoff live pass;
- the verification matrix and milestone table now include preflight as required evidence before live smoke;
- the current-head audit records that no-auth preflight/self-test/validator gates are passed, while P1-3B, P1-3C, P1-3D, P2-1C, and P2-2E customer gates remain pending authorization or deployment approval.

Remaining work:

- P1-3B still needs explicit approval to write a non-customer main-site smoke upload/document/run record;
- P1-3C still needs approved inbound bearer, `connection_id`, `source_id`, and safe video input;
- P1-3D still needs an approved 8-server deployment window before live handoff smoke, and external mode also needs bearer/context;
- P2-1C still needs operator approval record and playable source before any live capture;
- P2-2E still needs a customer/operator-authorized sample before the full three-category quality matrix can be closed.

Safety result:

- no business code, smoke runner, validator, worker, API, UI, or deployment script was changed in this plan-only pass;
- no live smoke, upload, third-party event, browser capture, FFmpeg command, service build, service restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Current-Head No-Auth Acceptance Refresh

Task source: continue toward full video PPT extraction completion without live-upload approval, third-party bearer/context, deployment window, or customer/operator sample authorization.

Scope:

- refresh the no-authorization acceptance evidence on current head `f2b2a1b`;
- run only no-network preflights, offline self-tests, authorized-capture dry self-test, quality-matrix self-test, and the deliverables validator;
- keep generated reports under `target/` and copy only redacted, high-level evidence into this ledger.

Validation:

```text
npm run smoke:video-ppt-upload-main -- --preflight --output-dir target/video-ppt-upload-main-preflight-current-head
npm run smoke:video-ppt-upload-main -- --self-test --output-dir target/video-ppt-upload-main-self-test-current-head
npm run smoke:external-video-ppt -- --preflight --allow-missing-bearer --output-dir target/external-video-ppt-preflight-current-head
npm run smoke:external-video-ppt -- --self-test --output-dir target/external-video-ppt-self-test-current-head
npm run smoke:video-ppt-handoff -- --preflight --allow-missing-bearer --output-dir target/video-ppt-handoff-preflight-current-head
npm run smoke:video-ppt-handoff -- --self-test --output-dir target/video-ppt-handoff-self-test-current-head
npm run capture:authorized-video -- --self-test --output-dir target/authorized-capture-self-test-current-head
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir target/video-ppt-quality-matrix-current-head
npm run test:video-deliverables
```

Result:

- main-site upload preflight passed with `ok=true`, `networkCallsRun=false`, `productionWriteAllowed=false`, `fixtureDownloaded=false`, `uploadAttempted=false`, `assistantRunCreated=false`, `mediaKind=video`, `supportedExtension=true`, and `liveWriteApprovalRequired=true`;
- main-site upload self-test passed with `ok=true` and the offline minimal PPTX/Markdown download contract intact;
- third-party video PPT preflight passed with `ok=true`, `networkCallsRun=false`, `fixtureRegistered=false`, `eventSent=false`, `replyPolled=false`, `deliverablesDownloadedFromNetwork=false`, `liveCredentialReady=false`, `mediaKind=video`, `supportedExtension=true`, and `expectedAction=extract_video_ppt_transcript`;
- third-party video PPT self-test passed with `ok=true` and the offline minimal PPTX/Markdown reply/download contract intact;
- video-channel/login-gated handoff preflight passed with `ok=true`, `targetModes=["main","external"]`, `sourcePageFetched=false`, `videoDownloaded=false`, `framesExtracted=false`, `ocrRun=false`, `pptGenerated=false`, `providerCalled=false`, `deploymentApprovalRequired=true`, `liveCredentialReady=false`, and `failureReason=login_gated_video_source_not_supported`;
- video-channel/login-gated handoff self-test passed with `ok=true`;
- authorized capture helper self-test passed with `ok=true`, `authorizationGateNegativeCaseCount=6`, and `authorizationGateNegativeCasesRejected=6`; shared evidence uses the redacted `sharedReceipt` contract only;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- video deliverables validator passed all 19 Node test cases.

Remaining work:

- this refresh does not run live main-site upload, live third-party registration/event, deployed handoff smoke, live authorized capture, or real customer-authorized quality review;
- P1-3B still needs explicit approval to write a non-customer main-site smoke upload/document/run record;
- P1-3C still needs approved inbound bearer, `connection_id`, `source_id`, and safe video input;
- P1-3D still needs an approved 8-server deployment window before live handoff smoke, and external mode also needs bearer/context;
- P2-1C and P2-2E still need operator/customer authorization and source inputs.

Safety result:

- no live smoke was run;
- no network source was fetched by these preflights/self-tests;
- no file was uploaded or registered;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no DataMax upload, dataset, document, assistant run, third-party event, reply poll, artifact poll, or HTML artifact was created;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no cookie, token, bearer, database URL, provider payload, raw customer row, full customer document, private object path, raw source URL, raw frame path, upload object key, generated artifact local path, raw approval id, or raw approved-by value was recorded in this shared refresh receipt.

## 2026-06-08 OCR-Only PPTX Notes Regression

Task source: EP6 local quality narrowing for OCR/subtitle evidence. Improve review evidence when a video slide has OCR text but no transcript/subtitle alignment, without treating OCR as a transcript page map.

Scope:

- add OCR snippets to screenshot PPTX speaker notes when selected slides have OCR evidence;
- run transcript/OCR note text through `video_safe_evidence_text` before writing `slide_notes.md` or PPTX speaker notes;
- preserve the existing transcript notes behavior when transcript segments are present;
- add an OCR-only media-worker regression proving no `subtitle_page_map` file is generated when transcript alignment is absent;
- keep this as local quality/test work only, with no live upload, third-party event, capture, or deployment.

Implemented behavior:

- PPTX notes now include `OCR evidence: ...` for selected slides with OCR snippets;
- token-like URLs, local paths, and secret-looking transcript/OCR text are redacted before being written into slide notes or PPTX speaker notes;
- OCR-only selected slides still report `missing_transcript`, do not create a `subtitle_page_map` artifact, and keep `has_subtitle_page_map=false`;
- slide notes, `video_slides.md`, PPTX speaker notes, selected slide manifest, and `slide_quality_report.json` all carry OCR-only evidence consistently, while unsafe text is represented as `[redacted]`;
- quality report records `ocr_mapped_count=1`, `ocr_snippet_count=2` for the redaction fixture, and keeps `missing_transcript_alignment` for the absent transcript.

Validation:

```text
cargo fmt --check
cargo test -p media-worker writes_ocr_notes_without_subtitle_page_map_when_transcript_missing --lib
cargo test -p media-worker writes_subtitle_page_map_and_transcript_notes_for_selected_slides --lib
cargo test -p media-worker final_video_deliverables_do_not_require_subtitle_page_map_without_transcript_alignment --lib
cargo test -p media-worker --lib
cargo check -p media-worker --bin video_ppt_offline_smoke
npm run test:video-deliverables
git diff --check
```

Result:

- new OCR-only regression passed, including token-like OCR text redaction across selected slides, slide notes, `video_slides.md`, and PPTX speaker notes;
- existing transcript/subtitle-page-map regression passed;
- no-transcript deliverables regression passed;
- media-worker lib suite passed: 53 tests;
- `video_ppt_offline_smoke` compiled successfully;
- video deliverables validator passed all 19 Node test cases;
- whitespace diff check passed.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- customer-facing quality still depends on real authorized samples and manual review for crop, duplicate, subtitle, OCR, and readability risks.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no service build, restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 PPTX Notes XML Validator Redaction Gate

Task source: continue local acceptance hardening after OCR evidence was added to PPTX speaker notes. The production writer now redacts transcript/OCR note text before writing speaker notes, so the public deliverables validator also needs to inspect PPTX notes XML rather than only checking the OOXML entry list.

Scope:

- extend `validate-video-deliverables` to read `ppt/notesSlides/*.xml` entries from the PPTX ZIP;
- support stored and deflated ZIP entries without adding a new package dependency;
- reject notes XML containing unredacted local paths or token/cookie/authorization/bearer/provider_key/secret/password-shaped evidence;
- keep normal public courseware links from being treated as automatic leaks;
- add a deterministic negative fixture with unsafe notes XML text;
- keep this as local validator/test work only, with no live upload, third-party event, capture, or deployment.

Implemented behavior:

- PPTX structure validation still requires the existing OOXML entries;
- when speaker notes XML entries are present, the validator reads their content and applies the shared redaction scan;
- unreadable notes XML entries now fail with `pptx_notes_xml_read_failed`;
- unsafe notes XML content fails with `unredacted_local_path_or_token` and `kind=pptx_notes_xml`;
- the Windows path detector was tightened so OOXML closing tags are not mistaken for drive-letter paths.

Validation:

```text
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts>
git diff --check
```

Result:

- video deliverables validator passed 20 Node test cases, including the new PPTX speaker notes XML negative case;
- a real public candidate deliverables package passed `validate-video-deliverables` after the notes XML gate was enabled;
- the public candidate check confirmed normal public courseware links are not rejected by the redaction scanner;
- no generated PPTX, frames, raw video, OCR text, notes XML body, local artifact path, source URL, token, cookie, bearer value, approval id, or customer content was copied into this ledger.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- customer-facing quality still depends on real authorized samples and manual review for crop, duplicate, subtitle, OCR, readability, and artifact visibility risks.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, notes XML body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Selected Slides Manifest Validator Gate

Task source: continue local acceptance hardening for the video PPT review package. The active plan treats `selected_slides_manifest.json` as a core review artifact for checking final page selection, requested-vs-final candidate indices, and dedupe behavior, but the public deliverables validator previously did not require or validate it.

Scope:

- make `selected_slides_manifest.json` a required local review-package file;
- require final/extraction manifests to reference the selected manifest and mark its path redacted;
- validate selected/requested candidate indices, selected count, requested count, dedupe status, and duplicate counters;
- cross-check selected count against `slide_rectangles_manifest.promoted_rectangle_count` and `slide_quality_report.slide_count`;
- keep selected manifest as a local review-package requirement, not a mandatory published download entry, because published artifact visibility still needs live main-site smoke evidence;
- keep this as local validator/test work only, with no live upload, third-party event, capture, or deployment.

Implemented behavior:

- missing `selected_slides_manifest.json` now fails with `missing_required_file`;
- malformed selected manifest rows fail with selected-manifest-specific error codes;
- selected/requested indices must be positive integers, and final selected indices must be a subset of requested indices;
- dedupe counters must be non-negative and internally consistent;
- real public deliverables that include selected manifest in final/extraction but not in published download lists remain valid.

Validation:

```text
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir <target-redacted>
git diff --check
```

Result:

- video deliverables validator passed 22 Node test cases, including malformed selected manifest and local-review-only published-boundary coverage;
- a real public candidate deliverables package passed `validate-video-deliverables` with `selected_slides_manifest.json` now listed and checked;
- public-course quality matrix passed with the public candidate still classified as `needs_manual_review`, not `not_deliverable`;
- selected manifest is now part of the local validator contract without overstating published artifact visibility.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- published/download visibility for review artifacts remains a live artifact-smoke question;
- customer-facing quality still depends on real authorized samples and manual review for crop, duplicate, subtitle, OCR, readability, and artifact visibility risks.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, selected manifest body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Output Slide Count Validator Gate

Task source: continue local acceptance hardening for the video PPT review package. The active plan requires PPTX slide count, Markdown slide count, and `selected_count` to agree, but the public deliverables validator previously only checked that the PPTX contained the minimum OOXML entries.

Scope:

- count PPTX slide XML entries under `ppt/slides/slideN.xml`;
- count `video_slides.md` slide headings using `##/### Slide N`;
- require both counts to match `selected_slides_manifest.selected_count`;
- fail Markdown deliverables that contain no slide headings;
- keep this as local validator/test work only, with no live upload, third-party event, capture, or deployment.

Implemented behavior:

- PPTX/selected-count mismatches fail with `output_slide_count_mismatch` and `kind=pptx`;
- Markdown/selected-count mismatches fail with `output_slide_count_mismatch` and `kind=video_slides_markdown`;
- Markdown without slide headings fails with `video_slides_markdown_slide_count_invalid`;
- PPTX notes XML count must match PPTX slide XML count, so speaker notes remain page-aligned.

Validation:

```text
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir <target-redacted>
```

Result:

- video deliverables validator passed 24 Node test cases, including PPTX and Markdown slide-count mismatch coverage;
- a real public candidate deliverables package passed `validate-video-deliverables`, with PPTX, Markdown, selected manifest, rectangles, and quality report counts aligned;
- quality matrix self-test passed;
- public-course quality matrix passed with the public candidate still classified as `needs_manual_review`, not `not_deliverable`;
- no generated PPTX, frame, Markdown body, OCR text, raw video, source URL, token, cookie, bearer value, approval id, customer content, or local artifact path was copied into this ledger.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- customer-facing quality still depends on real authorized samples and manual review for crop, duplicate, subtitle, OCR, readability, and artifact visibility risks.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, Markdown body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Validator Summary Counts for Quality Matrix

Task source: continue local acceptance hardening after the PPTX/Markdown/selected-count gate. The validator now has the authoritative artifact counts, but the quality matrix still derived `selected_count`, `pptx_slide_count`, and `markdown_slide_count` from `slide_quality_report.slide_count`.

Scope:

- return a structured `summary` from `validateVideoDeliverables`;
- include frame count, selected count, requested selected count, PPTX slide count, Markdown slide count, slide rectangle count, quality report slide count, and subtitle page count;
- make `smoke:video-ppt-quality-matrix` prefer validator summary counts for deliverables inputs;
- keep this as local validator/script/test work only, with no live upload, third-party event, capture, or deployment.

Implemented behavior:

- `validateVideoDeliverables(...).summary` exposes the artifact-count evidence already checked by the validator;
- quality matrix deliverables cases now report selected/PPTX/Markdown counts from validator summary, falling back to quality report only if summary data is absent;
- public-course reports include `selected_slides_manifest` in checked file kinds and preserve aligned selected/PPTX/Markdown counts.

Validation:

```text
node --check tools/validate-video-deliverables.mjs
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run test:video-deliverables
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts> --json
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir <target-redacted>
```

Result:

- video deliverables validator passed 24 Node test cases;
- real public candidate validator JSON summary showed selected/PPTX/Markdown/rectangle/quality counts all aligned at 3, with requested count 4;
- quality matrix self-test passed;
- public-course quality matrix passed with the public candidate still classified as a partial public-course review, with selected/PPTX/Markdown counts all 3 and `selected_slides_manifest` included in checked file kinds;
- no generated PPTX, frame, Markdown body, OCR text, raw video, source URL, token, cookie, bearer value, approval id, customer content, or local artifact path was copied into this ledger.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- quality matrix completion still requires a customer/operator-authorized sample before the customer category can be closed.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, Markdown body, validator JSON body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Validator Shared JSON Redaction Gate

Task source: M6N local hardening from the active executable plan. The validator internal API needs local paths so quality-matrix scripts can read generated files, but CLI/shared JSON output must be safe to copy into operator receipts.

Scope:

- add a redacted output helper for `validateVideoDeliverables` results;
- make CLI `--json` output use the redacted result by default;
- keep the internal `validateVideoDeliverables()` return unchanged for local smoke scripts;
- redact local artifact directory, per-file paths, and local paths embedded in error messages;
- keep this as local validator/test work only, with no live upload, third-party event, capture, or deployment.

Implemented behavior:

- shared JSON now includes `outputRedacted=true`;
- shared JSON reports `artifactsDir="[redacted]"` with `artifactsDirRedacted=true`;
- every `files[]` row reports `path="[redacted]"` with `pathRedacted=true`;
- summary counts remain available for shared evidence;
- error messages containing local filesystem paths are redacted before JSON output;
- quality matrix still uses the internal API and can read local deliverable files.

Validation:

```text
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts> --json
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir <target-redacted>
git diff --check
```

Result:

- video deliverables validator passed 27 Node test cases, including three shared JSON redaction tests;
- `node --check tools/validate-video-deliverables.mjs` passed;
- real public candidate validator JSON passed the redaction scan and kept `selected_count=3`, `requested_selected_count=4`, `pptx_slide_count=3`, and `markdown_slide_count=3`;
- `node --check scripts/smoke/video-ppt-quality-matrix.mjs` passed;
- quality matrix self-test passed with 3 cases, 1 deliverable case, and 2 pending cases;
- public-course quality matrix passed with 3 cases, 1 deliverable case, 1 `needs_manual_review` public-course case, and 1 pending customer-authorization case;
- quality matrix report redaction scan passed under the shared-report rule set.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- customer-facing quality still depends on real authorized samples and manual review for crop, duplicate, subtitle, OCR, readability, and artifact visibility risks;
- future shared receipts should copy only redacted summary fields, not raw validator JSON bodies.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg command was run;
- no MP4 was captured, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Missing OCR Evidence Quality Risk

Task source: EP6 local quality narrowing after shared validator JSON redaction. The slide quality report already exposed per-slide `ocr_risk`, but the summary risk list did not make missing OCR evidence visible to manual reviewers.

Scope:

- add a low-severity `missing_ocr_evidence` risk flag to `slide_quality_report.json` when selected slides have no OCR snippets;
- keep OCR as review evidence only, not a substitute for transcript/subtitle page mapping;
- prove OCR-only evidence does not trigger the new missing-OCR risk;
- keep this as local media-worker quality work only, with no live upload, third-party event, browser capture, FFmpeg recording, deployment, or 120-server action.

Implemented behavior:

- selected slides without OCR evidence now increment `summary.ocr_missing_count` and add `risk_flags[].code=missing_ocr_evidence`;
- the new risk uses `severity=low` and `review_action=run_or_review_ocr_evidence_before_customer_delivery`;
- OCR-only deliverables still write redacted OCR evidence into slide notes, `video_slides.md`, and PPTX speaker notes, keep `has_subtitle_page_map=false` when transcript alignment is absent, and do not emit `missing_ocr_evidence`;
- existing screenshot PPTX delivery remains non-blocking: the risk is for manual quality review, not a `final_pptx_ready` failure.

Validation:

```text
cargo fmt --check
cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
cargo test -p media-worker writes_ocr_notes_without_subtitle_page_map_when_transcript_missing --lib
cargo test -p media-worker writes_subtitle_page_map_and_transcript_notes_for_selected_slides --lib
cargo test -p media-worker --lib
cargo check -p media-worker --bin video_ppt_offline_smoke
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input <local-public-course-mp4> --output-root <target-redacted> --ffmpeg-bin <ffmpeg-bin> --interval-seconds 15 --title <public-course-title> --json-output <target-redacted>
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts> --json
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir <target-redacted>
```

Result:

- targeted media-worker tests passed for the base deliverable contract, OCR-only notes without subtitle page map, and subtitle page map with transcript/OCR notes;
- media-worker lib suite passed: 53 tests;
- `video_ppt_offline_smoke` compiled successfully;
- video deliverables validator passed 27 Node test cases;
- quality matrix self-test passed with 3 cases, 1 deliverable case, and 2 pending cases;
- public-course offline smoke completed locally with `deliverable_state=final_pptx_ready`, `frame_count=96`, `selected_count=7`, `has_pptx=true`, `has_video_slides_markdown=true`, `has_slide_quality_report=true`, and `has_subtitle_page_map=false`;
- public-course validator shared JSON passed with redacted paths and aligned counts: `selected_count=7`, `requested_selected_count=9`, `pptx_slide_count=7`, `markdown_slide_count=7`, `slide_rectangle_count=7`, and `quality_slide_count=7`;
- public-course slide quality report now contains `missing_ocr_evidence` with `count=7` and `severity=low`;
- public-course quality matrix remained `needs_manual_review`, with 3 cases, 1 deliverable case, 1 manual-review public-course case, and 1 pending customer-authorization case.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- customer-facing quality still depends on real authorized samples and manual review for crop, duplicate, subtitle, OCR, readability, and artifact visibility risks;
- future production reports should treat `missing_ocr_evidence` as a review prompt, not as proof that PPT screenshot extraction failed.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no MP4 was captured or recorded; the only frame extraction was the local offline smoke from an existing public-course fixture under `target/`;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Readability-Aware Sharpness Review

Task source: EP6 local quality narrowing after the public-course sample still had three high sharpness/readability review pages. Manual inspection showed those pages were readable high-contrast title or closing slides where the sparse-edge sharpness metric was too conservative.

Scope:

- add a conservative readability assessment to `slide_quality_report.json` with `readability_status`, `readability_score`, `readability_risk`, and matching summary counts;
- use high-contrast foreground coverage and spatial distribution to distinguish readable large-title slides from low-information or low-contrast frames;
- keep low-contrast text slides as high-risk review cases and emit `slide_readability_review_required`;
- extend `validate-video-deliverables` so optional readability fields are schema- and count-checked when present;
- tighten quality matrix verdicts so explicit review risks such as `manual_review_required` and `missing_transcript_alignment` keep public-course samples in `needs_manual_review`;
- keep the work local-only with no upload, third-party event, browser capture, FFmpeg recording, service deployment, or 120-server action.

Implemented behavior:

- high-contrast large-title frames with low raw sparse-edge sharpness can now report `sharpness_risk=low` when readability evidence is strong;
- unreadable or low-contrast text still reports `readability_risk=high` and remains a review risk;
- quality matrix no longer treats a public sample as clean merely because `quality_score` reaches 70; explicit review risk flags still force `needs_manual_review`;
- legacy deliverables without readability fields remain compatible.

Validation:

```text
cargo fmt --check
cargo test -p media-worker treats_high_contrast_title_slides_as_readable_even_with_sparse_edges --lib
cargo test -p media-worker flags_low_contrast_slide_text_in_quality_report --lib
cargo test -p media-worker measures_slide_frame_sharpness_for_quality_report --lib
cargo test -p media-worker --lib
cargo check -p media-worker --bin video_ppt_offline_smoke
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input <local-public-course-mp4> --output-root <target-redacted> --ffmpeg-bin <ffmpeg-bin> --interval-seconds 15 --title <public-course-title> --json-output <target-redacted>
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts> --json
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir <target-redacted>
```

Result:

- readable-title fixture passed and proved sparse-edge title pages can be low risk when readability evidence is strong;
- low-contrast text fixture passed and still emits high readability/sharpness review risk;
- sharpness helper fixture passed with measured readable, solid high-risk, and unavailable unknown cases;
- media-worker lib suite passed: 56 tests;
- `video_ppt_offline_smoke` compiled successfully;
- video deliverables validator passed 27 Node test cases, including readability optional-field validation;
- quality matrix self-test passed with 3 cases, 1 deliverable case, and 2 pending cases;
- public-course offline smoke completed locally with `deliverable_state=final_pptx_ready`, `frame_count=96`, `selected_count=7`, `has_pptx=true`, `has_video_slides_markdown=true`, `has_slide_quality_report=true`, and `has_subtitle_page_map=false`;
- public-course validator shared JSON passed with redacted paths and aligned counts: `selected_count=7`, `requested_selected_count=9`, `pptx_slide_count=7`, `markdown_slide_count=7`, `slide_rectangle_count=7`, and `quality_slide_count=7`;
- public-course quality report now has `full_frame_fallback_count=0`, `detector_crop_count=7`, `sharpness_high_count=0`, `sharpness_unknown_count=0`, `readability_high_count=0`, `readability_unknown_count=0`, `subtitle_missing_count=7`, and `ocr_missing_count=7`;
- public-course quality score increased to 70, but quality matrix correctly remains `needs_manual_review` because transcript/OCR evidence and manual review risk are still present.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- public-course quality still needs manual review for missing transcript/OCR evidence, duplicate choices, and final customer usability;
- true customer-facing quality still requires an authorized customer/operator sample and a live artifact visibility pass.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no MP4 was captured or recorded; the only frame extraction was the local offline smoke from an existing public-course fixture under `target/`;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Full-Frame Content Slide Detector

Task source: EP6 local crop-quality narrowing after the public-course sample still had one `full_frame_rectangle_fallback` page. Manual inspection showed that page was a full-screen dark title slide, where the whole frame is the courseware page rather than a missed inner rectangle.

Scope:

- add a conservative `full_frame_content_v1` detector after the existing border, edge, and bright-canvas detectors;
- use it only when a frame has a uniform border/background, enough high-contrast content regions, broad grid span, and not too much foreground complexity;
- keep sparse overlays, tiny watermarks, and low-information frames out of the full-frame-content path;
- keep the behavior as local crop-quality work only, with no live upload, third-party event, browser capture, FFmpeg recording, service deployment, or 120-server action.

Implemented behavior:

- full-screen content slides now get `rectangle_extraction_status=promoted_detector_crop` and `rectangle_extraction_mode=full_frame_content_v1`;
- the crop box remains full-frame because the whole frame is the slide page, but the quality report no longer treats it as a fallback detector miss;
- `slide_quality_report.summary.full_frame_fallback_count` drops when this detector applies;
- tiny overlays on otherwise dark frames still return no detector crop and remain fallback/review if selected.

Validation:

```text
cargo fmt --check
cargo test -p media-worker detects_full_frame_content_slides_without_marking_crop_fallback --lib
cargo test -p media-worker does_not_treat_sparse_overlay_as_full_frame_content_slide --lib
cargo test -p media-worker detects_slide_rectangle --lib
cargo test -p media-worker --lib
cargo check -p media-worker --bin video_ppt_offline_smoke
node --check tools/validate-video-deliverables.mjs
npm run test:video-deliverables
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
cargo run -p media-worker --bin video_ppt_offline_smoke -- --input <local-public-course-mp4> --output-root <target-redacted> --ffmpeg-bin <ffmpeg-bin> --interval-seconds 15 --title <public-course-title> --json-output <target-redacted>
node tools/validate-video-deliverables.mjs <public-candidate-generated_artifacts> --json
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <public-candidate-generated_artifacts> --pretty --output-dir <target-redacted>
```

Result:

- new full-frame-content fixture passed and produced `full_frame_content_v1`;
- sparse-overlay negative fixture passed and did not produce a detector crop;
- existing rectangle detector tests passed for noisy border, foreground component, edge projection, and bright canvas;
- media-worker lib suite passed: 55 tests;
- `video_ppt_offline_smoke` compiled successfully;
- video deliverables validator passed 27 Node test cases;
- quality matrix self-test passed with 3 cases, 1 deliverable case, and 2 pending cases;
- public-course offline smoke completed locally with `deliverable_state=final_pptx_ready`, `frame_count=96`, `selected_count=7`, `has_pptx=true`, `has_video_slides_markdown=true`, `has_slide_quality_report=true`, and `has_subtitle_page_map=false`;
- public-course validator shared JSON passed with redacted paths and aligned counts: `selected_count=7`, `requested_selected_count=9`, `pptx_slide_count=7`, `markdown_slide_count=7`, `slide_rectangle_count=7`, and `quality_slide_count=7`;
- public-course quality report improved from one fallback page to `full_frame_fallback_count=0` and `detector_crop_count=7`; rectangle modes include one `full_frame_content_v1` page and six `border_background_contrast_v2` pages;
- public-course quality score increased to 63, but quality matrix correctly remains `needs_manual_review` because transcript/OCR evidence is still missing and three selected pages still have high sharpness/readability risk.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- public-course quality still needs manual review for subtitle/OCR gaps, sharpness/readability, duplicate choices, and final customer usability;
- future crop-quality work should target the remaining sharpness/fade/readability risks rather than relaxing validator or matrix gates.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no MP4 was captured or recorded; the only frame extraction was the local offline smoke from an existing public-course fixture under `target/`;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports and deliverables stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Quality Matrix Review-Risk Regression Gate

Task source: M6Q exposed an important matrix edge case: once readability-aware scoring raised a public-course package to `quality_score=70` with no sharpness/readability high counts, the matrix still had to respect explicit review-risk flags such as missing transcript alignment and manual review requirements.

Scope:

- keep the quality matrix verdict conservative when a package is otherwise aligned and exactly at the quality-score threshold;
- add deterministic `--self-test` regression coverage without changing the three-category self-test report shape;
- do not create, upload, download, register, capture, deploy, or expose any real deliverable artifacts.

Implemented behavior:

- `smoke:video-ppt-quality-matrix -- --self-test` now internally constructs 70-point, page-count-aligned regression cases for:
  - `manual_review_required`;
  - `missing_transcript_alignment`;
  - `frame_sharpness_review_required`;
  - `slide_readability_review_required`.
- each internal regression case must evaluate to `needs_manual_review`;
- the visible self-test report still contains only the three required matrix categories, preserving the existing report contract.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
```

Result:

- script syntax check passed;
- quality matrix self-test passed with 3 visible cases, 1 deliverable case, and 2 pending cases;
- the new internal review-risk assertions passed for all four explicit risk flags.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- full customer-facing quality still depends on authorized customer/operator samples and live artifact visibility evidence.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no MP4 was captured or recorded;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Supported Video Extension Summary

Task source: M6AZ P1/no-live follow-up. Upload-main and third-party self-test child reports already proved supported video extension counts, but the top-level `acceptance_status.no_live_evidence_summary` did not directly expose whether both surfaces had extension readiness evidence. This slice makes the `.mp4/.mov/.m4v/.webm/.mkv/.avi` support evidence copyable from the top-level no-live report.

Scope:

- local no-live rollup code slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, upload, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- `acceptance_status.no_live_evidence_summary` now records `required_video_extension_count=6`;
- the summary copies `upload_main_supported_video_extension_count` and `external_supported_video_extension_count`;
- the summary records `supported_video_extension_min_count`, `supported_video_extension_evidence_surface_count`, `supported_video_extension_evidence_ready`, and `unsupported_non_video_extensions_rejected`;
- no-live rollup validation now fails unless upload-main and third-party evidence each expose at least 6 supported video extensions, both surfaces are counted, and unsupported non-video extensions are rejected.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-extension-summary-m6az
node -e "<redacted extension summary readback>"
rg -n "<local-path-url-token-approval-patterns>" target/video-ppt-no-live-rollup-extension-summary-m6az
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- report readback showed `required_video_extension_count=6`;
- report readback showed `upload_main_supported_video_extension_count=6`;
- report readback showed `external_supported_video_extension_count=6`;
- report readback showed `supported_video_extension_min_count=6`;
- report readback showed `supported_video_extension_evidence_surface_count=2`;
- report readback showed `supported_video_extension_evidence_ready=true`;
- report readback showed `unsupported_non_video_extensions_rejected=true`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- main upload and external preflights stayed ready, while external bearer remained pending;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, raw approval/operator values, or customer retention values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this top-level extension readiness evidence does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- full acceptance remains blocked on live/customer/deployment evidence or explicit non-executable reasons.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, local artifact paths, or customer retention values were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Pending Gate Requirements Summary

Task source: M6BC P1/no-live follow-up. The acceptance status already listed each gate with `requires`, but reviewers still had to inspect the full gate array to see which live/customer/deployment inputs were blocking full acceptance. This slice adds a compact pending requirements summary while preserving `full_acceptance_ready=false`.

Scope:

- local no-live rollup code slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, upload, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- `acceptance_status.pending_gate_requirements_summary` now uses schema `v3.video_ppt_pending_gate_requirements_summary.v1`;
- the summary records `pending_gate_count=7` and the exact pending gate ids for P2/P3/P4/P5/P6/P7/P8;
- the summary records booleans for main upload write approval, third-party credentials, login-gated handoff deployment approval, authorized capture approval record, customer authorized sample, server deployment window, and P8 live/customer/deployment closeout;
- the summary records `no_live_substitute_available_for_pending_gates=false`;
- no-live rollup validation fails if any required pending gate id or blocking requirement bit is missing.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-pending-summary-m6bc
node -e "<redacted pending summary readback>"
rg -n "<local-path-url-token-approval-patterns>" target/video-ppt-no-live-rollup-pending-summary-m6bc
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- report readback showed schema `v3.video_ppt_pending_gate_requirements_summary.v1`;
- report readback showed `pending_gate_count=7`;
- report readback listed `P2_main_upload_live_smoke`, `P3_external_video_ppt_live_smoke`, `P4_login_gated_handoff_live_pass`, `P5_authorized_capture_live_sample`, `P6_customer_authorized_quality_matrix`, `P7_server_deployment_gate`, and `P8_full_acceptance_close`;
- report readback showed all blocking requirement bits true: main upload write approval, external credentials, login-gated handoff deployment approval, authorized capture approval record, customer authorized sample, server deployment window, and full acceptance live/customer/deployment closeout;
- report readback showed `no_live_substitute_available_for_pending_gates=false`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, raw approval/operator values, or customer retention values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this top-level requirements summary does not replace any live gate; P2/P3/P4/P5/P6/P7/P8 still require their corresponding authorization, credentials, customer input, or deployment window;
- full acceptance remains blocked on live/customer/deployment evidence or explicit non-executable reasons.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, local artifact paths, or customer retention values were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Main Upload Artifact Summary

Task source: M6BB P1/no-live follow-up. After M6BA elevated third-party artifact surface evidence to the top-level no-live acceptance status, the main upload self-test still exposed artifact/download readiness only inside the upload child evidence. This slice makes the main upload artifact contract copyable from the top-level no-live report while keeping live main-site artifact downloads pending.

Scope:

- local no-live rollup code slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, upload, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- `acceptance_status.no_live_evidence_summary` now records `main_upload_artifact_surface_ready`, required file-kind count, PPTX slide count, Markdown slide heading count, and download validation state;
- `acceptance_status.live_gate_readiness_summary` now records `main_upload_artifact_self_test_ready` and keeps `main_upload_artifact_live_download_pending=true`;
- no-live rollup validation now fails unless the main upload self-test reaches `final_pptx_ready`, exposes at least 6 required file kinds, validates PPTX/Markdown locally, and keeps live upload/download pending behind main-site write authorization.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-main-artifact-m6bb
node -e "<redacted main artifact readback>"
rg -n "<local-path-url-token-approval-patterns>" target/video-ppt-no-live-rollup-main-artifact-m6bb
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- top-level evidence readback showed `main_upload_artifact_surface_ready=true`;
- top-level evidence readback showed `main_upload_artifact_required_file_kind_count=6`;
- top-level evidence readback showed `main_upload_artifact_pptx_slide_count=1`;
- top-level evidence readback showed `main_upload_artifact_markdown_slide_heading_count=1`;
- top-level evidence readback showed `main_upload_artifact_download_validation_ok=true`;
- live readiness readback showed `main_upload_artifact_self_test_ready=true`;
- live readiness readback showed `main_upload_artifact_live_download_pending=true`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, raw approval/operator values, or customer retention values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this top-level main upload artifact evidence does not replace main-site upload live smoke or live artifact downloads; user approval for a non-customer smoke write and a safe video input are still required;
- this does not replace third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, local artifact paths, or customer retention values were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Third-Party Artifact Surface Summary

Task source: M6BA P1/no-live follow-up. The third-party video PPT self-test already validates a reply surface with `download_exports`, artifact links, and local self-test download validation, but the top-level acceptance status did not directly state whether artifact surface visibility was represented. This slice makes the third-party artifact surface evidence copyable from the no-live rollup while keeping live artifact downloads pending.

Scope:

- local no-live rollup code slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, upload, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- external video PPT self-test evidence now copies reply surface counts: `surface_ok`, `surface_export_count`, `surface_export_kind_count`, `surface_missing_export_kind_count`, and `surface_artifact_link_count`;
- `acceptance_status.no_live_evidence_summary` now records `external_artifact_surface_ready`, required/export/missing kind counts, artifact link count, and download validation state;
- `acceptance_status.live_gate_readiness_summary` now records `external_artifact_surface_self_test_ready` and keeps `external_artifact_live_download_pending=true`;
- no-live rollup validation now fails unless the third-party self-test surface exposes all required export kinds, has no missing export kinds, includes at least one artifact link, and validates PPTX/Markdown downloads locally.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir target/video-ppt-no-live-rollup-artifact-surface-m6ba
node -e "<redacted artifact surface readback>"
rg -n "<local-path-url-token-approval-patterns>" target/video-ppt-no-live-rollup-artifact-surface-m6ba
git diff --check
git ls-files target | wc -l
```

Result:

- first rollup attempt failed locally because the new third-party surface assertions were accidentally placed in the upload-main evidence validator;
- the assertions were moved to `validateExternalVideoPptEvidence()` and the full no-live rollup was rerun;
- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- report readback showed `external_surface_ok=true`;
- report readback showed `external_surface_export_count=6`;
- report readback showed `external_surface_export_kind_count=6`;
- report readback showed `external_surface_missing_export_kind_count=0`;
- report readback showed `external_surface_artifact_link_count=1`;
- top-level evidence readback showed `external_artifact_surface_ready=true`;
- top-level evidence readback showed `external_artifact_required_export_kind_count=6`;
- top-level evidence readback showed `external_artifact_export_kind_count=6`;
- top-level evidence readback showed `external_artifact_missing_export_kind_count=0`;
- top-level evidence readback showed `external_artifact_link_count=1`;
- top-level evidence readback showed `external_artifact_download_validation_ok=true`;
- live readiness readback showed `external_artifact_surface_self_test_ready=true`;
- live readiness readback showed `external_artifact_live_download_pending=true`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, raw approval/operator values, or customer retention values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this top-level artifact surface evidence does not replace third-party live smoke or live artifact downloads; bearer, connection id, source id, safe input, and live authorization are still required;
- this does not replace main-site upload live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, local artifact paths, or customer retention values were recorded in this shared receipt.

## 2026-06-08 Final Executable Plan Doc-Only Closeout

Task source: user asked to continue the previous step and finish the complete executable plan, with the explicit scope that this is a plan-only pass and code should not be changed.

Scope:

- documentation-only plan closeout;
- no business code, smoke script, validator, worker, API, web app, or deployment change;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no video download, upload, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented documentation behavior:

- `docs/plans/datamax-active-execution-plan.md` now marks section `0.7.21 2026-06-08 最新完整可执行方案` as the latest execution entry;
- the plan top matter now says M6AV/M6AW/M6AX/M6AY are complete and pushed, and that this slice is plan-only;
- section `0.7.21` consolidates the path selector for P0 plan-only, P1 no-live baseline, P2 main upload live, P3 third-party live, P4 WeChat/login-gated handoff, P5 authorized capture, P6 customer quality matrix, P7 deployment, and P8 closeout;
- the plan restates the product boundary: extract PPT/slides/courseware already shown in a video, not create an authored PPT from an ordinary video;
- the plan keeps WeChat Video Channels and login-gated/private playback links on handoff unless an anonymous video file or authorized recording exists;
- the plan keeps GitHub sync separate from 8-server deployment, and keeps server memory as historical context that must be live-verified before deployment claims.

Validation for this plan-only slice:

```text
cmp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
git diff --check
git diff --cached --name-only
git diff --cached --stat
git ls-files target | wc -l
```

Result:

- repository plan and desktop plan copy match;
- whitespace check passes;
- staged files are documentation only: `docs/plans/datamax-active-execution-plan.md` and `docs/validation/video-ppt-deliverable-smoke.md`;
- cached diff was two Markdown files, 125 insertions and 2 deletions;
- no generated `target/` files are tracked.

Remaining work:

- this plan-only closeout does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- full acceptance remains blocked on live/customer/deployment evidence or explicit non-executable reasons.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Customer Quality Matrix Authorization Argument Gate

Task source: M6AV no-live follow-up. Customer-authorized deliverables are allowed only with explicit authorization metadata. The script already rejected missing approval ids at runtime; this slice turns that behavior into a self-test and no-live rollup gate.

Scope:

- local quality-matrix and no-live rollup argument-gate slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- quality matrix input-mode validation is now centralized in `validateInputModeArgs()`;
- self-test now verifies that `--customer-deliverables` without `--customer-approval-id` is rejected;
- self-test now verifies that `--customer-approval-id` without `--customer-deliverables` is rejected;
- self-test now verifies that `--self-test` cannot be combined with customer deliverables input flags;
- self-test verifies the valid customer argument shape can pass argument validation without reading a file;
- self-test exposes `customer_authorization_argument_gate_supported=true`;
- no-live rollup copies and validates that support bit in quality matrix evidence.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted customer authorization argument gate readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=0`, `not_deliverable_count=0`, and `pending_count=2`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- quality matrix report exposed `customer_authorization_argument_gate_supported=true`;
- quality matrix report retained `deliverables_mode_failure_class_gate_defaults_supported=true`;
- no-live rollup quality evidence copied `customer_authorization_argument_gate_supported=true`;
- no-live rollup quality evidence retained `deliverables_mode_failure_class_gate_defaults_supported=true`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this argument gate does not replace customer-authorized quality matrix execution; it only proves the customer input path cannot run without the required approval reference;
- real customer/operator sample processing still requires explicit authorization, input source, and retention policy.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Deliverables-Mode Quality Matrix Gate Metadata

Task source: M6AU no-live follow-up. The quality matrix self-test exposed stable failure and review gates, but real `--synthetic-deliverables`, `--public-course-deliverables`, and `--customer-deliverables` reports also need the same gate metadata so customer/operator sample reports remain self-describing.

Scope:

- local quality-matrix and no-live rollup evidence slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- `buildQualityMatrixReport()` now applies common review/failure gate metadata to every report, including deliverables input modes;
- every quality matrix report now carries `review_required_risk_flags`, `review_required_risk_flag_count`, and `review_required_risk_flag_object_shape_supported`;
- every quality matrix report now carries `not_deliverable_failure_classes`, `not_deliverable_failure_class_count`, `failure_class_summary_supported`, and `review_failure_class_summary_supported`;
- quality matrix validation now fails if a report lacks those common review/failure gate fields;
- self-test now runs an internal deliverables-mode gate metadata regression without writing a separate report;
- self-test exposes `deliverables_mode_failure_class_gate_defaults_supported=true`;
- no-live rollup copies and validates that support bit in quality matrix evidence.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted deliverables-mode gate evidence readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=0`, `not_deliverable_count=0`, and `pending_count=2`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- quality matrix report exposed `deliverables_mode_failure_class_gate_defaults_supported=true`;
- quality matrix report exposed `review_required_risk_flag_count=8`;
- quality matrix report exposed `not_deliverable_failure_class_count=5`;
- quality matrix report exposed `failure_class_summary_supported=true`;
- quality matrix report exposed `review_failure_class_summary_supported=true`;
- no-live rollup quality evidence copied `deliverables_mode_failure_class_gate_defaults_supported=true`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this metadata gate does not replace customer-authorized quality matrix execution; it only guarantees that future synthetic/public/customer deliverables reports carry the needed review/failure vocabulary;
- customer/operator sample processing still requires explicit authorization, input source, and retention policy.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Acceptance Status Live Gate Readiness Summary

Task source: M6AT no-live follow-up. M6AS made the no-live evidence coverage visible inside `acceptance_status`, but the live-gate blockers were still distributed across upload, external, handoff, and capture child evidence. This slice adds a compact readiness summary that states which live gates are preflight-ready and which authorization, credential, deployment, or customer inputs remain missing.

Scope:

- local no-live rollup evidence slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- no-live rollup now emits `acceptance_status.live_gate_readiness_summary` with schema `v3.video_ppt_live_gate_readiness_summary.v1`;
- the summary records main-upload preflight readiness while preserving that live write approval is still required;
- the summary records external video PPT preflight readiness while preserving that the live bearer is not ready;
- the summary records login-gated handoff preflight readiness while preserving that 8-server deployment approval is still required;
- the summary records authorized-capture dry-run readiness while preserving that no live capture was attempted;
- the summary records pending flags for main live write approval, external bearer, 8-server deployment approval, authorized capture live sample, and customer-authorized sample;
- no-live rollup validation now fails if a passed rollup does not include the expected live readiness and pending-gate flags.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted live gate readiness readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- quality matrix syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=0`, `not_deliverable_count=0`, and `pending_count=2`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- live gate readiness summary schema was `v3.video_ppt_live_gate_readiness_summary.v1`;
- main upload preflight was ready and still required live write approval;
- external video PPT preflight was ready, external context was present, and live credential readiness remained false;
- login-gated handoff preflight was ready, target mode count was `2`, and deployment approval was still required;
- authorized capture dry-run was ready, approval reference was present and redacted, and no capture was attempted;
- pending flags were true for main live write approval, external bearer, 8-server deployment approval, authorized capture live sample, and customer-authorized sample;
- `live_or_deploy_action_run=false`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this readiness summary does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- future live gates or deployment gates must update this readiness summary and no-live validation before they are treated as covered by P1.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Acceptance Status No-Live Evidence Summary

Task source: M6AS no-live follow-up. The no-live rollup already copied child evidence into `commands[].evidence`, but `acceptance_status` only showed the P1-P8 gate matrix. This slice makes the acceptance status itself carry a compact, redacted no-live evidence summary so reviewers can see what the local baseline actually covered without opening every child command record.

Scope:

- local no-live rollup evidence slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- no-live rollup now emits `acceptance_status.no_live_evidence_summary` with schema `v3.video_ppt_no_live_acceptance_evidence_summary.v1`;
- the summary records 11 passed commands with embedded child evidence;
- the summary records the three live preflight evidence gates for main upload, third-party video PPT, and login-gated handoff;
- the summary records quality matrix counts: `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- the summary records `quality_review_required_risk_flag_count=8`;
- the summary records `not_deliverable_failure_class_count=5`;
- the summary records `review_failure_class_summary_needs_manual_review_count=8`;
- the summary copies the six-class needs-manual-review count map and five-class not-deliverable regression count map;
- no-live rollup validation now fails if a passed rollup does not include the expected acceptance evidence summary.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted acceptance evidence summary readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- quality matrix syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `needs_manual_review_count=0`, `not_deliverable_count=0`, and `pending_count=2`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- acceptance evidence summary schema was `v3.video_ppt_no_live_acceptance_evidence_summary.v1`;
- `embedded_evidence_command_count=11`;
- upload-main self-test/preflight, external video PPT self-test/preflight, handoff self-test/preflight, authorized-capture self-test, authorized-capture dry-run, quality matrix, production scope-planner, and assistant-runtime evidence flags were all true;
- `live_preflight_evidence_count=3`;
- quality matrix counts in the acceptance summary were `case_count=3`, `deliverable_count=1`, and `pending_count=2`;
- review risk count was `8`;
- not-deliverable failure class count was `5`;
- review summary count was `8`;
- review summary counts were `manual_review=1`, `subtitle_alignment=1`, `ocr_evidence=1`, `crop_quality=1`, `selection_quality=2`, and `readability_quality=2`;
- not-deliverable regression counts were `source_access=1`, `video_has_no_ppt=1`, `frame_extraction=1`, `artifact_visibility=1`, and `selection_quality=1`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this acceptance evidence summary does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- future no-live evidence gates that are required for P1 should be added to this acceptance summary before they are treated as part of the local baseline.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Quality Matrix Object Risk Flag Gate

Task source: M7/EP6 no-auth hardening after M6AA centralized the review-required risk flag set. Real `slide_quality_report.risk_flags` entries are objects with `code`, `severity`, `count`, and `review_action`, while some internal quality-matrix regressions only used string risk codes. This slice makes the quality matrix verdict path normalize both shapes before deciding whether a sample must remain `needs_manual_review`.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, server deployment, or 8/120 server access;
- keep the same 8 review-required risk codes from M6AA;
- extend `scripts/smoke/video-ppt-quality-matrix.mjs` so `evaluateCase()` normalizes object-shaped and string-shaped risk flags through the same helper;
- extend self-test regression so each of the 8 risk codes is tested in both string and object shapes.

Implemented behavior:

- `evaluateCase()` now uses `normalizeRiskFlags()` before building the risk flag set;
- self-test internal regression checks all 8 review-required risk flags as strings;
- self-test internal regression also checks all 8 review-required risk flags as objects matching the shape emitted by `slide_quality_report.risk_flags[]`;
- self-test reports expose `review_required_risk_flag_object_shape_supported=true` and `review_required_risk_flag_object_shape_case_count=8`.

Validation:

```text
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted report summary readback>"
```

Result:

- no-live rollup baseline passed before the slice with `command_count=18`, `passed_count=18`, and `failed_count=0`;
- quality matrix syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`, and `expectation_mismatch_count=0`;
- self-test report exposed `review_required_risk_flag_count=8`;
- self-test report exposed `review_required_risk_flag_object_shape_supported=true`;
- self-test report exposed `review_required_risk_flag_object_shape_case_count=8`;
- guarded risk flags remained `manual_review_required`, `missing_transcript_alignment`, `missing_ocr_evidence`, `full_frame_rectangle_fallback`, `selected_slide_duplicates_removed`, `frame_sharpness_review_required`, `slide_readability_review_required`, and `single_slide_output_review_required`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- future media-worker risk flags that require manual review must be added to the same quality-matrix gate and rerun through no-live rollup.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Review Failure Class Summary Counts

Task source: M6AR no-live follow-up. The failure-class summary evidence covered not-deliverable cases, but `needs_manual_review` samples also need stable, aggregate review reasons for customer/operator quality reports.

Scope:

- local quality-matrix and no-live rollup evidence slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- quality matrix self-test gates now expose `review_failure_class_summary_supported=true`;
- quality matrix self-test gates now expose `review_failure_class_summary_needs_manual_review_count=8`;
- quality matrix self-test gates now expose `review_failure_class_summary_counts`;
- the expected review summary distribution is `manual_review=1`, `subtitle_alignment=1`, `ocr_evidence=1`, `crop_quality=1`, `selection_quality=2`, and `readability_quality=2`;
- no-live rollup copies those review summary fields into `quality_matrix_self_test.evidence`;
- no-live rollup validation fails if any review class count is missing or changed.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted review summary evidence readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`, and `not_deliverable_count=0`;
- quality matrix gates reported `review_failure_class_summary_supported=true`;
- quality matrix gates reported `review_failure_class_summary_needs_manual_review_count=8`;
- quality matrix gates reported review summary counts `manual_review=1`, `subtitle_alignment=1`, `ocr_evidence=1`, `crop_quality=1`, `selection_quality=2`, and `readability_quality=2`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- no-live quality evidence copied the same review summary count map;
- no-live quality evidence retained the five not-deliverable regression count map;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this review-summary evidence does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- future review-required risk flags must be mapped into this review summary gate before being used in customer/operator shared reports.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Failure Class Summary Regression Counts

Task source: M6AQ no-live follow-up. The no-live rollup now carries the summary maps, but the deterministic self-test's three primary cases leave those maps empty. This slice exposes the five-class not-deliverable regression counts as explicit, redacted evidence.

Scope:

- local quality-matrix and no-live rollup evidence slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- quality matrix self-test gates now expose `failure_class_summary_regression_not_deliverable_count=5`;
- quality matrix self-test gates now expose `failure_class_summary_regression_class_count=5`;
- quality matrix self-test gates now expose `failure_class_summary_regression_counts`;
- quality matrix self-test gates now expose `failure_class_summary_regression_not_deliverable_counts`;
- quality matrix validation requires each of `source_access`, `video_has_no_ppt`, `frame_extraction`, `artifact_visibility`, and `selection_quality` to have count `1` in both regression maps;
- no-live rollup copies those fields into `quality_matrix_self_test.evidence` and validates the same five-class count map.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted regression count evidence readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`, and `not_deliverable_count=0`;
- quality matrix gates reported `failure_class_summary_supported=true`;
- quality matrix gates reported `failure_class_summary_regression_not_deliverable_count=5`;
- quality matrix gates reported `failure_class_summary_regression_class_count=5`;
- quality matrix gates reported `source_access`, `video_has_no_ppt`, `frame_extraction`, `artifact_visibility`, and `selection_quality` each with count `1` in the regression maps;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- no-live quality evidence copied the same five-class regression count maps;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this regression-count evidence does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- future new failure classes must be added to the regression map and no-live rollup validation before they are used in customer/operator shared reports.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Failure Summary Maps

Task source: M6AP no-live follow-up. M6AO added the quality matrix summary maps, but the top-level no-live rollup only copied the support gate. This slice makes the rollup evidence carry the sanitized summary maps directly.

Scope:

- local no-live rollup evidence slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- no-live rollup quality-matrix evidence now includes `failure_class_counts`;
- no-live rollup quality-matrix evidence now includes `not_deliverable_failure_class_counts`;
- no-live rollup quality-matrix evidence now includes `needs_manual_review_failure_class_counts`;
- rollup extraction sanitizes those maps to nonnegative integer count values only;
- rollup validation now fails if any of the three summary maps is missing or malformed.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted rollup summary maps evidence readback>"
rg -n "<local-path-url-token-approval-patterns>" <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- quality matrix syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- rollup quality-matrix evidence reported `failure_class_summary_supported=true`;
- rollup quality-matrix evidence included `failure_class_counts={}`;
- rollup quality-matrix evidence included `not_deliverable_failure_class_counts={}`;
- rollup quality-matrix evidence included `needs_manual_review_failure_class_counts={}`;
- rollup quality-matrix evidence retained five not-deliverable failure classes: `source_access`, `video_has_no_ppt`, `frame_extraction`, `artifact_visibility`, and `selection_quality`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this rollup evidence hardening does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- the maps are empty in the deterministic self-test because the three primary self-test cases do not include a not-deliverable case; the five-class coverage remains proven by the quality matrix internal regression and `failure_class_summary_supported=true` gate.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Quality Matrix Failure Class Summary Counts

Task source: M6AO no-live follow-up. M6AN added stable failure classes; this slice adds explicit summary count objects so future customer/operator quality matrix reports can show both per-case failure classes and aggregate counts without hand-counting cases.

Scope:

- local quality-matrix and no-live rollup code slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- quality matrix summaries now always include `failure_class_counts`, `not_deliverable_failure_class_counts`, and `needs_manual_review_failure_class_counts`;
- summary counting increments from `evaluation.failure_class` and splits counts by `review_conclusion`;
- self-test exposes `failure_class_summary_supported=true`;
- internal regression builds one not-deliverable case for each required failure class and verifies each class increments both the all-failure and not-deliverable summary counts;
- no-live rollup copies `failure_class_summary_supported` from the quality-matrix child report into its redacted evidence and fails validation if the gate is missing.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted failure summary evidence readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`, and `not_deliverable_count=0`;
- quality matrix report included all three summary maps: `failure_class_counts`, `not_deliverable_failure_class_counts`, and `needs_manual_review_failure_class_counts`;
- self-test gate reported `failure_class_summary_supported=true`;
- self-test gate retained five not-deliverable failure classes: `source_access`, `video_has_no_ppt`, `frame_extraction`, `artifact_visibility`, and `selection_quality`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- no-live quality evidence copied `failure_class_summary_supported=true` and the same five not-deliverable failure classes;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this summary gate does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- additional live/customer failure classes must be represented in both quality matrix self-test regression and no-live rollup evidence before being used in shared reports.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Plan-Only Executable Plan Completion

Task source: user requested a complete executable plan only, without code changes or 8-server deployment.

Scope:

- plan-only consolidation for DataMax video PPT extraction;
- no business code change in this slice;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no customer sample processing;
- no 8-server pull/build/restart/deploy and no 120-server action.

Plan changes:

- added `0.7.19 2026-06-08 本次完整可执行计划编写收口` as the current plan-only execution entry;
- consolidated original plan, video test/extraction review, WeChat Video Channels and login-gated source handling, authorized capture fallback, GitHub sync, and 8-server deployment boundaries into one P0-P8 execution table;
- updated the current feature baseline wording from M6AM-only to M6AN, including not-deliverable failure taxonomy evidence;
- recorded that any local changes in `scripts/smoke/video-ppt-quality-matrix.mjs` or `scripts/smoke/video-ppt-no-live-rollup.mjs` are M6AO candidate code work and must not be mixed into a doc-only submission;
- kept the active plan synchronized to `/Users/manslive01/Desktop/datamax-active-execution-plan.md`.

Validation:

```text
cmp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
git diff --check
git diff --name-only
git status --short --branch
```

Result:

- desktop plan copy matched the repository plan by `cmp`;
- final `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`;
- intended doc-only staged files are limited to `docs/plans/datamax-active-execution-plan.md` and `docs/validation/video-ppt-deliverable-smoke.md`;
- existing script changes, if present in the worktree, are intentionally excluded from this plan-only scope.

Remaining work:

- P2 main-site upload live controlled smoke still needs explicit approval to write one non-customer smoke record;
- P3 third-party live smoke still needs bearer, `connection_id`, `source_id`, and safe input;
- P4 WeChat/login-gated handoff live pass still needs an approved 8-server deployment window;
- P5 authorized capture live sample still needs an approval record and playable source;
- P6 customer quality matrix still needs a customer/operator authorized sample and retention policy;
- P8 full acceptance cannot close until the live/customer/deployment gates have evidence or explicit non-executable reasons.

Safety result:

- no live upload was run;
- no third-party event was posted;
- no bearer or external credential was used;
- no source video was downloaded, uploaded, registered, frame-extracted, OCRed, transcribed, or converted;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Executable Plan Final Lock And Change Isolation

Task source: plan-only continuation to finish the consolidated executable plan, while the local working tree also contains an uncommitted M6AF no-live rollup script slice. This receipt records the plan/documentation boundary so doc-only synchronization does not accidentally include unconfirmed code changes or imply 8-server deployment.

Scope:

- update the active plan with a current final-lock section that separates EP0/doc-only synchronization from the M6AF code path and from live/customer/deployment gates;
- keep `docs/plans/datamax-active-execution-plan.md` as the only active plan under `docs/plans/`;
- update this validation ledger with the same boundary and fix the no-live rollup child-report path wording;
- synchronize the desktop plan copy after the document edits;
- do not change business code, smoke runner code, validator code, worker code, API code, UI code, or deployment scripts in this plan-only pass.

Plan result:

- active plan section `0.7.17` now lists the current executable paths: EP0/doc-only, M6AF code-slice closeout, M7 no-auth baseline, M8 main-site upload smoke, M9 third-party smoke, M10 deployed handoff, M11 authorized capture sample, and M12 customer quality matrix;
- EP0/doc-only now explicitly requires staged-file isolation so `scripts/smoke/video-ppt-no-live-rollup.mjs` is not included unless the user intentionally chooses the M6AF code path;
- M6AF code-slice closeout now has its own validation command list, redaction scan, evidence requirements, and `target/` tracking check;
- customer/operator input routing now explicitly distinguishes uploaded/direct video, public web page resolver, WeChat/login-gated handoff, authorized capture fallback, customer quality review, and ordinary video generation requests;
- extraction quality review now keeps the same three verdicts: deliverable, needs manual review, and not deliverable;
- 8-server internal capture remains a separate research path and is not enabled by this plan.

Validation:

```text
cp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
cmp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
git diff --check
git diff --name-only
git status --short --branch
```

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- if the user chooses doc-only GitHub sync, only plan/validation documents should be staged and pushed;
- if the user chooses to sync M6AF code, the script change must be validated and committed as a separate code slice with its own no-live rollup evidence;
- 8-server deployment still requires a separate explicit EP7 approval window.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Quality Matrix Evidence

Task source: M6AD follow-up after M6AC made quality matrix object-shaped risk flags explicit. The no-live rollup previously proved that the quality matrix self-test command passed, but the rollup report did not carry the child report's key gate evidence. This slice lets the rollup read the local quality-matrix child report and embed a narrow, redacted evidence summary in the top-level no-live report.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, server deployment, or 8/120 server access;
- read only the report path printed by the local `quality_matrix_self_test` command;
- accept relative `target/` report paths, or absolute report paths only when they resolve under the current repo `target/` directory;
- copy only counts and boolean gate fields into the rollup report, not the child report path or raw JSON body.

Implemented behavior:

- `scripts/smoke/video-ppt-no-live-rollup.mjs` extracts quality matrix evidence from the child self-test report;
- the top-level rollup command entry now has `evidence.schema=v3.video_ppt_quality_matrix_rollup_evidence.v1`;
- the evidence includes `case_count=3`, `deliverable_count=1`, `pending_count=2`, `expectation_mismatch_count=0`;
- the evidence includes `review_required_risk_flag_count=8`, `review_required_risk_flag_object_shape_supported=true`, and `review_required_risk_flag_object_shape_case_count=8`;
- the rollup validator now fails if the quality matrix evidence is missing or incomplete.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted no-live report summary readback>"
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report>
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`;
- top-level safety gates remained `live_smoke_run=false`, `production_write_allowed=false`, `network_download_allowed=false`, `file_upload_allowed=false`, `browser_capture_allowed=false`, `service_deployment_allowed=false`, `server_8_touched=false`, and `server_120_touched=false`;
- embedded quality matrix evidence recorded `review_required_risk_flag_count=8`;
- embedded quality matrix evidence recorded `review_required_risk_flag_object_shape_supported=true`;
- embedded quality matrix evidence recorded `review_required_risk_flag_object_shape_case_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, or password-like values.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- future no-live child reports that become acceptance-critical should expose similarly narrow evidence in the top-level rollup instead of requiring operators to inspect raw child JSON.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Upload And External Trigger Evidence

Task source: M6AE follow-up after M6AD embedded quality-matrix evidence in the no-live rollup. The final video PPT feature depends on both main-site uploaded video and third-party registered video respecting the same "extract PPT/slides/courseware already shown in a video" trigger boundary. This slice embeds the upload-main and external-video-ppt self-test contract evidence into the same top-level no-live report.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, server deployment, or 8/120 server access;
- read only local child reports printed by upload-main and external-video-ppt self-tests;
- accept relative `target/` report paths, or absolute report paths only when they resolve under the current repo `target/` directory;
- copy only counts, booleans, action ids, and file-kind counts into the no-live report, not child report paths or raw JSON bodies.

Implemented behavior:

- no-live rollup extracts `v3.video_ppt_upload_main_rollup_evidence.v1` from the upload-main self-test report;
- no-live rollup extracts `v3.external_video_ppt_rollup_evidence.v1` from the third-party video PPT self-test report;
- upload evidence records `selected_scope_intent=video_ppt_extraction`, `deliverable_state=final_pptx_ready`, 6 required file kinds, 6 supported video extensions, shared trigger fixture version 1, 12 shared trigger cases, 6 positive prompts, 6 negative prompts, and minimum PPTX/Markdown download validation;
- external evidence records `trigger_text_requests_video_ppt=true`, ordinary-video-to-PPT guard enabled, requested `video_ppt_extraction` skill, expected `extract_video_ppt_transcript` action, registered-document scope fields, 6 supported video extensions, non-video rejection, shared trigger fixture counts, and minimum PPTX/Markdown download validation;
- rollup validation now fails if upload-main or external self-test evidence is missing or incomplete.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted no-live report evidence readback>"
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report>
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`;
- upload evidence recorded `selected_scope_intent=video_ppt_extraction`, `deliverable_state=final_pptx_ready`, `trigger_shared_fixture_case_count=12`, `positive_prompt_count=6`, `negative_prompt_count=6`, `download_validation_ok=true`, and all upload no-live safety booleans false;
- external evidence recorded `trigger_text_requests_video_ppt=true`, `default_prompt_guards_ordinary_video_to_ppt=true`, `requested_video_ppt_skill=true`, `expected_action=extract_video_ppt_transcript`, `trigger_shared_fixture_case_count=12`, `positive_prompt_count=6`, `negative_prompt_count=6`, `unsupported_non_video_extensions_rejected=true`, `download_validation_ok=true`, and all external no-live safety booleans false;
- quality matrix evidence from M6AD remained present in the same report with 8 review-required risk flags and object-shape support;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, or password-like values.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- future acceptance-critical child reports should expose similarly narrow evidence in the top-level no-live rollup.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Video PPT No-Live Rollup Gate

Task source: after the video PPT surface accumulated several independent no-live gates, the active plan needed a repeatable one-command rollup that proves the current local baseline without running live upload, third-party, handoff, capture, deployment, or customer-authorized actions.

Scope:

- add a consolidated no-live rollup script for video PPT extraction readiness;
- include syntax checks, self-tests, preflights, quality matrix review-risk regression, and deliverables validator tests;
- write a redacted local report with explicit safety gates;
- keep the rollup strictly local: no live DataMax calls, no video downloads, no uploads, no browser capture, no FFmpeg capture, no service deployment, and no 8/120 server access.

Implemented behavior:

- new npm entrypoint: `smoke:video-ppt-no-live-rollup`;
- new script: `scripts/smoke/video-ppt-no-live-rollup.mjs`;
- the rollup runs 15 commands covering:
  - main-site upload smoke syntax, self-test, and preflight;
  - third-party video PPT smoke syntax, self-test, and preflight;
  - login-gated video handoff syntax, self-test, and preflight;
  - authorized capture helper syntax and self-test;
  - quality matrix syntax and self-test;
  - deliverables validator syntax and tests.
- the report schema is `v3.video_ppt_no_live_rollup.v1`;
- report gates set `live_smoke_run=false`, `production_write_allowed=false`, `network_download_allowed=false`, `file_upload_allowed=false`, `browser_capture_allowed=false`, `service_deployment_allowed=false`, `server_8_touched=false`, and `server_120_touched=false`;
- the report refuses unredacted local paths, URLs, or token-like text.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report>
```

Result:

- script syntax check passed;
- no-live rollup passed with `command_count=15`, `passed_count=15`, and `failed_count=0`;
- generated report summary contained all 15 command ids and all safety gates set to the no-live values;
- report redaction scan found no local paths, URLs, token/cookie/bearer-like text, or session directories.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- use this rollup as the local precondition before any approved live smoke or deployment window.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no MP4 was captured or recorded;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Media Worker Gate

Task source: M6S created a no-live rollup for the video PPT surfaces, but the first rollup did not include the core Rust media-worker library tests or the offline smoke binary compile check. Since the final feature depends on media-worker extraction and artifact generation, those checks now belong in the same local pre-live gate.

Scope:

- extend `smoke:video-ppt-no-live-rollup` to include media-worker Rust coverage;
- keep the rollup no-live and local-only;
- preserve the same redacted report schema and safety gates.

Implemented behavior:

- added `media_worker_lib_tests`: `cargo test -p media-worker --lib`;
- added `media_worker_offline_smoke_bin_check`: `cargo check -p media-worker --bin video_ppt_offline_smoke`;
- updated the script help and README to describe media-worker coverage;
- the rollup now runs 17 commands.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report>
```

Result:

- script syntax check passed;
- no-live rollup passed with `command_count=17`, `passed_count=17`, and `failed_count=0`;
- generated report included `media_worker_lib_tests` and `media_worker_offline_smoke_bin_check`;
- report safety gates remained all no-live values;
- report redaction scan found no local paths, URLs, token/cookie/bearer-like text, or session directories.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- use the 17-command rollup as the current local precondition before any approved live smoke or deployment window.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no MP4 was captured or recorded;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Rust Format Gate

Task source: after M6T added media-worker Rust coverage to the no-live rollup, the same local pre-live gate still needed the Rust formatting check that earlier Rust quality slices used separately. Keeping `cargo fmt --check` in the one-command rollup reduces drift before GitHub sync and before any approved live smoke or deployment window.

Scope:

- extend `smoke:video-ppt-no-live-rollup` to include Rust workspace formatting;
- keep the rollup no-live and local-only;
- preserve the same redacted report schema and safety gates.

Implemented behavior:

- added `rust_format_check`: `cargo fmt --check`;
- updated the script help and README to describe the Rust format check;
- the rollup now runs 18 commands.

Validation:

```text
cargo fmt --check
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report>
```

Result:

- standalone Rust format check passed;
- script syntax check passed;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`;
- generated report included `rust_format_check`, `media_worker_lib_tests`, and `media_worker_offline_smoke_bin_check`;
- report safety gates remained all no-live values;
- report redaction scan found no local paths, URLs, token/cookie/bearer-like text, or session directories.

Remaining work:

- this does not replace live main-site upload, third-party live, deployed handoff, authorized capture, or customer-authorized quality matrix gates;
- use the 18-command rollup as the current local precondition before any approved live smoke or deployment window.

Safety result:

- no live smoke was run;
- no network source was fetched;
- no file was uploaded or registered in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no MP4 was captured or recorded;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 External Video PPT Self-Test Contract Hardening

Task source: main-site upload self-test already checked video extension classification and video PPT extraction intent, while the third-party self-test mostly checked reply/download surface. Since the final feature needs third-party video registration plus a special "extract PPT from the video" trigger, the third-party self-test now validates that request contract locally before any bearer-backed live run.

Scope:

- keep the change no-live and self-test only;
- validate the third-party event payload shape used after video document registration;
- verify the supported video extension classifier matches the main upload gate;
- preserve self-test report redaction.

Implemented behavior:

- added `assertSelfTestExternalContract()` to `scripts/smoke/external-video-ppt.mjs`;
- self-test now asserts the event text requests video PPT extraction;
- self-test now asserts the default prompt guards against turning ordinary video into an authored PPT;
- self-test now asserts `requested_skills[].skill_id=video_ppt_extraction` and `expected_action=extract_video_ppt_transcript`;
- self-test now asserts the event is scoped to the registered document/source/dataset identifiers;
- self-test now asserts `.mp4`, `.mov`, `.m4v`, `.webm`, `.mkv`, and `.avi` classify as supported videos while non-video extensions do not;
- self-test now records a redacted contract summary without source URLs, local paths, bearer values, or object keys.
- upload-main and external self-test redaction probes now build synthetic URL probes from string parts instead of storing a complete URL/query-token shape in source.

Validation:

```text
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:external-video-ppt -- --self-test --output-dir <target-redacted>
rg -n "<contract-fields>" <external-self-test-report>
rg -n "<local-path-url-token-patterns>" <external-self-test-report>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
```

Result:

- external video PPT script syntax check passed;
- external self-test passed;
- self-test report contained the contract summary fields for expected action, supported video extensions, non-video rejection, and source summary redaction;
- self-test report redaction scan found no local paths, URLs, token/cookie/bearer-like text, or session directories;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`.

Remaining work:

- this does not replace third-party live smoke; bearer, connection id, source id, and approved input are still required;
- this does not prove third-party artifact download visibility in production; it only hardens the no-live payload and contract gate.

Safety result:

- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Video PPT Special Trigger Classifier Guard

Task source: the product boundary is that video PPT means extracting PPT/slides/courseware already shown in a video, not turning any ordinary video into an authored PPT. The upload-main and third-party smoke trigger classifier was too broad because a prompt containing only PPT/slides could pass. This slice tightens the local release gates before any live upload or third-party run.

Scope:

- keep the change no-live and self-test only;
- apply the same trigger classifier behavior to main-site upload and third-party video PPT smokes;
- add positive and negative regression prompts;
- keep ordinary video-to-PPT creation prompts out of the video PPT extraction trigger path.

Implemented behavior:

- `promptRequestsVideoPpt()` now requires a PPT/slides/courseware mention plus extraction/from-video intent;
- ordinary video-to-PPT wording such as "把普通视频变成PPT" or "Create a PowerPoint from this ordinary video" is rejected unless the prompt clearly asks to extract already-shown slides;
- upload-main self-test now records a trigger classifier contract with positive and negative prompt counts;
- external self-test now records the same trigger classifier contract.

Validation:

```text
node --check scripts/smoke/video-ppt-upload-main.mjs
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:video-ppt-upload-main -- --self-test --output-dir <target-redacted>
npm run smoke:external-video-ppt -- --self-test --output-dir <target-redacted>
rg -n "triggerClassifier|positivePromptCount|negativePromptCount" <upload-and-external-self-test-reports>
rg -n "<local-path-url-token-patterns>" <upload-and-external-self-test-reports>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
```

Result:

- upload-main and external script syntax checks passed;
- upload-main self-test passed;
- external self-test passed;
- both self-test reports recorded `positivePromptCount=4` and `negativePromptCount=4`;
- upload-main and external self-test report redaction scans found no local paths, URLs, token/cookie/bearer-like text, or session directories;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`.

Remaining work:

- this does not replace main-site upload live smoke or third-party live smoke;
- live gates still require explicit write approval or inbound bearer/context.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Quality Matrix Review-Risk Flag Gate

Task source: EP6 local quality hardening after the public-course samples remained `needs_manual_review`. The quality matrix already respected several explicit review risks, but some risk flags produced by media-worker still relied on score or summary side effects. This slice makes all review-required video PPT risk flags explicit in the matrix self-test gate.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, server deployment, or 8/120 server access;
- update only the local quality matrix smoke;
- keep `screenshot_based_pptx` as a non-blocking informational flag;
- ensure review-required risk flags force `needs_manual_review` even when slide counts align and `quality_score=70`.

Implemented behavior:

- added a centralized `REVIEW_REQUIRED_RISK_FLAGS` set to `scripts/smoke/video-ppt-quality-matrix.mjs`;
- the set now includes `manual_review_required`, `missing_transcript_alignment`, `missing_ocr_evidence`, `full_frame_rectangle_fallback`, `selected_slide_duplicates_removed`, `frame_sharpness_review_required`, `slide_readability_review_required`, and `single_slide_output_review_required`;
- `evaluateCase()` now treats any of those risk flags as a manual-review signal;
- self-test internal regression now iterates every review-required risk flag, not just the original four;
- the self-test report exposes `review_required_risk_flags` and `review_required_risk_flag_count` so shared evidence shows which risks are guarded.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
rg -n "<sensitive-value-patterns>" <quality-matrix-and-rollup-reports>
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- quality matrix self-test passed with 3 visible cases, 1 deliverable synthetic case, and 2 pending cases;
- self-test report exposed `review_required_risk_flag_count=8`;
- exposed review-required risk flags were `manual_review_required`, `missing_transcript_alignment`, `missing_ocr_evidence`, `full_frame_rectangle_fallback`, `selected_slide_duplicates_removed`, `frame_sharpness_review_required`, `slide_readability_review_required`, and `single_slide_output_review_required`;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`;
- sensitive-value report scan found no raw URLs, token query strings, object keys, bearer values, local absolute paths, generated-artifacts paths, provider keys, or password-like values;
- `git diff --check` passed;
- tracked `target/` file count remained 0.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- customer-facing quality still depends on real authorized samples and manual review for crop, duplicate, subtitle, OCR, readability, and artifact visibility risks.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Shared Video PPT Trigger Fixture

Task source: M6Z in the active execution plan. After M6X/M6Y tightened production and smoke trigger guards, the same positive/negative prompt corpus still lived in several places. This slice makes the trigger regression corpus shared by upload-main smoke, third-party smoke, front-end scope planner tests, and Rust assistant-runtime tests.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, server deployment, or 8/120 server access;
- add one shared trigger fixture for video PPT prompts;
- wire upload-main and third-party smoke self-tests to that fixture;
- wire front-end scope planner and Rust assistant-runtime tests to that fixture;
- keep the production trigger boundary unchanged except for aligning the front-end English source hint with the already-supported Rust/smoke `video` source hint.

Implemented behavior:

- added `fixtures/video-ppt-trigger-classifier/trigger-cases.json` with `schema=v3.video_ppt_trigger_classifier_fixture.v1`;
- the fixture currently has 6 positive prompts and 6 negative prompts;
- positive prompts cover uploaded-video Chinese, generic video Chinese, courseware extraction Chinese, English `Extract slides from this video`, and file-name English `talk.mkv`;
- negative prompts cover video summary, ordinary video-to-PPT, English ordinary video-to-PPT, video-introduction PPT generation, subtitle-only extraction, and document slide extraction;
- upload-main and third-party smoke self-tests now report `fixtureVersion=1`, `sharedFixtureCaseCount=12`, `scriptSpecificCaseCount=0`, `positivePromptCount=6`, and `negativePromptCount=6`;
- front-end scope planner tests and Rust assistant-runtime tests now read the same fixture and assert the expected `media.extract_ppt_transcript` / `media.resolve_video_url` action list;
- front-end `VIDEO_PPT_SOURCE_PATTERN` now includes English `video`, matching the Rust and smoke source hints and keeping `Extract slides from this video.` positive without accepting ordinary video-to-PPT generation.

Validation:

```text
node --check scripts/smoke/video-ppt-upload-main.mjs
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:video-ppt-upload-main -- --self-test --output-dir <target-redacted>
npm run smoke:external-video-ppt -- --self-test --output-dir <target-redacted>
node --check apps/web/app/lib/scope-planner.js
node --test apps/web/app/lib/scope-planner.test.mjs
cargo test -p assistant-runtime --lib
cargo fmt --check
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
git diff --check
git ls-files target | wc -l
```

Result:

- upload-main script syntax check passed;
- external script syntax check passed;
- upload-main self-test passed and recorded shared fixture counts `12/6/6`;
- external self-test passed and recorded shared fixture counts `12/6/6`;
- front-end scope planner syntax check passed;
- front-end scope planner tests passed, 21 tests;
- Rust assistant-runtime library tests passed, 34 tests;
- Rust formatting check passed after mechanical `cargo fmt`;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`;
- sensitive-value report scan found no raw URLs, token query strings, object keys, bearer values, local absolute paths, generated-artifacts paths, provider keys, or password-like values;
- `git diff --check` passed;
- tracked `target/` file count remained 0.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- future prompt additions should go into the shared fixture first, then let upload-main smoke, external smoke, front-end tests, and Rust tests consume the same corpus;
- if production code is later refactored into a shared classifier implementation, that must be a separate slice with its own tests and rollout gate.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Production Scope Video PPT Trigger Guard

Task source: after the smoke trigger classifier was tightened, production scope planning still had broader video PPT intent checks. This slice moves the same product boundary into the front-end scope planner and Rust assistant-runtime recommendation path so `media.extract_ppt_transcript` is not recommended for ordinary video-to-PPT generation or transcript-only video requests.

Scope:

- no live upload, third-party event, video download, browser capture, or server deployment;
- tighten `apps/web/app/lib/scope-planner.js` video PPT intent detection;
- tighten `crates/assistant-runtime/src/lib.rs` video PPT recommendation detection;
- preserve supported positive triggers for direct video URL, uploaded video, `.mkv`/`.avi`, and English `Extract slides from this video` prompts.

Implemented behavior:

- video PPT extraction now requires video/source context, PPT/slides/courseware output, and extraction/from-video intent;
- ordinary video-to-PPT generation wording is rejected in the scope planner and assistant-runtime recommendation path;
- transcript-only video requests no longer recommend `media.extract_ppt_transcript`;
- direct URL and uploaded-video prompts that explicitly ask to extract existing slides/courseware still recommend `media.extract_ppt_transcript`.

Validation:

```text
cargo fmt
node --check apps/web/app/lib/scope-planner.js
node --test apps/web/app/lib/scope-planner.test.mjs
cargo test -p assistant-runtime video_ppt_scope --lib
cargo test -p assistant-runtime transcript_only_video_request --lib
cargo fmt --check
cargo test -p assistant-runtime --lib
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
```

Result:

- front-end scope planner tests passed, 21 tests;
- Rust assistant-runtime video PPT scope tests passed, 3 tests;
- Rust transcript-only targeted test passed;
- Rust formatting check passed.
- full Rust assistant-runtime library tests passed, 34 tests;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- smoke scripts and production scope planner now share the same local trigger boundary, but live gates still require the approvals documented in the active plan.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Smoke Classifier Parity With Production Trigger Guard

Task source: after M6X tightened production scope planning, the upload-main and third-party smoke classifiers still had a weaker local classifier. In particular, a prompt with generation wording plus unrelated subtitle extraction could still pass the smoke classifier, and `extract slides` without video context was not explicitly rejected. This slice aligns smoke self-tests with the production trigger boundary.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, server deployment, or 8/120 server access;
- require video/source context in both smoke classifiers;
- keep PPT/slides/courseware output and extraction/from-video intent required;
- add negative prompts for generic generation plus subtitle extraction, and extraction wording without video context.

Implemented behavior:

- `promptRequestsVideoPpt()` in upload-main and external smokes now requires video context, slide output, and extraction intent;
- ordinary video-to-PPT generation also rejects generic generation wording when it is not extracting already-shown slides;
- `生成PPT介绍这段视频，并提取字幕。` is rejected because the extraction term applies to subtitles, not already-shown PPT pages;
- `Extract slides from the document.` is rejected because it has no video context;
- positive prompts for uploaded/direct video PPT extraction still pass.

Validation:

```text
node --check scripts/smoke/video-ppt-upload-main.mjs
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:video-ppt-upload-main -- --self-test --output-dir <target-redacted>
npm run smoke:external-video-ppt -- --self-test --output-dir <target-redacted>
rg -n "triggerClassifier|positivePromptCount|negativePromptCount" <upload-and-external-self-test-reports>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
```

Result:

- upload-main and external script syntax checks passed;
- an initial self-test run correctly exposed that `生成PPT介绍这段视频，并提取字幕。` was still accepted;
- after the generic generation guard fix, upload-main self-test passed;
- after the generic generation guard fix, external self-test passed;
- both final self-test reports recorded `positivePromptCount=4` and `negativePromptCount=6`;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- live gates still require the approvals documented in the active plan.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, or parsed in DataMax;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Handoff And Capture Evidence

Task source: M6AF follow-up after the no-live rollup already embedded quality-matrix, upload-main, and external-video-ppt evidence. The remaining local acceptance gap was that the top-level no-live report proved the handoff and authorized-capture commands passed, but did not copy their critical safety evidence into the rollup itself.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, service deployment, or 8/120 server access;
- read only local child reports printed by the video PPT handoff self-test and authorized-capture self-test;
- accept relative `target/` report paths, or absolute report paths only when they resolve under the current repo `target/` directory;
- copy only redacted counts, booleans, status codes, and failure reasons into the no-live report, not child report paths or raw JSON bodies.

Implemented behavior:

- no-live rollup extracts `v3.video_ppt_handoff_rollup_evidence.v1` from the handoff self-test report;
- handoff evidence records WeChat/video slide-output prompt recognition, no network/provider/ReAct/video fetch/download/frame/OCR/PPT/success/download signals, main/external `login_gated_video_source_not_supported`, 3 actionable next steps in each mode, and 5 negative fixtures rejected;
- no-live rollup extracts `v3.authorized_capture_rollup_evidence.v1` from the authorized-capture self-test report;
- authorized-capture evidence records dry-run/self-test mode, approval and approved-by redaction, 6 authorization-gate negative cases rejected, `capture_attempted=false`, dry-run browser/FFmpeg intent only, no persistent browser profile, no cookie/HAR/QR capture, no credentials, and no provider payloads;
- rollup validation now fails if handoff or authorized-capture evidence is missing or incomplete.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted no-live report evidence readback>"
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=18`, `passed_count=18`, and `failed_count=0`;
- handoff evidence recorded `prompt_mentions_wechat_video=true`, `prompt_wants_slide_output=true`, `network_calls_run=false`, `provider_called=false`, `video_downloaded=false`, `frames_extracted=false`, `ocr_run=false`, `ppt_generated=false`, and `final_pptx_ready_exposed=false`;
- handoff evidence recorded both main and external `failure_reason=login_gated_video_source_not_supported`, `next_step_count=3`, `negative_fixture_count=5`, and `negative_fixtures_rejected=5`;
- authorized-capture evidence recorded `mode=dry-run`, `self_test=true`, approval and approved-by redaction present, `authorization_gate_negative_case_count=6`, and `authorization_gate_negative_cases_rejected=6`;
- authorized-capture evidence recorded `capture_attempted=false`, `dry_run=true`, `browser_would_run=true`, `ffmpeg_would_run=true`, `cookies_stored=false`, `har_stored=false`, `qr_screenshot_stored=false`, `credentials_included=false`, and `provider_payloads_included=false`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, raw self-test approval values, or raw self-test operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- M6AF is a no-live evidence hardening slice only; full video PPT acceptance still requires the live/customer/deployment gates documented in the active plan.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Production Trigger Tests

Task source: M6AH follow-up after M6AF embedded handoff and authorized-capture evidence. The one-command no-live baseline still did not execute the production trigger boundary tests from the front-end scope planner and Rust assistant-runtime, so a green no-live rollup did not itself prove that ordinary video-to-PPT generation and transcript-only video prompts remain rejected in production recommendation paths.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, service deployment, or 8/120 server access;
- extend only `scripts/smoke/video-ppt-no-live-rollup.mjs`;
- add production trigger boundary checks to the same no-live rollup command;
- keep existing upload, external, handoff, authorized-capture, quality-matrix, media-worker, and deliverables-validator gates unchanged.

Implemented behavior:

- no-live rollup now runs `node --check apps/web/app/lib/scope-planner.js`;
- no-live rollup now runs `node --test apps/web/app/lib/scope-planner.test.mjs`;
- no-live rollup now runs `cargo test -p assistant-runtime video_ppt_scope --lib`;
- no-live rollup command count increased from 18 to 21;
- the existing five embedded evidence schemas remain present for upload-main, external-video-ppt, handoff, authorized-capture, and quality-matrix self-tests.

Validation:

```text
node --check apps/web/app/lib/scope-planner.js
node --test apps/web/app/lib/scope-planner.test.mjs
cargo test -p assistant-runtime video_ppt_scope --lib
cargo test -p assistant-runtime transcript_only_video_request --lib
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted no-live report summary readback>"
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- front-end scope planner syntax check passed;
- front-end scope planner tests passed with 21 tests;
- Rust assistant-runtime video PPT scope tests passed with 3 tests;
- Rust transcript-only targeted test passed with 1 test;
- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=21`, `passed_count=21`, and `failed_count=0`;
- no-live report recorded `scope_planner_syntax=passed`, `scope_planner_tests=passed`, and `assistant_runtime_video_ppt_scope_tests=passed`;
- embedded evidence schemas remained present: `v3.video_ppt_upload_main_rollup_evidence.v1`, `v3.external_video_ppt_rollup_evidence.v1`, `v3.video_ppt_handoff_rollup_evidence.v1`, `v3.authorized_capture_rollup_evidence.v1`, and `v3.video_ppt_quality_matrix_rollup_evidence.v1`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, raw self-test approval values, or raw self-test operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- M6AH only strengthens the no-live baseline so production trigger boundaries are checked together with smoke trigger boundaries.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Production Trigger Evidence

Task source: M6AI follow-up after M6AH added production trigger tests to the one-command no-live rollup. The rollup executed the front-end and Rust production trigger tests, but the top-level report still only showed command status. This slice copies a narrow, redacted summary of the test counts and critical positive/negative coverage points into the rollup evidence.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, service deployment, or 8/120 server access;
- read only stdout from the local scope planner and assistant-runtime production trigger tests;
- copy only counts and boolean coverage fields into the no-live report;
- do not store raw test stdout, prompt text bodies, local paths, source URLs, credentials, provider payloads, generated artifacts, or customer data in the report.

Implemented behavior:

- no-live rollup extracts `v3.scope_planner_video_ppt_rollup_evidence.v1` from the front-end TAP output;
- scope planner evidence records total/pass/fail/cancelled/skipped/todo counts and booleans for direct video, public page video, uploaded video, mkv/avi video, transcript-only negative, shared negative fixture, and shared positive fixture coverage;
- no-live rollup extracts `v3.assistant_runtime_video_ppt_scope_rollup_evidence.v1` from the Rust test output;
- assistant-runtime evidence records total/pass/fail/ignored/measured/filtered counts and booleans for transcript-only negative, shared negative fixture, and shared positive fixture coverage;
- rollup validation now fails if either production trigger evidence block is missing or incomplete.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted production trigger evidence readback>"
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=21`, `passed_count=21`, and `failed_count=0`;
- scope planner evidence recorded `test_count=21`, `pass_count=21`, `fail_count=0`, `cancelled_count=0`, `skipped_count=0`, and `todo_count=0`;
- scope planner evidence recorded direct video, public video page, uploaded video, mkv/avi, transcript-only negative, shared negative fixture, and shared positive fixture coverage as `true`;
- assistant-runtime evidence recorded `test_count=3`, `pass_count=3`, `fail_count=0`, `ignored_count=0`, `measured_count=0`, and `filtered_out_count=31`;
- assistant-runtime evidence recorded transcript-only negative, shared negative fixture, and shared positive fixture coverage as `true`;
- the existing upload-main, external-video-ppt, handoff, authorized-capture, and quality-matrix evidence schemas remained present in the same report;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, raw self-test approval values, or raw self-test operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- M6AI only makes the no-live production trigger evidence copyable and self-validating.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Live Preflight Evidence

Task source: M6AJ follow-up after the no-live rollup had self-test evidence and production trigger evidence. The upload-main, external-video-ppt, and handoff preflight commands already produced local JSON reports, but the top-level no-live rollup only recorded command status. This slice copies narrow, redacted live-gate evidence into the rollup so reviewers can verify live authorization boundaries without opening child reports.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, service deployment, or 8/120 server access;
- read only local preflight reports emitted under `target/`;
- copy only counts, booleans, status codes, and redaction flags into the no-live report;
- do not store raw URLs, bearer values, cookies, local paths, object keys, provider payloads, generated artifacts, or customer data in the shared report.

Implemented behavior:

- no-live rollup extracts `v3.video_ppt_upload_main_preflight_rollup_evidence.v1`;
- upload-main preflight evidence proves live write approval is required, no network/upload/dataset/document/assistant-run action occurred, fixture shape is video/PPT capable, planned write scope is smoke-only, no deploy is planned, and redaction flags are safe;
- no-live rollup extracts `v3.external_video_ppt_preflight_rollup_evidence.v1`;
- external preflight evidence proves bearer/context gating, no network/register/event/reply/download action occurred, requested skill/action and document/dataset scope are present, write scope is smoke-only, no deploy is planned, and redaction flags are safe;
- no-live rollup extracts `v3.video_ppt_handoff_preflight_rollup_evidence.v1`;
- handoff preflight evidence proves both main/external modes are represented, deployment approval is required, no source fetch/download/frame/OCR/PPT/provider action occurred, expected failure reason and three next steps are fixed, success signals are rejected, and redaction flags are safe;
- rollup validation now fails if any of the three preflight evidence blocks is missing or incomplete.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted preflight evidence readback>"
rg -n "<local-path-url-token-patterns>" <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=21`, `passed_count=21`, and `failed_count=0`;
- upload-main preflight evidence recorded `live_write_approval_required=true`, `network_calls_run=false`, `production_write_allowed=false`, `upload_attempted=false`, `dataset_created=false`, `document_registered=false`, `assistant_run_created=false`, `fixture_media_kind=video`, `fixture_supported_extension=true`, `trigger_prompt_requests_video_ppt=true`, `planned_step_count=7`, and `deploys_services=false`;
- external preflight evidence recorded `live_write_approval_required=true`, `credential_gate_satisfied=true`, `live_credential_ready=false`, `allow_missing_bearer=true`, `network_calls_run=false`, `fixture_registered=false`, `event_sent=false`, `reply_polled=false`, `deliverables_downloaded_from_network=false`, `requested_video_ppt_skill=true`, `expected_action=extract_video_ppt_transcript`, `planned_step_count=5`, and `deploys_services=false`;
- handoff preflight evidence recorded `mode=both`, `target_mode_count=2`, `deployment_approval_required=true`, `network_calls_run=false`, `source_page_fetched=false`, `video_downloaded=false`, `frames_extracted=false`, `ocr_run=false`, `ppt_generated=false`, `provider_called=false`, `failure_reason=login_gated_video_source_not_supported`, `supported_next_step_count=3`, and `success_signal_rejected_count=4`;
- all preflight redaction flags for raw URLs, cookies, bearer values, local paths, object keys, and provider payloads remained false;
- total top-level evidence schema count was 10;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, raw self-test approval values, or raw self-test operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- M6AJ only makes live-gate preflight evidence copyable and self-validating before any authorized live run.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Complete Executable Plan Plan-Only Rollup

Task source: user requested the previous planning step be completed as a full executable plan, with the explicit boundary that this is a plan and code should not be changed.

Scope:

- plan-only documentation update;
- no business code, smoke script, validator, worker, API, web app, or deployment change;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented documentation behavior:

- `docs/plans/datamax-active-execution-plan.md` now adds section `0.7.18 2026-06-08 完整可执行计划收口版`;
- the new section is the current short execution entry and separates P0 plan-only, P1 no-live baseline, P2 main upload live smoke, P3 third-party live smoke, P4 video号/login-gated handoff, P5 authorized capture fallback, P6 quality matrix, P7 8-server deployment, and P8 full acceptance close;
- the plan restates that video PPT means extracting already-playing slides/courseware from a video, not creating an authored PPT from ordinary video;
- the plan explicitly keeps WeChat Video Channels/login-gated/private playback sources on the handoff path unless a direct anonymous video file or authorized recording exists;
- the plan includes an 8-server internal recording research path, but keeps it disabled by default and gated behind separate approval, read-only audit, isolation, retention, cleanup, and redaction requirements;
- the plan keeps GitHub sync separate from 8-server deployment.

Validation to run for this plan-only slice:

```text
cmp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
git diff --check
git diff --name-only
git diff --cached --name-only
```

Expected result:

- repository plan and desktop plan copy match;
- whitespace check passes;
- changed/staged files are documentation only;
- no generated `target/` files are tracked.

Remaining work:

- this plan-only rollup does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- next implementation work should follow the P0-P8 path selector in section `0.7.18`, starting with P1 no-live work if no new authorization is available.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Authorized Capture Dry-Run Evidence

Task source: P1 no-live follow-up from section `0.7.18`, after the plan identified authorized-capture dry-run evidence as the next local slice available without live, customer, or deployment authorization.

Scope:

- no live upload, third-party event, video download, browser capture, FFmpeg capture, service deployment, or 8/120 server access;
- add one no-live rollup command that runs `capture-authorized-video` in dry-run mode only;
- pass the dry-run approval id, approved-by, source URL, and purpose through child-process environment so the top-level no-live report command field does not store raw URL or approval values;
- copy only the helper report's redacted `sharedReceipt` fields and safety booleans into the no-live rollup report;
- do not store raw source URLs, local report paths, browser profile paths, output MP4 paths, approval values, provider payloads, cookies, HAR files, QR screenshots, or handoff command text in shared rollup evidence.

Implemented behavior:

- no-live rollup now runs `authorized_capture_dry_run`;
- the command uses `--dry-run --ack-authorized --duration-seconds 30 --handoff upload-main`;
- the top-level report embeds `v3.authorized_capture_dry_run_rollup_evidence.v1`;
- embedded dry-run evidence records `mode=dry-run`, `source_scheme=https`, source host presence, `duration_seconds=30`, `retention_days=7`, `capture_audio_allowed=false`, `handoff=upload-main`, approval/reference redaction booleans, output file name, local/profile path redaction booleans, and no-cookie/HAR/QR safety booleans;
- rollup validation now fails if the dry-run evidence is missing, if browser/FFmpeg capture was attempted, if handoff is not `upload-main`, or if any raw URL/local path/approval/credential/provider redaction flag is unsafe.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted authorized-capture dry-run evidence readback>"
rg -n "<local-path-url-token-approval-patterns>" <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- `authorized_capture_dry_run` passed;
- dry-run evidence schema was `v3.authorized_capture_dry_run_rollup_evidence.v1`;
- dry-run evidence recorded `mode=dry-run`, `duration_seconds=30`, `retention_days=7`, `capture_audio_allowed=false`, `handoff=upload-main`, `handoff_mode=upload-main`, and `output_file_name=authorized-capture.mp4`;
- dry-run evidence recorded `capture_attempted=false`, `dry_run=true`, `browser_would_run=true`, and `ffmpeg_would_run=true`, meaning no browser or FFmpeg process was actually run by the dry-run;
- dry-run evidence recorded approval and approved-by references as present and redacted;
- dry-run evidence recorded `raw_url_stored=false`, `cookies_stored=false`, `har_stored=false`, `qr_screenshot_stored=false`, `approval_values_included=false`, `raw_source_url_included=false`, `local_paths_included=false`, `credentials_included=false`, and `provider_payloads_included=false`;
- top-level evidence schema count increased to 11;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, raw self-test approval/operator values, or raw dry-run approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, or customer-authorized quality matrix;
- M6AK only proves the dry-run authorization receipt and upload-main handoff planning path can be represented safely in the no-live rollup.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Plan-Only Acceptance Status Matrix Rollup

Task source: user asked to continue and complete the previous full executable planning step, with the explicit boundary that this is still a plan and code should not be changed.

Scope:

- plan-only documentation update;
- no business code, smoke script, validator, worker, API, web app, or deployment change;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented documentation behavior:

- `docs/plans/datamax-active-execution-plan.md` now marks M6AL as a plan-only acceptance status matrix;
- section `0.7.18` now includes a full P0-P8 acceptance matrix that separates completed plan/no-live gates from pending live, customer, capture, and deployment gates;
- the matrix records required inputs, completion evidence, and current blockers for P2 main-site upload live, P3 third-party live, P4 video号/login-gated handoff live, P5 authorized capture live sample, P6 customer quality matrix, P7 8-server deployment, and P8 final acceptance;
- the plan adds a stage-by-stage next execution cadence and a single-gate operator request template;
- the plan explicitly states that unverified script/code changes must not be mixed into P0 plan-only commits.

Validation for this plan-only slice:

```text
cmp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
git diff --check
git diff --cached --name-only
git diff --cached --stat
git ls-files target | wc -l
```

Expected result:

- repository plan and desktop plan copy match;
- whitespace check passes;
- staged files are documentation only;
- no generated `target/` files are tracked.

Remaining work:

- this plan-only rollup does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- if the planned `acceptance_status` field is later added to the no-live rollup JSON, it must be handled as a separate P1 code slice with `node --check`, full no-live rollup, redaction scan, and validation ledger evidence.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 No-Live Rollup Acceptance Status Evidence

Task source: M6AM follow-up after the plan-only M6AL matrix. This slice turns the plan's acceptance-status matrix into a top-level no-live rollup report field so a green local rollup cannot be misread as full live/customer/deployment acceptance.

Scope:

- local no-live code slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- no-live rollup now emits `acceptance_status` with schema `v3.video_ppt_acceptance_status_rollup.v1`;
- the status records `full_acceptance_ready=false`, `current_phase=no_live_local_baseline`, and exact no-live command/pass/fail counts;
- the status records 8 gates: P1 no-live baseline passed, P2 main upload pending authorization, P3 external video PPT pending credentials, P4 login-gated handoff pending deployment approval, P5 authorized capture live sample pending authorization, P6 customer quality matrix pending customer input, P7 server deployment pending deployment approval, and P8 full acceptance pending live/customer gates;
- every live/customer/deployment gate records `no_live_substitute_available=false` and nonempty `requires`;
- the status records safety flags showing no live smoke, production write, browser capture, service deployment, 8-server action, or 120-server action occurred;
- rollup validation now fails if the acceptance status schema, gate count, exact pending counts, P1 status, pending gate requirements, or safety flags are incomplete.

Validation:

```text
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted acceptance status readback>"
rg -n "<local-path-url-token-approval-patterns>" <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- no-live rollup syntax check passed;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- acceptance status schema was `v3.video_ppt_acceptance_status_rollup.v1`;
- acceptance status recorded `full_acceptance_ready=false`, `current_phase=no_live_local_baseline`, `no_live_status=passed`, `gate_count=8`, and `completed_gate_count=1`;
- pending counts were exact: authorization 2, credentials 1, deployment 2, customer 1, full acceptance 1;
- P1 was `passed` and marked as the only no-live-substitutable gate;
- P2, P3, P4, P5, P6, P7, and P8 were all pending with `no_live_substitute_available=false` and nonempty requirements;
- safety flags recorded no live smoke, no production write, no browser capture, no service deployment, no 8-server touch, and no 120-server touch;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, raw self-test approval/operator values, or raw dry-run approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- the acceptance status is deliberately `full_acceptance_ready=false` until those live/customer/deployment gates have real evidence or explicit non-executable reasons.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.

## 2026-06-08 Quality Matrix Not-Deliverable Failure Taxonomy

Task source: M6AN no-live follow-up. The quality matrix could already distinguish deliverable, needs manual review, pending accessible sample, and pending authorization, but customer/operator samples that fail later need stable failure classes rather than a generic extraction failure.

Scope:

- local quality-matrix and no-live rollup code slice only;
- no live main-site upload;
- no third-party live event;
- no bearer/context use;
- no new video download, frame extraction, OCR, PPT generation, browser capture, or FFmpeg capture;
- no 8-server pull/build/restart/deploy and no 120-server action.

Implemented behavior:

- quality matrix `evaluateCase()` now attaches a `failure_class` to pending, review, not-deliverable, and deliverable evaluations;
- self-test still reports the same three primary P2-2E cases, but now exposes a `not_deliverable_failure_classes` gate;
- internal self-test regressions cover five not-deliverable classes: `source_access`, `video_has_no_ppt`, `frame_extraction`, `artifact_visibility`, and `selection_quality`;
- no-live rollup copies the failure taxonomy from the quality-matrix child report into `v3.video_ppt_quality_matrix_rollup_evidence.v1`;
- no-live rollup validation now fails if the taxonomy count or any of the five required failure classes are missing.

Validation:

```text
node --check scripts/smoke/video-ppt-quality-matrix.mjs
node --check scripts/smoke/video-ppt-no-live-rollup.mjs
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty --output-dir <target-redacted>
npm run smoke:video-ppt-no-live-rollup -- --self-test --pretty --output-dir <target-redacted>
node -e "<redacted taxonomy evidence readback>"
rg -n "<local-path-url-token-approval-patterns>" <quality-matrix-report-dir> <no-live-rollup-report-dir>
git diff --check
git ls-files target | wc -l
```

Result:

- quality matrix syntax check passed;
- no-live rollup syntax check passed;
- quality matrix self-test passed with `case_count=3`, `deliverable_count=1`, `pending_count=2`, and `not_deliverable_count=0`;
- quality matrix report exposed `not_deliverable_failure_class_count=5`;
- quality matrix report exposed `source_access`, `video_has_no_ppt`, `frame_extraction`, `artifact_visibility`, and `selection_quality`;
- no-live rollup passed with `command_count=22`, `passed_count=22`, and `failed_count=0`;
- no-live quality evidence copied the same five failure classes and retained `review_required_risk_flag_count=8`;
- no-live quality evidence retained `live_smoke_run=false`, `production_write_allowed=false`, and `generated_artifacts_committable=false`;
- acceptance status stayed `full_acceptance_ready=false` with `gate_count=8`;
- report redaction scan found no raw URLs, token query strings, bearer values, local absolute paths, generated-artifacts paths, provider keys, password-like values, approval ids, or raw approval/operator values;
- `git diff --check` passed;
- `git ls-files target | wc -l` returned `0`.

Remaining work:

- this taxonomy does not replace main-site upload live smoke, third-party live smoke, video号 handoff live pass, authorized capture live sample, 8-server deployment validation, or customer-authorized quality matrix;
- additional live/customer failures may add classes later, but new classes must be represented in quality matrix self-test and no-live rollup evidence before being used in shared reports.

Safety result:

- no live upload was run;
- no live third-party event was posted;
- no bearer was used;
- no network source was fetched;
- no file was uploaded, registered, downloaded, OCRed, frame-extracted, or converted through a live workflow;
- no browser was opened and no FFmpeg capture command was run;
- no service build, restart, 8-server deployment, or 120-server action was run;
- generated reports stayed under `target/` and were not committed;
- no generated artifacts, raw videos, frames, PPTX files, customer files, raw source URLs, bearer values, cookies, provider payloads, database URLs, private object paths, approval ids, validator JSON body, report body, or local artifact paths were recorded in this shared receipt.
