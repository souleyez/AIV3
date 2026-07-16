# DataMax V3 Active Execution Plan

**Status:** TASK 2 FINAL-IDENTITY WORKSPACE HOTFIX — TASK 3 NOT STARTED

**Cutover date:** 2026-07-16

**Unique implementation plan:**

- `docs/plans/2026-07-16-datamax-v3-quality-and-production-readiness.md`

**Completed implementation:**

- Task 1 — redact PostgreSQL/NATS connection endpoints from logs and errors; implementation commit `6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7`.
- Task 2 runtime — reconcile GitHub main, deterministic CI and the 8-server runtime; deployed runtime commit `ff60d0deee81a2cbb98a70a4cbda9693962349d0`.

**Current executable task:** Task 2 only — anchor the release ancestry check to the Actions checkout, pass exact PR/main CI, fast-forward the workflow/docs-only delta, pass final release identity, then remove the temporary runner.

**Next task after that gate:** Task 3 — make PostgreSQL and mock integration tests fail instead of silently skipping. Task 3 has not started.

**Unique validation ledger:**

- `docs/validation/2026-07-16-datamax-v3-quality-readiness.md`

**Release identity:**

- frozen deployed ancestor: `1edff9cdac585671f2ed64ed25690f2be121c750`;
- connection-redaction floor: `6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7`;
- running binary source: `ff60d0deee81a2cbb98a70a4cbda9693962349d0`;
- docs-only receipt SHA: `45eeb6ad623869bfb1c3f63f7a73951dc637307b`;
- final Task 2 SHA: derived from the bounded final-identity workspace hotfix and accepted only when local `main`, GitHub `origin/main` and `/srv/aiv3/repo` agree and the final read-only identity run succeeds;
- graph display/cross-graph may remain enabled, but `ASSISTANT_RUN_SEMANTIC_SUPPLY_MODE=off`;
- asset import/parser feature gates remain off;
- live retrieval defaults to `legacy_scan` until Task 7 passes.

The live platform PostgreSQL role currently has no password verifier and its URL has no password component. Task 2 therefore classifies platform password rotation as not applicable; it did not create a new secret under loopback `trust`. HBA authentication hardening and attributable external-datasource rotation are separate future work.

Execution order remains fixed: log redaction → main/CI reconciliation → disposable PostgreSQL gate → governed migrations → field semantic contracts → dataset processing details → PostgreSQL lexical qualification → runtime observability → real-provider QA → release closeout.

DataMax may organize authorized evidence but must not orchestrate the user's conversation, the model's wording, answer structure, conclusion, route or action.

Everything under `docs/archive/plans/` is historical and non-executable. Do not continue old Task numbers, approvals, provider windows, feature-flag scopes, SHA baselines or deployment commands.

Do not append execution receipts to this pointer. Update only status, current/next task, exact release identity and links; detailed evidence belongs in the unique validation ledger.
