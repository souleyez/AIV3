# DataMax 主线缺口收口 Implementation Plan

> **执行要求：** 按本文档逐项推进；涉及公开第三方接口、URL、鉴权、请求字段或响应字段的变更，必须先停下来确认。

**Goal:** 把 DataMax 当前生产主线从“功能持续叠加”收口到“计划清晰、验证稳定、关键链路可回归、工程风险可控”。

**Architecture:** 继续保持 Rust 平台核心、Next.js 主站、Worker 异步任务、PostgreSQL 状态库、Codex 执行器和静态页产物发布的现有架构。短期不做大拆大改，优先补齐验证、文档、线上配置一致性和高风险模块边界；中期再拆分过大的 `platform-api` 与前端页面文件。

**Tech Stack:** Rust workspace、Axum、SQLx、PostgreSQL、NATS、Next.js、React、Node smoke scripts、systemd、8 服务器部署环境。

---

## 0. 当前基线

- 当前功能代码基线：`8f31a4a Consolidate DataMax plan and validation`。
- 当前 8 服务器 `/srv/aiv3/repo` 运行时代码：`8f31a4a77`。
- 当前 8 服务器核心服务抽查：`aiv3-platform-api.service`、`aiv3-web.service`、`aiv3-assistant-run-worker.service`、`aiv3-chat-session-worker.service`、`aiv3-dataset-output-worker.service`、`aiv3-external-action-worker.service`、`aiv3-external-source-worker.service`、`aiv3-ingest-worker.service`、`aiv3-media-worker.service`、`aiv3-memory-worker.service`、`aiv3-report-planner-worker.service`、`aiv3-report-render-worker.service`、`aiv3-retrieval-worker.service`、`aiv3-static-page-worker.service` 均为 `active`。
- 当前公开入口：
  - 主站直接问答：`https://doc.elepcloud.com/`
  - 管理台：`https://v3.elepcloud.com/`
  - 第三方接口文档：`https://v3.elepcloud.com/external-integrations`
- 当前计划入口只保留本文件。旧计划已归档：
  - `docs/archive/plans/datamax-active-execution-plan-20260611-pre-cleanup.md`
  - `docs/archive/plans/aiv3-rag-retrieval-executable-plan-20260611.md`

## 1. 不变量

- 不默默修改第三方公开接口、URL、鉴权、请求字段、响应字段。
- 不把 self-test、离线 fixture、只读检查说成真实 live 验收。
- 不记录或提交密钥、cookie、bearer、数据库 URL、原始客户行、完整客户文档、provider payload、私有 object path。
- 8 服务器部署必须显式执行 pull、build、restart、smoke；120 服务器不在本计划范围内。
- 质量门禁继续保持安全禁用或被动采样，不恢复会阻断正常回答的硬门禁。
- 数据接入分析只生成安全摘要和 staging plan；生产写入、schema 变更、覆盖导入必须人工确认。
- 静态页/报表默认先复用已发布模板；只有明确要求换风格或无可用模板时才走高成本 Image2 流程。

## 2. P0：计划和验证收口

### Task 2.1：保持单一主线计划

**Files:**
- Modify: `docs/plans/datamax-active-execution-plan.md`
- Read: `docs/validation/datamax-main-gap-closure.md`
- Read: `docs/validation/datamax-main-gap-closure-completion-audit.md`

**Steps:**
1. 每次完成主线功能或部署后，只更新本文件的当前基线和下一步任务。
2. 历史专项计划不得继续堆到本文件正文；需要保留时移入 `docs/archive/plans/`。
3. 本文件只保留当前 1-2 周可执行事项。

**Verify:**

```bash
rg --files docs/plans
```

Expected: `docs/plans` 下只有 `datamax-active-execution-plan.md`。

### Task 2.2：更新部署和验证台账

**Files:**
- Modify: `docs/validation/datamax-main-gap-closure.md`
- Modify: `docs/validation/README.md` if index changes are needed

**Steps:**
1. 追加当前 `376262f/376262ff5` 发布回执。
2. 记录本轮 8 服务器 build 使用 `CC=clang CXX=clang++`。
3. 记录核心服务 active 状态。
4. 记录公开页面和新百模板 artifact URL 的 HTTP smoke。

**Verify:**

```bash
git diff --check
rg -n "376262f|376262ff5|clang|xinbai-functional-modular-template-20260604" docs/validation/datamax-main-gap-closure.md
```

**Progress:**
- 2026-06-11: `8f31a4a77` 已部署到 8 服务器。
  - `git pull --ff-only origin main` 已快进到 `8f31a4a77`。
  - `pnpm build` 在 `apps/web` 通过，仅有既有 Next middleware deprecation 和 Turbopack NFT trace warning。
  - `CC=clang CXX=clang++ cargo build --release -p platform-api -p assistant-run-worker -p chat-session-worker -p dataset-output-worker -p external-action-worker -p external-source-worker -p ingest-worker -p media-worker -p memory-worker -p report-planner-worker -p report-render-worker -p retrieval-worker -p static-page-worker` 通过；第一次 SSH 长连接中断后改用后台日志确认 `Finished release profile`。
  - 已重启计划内 14 个服务，服务状态均为 `active`。
  - 120 服务器未触碰。

### Task 2.3：建立当前 8 服务器正式 smoke 矩阵

**Files:**
- Read: `package.json`
- Read: `scripts/README.md`
- Modify: `docs/validation/datamax-main-gap-closure.md`

**Required smoke:**

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
npm run smoke:customer-web-codex-live -- --preflight --allow-missing-gates --base-url https://v3.elepcloud.com
npm run smoke:customer-web-codex-readiness -- --json-stdout --allow-not-ready
node --test apps/web/app/lib/codex-customer-artifacts.test.mjs
npm --prefix apps/web run build
git diff --check
```

**8-server read-only smoke:**

```bash
curl -I -L https://v3.elepcloud.com/
curl -I -L https://doc.elepcloud.com/
curl -I https://v3.elepcloud.com/external-integrations
curl -I https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
curl -I https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/table-data.csv
curl -I https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/report.md
```

**Live smoke requiring credentials:**
- Third-party ordinary question.
- Third-party scoped temporary document question.
- Third-party SSE streaming.
- Third-party real report trigger with bearer.
- Model gateway operator smoke.

If credentials are missing, record as `pending credential` rather than skipped silently.

**Progress:**
- 2026-06-11: local non-credential release-candidate smoke passed for the current uncommitted batch.
  - `npm run smoke:production-placeholder-readiness -- --self-test` passed with `ready=true`.
  - `npm run smoke:external-report-focus -- --self-test` passed with `reportCases=7`, `ordinaryGuards=4`.
  - `npm run smoke:external-report-export -- --self-test` passed with `modeCount=2`, `okCount=2`, expected title `新世界百货经营管理月报表`, focus `取高机会`, and export files `table-data.csv`, `report.ppt`, `report.md`.
  - `node --test apps/web/app/lib/codex-customer-artifacts.test.mjs` passed 21/21 tests.
  - `cargo test -p report-planner-worker` passed 4/4 tests.
  - `npm --prefix apps/web run build` passed with the existing Next middleware deprecation warning and Turbopack NFT trace warning only.
  - `cargo fmt --check`, `git diff --check`, and `rg --files docs/plans` passed; `docs/plans` contains only this plan.
  - Credential/live checks remain pending until operator credentials or deployment approval are available.
- 2026-06-11: 8-server post-deploy non-credential smoke passed for runtime code `8f31a4a77`.
  - `npm run smoke:production-placeholder-readiness -- --env-file /etc/aiv3/aiv3.env --env-file /etc/aiv3/minimax.env --allow-not-ready --json-stdout` passed with `ready=true`, `dataset_output.ready=true`, `report_planner.source_status=deterministic_business_template`, and no raw env values printed.
  - `npm run smoke:external-report-focus -- --self-test` passed with `reportCases=7`, `ordinaryGuards=4`.
  - `npm run smoke:external-report-export -- --self-test` passed with `modeCount=2`, `okCount=2`, expected title `新世界百货经营管理月报表`, focus `取高机会`, and export files `table-data.csv`, `report.ppt`, `report.md`.
  - `curl -I` checks for `https://v3.elepcloud.com/`, `https://doc.elepcloud.com/`, `/external-integrations`, and the Xinbai template `index.html`/`table-data.csv`/`report.md` returned HTTP 200.
  - `CUSTOMER_WEB_CODEX_REPO_ROOT=/srv/aiv3/repo npm run smoke:customer-web-codex-readiness -- --json-stdout --allow-not-ready` returned `ready=true` without printing secrets.
  - `npm run smoke:customer-web-codex-live -- --preflight --allow-missing-gates --base-url https://v3.elepcloud.com` completed preflight without live writes.
  - `node scripts/smoke/model-gateway-operator.mjs --base-url https://v3.elepcloud.com --allow-missing-credentials` completed as `pending=true`, `failed=false`; no credential bypass was added.

## 3. P0：模型和主站稳定性

### Task 3.1：确认主站默认模型与 fallback

**Files:**
- Read: `crates/llm-gateway/src/lib.rs`
- Read: `docs/operations/model-gateway-rollout.md`
- Read: `docs/validation/datamax-main-gap-closure.md`

**Steps:**
1. 在 8 服务器只读确认模型池状态，不打印密钥。
2. 确认 RightCode 主力模型可用时为默认。
3. 确认 MiniMax fallback 的鉴权头与 env key 正确，避免再次出现 `login fail: Please carry the API secret key in the Authorization field`。
4. 更新验证台账。

**Verify:**

```bash
npm run smoke:model-gateway-operator -- --preflight
npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com
```

Operator cookie 缺失时，只记录鉴权缺口。

### Task 3.2：主站聊天体验修复回归

**Files:**
- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/globals.css`
- Test: `apps/web/app/lib/assistant-run-progress.test.mjs`
- Test: `apps/web/app/lib/local-chat-sessions.test.mjs`

**Focus:**
- 输出追加时自动滚动到底部。
- 进度/思考/执行状态允许以安全摘要展示，不泄露 raw prompt、密钥、provider payload。
- 新对话按钮不能丢失当前会话列表状态。
- `doc.elepcloud.com` 保持直接问答入口，`v3.elepcloud.com` 保持管理台入口。

**Verify:**

```bash
node --test apps/web/app/lib/assistant-run-progress.test.mjs
node --test apps/web/app/lib/local-chat-sessions.test.mjs
npm --prefix apps/web run build
```

## 4. P0：第三方报表和静态页链路

### Task 4.1：新百默认模板继续作为主模板

**Files:**
- Read: `docs/static-page-templates/xinbai-functional-modular-template-20260604/template-contract.json`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/HomePageClient.js`
- Test: `scripts/smoke/external-report-focus.mjs`
- Test: `scripts/smoke/external-report-export.mjs`

**Rules:**
- 新百经营类需求默认使用 `xinbai-functional-modular-template-20260604`。
- 触发词仅在新百/经营数据上下文内放宽，不全局放宽。
- `取高`、`经营状况`、`经营情况`、`风险识别`、`低活跃品牌`、`销售缺口`、`需要助推` 可以触发报表。
- `取高是什么意思？`、`风险识别系统有哪些项目经历？` 不触发报表。
- 报表链接只出现一次；正常问答不能被截断。
- 有报表产物时，固定结构字段必须带 URL，不能只在文本里出现。

**Verify:**

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api external_channel_static_page_artifact --lib
```

### Task 4.2：模板复用和低负载预热

**Files:**
- Modify: `crates/static-page-worker/src/main.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `crates/static-page-worker/src/main.rs`

**Rules:**
- 相同或有交集的数据集组合，优先复用已有模板。
- `default_prompt` 相同或相近时优先复用同模板。
- 用户未要求报表时，可以低优先级预热模板，但不主动打扰客户。
- 已有模板时不重复走 Image2。
- Cloudflare/Codex 不可用时，必须能本地生成可用页面或返回明确状态。

**Verify:**

```bash
CC=clang CXX=clang++ cargo test -p static-page-worker --lib
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
```

## 5. P1：数据接入和企业记忆

### Task 5.1：数据接入分析闭环

**Files:**
- Read: `docs/validation/data-ingestion-analysis-smoke.md`
- Read: `docs/validation/data-ingestion-staging-sync-smoke.md`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Test: `scripts/run-cloudflare-codex-fixed-task-smoke.ps1`

**Rules:**
- 用户或第三方提出数据库/API/表格/ERP 接入需求时，必须落到一个目标 DataMax 数据集。
- 如果当前已选择数据集，优先作为目标数据集。
- 如果没有目标数据集，只能提出创建或挂载建议，不直接写生产。
- 凭据、生产 schema、公开 API 字段变更一律转人工。

**Verify:**

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis -Json
```

### Task 5.2：库级事实和后台深化解析

**Files:**
- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/memory-worker/src/main.rs`
- Modify: `crates/retrieval-worker/src/main.rs`
- Modify: `crates/storage/src/*`
- Read: `docs/validation/document-understanding-smoke.md`

**Scope:**
- 文档入库后异步进行事实抽取、表格结构抽取、实体/别名抽取、简历项目经历抽取、护理操作步骤抽取。
- 结果写入平台侧数据库，不依赖模型侧记忆。
- 查询时先拆解问题，再由本地检索/聚合供料给模型。
- 不把完整检索内容无限制喂给模型。

**Verify:**

```bash
bash scripts/run-document-understanding-smoke.sh
bash scripts/run-retrieval-quality-smoke.sh
CC=clang CXX=clang++ cargo test -p storage enrichment --lib
```

### Task 5.3：数据源行级语义决策

**Files:**
- Read: `docs/operations/data-source-row-identity-decision.md`
- Modify only after decision: `crates/external-source-worker/src/*`
- Modify only after decision: `crates/storage/src/*`

**Decision needed:**
- `bi_contract_warning`、`bi_rentsales_detail` 是按实体最新快照回答，还是按源行级明细完整物化。

**Before decision:**
- 不改生产映射。
- 不声称行级完整。
- 继续在报表里用已有可解释口径。

## 6. P1：第三方临时文档和权限范围

### Task 6.1：临时附件加入本轮/本会话问答范围

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/ingest-worker/src/main.rs`
- Test: `scripts/smoke/external-scoped-document-chat.mjs`

**Rules:**
- 第三方消息带临时附件时，应加入当前 `conversation_external_id` 的可见范围。
- 同一会话后续问题继续可见。
- 换会话后不得继承。
- 与 `dataset_external_ids` 和 `available_document_external_ids` 并集生效；文档已在数据集内时去重。

**Verify:**

```bash
npm run smoke:external-scoped-document-chat -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api external_channel --lib
```

### Task 6.2：第三方接口文档同步

**Files:**
- Modify: `docs/integrations/third-party-integration-api.zh-CN.md`
- Modify: `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- Generated: `apps/web/public/external-integrations/*.md`
- Generated: `apps/web/public/external-integrations/*.html`

**Rules:**
- 只做兼容性说明和新增能力说明。
- 不删除旧字段，不改旧字段语义。
- 明确说明 `dataset_external_ids` 支持多个分组，文档和分组可同时传，服务端并集去重。

**Verify:**

```bash
npm run check:pure-third-party-guide-html
npm run test:pure-third-party-guide-html
```

## 7. P1：Codex 执行器和 CC 模式

### Task 7.1：CC 模式只接管复杂执行，不越权改产品

**Files:**
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/codex-host-agent/src/lib.rs`
- Modify: `apps/web/app/lib/codex-customer-artifacts.js`
- Test: `apps/web/app/lib/codex-customer-artifacts.test.mjs`

**Rules:**
- `cc` 模式下，复杂分析、数据/API/数据库接入规划可以交给 Codex。
- 必须落一个目标 DataMax 数据集或提出目标数据集建议。
- DataMax 产品自身代码、接口、鉴权、部署、密钥、公开 API 变更必须阻断为人工确认。
- Codex 结果只显示安全摘要、产物链接、状态卡片，不显示 raw stdout/stderr、绝对路径或密钥。

**Verify:**

```bash
node --test apps/web/app/lib/codex-customer-artifacts.test.mjs
CC=clang CXX=clang++ cargo test -p platform-api customer_codex --lib
CC=clang CXX=clang++ cargo test -p codex-host-agent
```

## 8. P2：工程治理和拆分

### Task 8.1：拆分超大 `platform-api`

**Current risk:**
- `crates/platform-api/src/lib.rs` 约 15 万行，已经是主要维护风险。

**First split targets:**
- `external_channel`
- `assistant_run`
- `static_page_artifacts`
- `data_ingestion`
- `model_gateway`
- `admin_auth`

**Rules:**
- 每次只拆一个边界。
- 先移动纯函数和测试，再移动 handler。
- 不改行为，不改路由。

**Verify each slice:**

```bash
CC=clang CXX=clang++ cargo test -p platform-api <module_keyword> --lib
npm --prefix apps/web run build
git diff --check
```

**Progress:**
- 2026-06-11: first source-only slice completed for `model_gateway` runtime limiting.
  - Added `crates/platform-api/src/model_gateway_runtime.rs`.
  - Moved `GatewayRuntimeLimiter`, queue/rate/circuit-breaker snapshots, provider failure helpers, and model-gateway permit types out of `lib.rs`.
  - Kept behavior unchanged; `lib.rs` imports the internal module with `use model_gateway_runtime::*`.
  - Local Windows verification used default toolchain because `clang` is not installed locally: `cargo test -p platform-api gateway_limiter --lib` passed 3/3; `cargo fmt --check` passed.
  - Deployment/8-server verification with `CC=clang CXX=clang++` is still required before release.
- 2026-06-11: second source-only slice completed for `model_gateway` admin helper logic.
  - Added `crates/platform-api/src/model_gateway_admin.rs`.
  - Moved preset list/lookup, profile create/update normalization, profile view redaction, auth-env presence check, validation helpers, and not-found helper out of `lib.rs`.
  - Kept route handlers, operator authentication, storage calls, and public response structs unchanged.
  - Local Windows verification used default toolchain: `cargo test -p platform-api model_gateway_profile --lib` passed 5/5, `cargo test -p platform-api model_gateway_profile_presets --lib` passed 1/1, `cargo test -p platform-api gateway_limiter --lib` passed 3/3, `cargo fmt --check` passed.
  - Deployment/8-server verification with `CC=clang CXX=clang++` is still required before release.
- 2026-06-11: third source-only slice completed for `model_gateway` status/source helper logic.
  - Added `crates/platform-api/src/model_gateway_status.rs`.
  - Moved `ModelGatewayStatusProviderSource`, database/env provider-source builders, i32/u32 conversions, percentage helper, active-source counting, eligibility, and dormant-reason logic out of `lib.rs`.
  - Kept `/v1/model-gateway/status` handler, operator authentication, storage queries, runtime status collection, and public response structs unchanged.
  - Local Windows verification used default toolchain: `cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib` passed 1/1, `cargo test -p platform-api model_gateway_profile --lib` passed 5/5, `cargo test -p platform-api gateway_limiter --lib` passed 3/3, `cargo fmt --check` passed.
  - Deployment/8-server verification with `CC=clang CXX=clang++` is still required before release.
- 2026-06-11: fourth source-only slice completed for `admin_auth` session/support helper logic.
  - Added `crates/platform-api/src/auth_session_support.rs`.
  - Moved email normalization validation, auth-purpose session decision, auth metadata construction, auth env fallback, session token hash/generation, device fingerprint fallback, session cookie helpers, auth session/user view mapping, and audit detail redaction whitelist out of `lib.rs`.
  - Kept auth route handlers, DB/session storage calls, audit event writes, cookie name/TTL, public routes, auth behavior, URLs, request fields, and response fields unchanged.
  - Local Windows verification used default toolchain: `cargo test -p platform-api auth_session_endpoint_returns_current_user --lib` passed 1/1, `cargo test -p platform-api auth_logout_revokes_session --lib` passed 1/1, `cargo test -p platform-api auth_audit_events_endpoint_returns_current_user_redacted_events --lib` passed 1/1, `cargo test -p platform-api auth_key_login_resolves_user_and_session --lib` passed 1/1, `cargo fmt --check` passed, and `git diff --check` passed.
  - Deployment/8-server verification with `CC=clang CXX=clang++` is still required before release.
- 2026-06-11: fifth source-only slice completed for `model_gateway` runtime pool helper logic.
  - Extended `crates/platform-api/src/model_gateway_status.rs`.
  - Moved runtime worker-pool status construction, runtime database-pool status construction, worker-pool env parsing, and worker concurrency clamp helper out of `lib.rs`.
  - Kept `/v1/model-gateway/status` handler, external-channel runtime sampling, DB query for pending preclaims, public response structs, URLs, auth behavior, request fields, and response fields unchanged.
  - Local Windows verification used default toolchain: `cargo test -p platform-api parse_worker_pool_concurrency_prefers_first_valid_value_and_clamps --lib` passed 1/1, `cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib` passed 1/1, `cargo fmt --check` passed, and `git diff --check` passed.
  - Deployment/8-server verification with `CC=clang CXX=clang++` is still required before release.
- 2026-06-11: sixth source-only slice completed for `external_channel` dispatch auth helper logic.
  - Added `crates/platform-api/src/external_channel_support.rs`.
  - Moved `ExternalActionDispatchAuth`, action dispatch auth config parsing, reply-specific dispatch auth config parsing, outbound reply auth fallback selection, auth configured/mode helpers, and external-channel auth failure construction out of `lib.rs`.
  - Kept external-channel handlers, action dispatch HTTP signing/header construction, inbound bearer validation flow, DB queries, public routes, URLs, auth semantics, request fields, and response fields unchanged.
  - Local Windows verification used default toolchain: `cargo test -p platform-api external_action_dispatch_auth_uses_only_explicit_dispatch_credentials --lib` passed 1/1, `cargo test -p platform-api external_outbound_reply_dispatch_uses_reply_specific_endpoint_and_credentials --lib` passed 1/1, `cargo test -p platform-api external_action_dispatch_headers_include_bearer_and_signature --lib` passed 1/1, `cargo test -p platform-api external_channel_inbound_bearer_auth_accepts_only_configured_token --lib` passed 1/1, `cargo fmt --check` passed, and `git diff --check` passed.
  - Deployment/8-server verification with `CC=clang CXX=clang++` is still required before release.

### Task 8.2：分类 placeholder/stub

**Files:**
- Read: `crates/dataset-output-worker/src/*`
- Read: `crates/report-planner-worker/src/*`
- Read: `crates/platform-api/src/react_agent_tools.rs`
- Create: `docs/operations/placeholder-stub-inventory.md`

**Classification:**
- `test_only`: 测试专用，不进生产。
- `safe_fallback`: 生产可见但必须明确是降级。
- `customer_hidden`: 生产运行但不直接展示给客户。
- `replace_required`: 应替换为真实实现。

**Verify:**

```bash
rg -n "placeholder|stub" crates apps docs -g "!target" > target/placeholder-stub-scan.txt
```

Do not remove stubs before classification.

## 9. P2：低质量回答自动优化

### Task 9.1：继续被动采样，不恢复硬门禁

**Files:**
- Read: `docs/operations/answer-quality-autofix.md`
- Test: `scripts/smoke/answer-quality-offline-corpus.mjs`

**Rules:**
- 低质量识别只做离线/后台采样。
- 不阻断客户正常回答。
- 自动优化范围限定为问答回复优化，不改接口、不改鉴权、不改数据源映射。
- 明显客户不满、资料不足、答非所问、报表未触发、链接缺失等进入样本库。

**Verify:**

```bash
node scripts/smoke/answer-quality-offline-corpus.mjs --self-test --pretty
```

## 10. P2：视频/PPT专项降级为独立能力

视频/PPT 抽取已有大量历史计划和验证，但不是当前 DataMax 主线的 P0。后续只在客户明确提出视频/PPT时进入专项。

**Files:**
- Archived plan: `docs/archive/plans/datamax-active-execution-plan-20260611-pre-cleanup.md`
- Validation: `docs/validation/video-ppt-deliverable-smoke.md`
- Scripts: `scripts/smoke/video-ppt-*.mjs`

**Rules:**
- 不把视频/PPT专项内容继续堆入主线计划。
- 不绕过登录态、DRM 或平台权限。
- 登录态/视频号只走 handoff 或授权录屏。

## 11. 发布流程

### Before commit

```bash
git status -sb
git diff --check
npm --prefix apps/web run build
```

按修改范围追加对应测试：

```bash
node --test apps/web/app/lib/codex-customer-artifacts.test.mjs
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api assistant_run_customer_codex_sidecar --lib -- --nocapture
CC=clang CXX=clang++ cargo test -p platform-api data_ingestion_analysis --lib -- --nocapture
```

### GitHub

```bash
git add <changed-files>
git commit -m "<concise message>"
git push origin main
```

### 8 服务器部署

```bash
ssh root@8.155.8.7 "cd /srv/aiv3/repo && git pull --ff-only origin main"
ssh root@8.155.8.7 "cd /srv/aiv3/repo/apps/web && pnpm build"
ssh root@8.155.8.7 "cd /srv/aiv3/repo && CC=clang CXX=clang++ cargo build --release -p platform-api -p assistant-run-worker -p chat-session-worker -p dataset-output-worker -p external-action-worker -p external-source-worker -p ingest-worker -p media-worker -p memory-worker -p report-planner-worker -p report-render-worker -p retrieval-worker -p static-page-worker"
ssh root@8.155.8.7 "systemctl restart aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-chat-session-worker.service aiv3-dataset-output-worker.service aiv3-external-action-worker.service aiv3-external-source-worker.service aiv3-ingest-worker.service aiv3-media-worker.service aiv3-memory-worker.service aiv3-report-planner-worker.service aiv3-report-render-worker.service aiv3-retrieval-worker.service aiv3-static-page-worker.service"
```

### Post deploy

```bash
ssh root@8.155.8.7 "cd /srv/aiv3/repo && git rev-parse --short HEAD && git status -sb"
ssh root@8.155.8.7 "systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-chat-session-worker.service aiv3-static-page-worker.service"
curl -I -L https://v3.elepcloud.com/
curl -I -L https://doc.elepcloud.com/
curl -I https://v3.elepcloud.com/external-integrations
```

## 12. 当前推荐下一步

1. 补 P0 Task 2.3/3.1 的凭据型 live smoke：第三方普通问答、第三方临时文档、第三方 SSE、第三方真实报表触发、main-site streaming、model-gateway operator。
2. 对 P0 Task 3.2 做一次浏览器级手工/自动验证：确认流式输出时消息框实际自动到底，确认新对话按钮不会丢当前会话。
3. 对 `platform-api` 做第七轮只移动不改行为的模块拆分，优先继续 `external_channel` 纯解析/响应 helper，或拆 `assistant_run` 内部纯 helper；handler 迁移暂缓。
4. 部署下一版后在 8 服务器复跑 `production-placeholder-readiness`，确认 dataset-output runtime 仍为 provider 模式且 report planner source status 为 `deterministic_business_template`。
5. 部署下一版后在 8 服务器用 `CC=clang CXX=clang++ cargo test -p platform-api gateway_limiter --lib`、`CC=clang CXX=clang++ cargo test -p platform-api model_gateway_profile --lib`、`CC=clang CXX=clang++ cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib`、`CC=clang CXX=clang++ cargo test -p platform-api parse_worker_pool_concurrency_prefers_first_valid_value_and_clamps --lib`、`CC=clang CXX=clang++ cargo test -p platform-api auth_session_endpoint_returns_current_user --lib`、`CC=clang CXX=clang++ cargo test -p platform-api external_action_dispatch_auth_uses_only_explicit_dispatch_credentials --lib` 复验本轮拆分。
6. 继续把 report planner 从确定性业务模板推进到证据感知模块绑定；这属于增强项，不阻塞当前报表模板复用链路。
7. 等待业务决策后再处理数据源行级语义；当前不改生产映射。
