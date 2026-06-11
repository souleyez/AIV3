# DataMax 主线执行计划

> 本文件是当前唯一主线计划。历史专项计划只归档，不再继续堆叠到 `docs/plans/`。

## 1. 目标

把 DataMax 当前生产主线收口到以下状态：

- 线上入口清晰：`doc.elepcloud.com` 是直接问答主站，`v3.elepcloud.com` 是管理台和第三方接口文档入口。
- 第三方问答、临时文档授权、报表触发、SSE 流式输出、静态页产物返回可稳定回归。
- 新百经营月报模板作为默认经营分析模板，支持按用户关注焦点调整模块顺序。
- 主站聊天体验稳定：真流式、自动滚动、新对话不丢上下文。
- 模型网关具备主力模型和 fallback 可观测能力，密钥不外泄。
- 计划、验证、部署记录可持续维护，不再分散到多份计划文档。

## 2. 当前基线

- GitHub `main`：以 `git log --oneline -1` 为准；最近一次已同步验证记录为 `99cd34c Record DataMax live smoke progress`。
- 8 服务器 `/srv/aiv3/repo`：已同步到 `99cd34c2d`；本轮后续文档同步不需要重启服务。
- 8 服务器当前运行时代码：`8f31a4a Consolidate DataMax plan and validation`，其后提交主要为文档和验证台账更新。
- 已部署并重启的 8 服务器服务：
  - `aiv3-platform-api.service`
  - `aiv3-web.service`
  - `aiv3-assistant-run-worker.service`
  - `aiv3-chat-session-worker.service`
  - `aiv3-dataset-output-worker.service`
  - `aiv3-external-action-worker.service`
  - `aiv3-external-source-worker.service`
  - `aiv3-ingest-worker.service`
  - `aiv3-media-worker.service`
  - `aiv3-memory-worker.service`
  - `aiv3-report-planner-worker.service`
  - `aiv3-report-render-worker.service`
  - `aiv3-retrieval-worker.service`
  - `aiv3-static-page-worker.service`
- 当前公开入口：
  - 主站直接问答：`https://doc.elepcloud.com/`
  - 管理台：`https://v3.elepcloud.com/`
  - 第三方接口文档：`https://v3.elepcloud.com/external-integrations`
- 当前主计划文件：`docs/plans/datamax-active-execution-plan.md`。
- 已归档旧计划：
  - `docs/archive/plans/datamax-active-execution-plan-20260611-pre-cleanup.md`
  - `docs/archive/plans/aiv3-rag-retrieval-executable-plan-20260611.md`

## 3. 不变量

- 不默默修改第三方公开接口、URL、鉴权、请求字段、响应字段。
- 不把 self-test、fixture、只读 preflight 说成真实 live 验收。
- 不记录或提交密钥、cookie、bearer、数据库 URL、原始客户行、完整客户文档、provider payload、私有 object path。
- 8 服务器部署必须明确记录 pull、build、restart、smoke；120 服务器不在本计划范围。
- 质量门禁继续保持禁用或被动采样，不恢复会阻断正常回答的硬门禁。
- 数据接入分析只生成安全摘要和 staging plan；生产写入、schema 变更、覆盖导入必须人工确认。
- 静态页和报表默认优先复用已发布模板；只有明确要求换风格或无可用模板时才走高成本 Image2 流程。
- 低负载预热可以后台做，但不能打扰客户，也不能影响实时问答。

## 4. 已完成且可作为当前事实

### 4.1 计划和文档收口

- `docs/plans/` 已只保留一份主计划。
- 旧计划已移入 `docs/archive/plans/`。
- `.gitattributes` 已加入 shell 脚本 LF 规则。
- `docs/operations/placeholder-stub-inventory.md` 已记录 placeholder/stub 盘点。
- `scripts/smoke/production-placeholder-readiness.mjs` 已加入，并挂到 `package.json` 的 `smoke:production-placeholder-readiness`。

### 4.2 报表和后端收口

- `report-planner-worker` 已从 placeholder skeleton 改为确定性业务模板 AST：
  - `schema_version=0.2.0`
  - `planner_mode=deterministic_business_template`
- `platform-api` 已拆出以下模块，降低 `lib.rs` 聚集风险：
  - `model_gateway_runtime.rs`
  - `model_gateway_admin.rs`
  - `model_gateway_status.rs`
  - `auth_session_support.rs`
  - `external_channel_support.rs`
- 上述拆分未改变公开路由、鉴权、请求字段或响应字段。

### 4.3 本地验证

已通过：

```bash
npm run smoke:production-placeholder-readiness -- --self-test
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
node --test apps/web/app/lib/codex-customer-artifacts.test.mjs
npm --prefix apps/web run build
cargo test -p report-planner-worker
cargo test -p platform-api gateway_limiter --lib
cargo test -p platform-api model_gateway_profile --lib
cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib
cargo test -p platform-api auth_session_endpoint_returns_current_user --lib
cargo test -p platform-api external_action_dispatch_auth_uses_only_explicit_dispatch_credentials --lib
cargo fmt --check
git diff --check
```

### 4.4 8 服务器验证

已通过：

```bash
npm run smoke:production-placeholder-readiness -- --env-file /etc/aiv3/aiv3.env --env-file /etc/aiv3/minimax.env --allow-not-ready --json-stdout
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
CUSTOMER_WEB_CODEX_REPO_ROOT=/srv/aiv3/repo npm run smoke:customer-web-codex-readiness -- --json-stdout --allow-not-ready
npm run smoke:customer-web-codex-live -- --preflight --allow-missing-gates --base-url https://v3.elepcloud.com
node scripts/smoke/model-gateway-operator.mjs --base-url https://v3.elepcloud.com --allow-missing-credentials
npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com --timeout-ms 120000 --require-live-delta --require-multiple-deltas
```

8 服务器 HTTP smoke 已通过：

```bash
curl -I -L https://v3.elepcloud.com/
curl -I -L https://doc.elepcloud.com/
curl -I https://v3.elepcloud.com/external-integrations
curl -I https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
curl -I https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/table-data.csv
curl -I https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/report.md
```

主站真流式 smoke 回执：

- `createDeltaCount=20`
- `continueDeltaCount=221`
- `createFirstDeltaAtMs=1419`
- `continueFirstDeltaAtMs=1318`
- 8 服务器回执：`/srv/aiv3/repo/target/main-assistant-streaming-smoke-52f1e39/20260611154652.json`

第三方凭据型 live smoke 已通过：

- 普通第三方问答小规模线上检查：`concurrency=3`，`okCount=3`，回执 `/srv/aiv3/repo/target/external-channel-ordinary-smoke-52f1e39-small/20260611155845.json`。
- 第三方 SSE/报表触发小规模线上检查：普通问答 2、静态页报表 1、断线重连 1，`okCount=4`，`artifactCount=1`，`duplicateFinalMessageCount=0`，回执 `/srv/aiv3/repo/target/external-channel-streaming-smoke-52f1e39-small/20260611155856.json`。
- 第三方临时文档/会话范围线上检查：`caseCount=4`，回执 `/srv/aiv3/repo/target/external-scoped-document-chat-smoke-52f1e39/20260611155933.json`。
- 第三方报表导出字段线上检查：JSON/SSE 两种模式 `okCount=2`，标题 `新世界百货经营管理月报表`，focus `取高机会`，导出字段和文件检查覆盖 `table-data.csv`、`report.ppt`、`report.md`，回执 `/srv/aiv3/repo/target/external-report-export-smoke-52f1e39-live/20260611160219.json`。
- 上述 smoke 使用服务器侧已有 `generic-chat-main` inbound bearer 注入子进程；token 未打印、未提交、未写入回执。

主站浏览器级回归已通过：

- `https://doc.elepcloud.com/` 未登录状态可直接发起普通聊天；测试问题为 `浏览器回归测试：请用三句话介绍 DataMax。`。
- 线上页面显示思考摘要和最终答案；生成结束后 `正在生成回复` 消失。
- 消息容器 `.chat-messages` 实测 `scrollHeight=540`、`clientHeight=446`、`scrollTop=94`，`nearBottom=true`，证明长回复后自动滚到底。
- 按真实路径点击顶部“当前对话”菜单，再点击“新建对话”，页面切到空白草稿；原对话仍出现在列表中。
- 点回原对话后，原问题和最终答案恢复，消息容器仍 `nearBottom=true`。
- `https://v3.elepcloud.com/` 显示管理入口落地页，含 `DATAMAX V3`、`企业级数据处理助手`、`管理台登录`、`公开接口文档`，且不显示主站直接聊天输入框。

## 5. P0：凭据型线上 smoke 补齐

### 目标

把目前只能做 self-test/preflight 的第三方链路，补成真实线上验收。

### 已完成

- 第三方普通问答。
- 第三方临时上传文档加入当前问答范围。
- 第三方 SSE 流式输出。
- 第三方真实报表触发，返回固定结构字段里的报表 URL。
- 有报表时文本只出现一个可点击链接，不重复刷屏。

### 待补

- 模型网关 operator smoke 使用合法 operator 会话或 approved receipt 验收。

### 前置条件

- 合法 operator session/cookie，或由运维侧提供不含密钥的执行回执。
- 验证过程不得打印 bearer、cookie、数据库 URL、原始客户数据。

### 建议命令

```bash
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 5 --timeout-ms 120000
npm run smoke:external-channel-streaming-10way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --normal-count 3 --static-page-count 1 --reconnect-count 1 --timeout-ms 180000
node scripts/smoke/model-gateway-operator.mjs --base-url https://v3.elepcloud.com
```

### 完成标准

- 验证结果写入 `docs/validation/datamax-main-gap-closure.md`。
- 若缺凭据，明确记录 `pending credential`，不得写成通过。
- 若失败，先定位线上配置、数据、代码差异，不用话术掩盖失败。

## 6. P0：主站聊天体验回归

### 目标

确保主站面对真实用户时可用，不只是在接口层可用。

### 已完成

- 输出追加时消息框自动滚动到底部。
- 进度、思考、执行状态可以展示安全摘要，但不能泄露 raw prompt、密钥、provider payload。
- 新对话按钮不能导致当前会话列表或当前对话丢失。
- `doc.elepcloud.com` 保持无需登录即可直接问答。
- `v3.elepcloud.com` 保持管理台入口。

### 重点文件

- `apps/web/app/HomePageClient.js`
- `apps/web/app/globals.css`
- `apps/web/app/lib/assistant-run-progress.test.mjs`
- `apps/web/app/lib/local-chat-sessions.test.mjs`

### 验证

```bash
node --test apps/web/app/lib/assistant-run-progress.test.mjs
node --test apps/web/app/lib/local-chat-sessions.test.mjs
npm --prefix apps/web run build
npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com --timeout-ms 120000 --require-live-delta --require-multiple-deltas
```

浏览器级验证已经完成，详见 `docs/validation/datamax-main-gap-closure.md` 的 `2026-06-12 Main-Site Browser Interaction Smoke`。

## 7. P0：第三方报表触发和新百模板

### 目标

第三方客户表达经营分析意图时，系统一边正常回答，一边触发报表链接，不截断对话。

### 规则

- 新百经营类需求默认使用 `xinbai-functional-modular-template-20260604`。
- 触发词只在新百或经营数据上下文内放宽，不全局放宽。
- 可触发词包括：
  - `取高`
  - `经营状况`
  - `经营情况`
  - `风险识别`
  - `低活跃品牌`
  - `销售缺口`
  - `需要助推`
  - `经营健康度`
- 不应误触发：
  - `取高是什么意思？`
  - `风险识别系统有哪些项目经历？`
- 报表名称使用 `新世界百货经营管理月报表`。
- 输出文本只保留一个主链接。
- 固定结构字段必须带：
  - `index.html`
  - `table-data.csv`
  - `report.ppt`
  - `report.md`

### 验证

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api external_channel_static_page_artifact --lib
```

凭据齐备后，再补第三方真实触发 smoke。

## 8. P1：模板复用和低负载预热

### 目标

提高报表返回速度，降低 Image2 消耗。

### 规则

- 相同数据集组合优先复用已有模板。
- 数据集有交集时可复用模板。
- `default_prompt` 相同或相近时优先复用同模板。
- 用户没有明确要求报表时，可以低优先级预热模板，但不主动回复客户。
- 已有模板时不重复走 Image2。
- Cloudflare/Codex 不可用时，必须能本地生成可用页面或返回明确状态。

### 验证

```bash
CC=clang CXX=clang++ cargo test -p static-page-worker --lib
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
```

## 9. P1：数据接入和数据源页面

### 目标

让主站数据源页面能表达 DataMax 的接入能力，并让 CC 模式或 Codex 执行器能把新接入统一落到目标数据集。

### 待处理

- 数据源页面列出接入方式：
  - 网页采集
  - 数据库接入
  - ERP 登录
  - API 对接
  - MCP 对接
  - 文件和模板上传
- 已接入数据做成可展开列表。
- 页面除首页外增加滚动能力，避免内容被截断。
- CC 模式下允许 Codex 接管数据库/API 接入，但必须落到明确目标数据集。
- 数据接入分析只生成 staging plan，不自动写生产。

### 验证

```bash
npm --prefix apps/web run build
npm run smoke:data-ingestion-analysis -- --self-test
```

## 10. P1：第三方临时文档和权限范围

### 目标

第三方一次对话中传入临时文档时，能够进入本轮和同会话后续问答范围。

### 规则

- `dataset_external_ids` 表示本会话可用的数据集分组权限。
- `available_document_external_ids` 表示本会话额外可用文档权限。
- 文档和分组可以同时传。
- 如果文档已经在分组内，去重即可。
- 授权按会话 ID 持续有效，直到会话 ID 变化。
- 第三方内部权限不由 DataMax 推断；传入授权即视为允许使用。

### 验证

```bash
npm run smoke:external-channel-scoped-documents -- --self-test
npm run smoke:external-channel-temporary-attachment -- --self-test
```

凭据齐备后补线上真实临时文档问答。

## 11. P1：模型网关和并发

### 目标

支持主站和第三方 20 路问答并发，复杂报表/HTML/Image2 任务限流执行。

### 当前策略

- 普通问答目标：20 路并发。
- 复杂任务目标：本地 5 路并发。
- Cloudflare/Codex 兜底目标：2 路并发。
- RightCode 主力模型优先，MiniMax fallback 必须鉴权正确。
- operator/status 接口只展示 provider、lane、计数、状态，不展示密钥。

### 验证

```bash
CC=clang CXX=clang++ cargo test -p platform-api gateway_limiter --lib
CC=clang CXX=clang++ cargo test -p platform-api model_gateway_profile --lib
CC=clang CXX=clang++ cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib
CC=clang CXX=clang++ cargo test -p platform-api parse_worker_pool_concurrency_prefers_first_valid_value_and_clamps --lib
```

## 12. P2：企业记忆和解析深化

### 目标

在不依赖模型侧记忆的前提下，持续增强企业事实库和文档后处理能力。

### 方向

- 文档入库后异步深化解析，不阻塞上传。
- 可选深化任务：
  - 表格结构抽取
  - 实体和别名抽取
  - 程序步骤抽取
  - 时间有效性和事实来源记录
  - 合同面积、客流、租售比等经营字段抽取
- 对 8 服务器本地重复文档做安全去重规划。
- 对问答低质量样本做被动采样和人工/自动复盘，但不恢复硬门禁。

### 验证

```bash
CC=clang CXX=clang++ cargo test -p retrieval-worker --lib
CC=clang CXX=clang++ cargo test -p memory-worker --lib
```

历史文档 backfill 必须使用 dry-run 和 summary-only，真实写入需要单独确认。

## 13. P2：工程治理

### 目标

降低长期维护成本。

### 待处理

- 继续拆小 `platform-api/src/lib.rs` 中的高风险大块逻辑。
- 前端 `HomePageClient.js` 按页面和 hook 拆分。
- 清理旧静态页模板，只保留当前主模板和少量必要历史样例。
- 为新增 smoke 补 README，明确哪些是 self-test、哪些是 live。
- 补 CI 最小矩阵：
  - Rust fmt/test
  - Web build
  - 报表 focus/export self-test
  - placeholder readiness self-test

## 14. 下一步执行顺序

1. 补 model-gateway operator 鉴权 smoke；当前 8 服务器未配置专用 operator smoke cookie/email/local-key，不能绕过。
2. 如发现线上失败，优先定位配置、数据、部署差异；确认不是环境问题后再改代码。
3. P0 剩余验证稳定后，推进 P1 的模板预热、数据源页面、临时文档权限增强。
