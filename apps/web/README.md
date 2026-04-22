# apps/web

Next.js experience layer for AI Data Platform V3.

Current scope:

- reuse the old smart-assistant shell instead of reusing the old controller
- sidebar dataset selection and in-place dataset creation
- dataset-scoped chat session list, message polling, dataset outputs, and published reports
- host-side `chat_session.report_entry` gate wired into the UI
- a thin `/api/v3/*` proxy that forwards to `platform-api /v1/*`

Run notes:

- web dev server defaults to `http://127.0.0.1:3100`
- Rust `platform-api` still defaults to `http://127.0.0.1:3000`
- override backend target with `PLATFORM_API_BASE_URL`
