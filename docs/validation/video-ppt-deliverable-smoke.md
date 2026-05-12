# Video/PPT Deliverable Smoke

Use this checklist for controlled video/PPT extraction validation. It is meant to prove the V3 deliverable contract, not to bypass video-platform access limits.

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
- `extraction_artifacts_manifest.json`
- `slide_notes.md`
- `subtitle_page_map.json`
- final manifest status flags and output groups
- extraction manifest file-kind coverage
- public JSON redaction for local paths and token-like URLs

## Jump-Host Smoke Check

Real Codex validation belongs on `windows-jump` or the later Mac host. Do not run real `codex exec` on the developer workstation.

After a jump-host media smoke produces a `video-extraction-<document_id>` session directory, run the validator against that session directory or its `generated_artifacts` child:

```powershell
npm run validate:video-deliverables -- "C:\Users\soulz\codex-host\tasks\<task>\video-extraction-<document_id>"
```

For machine-readable output:

```powershell
npm run validate:video-deliverables -- "C:\Users\soulz\codex-host\tasks\<task>\video-extraction-<document_id>" --json
```

Expected result:

```text
OK video deliverables: ...
ok pptx video_slides_screenshot_based.pptx ...
ok final_deliverables_manifest final_deliverables_manifest.json ...
ok extraction_artifacts_manifest extraction_artifacts_manifest.json ...
ok slide_notes slide_notes.md ...
ok subtitle_page_map subtitle_page_map.json ...
```

Do not commit generated smoke artifacts, local task directories, provider keys, `.storage`, or raw logs. If the validator fails, fix the first concrete contract gap in the worker/API/UI path and rerun the smallest relevant tests.
