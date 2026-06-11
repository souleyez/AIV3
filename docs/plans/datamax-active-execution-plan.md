# DataMax 主线补缺执行计划

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.
>
> 本文件是 `docs/plans/` 下唯一主线计划。历史计划只保留在 `docs/archive/plans/`；验证证据写入 `docs/validation/`，不要再新增散落计划。

**Goal:** 把 DataMax 稳定在“主站可问、第三方可接、报表可出、数据可入、解析可深化、部署可回归”的生产状态。

**Architecture:** 主站和第三方入口只处理会话、权限、任务编排和结果展示；企业资料、会话授权、报表模板、静态页产物、数据接入状态都由 DataMax 平台侧管理。模型调用保持无状态，只接收本轮由平台裁剪后的供料和工具结果。

**Tech Stack:** Rust workspace services, Next.js `apps/web`, Node smoke scripts, PostgreSQL, static-page/report workers, Cloudflare/Codex executor where available, 8 服务器 systemd deployment.

---

## 1. 维护规则

- `docs/plans/` 只保留本文件。
- 本文件只记录当前状态、执行队列、验收命令和完成标准；长日志、回执、失败定位写入 `docs/validation/datamax-main-gap-closure.md`。
- 不默默修改第三方公开接口、URL、鉴权、请求字段、响应字段；确需变更先问。
- 不提交或记录密钥、cookie、bearer、数据库 URL、provider payload、原始客户行、完整客户文档、私有对象路径。
- 不把 self-test、fixture、只读 preflight 写成真实 live 验收。
- 质量门禁保持禁用或被动采样，不恢复会拦截正常回答的硬门禁。
- 数据接入分析只产出安全摘要和 `staging_plan`；生产写入、schema 变更、覆盖导入必须人工确认。
- 8 服务器部署必须记录 pull、build、restart、smoke；纯文档或纯 smoke 脚本更新不重启服务。
- 不动 120 服务器。

## 2. 当前生产基线

已验证并作为后续回归基线：

- `https://doc.elepcloud.com/` 是无需登录直接问答主站。
- `https://v3.elepcloud.com/` 是管理台和第三方接口文档入口。
- 主站普通聊天、自动滚动、新建对话保留原会话、管理台入口隔离已通过浏览器回归。
- 第三方普通问答、SSE/报表、临时文档范围、报表导出字段已通过小规模 smoke。
- 新百经营月报默认模板为 `xinbai-functional-modular-template-20260604`。
- 报表名称统一为 `新世界百货经营管理月报表`。
- 报表导出文件为 `table-data.csv`、`report.ppt`、`report.md`。
- 数据源页已展示网页采集、数据库接入、ERP 登录、API 对接、MCP 对接、文件与模板接入，并支持已接入对象折叠展示。
- Cloudflare/Codex executor 不可用时，静态页能力优先验证本地模板复用和本地生成通路。
- P2 解析深化已在 8 服务器单文档小样本完成 summary-only dry-run：fingerprint would-record、fact-index 56 条派生事实、enrichment 2 个 would-enqueue；未写入、未入队。

详细证据入口：`docs/validation/datamax-main-gap-closure.md`。

## 3. P0 发布回归门

### P0-1 第三方问答与范围授权

**目标:** 第三方普通问答、临时文档、数据集分组、同会话持续授权稳定可用。

**规则:**
- `dataset_external_ids` 是稳定业务分组权限，可多个。
- `available_document_external_ids` 是额外文档权限，可与分组同时传。
- 文档已在分组内时去重。
- 授权按 conversation/session 持续，直到会话 ID 变化。

**验证命令:**

```bash
npm run smoke:external-scoped-document-chat -- --self-test
npm run smoke:external-video-ppt -- --self-test
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 5 --timeout-ms 120000
```

**完成标准:** 外部临时文档问答可命中；范围变更不污染其他会话；live 回执写入 validation。

### P0-2 主站聊天体验

**目标:** 主站直接问答在多轮、长回复、流式进度下可用。

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

### P0-3 新百报表触发、焦点和导出

**目标:** 新百经营类需求能触发报表，但不截断正常回答，不误触发非经营报表问题。

**触发词范围:**
- 应触发：`取高`、`经营状况`、`经营情况`、`风险识别`、`低活跃品牌`、`销售缺口`、`需要助推`、`经营健康度`。
- 不应触发：`取高是什么意思？`、`风险识别系统有哪些项目经历？`。

**验证命令:**

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api external_channel_static_page_artifact --lib
npm run validate:xinbai-report-template
```

**完成标准:** 焦点正确；链接可点且只出现一次；固定结构字段含 `index.html`、`table-data.csv`、`report.ppt`、`report.md`。

## 4. P1 近期补缺

### P1-1 模型网关与 20 路并发

**目标:** 主站和第三方普通问答支持 20 路并发；复杂静态页/Image2/HTML 任务本地 5 路，Cloudflare fallback 2 路。

**待完成:**
- 拿到合法 operator 会话或运维安全回执后，跑 authenticated model-gateway operator smoke。
- 在部署窗口跑主站 20 路和第三方 20 路真实抽样。
- 证明 RightCode 主力模型不可用或耗尽时能正确切 MiniMax fallback。

**已补自检:**
- `npm run smoke:main-chat-20way -- --self-test` 验证主站 20 路脚本的 payload、summary、percentile 和 poll-result fixture，不调用 DataMax。
- `npm run smoke:external-channel-20way -- --self-test` 验证第三方 20 路脚本的 payload、SSE parser、summary 和 percentile fixture，不调用 DataMax。

**验证命令:**

```bash
CC=clang CXX=clang++ cargo test -p platform-api gateway_limiter --lib
CC=clang CXX=clang++ cargo test -p platform-api model_gateway_profile --lib
CC=clang CXX=clang++ cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib
npm run smoke:main-chat-20way -- --self-test
npm run smoke:external-channel-20way -- --self-test
npm run smoke:main-chat-20way -- --base-url https://doc.elepcloud.com --concurrency 20 --timeout-ms 180000
npm run smoke:external-channel-20way -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --concurrency 20 --timeout-ms 180000
node scripts/smoke/model-gateway-operator.mjs --base-url https://v3.elepcloud.com
```

**完成标准:** 普通问答 20 路稳定；复杂任务排队可观测；operator 状态不泄密。

### P1-2 静态页复用、预热和 fallback

**目标:** 大多数报表需求优先命中模板；低负载时后台预热；Cloudflare/Codex 不通时本地链路仍能给出可用页面或明确状态。

**已验证:**
- `npm run smoke:static-page-5way -- --self-test`
- `npm run smoke:static-page-prewarm-observability -- --self-test`
- `npm run smoke:cloudflare-fallback-2way -- --self-test`
- `CC=clang CXX=clang++ cargo test -p static-page-worker --lib`

**待完成:**
- 部署窗口内跑真实 5 路静态页 live smoke。
- 对低负载预热任务跑部署目标只读 queue-stats smoke，持续观察 queued、running、published、failed、skipped_existing_template、waiting_for_low_load。
- 确认相同数据集组合或有交集的数据集组合优先复用模板。

**完成标准:** 用户要报表时优先复用模板；后台预热不主动打扰用户；失败状态可从管理台或 smoke receipt 追踪。

### P1-3 数据接入与 CC 模式

**目标:** CC/Codex 可以协助数据库/API 接入，但必须落到明确目标数据集；生产写入前保持人工确认。

**已验证:**
- 数据源页接入方式介绍和已接入对象折叠展示。
- `data_ingestion_analysis` 可产出 `staging_plan`。
- MySQL source sync 必须解析到有效目标数据集。
- staging-sync self-test 和 live-readiness self-test 已在本地和 8 服务器通过。

**验证命令:**

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case data-ingestion-analysis -Json
```

```bash
bash scripts/run-data-ingestion-staging-live-smoke.sh --self-test
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

**完成标准:** 产出 `staging_plan`，不写生产；目标数据集明确；无密钥和 raw 连接信息泄漏。

## 5. P2 解析深化与企业事实库

### P2-1 异步深化解析

**目标:** 文档入库后，在空闲队列持续抽取结构化事实，不阻塞上传和即时问答。

**候选能力:**
- 表格结构抽取。
- 实体、别名、组织、人名、岗位、项目经历抽取。
- 操作步骤、护理规范、合同指标、面积、客流、租售比抽取。
- 事实有效期、来源 episode、证据 provenance。

**下一步:**
1. 继续用更多对象可达小样本跑 summary-only dry-run，扩大文档类型覆盖。
2. 汇总不同文档类型的 would-enqueue、would-record、derived-fact 规模。
3. 明确哪些事实进入检索增强，哪些进入报表聚合，哪些只做证据索引。
4. 真实历史 backfill 前单独确认范围、批量、回滚方式。

**完成标准:** 默认 dry-run 和 summary-only；真实写入或入队必须另行确认。

**当前证据:** 8 服务器已完成同一数据集 limit5 summary-only dry-run：fingerprint `would_record_count=5` 且 `duplicate_count=5`，fact-index `derived_fact_count=281`，enrichment `would_enqueue_count=2`；均未写入、未入队。

**固定入口:**

```bash
npm run smoke:p2-summary-only-dry-run -- --self-test
npm run smoke:p2-summary-only-dry-run -- --dataset-id <dataset-uuid> --limit 5 --env-file /etc/aiv3/aiv3.env
```

该入口只封装 `document-fingerprint-backfill`、`fact-index-backfill`、`document-enrichment-backfill` 的 `--dry-run --summary-only --pretty` 路径，不提供 `--confirm-real-run`。

**8 服务器状态:** 固定入口已在 8 服务器通过 self-test 和同数据集 limit5 dry-run；结果仍为不写入、不入队。

### P2-2 重复文档与对象治理

**目标:** 8 服务器本地文档按解析内容和 fingerprint 去重，文档可归属多个数据集，不重复存储不可控副本。

**下一步:**
1. 对对象可达样本做 fingerprint summary-only 统计。
2. 对对象缺失样本只记录缺失原因，不重试下载、不删除。
3. 区分“索引映射去重”和“对象文件清理”两类动作。
4. 清理对象文件前必须给出 dry-run、影响清单和回滚方案。

**完成标准:** 不直接删除源文件；不改变数据集可见权限；清理前有可审计预检。

## 6. P3 工程治理

**目标:** 降低长期维护成本，减少大文件改动和 smoke 不可复现。

**任务:**
- 继续拆小 `crates/platform-api/src/lib.rs` 中高风险逻辑。
- 前端按页面、hook、数据适配拆分 `HomePageClient.js`。
- `scripts/README.md` 持续标注 self-test、preflight、live 的边界。
- CI 最小矩阵已落到 `.github/workflows/datamax-ci.yml`，覆盖 Node smoke syntax/self-test、外部报表 focus/export、静态页/fallback/预热可观测、placeholder readiness、Web build、Rust fmt、模型网关最小测试和 static-page-worker 测试。
- CI 首次 GitHub Actions 运行被 GitHub 账号付款/额度限制拦截，job 未启动；额度恢复前，用本地和 8 服务器等价命令作为临时回归依据。
- CI 恢复后需要回看首轮真实 Actions 结果；如果失败，优先修 workflow 环境差异，不改线上接口。

## 7. 8 服务器发布流程

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

## 8. 下一步队列

1. 恢复 GitHub Actions 账号额度后，重跑 DataMax CI 首轮真实 Actions，并把失败项或通过回执写入 validation。
2. 补 P1-1：获取合法 operator 会话或运维安全回执后跑 authenticated model-gateway smoke。
3. 部署窗口跑主站 20 路和第三方 20 路真实抽样，记录 p50、p95、max、失败原因。
4. 跑真实静态页 5 路 live smoke，确认本地模板复用、后台预热、fallback 状态。
5. P2 继续对象可达小样本 summary-only，扩大文档类型覆盖；不做真实历史 backfill 或对象清理，除非用户单独确认。
6. 继续拆分 `platform-api` 和主站前端大文件，但每次只收可回归的小改动。
