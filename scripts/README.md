# scripts

Bootstrap, smoke, and developer automation scripts live here.

Current smoke entrypoints:

- `run-assistant-run-worker-smoke.sh`: non-destructive Linux/deployment-target smoke for the AssistantRun background completion worker queue, workflow registration, and redacted video completion dispatch contract.
- `run-assistant-chat-contract-smoke.sh`: non-destructive local/deployment-target smoke for the user-facing chat contract: ordinary chat stays unrestricted and V3-aware, ReAct fallbacks stay natural-language only, and external callback payloads do not expose internal observability fields.
- `run-static-page-quality-smoke.sh`: non-destructive local/deployment-target smoke for static-page data quality: evidence field candidates, media candidates, section-title hints, preview data gates, and Codex plan-only quality gates.
- `run-static-page-render-smoke.sh`: non-destructive local/deployment-target smoke that generates a representative static-page artifact and validates the final HTML, DOM/SVG chart fallback, safe ECharts JSON hydration island, manifest, written handoff files, standalone export-artifact validator, and export package contract.
- `run-static-page-browser-smoke.sh`: non-destructive local/deployment-target smoke that opens the generated static-page `index.html` in Chrome desktop/mobile viewports, captures screenshots, and checks visible modules, SVG/ECharts presentation, horizontal overflow, module overlap, text overflow, mobile module order, and delivery-ready manifest state.
- `run-ingest-runtime-gate.sh`: non-destructive Linux/deployment-target gate for ingest fallback dependencies; it checks the service `PYTHON_BIN -m markitdown --version` path and fails when the pinned MarkItDown fallback version is missing.
- `run-document-understanding-smoke.sh`: non-destructive local/deployment-target smoke for PaddleOCR parser contracts, low-quality PDF gating, third-party parse compatibility, and resume company/entity scan rules. Set `DOCUMENT_UNDERSTANDING_SMOKE_RUNTIME_GATE=true` to also run the deployment ingest runtime gate.
- `run-jump-host-codex-shim-smoke.ps1`: jump-host smoke for the Codex host shim.
- `run-jump-host-video-deliverable-smoke.ps1`: jump-host smoke for video/PPT deliverable validation.

Static-page handoff checks:

- `npm run test:static-page-export-artifact`: unit coverage for the reusable static-page export artifact validator.
- `npm run validate:static-page-export-artifact -- --artifact target/static-page-render-smoke/<artifact-dir>`: validates a generated static-page export directory outside the smoke runner.

External integration checks:

- `bash scripts/run-external-direct-reply-smoke.sh`: Rust smoke coverage for the external ordinary-chat direct-reply contract, provider fallback/timeout behavior, SSE started/delta/completed semantics, and low-quality PDF parse gating.
- `npm run test:pure-third-party-guide-html`: checks the current public HTML/Markdown integration guides and their published `/external-integrations/...` copies.
