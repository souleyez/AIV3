# DataMax 主线执行计划

> **For Codex:** 本文件是唯一主线计划。执行时按“下一步队列”逐项推进；验证证据写入 `docs/validation/`，不要继续新增散落计划。

**Goal:** 把 DataMax 生产主线稳定在“主站可问、第三方可接、报表可出、数据可入、解析可深化、部署可回归”的状态。

**Architecture:** 主站和第三方入口只负责会话、权限、任务编排和结果展示；企业资料、会话授权、报表模板、静态页产物、数据接入状态都由 DataMax 平台侧管理。模型调用保持无状态，模型只接收本轮经平台裁剪后的供料和工具结果。

**Tech Stack:** Rust workspace services, Next.js `apps/web`, Node smoke scripts, PostgreSQL-backed platform state, static-page/report workers, Cloudflare/Codex executor where可用, 8 服务器 systemd 部署。

---

## 1. 使用规则

- `docs/plans/` 只保留本文件；历史计划放 `docs/archive/plans/`。
- 本文件只放当前状态、任务、命令和完成标准；长回执、日志摘要和失败定位写到 `docs/validation/datamax-main-gap-closure.md` 或对应专项 validation。
- 不默默修改第三方公开接口、URL、鉴权、请求字段、响应字段；确需变更先问。
- 不提交或记录密钥、cookie、bearer、数据库 URL、provider payload、原始客户行、完整客户文档、私有对象路径。
- 不把 self-test、fixture、只读 preflight 写成真实 live 验收。
- 质量门禁保持禁用或被动采样，不恢复会拦截正常回答的硬门禁。
- 数据接入分析只产出安全摘要和 staging plan；生产写入、schema 变更、覆盖导入必须人工确认。
- 8 服务器部署必须记录 pull、build、restart、smoke；不动 120 服务器。

## 2. 当前基线

- 入口分工：`https://doc.elepcloud.com/` 是无需登录直接问答主站；`https://v3.elepcloud.com/` 是管理台和第三方接口文档入口。
- 第三方主链路已通过小规模 live smoke：普通问答、SSE/报表、临时文档范围、报表导出字段。
- 主站浏览器回归已通过：普通聊天可用，生成后自动滚到底，新建对话不丢原会话，`v3.elepcloud.com` 不出现主站直接聊天框。
- 新百经营月报默认模板为 `xinbai-functional-modular-template-20260604`；报表名称为 `新世界百货经营管理月报表`；导出文件为 `table-data.csv`、`report.ppt`、`report.md`。
- 数据源页已具备接入方式介绍：网页采集、数据库接入、ERP 登录、API 对接、MCP 对接、文件与模板；已接入对象以可展开分组展示。
- 模型网关 operator 状态面仍需要合法 operator 会话或运维侧安全回执才能做 authenticated smoke；不得加绕过口。
- 详细证据入口：`docs/validation/datamax-main-gap-closure.md`。

## 3. 当前不做

- 不在本轮恢复会阻断回答的质量门禁。
- 不清理或删除线上旧静态页产物，除非用户单独确认清理范围。
- 不把 Cloudflare/Codex executor 不可用当成静态页失败的唯一原因；优先验证本地模板复用和本地生成通路。
- 不用模型侧记忆承载企业记忆；会话恢复、资料范围和供料裁剪都在 DataMax 平台侧完成。

## 4. P0：发布前必须稳定

### P0-1 第三方主链路

**状态:** 基本完成，后续每次部署抽样回归。

**覆盖范围:**
- 普通第三方问答。
- 临时文档和数据集范围复用。
- SSE 流式进度和最终答案。
- 报表触发不截断正常回答。
- 报表链接只出现一次，并写入第三方固定结构字段。

**验证命令:**

```bash
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 5 --timeout-ms 120000
npm run smoke:external-channel-streaming-10way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --normal-count 3 --static-page-count 1 --reconnect-count 1 --timeout-ms 180000
npm run smoke:external-scoped-document-chat -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main
npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main
```

**完成标准:** live receipt 记录到 validation；token 只从服务器侧环境或私有配置注入，不打印。

### P0-2 主站聊天体验

**状态:** 已通过浏览器级回归，部署后继续抽查。

**重点文件:**
- `apps/web/app/HomePageClient.js`
- `apps/web/app/components/WorkspaceDirectoryPanel.js`
- `apps/web/app/globals.css`
- `apps/web/app/lib/assistant-run-progress.test.mjs`
- `apps/web/app/lib/local-chat-sessions.test.mjs`

**验证命令:**

```bash
node --test apps/web/app/lib/assistant-run-progress.test.mjs
node --test apps/web/app/lib/local-chat-sessions.test.mjs
npm --prefix apps/web run build
npm run smoke:main-assistant-streaming -- --base-url https://v3.elepcloud.com --timeout-ms 120000 --require-live-delta --require-multiple-deltas
```

**完成标准:** 未登录问答可用；长回复自动滚动；新建对话不丢历史；思考/进度只展示安全摘要。

### P0-3 新百报表和模板默认行为

**状态:** 基本完成，重点防回归。

**规则:**
- 新百经营数据上下文内，`取高`、`经营状况`、`经营情况`、`风险识别`、`低活跃品牌`、`销售缺口`、`需要助推`、`经营健康度` 可触发报表。
- `取高是什么意思？`、`风险识别系统有哪些项目经历？` 不应误触发报表。
- 默认复用当前主模板，除非用户明确要求换风格。
- 有报表时可以继续正常回答，不截断对话。

**验证命令:**

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api external_channel_static_page_artifact --lib
npm run validate:xinbai-report-template
```

**完成标准:** 触发焦点正确；链接可点且只出现一次；固定结构字段含 `index.html`、`table-data.csv`、`report.ppt`、`report.md`。

## 5. P1：近期优化

### P1-1 静态页复用、低负载预热和 fallback

**目标:** 大多数报表需求优先命中模板，低负载时后台预热，Cloudflare/Codex 不通时本地链路仍能给出可用页面或明确状态。

**当前动作:**
- `npm run smoke:static-page-5way -- --self-test` 用 fixture 验证五路静态页 payload、artifact/status URL、终态失败识别。
- `npm run smoke:cloudflare-fallback-2way -- --self-test` 用 fixture 验证 Codex host 2 路限制和 watched queue 统计。

**验证命令:**

```bash
node --check scripts/smoke/static-page-5way.mjs
node --check scripts/smoke/cloudflare-fallback-2way.mjs
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
CC=clang CXX=clang++ cargo test -p static-page-worker --lib
```

**下一步:** 部署窗口内把这两个 self-test 在 8 服务器也跑一遍，再决定是否跑真实 5 路静态页 live smoke。

**8 服务器验证:** 已在 `c14e97091` 通过 `static-page-5way --self-test` 和 `cloudflare-fallback-2way --self-test`；本批次未重启服务。

### P1-2 数据源页面和 CC 数据接入

**目标:** 主站清楚表达接入能力；CC 模式或 Codex 执行器可以协助数据库/API 接入，但必须落到明确目标数据集。

**已完成:**
- 数据源页已展示六类接入方式。
- 已接入对象已按类型折叠展示。
- 页面内容可滚动，不应被视口截断。
- 本地 CC/data-ingestion 回归已通过：CC 数据接入会携带目标数据集要求；无来源时保持只读；MySQL source sync 必须解析到有效目标数据集。
- 本地 staging-sync smoke 已通过：data-ingestion analysis、staging plan、confirm/sync contract、external-source materialization、ingest、retrieval、live readiness self-test、公开第三方指南检查。

**待补:**
- 数据库接入样例只读摘要，不展示 raw 连接串、密码、原始表 dump。

**8 服务器验证:** 已在 `c14e97091` 通过 `run-data-ingestion-staging-sync-smoke.sh`、live readiness self-test 和公开第三方指南检查；本批次未重启服务。

**验证命令:**

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis -Json
```

```bash
DATA_INGESTION_LIVE_SMOKE_SELF_TEST=true DATA_INGESTION_LIVE_SMOKE_REPORT_DIR=target/data-ingestion-staging-live-smoke-self-test bash scripts/run-data-ingestion-staging-live-smoke.sh
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

**完成标准:** 产出 `staging_plan`，不写生产；目标数据集明确；无密钥和 raw 连接信息泄漏。

### P1-3 第三方临时文档和权限范围

**目标:** 第三方一次对话中传入的临时文档、数据集分组和额外文档权限，在同会话后续持续生效。

**状态:** 本地和 8 服务器 self-test 已通过；线上 live 回归沿用 P0 第三方凭据型 smoke，在部署窗口抽样执行。

**规则:**
- `dataset_external_ids` 是稳定业务分组权限，可多个。
- `available_document_external_ids` 是额外文档权限，可与分组同时传。
- 文档已在分组内时去重。
- 授权按 conversation/session 持续，直到会话 ID 变化。

**验证命令:**

```bash
npm run smoke:external-scoped-document-chat -- --self-test
npm run smoke:external-video-ppt -- --self-test
```

**完成标准:** 外部临时文档问答可命中；范围变更不会污染其他会话。

### P1-4 模型网关和 20 路并发

**目标:** 主站和第三方普通问答支持 20 路并发；复杂静态页/Image2/HTML 任务本地 5 路，Cloudflare fallback 2 路。

**待补:**
- authenticated model-gateway operator smoke。
- 第三方 20 路和主站 20 路在部署窗口的真实回归。
- MiniMax fallback 鉴权和 RightCode 主力模型耗尽后的切换证据。

**本地已验证:**
- gateway limiter、model gateway profile、status redaction、worker-pool concurrency clamp 测试已通过。
- 无凭据 operator smoke 返回 `pending=true`、`failed=false`，说明管理面仍受 operator 会话保护。

**验证命令:**

```bash
CC=clang CXX=clang++ cargo test -p platform-api gateway_limiter --lib
CC=clang CXX=clang++ cargo test -p platform-api model_gateway_profile --lib
CC=clang CXX=clang++ cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib
npm run smoke:main-chat-20way -- --base-url https://doc.elepcloud.com --concurrency 20 --timeout-ms 180000
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20 --timeout-ms 180000
node scripts/smoke/model-gateway-operator.mjs --base-url https://v3.elepcloud.com
```

**完成标准:** 普通问答 20 路稳定；复杂任务排队可观测；operator 状态不泄密。

## 6. P2：解析深化和企业事实库

### P2-1 异步深化解析

**目标:** 文档入库后，在空闲队列持续抽取结构化事实，不阻塞上传和即时问答。

**候选能力:**
- 表格结构抽取。
- 实体、别名、组织、人名、岗位、项目经历抽取。
- 操作步骤、护理规范、合同指标、面积、客流、租售比抽取。
- 事实有效期、来源 episode、证据 provenance。

**验证命令:**

```bash
CC=clang CXX=clang++ cargo test -p retrieval-worker --lib
CC=clang CXX=clang++ cargo test -p memory-worker --lib
```

**完成标准:** 先 dry-run 和 summary-only；真实历史 backfill 需要单独确认。

**8 服务器验证:** 已在单文档小样本上完成 summary-only dry-run：fingerprint would-record、fact-index 56 条派生事实、enrichment 2 个 would-enqueue；未写入、未入队。

### P2-2 重复文档和本地对象治理

**目标:** 8 服务器本地文档按解析内容和 fingerprint 去重，文档可归属多个数据集，不重复存储不可控副本。

**待补:**
- 找一组对象可达的非客户样本做 fingerprint/enrichment dry-run。
- 对历史对象缺失原因做 summary-only 统计。
- 明确哪些去重动作只改索引映射，哪些会影响对象文件。

**完成标准:** 不直接删除源文件；清理前有 dry-run 和可回滚方案。

**8 服务器验证:** 已使用对象可达的单文档小样本完成 fingerprint/fact-index/enrichment summary-only dry-run，证明后续治理可以先走可审计预检，不直接删除或写入。

## 7. P3：工程治理

**目标:** 降低长期维护成本，减少“大文件一改全动”和 smoke 不可复现的问题。

**任务:**
- 继续拆小 `crates/platform-api/src/lib.rs` 中高风险逻辑。
- 前端按页面、hook、数据适配拆分 `HomePageClient.js`。
- 为新增 smoke 补 `scripts/README.md`，明确 self-test、preflight、live 的边界。
- CI 最小矩阵覆盖 Rust fmt/test、Web build、外部报表 focus/export self-test、静态页 self-test、placeholder readiness self-test。

## 8. 8 服务器发布流程

只在用户要求部署时执行。

```bash
git pull --ff-only origin main
npm --prefix apps/web run build
CC=clang CXX=clang++ cargo build --release -p platform-api
sudo systemctl restart aiv3-platform-api.service
sudo systemctl restart aiv3-web.service
sudo systemctl restart aiv3-assistant-run-worker.service
sudo systemctl restart aiv3-chat-session-worker.service
sudo systemctl restart aiv3-static-page-worker.service
systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-chat-session-worker.service aiv3-static-page-worker.service
```

按改动范围追加对应 worker build/restart。纯文档或纯 smoke 脚本更新不需要重启服务。

部署后至少跑：

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
npm run smoke:production-placeholder-readiness -- --env-file /etc/aiv3/aiv3.env --env-file /etc/aiv3/minimax.env --allow-not-ready --json-stdout
```

## 9. 下一步队列

1. 完成本轮计划文档重建、脚本 README 更新、两个 static/fallback self-test 验证，并写入 validation。
2. 如果用户要求收版：提交、推 GitHub、8 服务器 pull；纯文档/smoke 脚本不重启服务，只跑 self-test。
3. 补 P1-4：拿到合法 operator 凭据或运维安全回执后跑 model-gateway authenticated smoke。
4. 在部署窗口跑主站 20 路和第三方 20 路并发抽样，记录 p50/p95/max 和失败原因。
5. P2 后续只在明确批准后推进真实历史 backfill 或去重；默认继续使用 dry-run、summary-only 和小批量。
