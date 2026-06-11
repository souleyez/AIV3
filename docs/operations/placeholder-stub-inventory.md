# Placeholder / Stub Inventory

Date: 2026-06-11

Purpose: classify current placeholder and stub surfaces before replacing anything. This document is an inventory only; no runtime behavior is changed by this classification.

## Classification

- `test_only`: test fixture or deterministic unit-test helper; must not be interpreted as production behavior.
- `safe_fallback`: production code path that intentionally returns bounded fallback output; customer-visible text must clearly signal limited capability or use provider-authored runtime output.
- `customer_hidden`: production code path used for internal routing, safety, or handoff; should not appear as a final customer answer.
- `replace_required`: production behavior that is still a skeleton and should be replaced by real implementation before being treated as a complete product capability.

## Inventory

| Area | File | Current behavior | Classification | Risk | Next action |
| --- | --- | --- | --- | --- | --- |
| Dataset output worker default runtime | `crates/dataset-output-worker/src/main.rs` | Defaults to `DATASET_OUTPUT_RUNTIME_MODE=placeholder`, provider `placeholder`, model `placeholder-dataset-output-v1` when env is absent. | `replace_required` for production default; `safe_fallback` only for local/dev smoke. | If production env is missing, dataset output may look like a synthetic summary instead of a real model-backed answer. | Require provider runtime in production readiness checks; keep placeholder only for tests/local fallback. |
| Dataset output generator | `crates/dataset-output-worker/src/lib.rs` | `PlaceholderDatasetOutputGenerator` builds a bounded markdown prompt with counts and dataset id, then calls the configured provider. Tests assert runtime manifest and no tool trace leakage. | `safe_fallback` in controlled environments; `replace_required` for user-facing dataset output quality. | Output can be structurally valid but semantically shallow. | Add a provider-backed generator path with retrieval/fact aggregation before treating dataset output as complete. |
| Report planner AST | `crates/report-planner-worker/src/main.rs` | Source-side planner now builds a deterministic business-template AST with Xinbai operations, take-high opportunity, and risk focus routing. | `safe_fallback` / enhancement pending. | Generic non-Xinbai plans still use a deterministic dataset-report structure rather than evidence-aware planning. | Keep production readiness smoke on; next improvement is evidence-aware module binding and live report-planner task verification after deploy. |
| Video URL resolution placeholder | `crates/platform-api/src/react_agent_tools.rs` | `video_url_resolution_placeholder_result` returns safe rejections for missing/login-gated sources and can resolve direct video URLs into remote-unregistered assets. | `customer_hidden` / `safe_fallback`. | The function name says placeholder, but behavior is a safety boundary; do not remove without replacing login-gated protections. | Rename later to `video_url_resolution_guard_result` or equivalent after tests are in place. |
| Video PPT extraction placeholder | `crates/platform-api/src/react_agent_tools.rs` | `video_ppt_extraction_placeholder_result` rejects extraction unless an uploaded or resolved video asset is available. | `customer_hidden` / `safe_fallback`. | Safe as a guard; not a completed extractor by itself. | Keep until registered-video extraction path is fully live-smoked; rename to clarify it is a guard. |
| OpenClaw memory bridge | `crates/platform-api/src/react_agent_tools.rs` | When extension and memory flags are enabled, returns `bridge_stub` with truncated query and thread id. | `replace_required` behind feature flag. | If enabled in production without a real bridge, it may mislead model/tool planning. | Keep disabled unless OpenClaw bridge is implemented; readiness should expose if it is only a stub. |
| OpenClaw readonly execution bridge | `crates/platform-api/src/react_agent_tools.rs` | When extension and readonly flags are enabled, returns `bridge_stub` for allowlisted capabilities. | `replace_required` behind feature flag. | Same as memory bridge; safe only if clearly internal and disabled by default. | Keep disabled until a real readonly execution bridge exists. |
| Codex/static-page anti-placeholder prompt rules | `crates/codex-host-agent/src/main.rs`, `crates/codex-host-agent/src/lib.rs` | Prompts reject simplified/demo/placeholder/fallback pages as success. | `customer_hidden`. | This is a protective instruction, not an incomplete implementation. | Keep and test with artifact validation; do not classify as replace-required. |
| Generated artifact placeholder rejection tests | `crates/codex-host-agent/src/main.rs` | Tests reject pending placeholder generated-artifact URLs. | `test_only` / `customer_hidden`. | None; protective. | Keep as regression coverage. |
| UI placeholders | `apps/web/app/**/*.js`, `apps/web/app/globals.css` | Input placeholder text and visual empty states. | `test_only` or normal UI copy, not runtime stub. | Not part of backend capability completion. | Exclude from runtime replacement work unless UX copy is wrong. |
| Validation docs mentioning placeholder runtime | `docs/validation/*.md` | Historical receipts describe placeholder deployments or placeholder runtime smoke. | `test_only` documentation. | Can confuse plan readers if mixed into active plan. | Keep historical docs, but do not treat them as current production readiness. |

## Production Readiness Rules

1. Production services should not silently fall back to placeholder runtime for customer-facing answers.
2. Guard-style placeholder functions are acceptable only when they reject or defer unsafe work clearly.
3. Feature-flagged bridge stubs must remain disabled or visibly marked as stub in readiness diagnostics.
4. Dataset output placeholders should be replaced before those capabilities are marketed as fully automated generic output generation.
5. Report planner AST must continue passing source-level readiness checks; deterministic planning is acceptable as a bounded fallback, but evidence-aware module binding remains an enhancement target.
6. Static-page and Codex prompts that reject placeholder artifacts are protective and should remain.

## Immediate Follow-Up

1. Add readiness checks for `DATASET_OUTPUT_RUNTIME_MODE`, `DATASET_OUTPUT_RUNTIME_PROVIDER`, and `DATASET_OUTPUT_RUNTIME_MODEL` on 8 server.
2. Rename video placeholder guard functions when touching that module, after tests are locked.
3. After deploy, run `production-placeholder-readiness` on 8 server to confirm report planner source status is `deterministic_business_template`.
4. Keep OpenClaw bridge flags disabled until the bridge is real.
5. Keep this inventory updated whenever a new `placeholder` or `stub` production path is introduced.
