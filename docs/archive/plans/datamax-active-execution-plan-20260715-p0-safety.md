> **ARCHIVED 2026-07-16 — NON-EXECUTABLE:** 这是 2026-07-15 活动指针的原样快照；禁止从正文继续旧 Task、继承 approval/基线/开关或执行部署命令。唯一活动入口为 `docs/plans/datamax-active-execution-plan.md`。

# DataMax V3 Active Execution Plan

**Status:** DEPLOYED P0 SAFETY CANDIDATE — REAL LIVE QUALITY, DISPOSABLE DB, AND FIELD SEMANTIC CONTRACT PENDING

Current independent plan:

- `docs/plans/2026-07-15-datamax-answer-quality-p0-safety-implementation-plan.md`
- design: `docs/plans/2026-07-15-datamax-answer-quality-p0-safety-design.md`

Current goal:

- stop ordinary questions from being mistaken for artifact-generation requests;
- stop identifier-like or unrelated database fields and tables from becoming analytical supplies;
- make the live answer-quality harness fail closed on inner evaluator failures and captured side effects;
- preserve the principle that DataMax supplies authorized evidence but does not orchestrate conversation, answer structure, conclusions, wording, routes or actions.

Current release receipt:

- the deterministic P0 action, database-selection, connector, evaluator, and offline wrapper gates pass;
- `platform-api` discovered 2900 library tests and reported 2898 passed, 0 failed, and 2 ignored;
- 182 PostgreSQL-backed test bodies returned early because no disposable database was configured, so this is not a database integration pass;
- implementation commits `a81fcca11ab5f9023adc811632245f8a72f48f52` and `1deca3aa5e6bfc22af5ff66cdf45292520a42818` are pushed to `origin/codex/dataset-understanding-mvp`;
- 8-server `/srv/aiv3/repo` fast-forwarded from `a8171c468d4e339bd10cbb03765f1038301f9d88` to `1deca3aa5e6bfc22af5ff66cdf45292520a42818`; only `aiv3-platform-api.service` and `aiv3-assistant-run-worker.service` were rebuilt and restarted;
- both services are active with zero restarts and zero warning-priority journal entries since deployment; platform health/ready are green and both public Web entry points return HTTP 200;
- no real provider/live quality run, disposable database run, migration, source write, configuration edit, feature enablement, or Web deployment was performed;
- the next gates are disposable PostgreSQL verification, controlled real-provider quality evidence, and the P1 field semantic contract. Case 012 remains intentionally fail-closed for the uncontracted sales-gap aggregate.

The previous graph-assisted supply plan completed on 2026-07-15 with `keep_graph_visual_only`; it is frozen at `docs/plans/2026-07-15-datamax-knowledge-graph-assisted-supply-answer-quality-plan.md`. Its feature remains off and it provides no promotion candidate. The new P0 plan is independent and responds to the extended live QA evidence; it does not reopen graph rollout, shadow execution, or provider authorization.

R5 已于 2026-07-12 完成、冻结并整体归档：

- 完整归档：`docs/archive/plans/datamax-active-execution-plan-r5-completed-20260712.md`
- 完成审计：`docs/validation/datamax-r5-completion-audit-20260712.md`
- 详细验证账本：`docs/validation/datamax-main-gap-closure.md`

不得从本指针、R5 归档或已冻结的知识图谱计划继续旧 Task，也不得恢复旧基线、旧 approval 或旧 live scope。后续只按当前 P0 独立计划的 Task 1–6 和其中的停止门禁执行。
