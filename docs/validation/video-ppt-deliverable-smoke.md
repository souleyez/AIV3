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

The public deliverable contract now also requires `slide_rectangles_manifest.json` for complete video/PPT packages. The current promoted mode is intentionally conservative: selected keep-list frames are de-duplicated in selection order, exact duplicate selected frame bytes are removed before PPTX generation, and decodable JPEG/PNG frames also pass through a conservative visual-similarity dedupe to reject near-identical selected frames caused by compression or tiny capture differences. Remaining JPEG/PNG frames may be exported as `border_background_contrast_v2` detector crops when a clear non-background rectangle exists, refined as `foreground_component_v1` crops when a dominant connected slide component should exclude external foreground overlays, or `edge_projection_v1` crops when a non-uniform background makes the median-background crop unsafe but strong rectangular edge lines remain. The public validator still accepts older `simple_background_contrast_v1` manifests. Ambiguous or undecodable frames remain relative full-frame fallback crops. Every crop still requires `review_required=true`.

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
