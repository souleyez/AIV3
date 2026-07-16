# DataMax V3 Active Execution Plan

**Status:** READY FOR TASK 2 — TASK 1 CODE COMPLETE; DEPLOYMENT/POSTGRESQL ROTATION PENDING

**Cutover date:** 2026-07-16

**Unique implementation plan:**

- `docs/plans/2026-07-16-datamax-v3-quality-and-production-readiness.md`

**Completed task:** Task 1 — redact PostgreSQL/NATS connection endpoints from logs and errors.

**Task 1 implementation commit:** `6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7`

**Next task:** Task 2 — reconcile GitHub main, required CI and the 8-server release line; deploy the redaction fix before rotating the historically exposed PostgreSQL credential.

**Unique validation ledger:**

- `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Frozen cutover baseline:**

- local/GitHub release branch: `1edff9cdac585671f2ed64ed25690f2be121c750`;
- `origin/main`: `33e766a904bf28d63e71d170a463f33830c3cf86`;
- 8-server checkout: `1edff9cdac585671f2ed64ed25690f2be121c750` at the 2026-07-16 read-only inventory;
- release branch versus `origin/main`: 43 ahead, 0 behind;
- graph display/cross-graph may remain enabled, but `ASSISTANT_RUN_SEMANTIC_SUPPLY_MODE=off`;
- asset import/parser feature gates remain off;
- live retrieval defaults to `legacy_scan` until the new plan's lexical gate passes.

Execution order is fixed: log redaction → main/CI reconciliation → disposable PostgreSQL gate → governed migrations → field semantic contracts → dataset processing details → PostgreSQL lexical qualification → runtime observability → real-provider QA → release closeout.

DataMax may organize authorized evidence but must not orchestrate the user's conversation, the model's wording, answer structure, conclusion, route or action.

Everything under `docs/archive/plans/` is historical and non-executable. Do not continue old Task numbers, approvals, provider windows, feature-flag scopes, SHA baselines or deployment commands.

Do not append execution receipts to this pointer. Update only status, current/next task, exact release identity and links; detailed evidence belongs in the unique validation ledger.
