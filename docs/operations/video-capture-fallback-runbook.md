# DataMax Video Capture Fallback Runbook

**Status:** P2-1 operator runbook; isolated MVP only.
**Last updated:** 2026-06-07 22:52 CST

## Scope

This fallback exists only for video PPT extraction requests where all preferred paths failed:

- the user cannot upload the video file;
- the third-party system cannot provide an anonymous direct video URL;
- the public page does not expose a direct video asset in HTML metadata;
- an operator has explicitly approved short, bounded capture of a playable page.

The captured MP4 is treated as an ordinary video input after review. DataMax still extracts PPT, slides, and courseware already shown inside the video. This workflow must not be used to turn arbitrary ordinary video into authored PPT.

## Non-Goals

- Do not bypass platform access controls.
- Do not request or store account passwords, cookies, QR screenshots, browser storage, HAR files, playback API payloads, or private player internals.
- Do not run capture automatically from normal chat.
- Do not integrate with the 8-server production services until a separate deployment review approves it.
- Do not claim DataMax has watched the source or generated PPT before the MP4 exists and the normal video extraction pipeline succeeds.

## Required Approval Record

Every capture job must have an approval record before dry-run or live capture:

| Field | Required | Notes |
| --- | --- | --- |
| `approval_id` | yes | Stable operator approval id, for example `APPROVAL-20260607-001`. |
| `approved_by` | yes | Operator name, ticket id, or approved workflow name. |
| `source_host_redacted` | yes | Store host and minimal path summary only; do not store the full source URL in shared logs. |
| `purpose` | yes | Why upload/direct URL/public resolver cannot be used. |
| `max_duration_seconds` | yes | Default 60 seconds; hard script limit 300 seconds. |
| `capture_audio_allowed` | yes | Default false. Audio requires explicit approval. |
| `retention_days` | yes | 0-7 days. Use the shortest practical value. |
| `cleanup_policy` | yes | Browser profile always deleted unless operator is debugging locally; failed output deleted unless explicitly retained. |
| `handoff_to_datamax` | yes | `upload-main`, `external-video`, or `none` for offline review only. |

## Preferred Operator Flow

1. Ask again for upload or anonymous direct video URL first.
2. Confirm the page is authorized for operator playback and short capture.
3. Run the dry-run command and inspect the redacted report.
4. Run live capture only on an operator-controlled workstation or jump host.
5. Review the MP4 manually for source and duration correctness.
6. Feed the MP4 into the normal DataMax video path:
   - main-site upload smoke / upload UI, or
   - third-party video registration smoke.
7. Run the normal video PPT extraction validation.
8. Delete temporary browser profile and captured files according to the approval record.

## Commands

Self-test, no browser and no FFmpeg:

```bash
npm run capture:authorized-video -- --self-test
```

Dry-run, no browser and no FFmpeg:

```bash
npm run capture:authorized-video -- \
  --dry-run \
  --ack-authorized \
  --approval-id APPROVAL-20260607-001 \
  --approved-by operator-name \
  --url https://example.com/authorized-video-page \
  --purpose "authorized courseware video; upload/direct URL unavailable" \
  --duration-seconds 60 \
  --handoff upload-main
```

macOS live capture example:

```bash
npm run capture:authorized-video -- \
  --run-capture \
  --ack-authorized \
  --approval-id APPROVAL-20260607-001 \
  --approved-by operator-name \
  --url https://example.com/authorized-video-page \
  --purpose "authorized courseware video; upload/direct URL unavailable" \
  --browser-bin "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --capture-mode macos-avfoundation \
  --ffmpeg-input "1:none" \
  --duration-seconds 60 \
  --handoff upload-main
```

Linux controlled-host example:

```bash
npm run capture:authorized-video -- \
  --run-capture \
  --ack-authorized \
  --approval-id APPROVAL-20260607-002 \
  --approved-by operator-name \
  --url https://example.com/authorized-video-page \
  --purpose "authorized courseware video; upload/direct URL unavailable" \
  --browser-bin google-chrome \
  --capture-mode linux-x11grab \
  --ffmpeg-input "${DISPLAY:-:0.0}" \
  --duration-seconds 60 \
  --handoff external-video
```

The script prints a handoff command after capture. Review the MP4 before running that command.

## Report Contract

The script writes a local report under `target/authorized-capture-smoke/`. The full local report may contain temporary paths because it is not committed or copied into shared validation logs. For shared validation records, copy only the report's `sharedReceipt` object and the high-level pass/fail result. `sharedReceipt` redacts approval values, local paths, and the full source URL.

- approval reference present/hash only, not the raw approval id;
- approved-by reference present/hash only, not the raw operator value;
- source host summary;
- duration seconds;
- capture mode;
- file name;
- byte size;
- hash prefix;
- handoff mode;
- cleanup result;
- follow-up extraction status.

Do not copy full source URLs, browser profile paths, local object paths, cookies, tokens, QR images, HAR files, provider payloads, or full customer documents into shared docs.

## Failure Split

| Failure | Meaning | Next Action |
| --- | --- | --- |
| `approval_missing` | No explicit operator authorization. | Stop. Collect approval record. |
| `browser_launch_failed` | Isolated browser could not open. | Fix local workstation/browser path; do not switch to user profile. |
| `playback_unavailable` | Operator cannot play the source. | Ask user for upload/direct URL or different authorized source. |
| `capture_failed` | FFmpeg/system capture failed. | Adjust capture device/input locally; do not change DataMax pipeline. |
| `capture_too_long` | Requested duration exceeds approval or hard limit. | Shorten clip or get new approval. |
| `video_has_no_ppt` | Captured video is playable but contains no slide/courseware frames. | Return unsupported content result; do not fabricate PPT. |
| `extraction_failed` | MP4 exists but normal VideoExtraction failed. | Debug normal video pipeline with the MP4 as an ordinary fixture. |
| `quality_insufficient` | PPTX/Markdown generated but crop/dedupe/slide count is poor. | Enter P2-2 quality enhancement. |

## 8-Server Gate

8-server capture is disabled by default and not part of this MVP.

Minimum review requirements before any 8-server evaluation:

- explicit user approval for 8-server deployment review;
- `CAPTURE_FALLBACK_ENABLED=false` remains default;
- concurrent capture limit is 1;
- Xvfb/PipeWire/PulseAudio/FFmpeg dependencies reviewed without service restart;
- temp directory and retention cleanup proven;
- logs redact source URLs and never store cookies/storage/HAR;
- rollback plan does not touch existing AIV3 workers unless separately approved.

Until those requirements are approved, use only operator workstation or controlled jump-host capture.

## Acceptance

P2-1 is acceptable only when all of the following are true:

- runbook is present and reviewed;
- script self-test and dry-run pass;
- live capture is attempted only with an approval id;
- capture report is redacted;
- temporary browser profile is deleted by default;
- MP4 handoff reuses the existing main upload or third-party video registration path;
- no production service is deployed, restarted, or reconfigured without a separate approval.
