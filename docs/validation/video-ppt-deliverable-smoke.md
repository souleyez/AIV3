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
