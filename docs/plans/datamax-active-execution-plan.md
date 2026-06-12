# DataMax 主线缺口补齐 Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.
>
> 本文件是 `docs/plans/` 下唯一有效计划。旧计划只作为历史材料保留在 `docs/archive/plans/`；验证记录统一写入 `docs/validation/datamax-main-gap-closure.md`。

**目标:** 让 DataMax 稳定支撑主站问答、第三方问答、范围授权、报表/静态页生成、数据接入分析、异步文档理解和生产观测，并把剩余工作压缩成可执行、可验证、可回滚的队列。

**架构:** DataMax 平台侧负责企业记忆、会话授权、文档范围、报表模板、任务编排和产物发布。模型调用保持无状态，只接收平台临时裁剪后的上下文、检索证据和工具结果。重任务由 worker、smoke 脚本、本地模板链路和可选 Cloudflare/Codex fallback 承接；生产写入和第三方公开契约变更必须显式确认。

**Tech Stack:** Rust workspace services、Next.js `apps/web`、Node smoke scripts、PostgreSQL、8 服务器 systemd workers、静态页/报表产物、DataMax 模板库、本地生页链路和可选 Cloudflare/Codex fallback。

**重建日期:** 2026-06-12

---

## 1. 维护规则

- `docs/plans/` 只保留本文件；不要再新增分散计划。
- 长历史、旧方向和已关闭计划放 `docs/archive/plans/`；不要从归档里继续执行。
- 本文件只记录当前状态、待办顺序、验收命令和阻塞项；不要写流水账。
- 详细 smoke、回执、8 服务器结果、失败原因写入 `docs/validation/datamax-main-gap-closure.md`。
- 不记录密钥、bearer、cookie、provider key、数据库 URL、原始客户行、原始 provider payload、完整文档内容、私有对象路径、对象 key 或 content hash。
- 不默默修改第三方公开 API URL、鉴权、必填请求字段或已有响应字段；必须变更时先问。
- 不动 120 服务器。
- 质量门禁保持禁用或被动采样；不要恢复会拦截正常回答的硬门禁。
- 数据接入可以产出分析和 `staging_plan`；生产 schema/data 写入必须人工确认。
- P2 解析、fingerprint、事实抽取和对象治理默认只跑 `--dry-run --summary-only` 或只读聚合；真实 backfill、入队、对象清理或源同步必须单独确认。
- 纯文档或纯 smoke 脚本更新不重启 8 服务器服务。

## 2. 当前基线

| 模块 | 已确认 | 仍需处理 |
| --- | --- | --- |
| 主站 | `https://doc.elepcloud.com/` 是无需登录直接问答入口；主站 20 路 self-test 已具备。 | UI/运行时改动后要回归真流式、自动滚动、普通问答不被报表链路截断。 |
| 管理台/文档 | `https://v3.elepcloud.com/` 是管理台和第三方接口文档入口。 | 保持域名分工稳定；不做未经确认的公开契约变更。 |
| 第三方问答 | 20 路并发 self-test 已具备；数据集/文档范围授权已实现。 | 客户新失败样例需要按实际 conversation scope 做 targeted live smoke。 |
| 报表/静态页 | 新百默认模板是 `xinbai-functional-modular-template-20260604`；标准产物包含 `index.html`、`table-data.csv`、`report.ppt`、`report.md`。 | focus 命中、模块前置、链接只出现一次、导出字段可访问需要持续回归。 |
| 数据源/CC 模式 | 数据源页和 staging analysis 链路已存在；CC/Codex 可协助 DB/API/ERP/MCP 接入。 | 所有接入必须落到明确目标数据集；生产同步仍需人工确认。 |
| 企业记忆/P2 | 多类型 summary-only dry-run 已覆盖 DOC/DOCX/PDF/XLSX/PPTX/MP4、简历、制度手册、经营/考勤表格。 | 新客户失败样例、新数据类型、新事实用途策略再扩样；真实写入仍未开放。 |
| fingerprint/对象治理 | inventory、原因聚合、对象清理 dry-run plan、文件系统 preflight 已具备。 | missing object 修复、真实对象清理、重复对象合并仍停留在只读计划阶段。 |
| 生产观测 | 未授权 operator/queue stats 路径已验证返回 401；观测 key 路径返回脱敏聚合。 | authenticated operator live 需要合法 operator 凭证或运维侧脱敏回执。 |
| CI | 本地和 8 服务器 smoke 覆盖主要缺口。 | GitHub Actions 因账号付款/额度状态在 job 启动前失败，账号恢复后重跑。 |

## 3. P0 发布前固定回归

**目标:** 每次准备发 8 服务器前，先确认主站、第三方、报表、静态页、导出字段和 scoped document 没有回归。

**Files:**
- Validate: `scripts/smoke/external-report-focus.mjs`
- Validate: `scripts/smoke/external-report-export.mjs`
- Validate: `scripts/smoke/external-scoped-document-chat.mjs`
- Validate: `scripts/smoke/external-video-ppt.mjs`
- Validate: `scripts/smoke/static-page-5way.mjs`
- Validate: `scripts/smoke/cloudflare-fallback-2way.mjs`
- Validate: `apps/web/app/lib/assistant-run-progress.test.mjs`
- Validate: `apps/web/app/lib/local-chat-sessions.test.mjs`
- Record: `docs/validation/datamax-main-gap-closure.md`

**Commands:**

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
npm run smoke:external-scoped-document-chat -- --self-test
npm run smoke:external-video-ppt -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
node --test apps/web/app/lib/assistant-run-progress.test.mjs
node --test apps/web/app/lib/local-chat-sessions.test.mjs
```

**Rust/frontend touched 时追加:**

```bash
cargo fmt --check
cargo test -p platform-api external_channel_static_page_artifact --lib
npm --prefix apps/web run build
```

**Done when:**
- 普通问答不被报表触发截断。
- 报表链接只出现一次，且第三方结构字段承载产物 URL。
- `table-data.csv`、`report.ppt`、`report.md` 可访问。
- `取高是什么意思？`、`风险识别系统有哪些项目经历？` 不误触发报表。

## 4. P0 部署边界

**目标:** 区分 docs-only 同步、代码发布、服务重启，避免无必要重启。

**Docs-only sync:**

```bash
cd /srv/aiv3/repo
git pull --ff-only origin main
git rev-parse --short HEAD
git status --short --branch
systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-chat-session-worker.service aiv3-static-page-worker.service
```

**Code deploy:**

```bash
cd /srv/aiv3/repo
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

**Done when:**
- docs-only 不 build、不 restart。
- code deploy 后服务全 active。
- validation 写明服务器 head、命令和 smoke 结果。

## 5. P1 生产观测与并发

### P1-1 Operator 观测闭环

**目标:** 看清 model gateway、provider fallback、queue stats 和静态页预热状态，同时不暴露密钥或任务 payload。

**Status:** 未授权路径已完成部署验证；带观测 key 的 queue stats 可返回脱敏聚合。authenticated operator live 仍等待合法 operator cookie/bearer/local-key，或运维侧提供脱敏回执。

**Commands:**

```bash
npm run smoke:model-gateway-operator -- --self-test
npm run smoke:model-gateway-operator -- --base-url https://v3.elepcloud.com --allow-missing-credentials
npm run smoke:static-page-prewarm-observability -- --self-test
npm run smoke:static-page-prewarm-observability -- --base-url https://v3.elepcloud.com --allow-missing-credentials
```

**Done when:**
- 未授权访问仍为 HTTP 401。
- 已授权回执只含 provider lane、限流、fallback、queued/running/published/failed/skipped 状态。
- 回执不含 token、cookie、provider key、任务原文或客户数据。

### P1-2 20 路并发与重任务并发

**目标:** 主站、第三方、静态页和 fallback 都有明确并发验收口径。

**Commands:**

```bash
npm run smoke:main-chat-20way -- --self-test
npm run smoke:external-channel-20way -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
```

**Live target:**
- 主站问答 20 路。
- 第三方问答 20 路。
- 本地 image2/html 重任务 5 路。
- Cloudflare/Codex fallback 2 路。

**Done when:**
- live smoke 在受控窗口通过。
- 失败时能区分模型供应商、队列拥塞、权限、模板命中和产物发布问题。

## 6. P2 企业记忆与文档理解

### P2-1 异步深解析 dry-run 扩样

**目标:** 文档入库后，空闲时可持续深化实体、别名、组织、项目、操作步骤、表格事实、合同指标、面积、客流、租售比、有效期和证据来源。

**Current coverage:**
- DOC/DOCX/PDF/XLSX/PPTX/MP4 小样本。
- 制度手册/护理流程类 Word 样本。
- 简历类 14 份 PDF 样本。
- 考勤/缺勤/工时表格候选样本。
- 经营表格类 XLSX 样本。

**Command:**

```bash
npm run smoke:p2-summary-only-dry-run -- --self-test
npm run smoke:p2-summary-only-dry-run -- --dataset-id <dataset-uuid> --limit 5 --env-file /etc/aiv3/aiv3.env
```

**Done when:**
- 所有 fact 类型明确归入 `report_aggregation`、`retrieval_enhancement`、`evidence_index_only` 或 `review_required`。
- 未知 fact 类型固定进入 review。
- 未经确认不写 facts、fingerprints、snapshots，也不入队。

### P2-2 Fingerprint 和对象治理

**目标:** 让同一文档可安全归属多个数据集，降低重复存储，同时不破坏授权语义。

**Commands:**

```bash
npm run smoke:document-fingerprint-inventory -- --env-file /etc/aiv3/aiv3.env --dataset-limit 20
npm run smoke:document-object-cleanup-plan -- --env-file /etc/aiv3/aiv3.env --dataset-limit 20
npm run smoke:document-object-filesystem-preflight -- --env-file /etc/aiv3/aiv3.env --probe-limit 200
npm run smoke:document-object-repair-plan -- --env-file /etc/aiv3/aiv3.env --probe-limit 200
```

**Done when:**
- 只输出聚合计数和原因分布。
- 不输出对象路径、对象 key、hash、文档标题或正文。
- missing object / missing fingerprint 修复只输出 readiness 分类；真实 hash、backfill、对象清理或源同步前必须有 operator-reviewed manifest、回滚说明和单独确认。

**Current status:**
- 2026-06-12: 已补 `smoke:document-object-repair-plan`。8 服务器只读 live 抽样 200 个 local missing fingerprint：`repair_ready_local_file_found=57`、`review_required_local_file_missing=143`、`not_sampled_local_candidate=2402`、`review_required_remote_locator=3`；未计算 hash、未读文件内容、未写库、未清理对象。

## 7. P3 报表模板与第三方触发

### P3-1 新百默认模板

**目标:** 相同或有交集的数据集组合优先复用新百模块化月报模板，按用户关注焦点调整模块顺序，不重复走高消耗 image2 设计链路。

**Commands:**

```bash
npm run validate:xinbai-report-template
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
```

**Done when:**
- `取高`、`经营状况`、`风险识别`、`低活跃品牌`、`销售缺口`、`助推门店` 能触发对应 focus。
- focus 只影响模块前置，不在顶部展示冗余解释。
- 报表命中模板时仍表达为“依据客户需求生成/调整”，不暴露内部复用细节。

### P3-2 临时文档和模板参考

**目标:** 第三方临时上传文档能进入本次会话范围；模板文档作为格式参考，不误当业务事实。

**Commands:**

```bash
npm run smoke:external-scoped-document-chat -- --self-test
npm run smoke:external-video-ppt -- --self-test
```

**Done when:**
- 同一 conversation 的授权范围可延续。
- 传 `dataset_external_ids` 与额外 document refs 时，分组内重复文档不重复供料，分组外显式文档可补充生效。
- 模板参考文档不污染事实回答。

## 8. P4 数据接入与 CC/Codex 模式

**目标:** 用户通过主站或第三方提出数据库/API/ERP/MCP 接入需求时，系统可让 CC/Codex 辅助分析，但必须落到明确目标数据集，并先产出 staging plan。

**Commands:**

```bash
bash scripts/run-data-ingestion-staging-live-smoke.sh --self-test
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

**Done when:**
- 无目标数据集时拒绝生产同步。
- 有目标数据集时只输出分析和 `staging_plan`。
- confirm 前不创建/更新生产数据集。

## 9. P5 工程治理

**目标:** 降低 `platform-api` 和主站前端核心文件复杂度，但每次只做一个行为保持的小切片。

**Preferred order:**
1. `crates/platform-api/src/lib.rs` 按已有模块边界继续拆。
2. `apps/web/app/HomePageClient.js` 先补测试，再抽 hook/component。
3. smoke 脚本持续标注 self-test、preflight、live 边界。
4. integration HTML 若只是行尾/stat 噪声，不纳入提交。

**Current progress:**
- 2026-06-12: `external_observability` helper 已从 `crates/platform-api/src/lib.rs` 拆到独立模块；header 名称、环境变量、默认放行逻辑和哈希比较逻辑保持不变。
- 2026-06-12: external integration management access wrapper 已并入 `external_observability` 模块；错误码、提示文本和授权判断保持不变。
- 2026-06-12: external integration JSON 脱敏 helper 已拆到 `external_integration_summary` 模块；敏感字段规则、递归脱敏和 redacted 标准化保持不变。
- 2026-06-12: external action 响应/结果 payload 摘要 helper 已并入 `external_integration_summary` 模块；白名单响应字段、body redacted 标记和结果形状摘要保持不变。
- 2026-06-12: external action/artifact/search/audit 聚合摘要 helper 已并入 `external_integration_summary` 模块；signal 优先级和安全字段摘要保持不变。
- 2026-06-12: external channel/source drift 和 database dataset readiness 摘要 helper 已并入 `external_integration_summary` 模块；signal 优先级、非负计数和 readiness 嵌入保持不变。
- 2026-06-12: external bot message/requested skill/artifact template payload 摘要 helper 已拆到 `external_message_summary` 模块；脱敏字段、指纹生成、dataset 计数和模板摘要字段保持不变。
- 2026-06-12: external channel platform/message type wire value helper 已并入 `external_channel_support` 模块；`feishu`、`lark`、`we_com`、`generic_chat`、`third_party`、`aigolf` 兼容映射和消息类型字段值保持不变。
- 2026-06-12: external action/reply dispatch URL config helper 已并入 `external_channel_support` 模块；action-specific endpoint 优先级、通用 fallback 和 redacted URL 跳过逻辑保持不变。
- 2026-06-12: external inbound bearer/default source id config helper 已并入 `external_channel_support` 模块；inbound token alias、generic-chat alias 和 source id alias 读取逻辑保持不变。
- 2026-06-12: `EXTERNAL_OBSERVABILITY_ACCESS_HEADER` 已改为 test-only import；普通 `platform-api` 编译不再产生 unused import warning，测试中 header 常量仍可用。
- 2026-06-12: `crates/platform-api/src/lib.rs` workflow runtime summary 基础格式化和工具执行状态计数 helper 已拆到 `workflow_runtime_summary` 模块；summary 文本缩进、可选字段输出和 `requested/completed/failed` 计数语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` model-facing 协议字符串和默认工具 key helper 已拆到 `model_facing_format` 模块；capability/evidence/service lane/report entry/next action/chat turn 字段值和工具 key 去重顺序保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` model-facing summary 构造、通用 service lane/report entry/continuation/recommended action 推断和 degraded summary helper 已拆到 `model_facing_policy` 模块；allowed action 去重、推荐动作优先级、工具 key 写入和失败信号补全语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` model-facing 文档聚焦枚举、协议字符串和 distinct/indexed 文档数推断 helper 已拆到 `model_facing_document_focus` 模块；`unknown`、`single_document`、`multi_document` 字段值和 distinct 优先于 indexed 的推断语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` model-facing service handoff capability/next-action/signals helper 已拆到 `model_facing_handoff` 模块；handoff source/service lane/report entry/resolution 信号、确认/已确认/降级 next action 语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` report render output asset path/kind helper 已拆到 `report_render_output_asset` 模块；asset path trim、空 path 拒绝、kind 字符串读取和发布前可用性判断语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` 文档 model-facing lifecycle 字符串和 retrieval evidence 失败状态 helper 已拆到 `document_model_facing_support` 模块；document lifecycle wire 字段值、embedding/recall 任一失败即 degraded、缺失 manifest 不判失败语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` 文档媒体 detail 的 model-facing summary/signal helper 已拆到 `document_media_model_facing` 模块；failed/partial/unknown/completed parse status 映射、媒体证据 live detail 判断和 provider capability 计数语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` compare documents 的 model-facing summary/signal helper 已拆到 `document_compare_model_facing` 模块；multi-document mixed 判断、failed document/retrieval degraded 判断、ReadDocumentDetail/AnswerDirectly next action 顺序和 compare signals 保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` report plan 的 model-facing summary/signal helper 已拆到 `report_plan_model_facing` 模块；Draft/Planned/Rendered/Published evidence state、GenerateReportOutput/ContinueReportPlanning/RetryExecution next action、service handoff signal 和 service lane/report entry override 语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` report render output 的 model-facing summary/signal helper 已拆到 `report_render_model_facing` 模块；Rendered/Failed evidence state、PublishReport/RetryExecution next action、asset path/kind signal、service handoff signal 和 service lane/report entry override 语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` chat message 的 model-facing summary/signal helper 已拆到 `chat_message_model_facing` 模块；assistant-only summary、retrieval evidence 计数、multi-document/LiveDetail/Mixed/CatalogMemory/Degraded 判断、tool loop/artifact commit next action 和 service handoff 语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` dataset output 的 model-facing summary/signal helper 已拆到 `dataset_output_model_facing` 模块；EvidenceRetrieval/LiveDetail/Mixed/CatalogMemory/Degraded 判断、document focus 计数、service handoff 和 report entry next action 语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` chat session 的 model-facing summary/signal helper 已拆到 `chat_session_model_facing` 模块；latest assistant/dataset output 供料计数、last turn 降级、runtime wait、report entry confirmation/confirmed 和 document focus 语义保持不变。
- 2026-06-12: `chat_session_model_facing` 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` document detail 的 model-facing summary/signal helper 已拆到 `document_detail_model_facing` 模块；document lifecycle、retrieval evidence degraded、section title/noun hint、parse quality signal 和 AnswerDirectly/RetryExecution 语义保持不变。
- 2026-06-12: `document_detail_model_facing` 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` workflow runtime inspect 的 model-facing summary/signal helper 已拆到 `workflow_runtime_model_facing` 模块；sub-view summary 优先级、failed/dead-letter 降级、memory/retrieval mixed 判断、runtime wait action 和 workflow kind signal 语义保持不变。
- 2026-06-12: `workflow_runtime_model_facing` 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` workflow runtime pretty summary 渲染 helper 已拆到 `workflow_runtime_summary` 模块并保留 crate root `pub use`；summary 顺序、model-facing/execution-scope/dataset-output/report-plan/report-render/latest-assistant-turn 文本形状、可选字段输出和工具状态计数语义保持不变。
- 2026-06-12: `workflow_runtime_summary` pretty summary 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` model gateway operator 权限判定 helper 已拆到 `model_gateway_admin` 模块；operator 邮箱/allowlist/角色/env flag 配置名、内置角色、错误码和错误文案保持不变。
- 2026-06-12: `model_gateway_admin` operator helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` chat session 标题和 report plan 标题/objective 派生 helper 已拆到 `chat_session_titles` 模块并补模块单测；空标题 fallback、空白规范化、72 字符截断、Report 后缀和 last/initial prompt 优先级保持不变。
- 2026-06-12: `chat_session_titles` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` 通用文本规范化 helper 已拆到 `text_normalization` 模块并补模块单测；`Option<String>` trim、空字符串丢弃、非空字符串 trim 语义保持不变。
- 2026-06-12: `text_normalization` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `HomePageClient.js` 对话标题生成逻辑已拆到 `apps/web/app/lib/conversation-title.js`，并补充 Node 单测；对话标题时间格式、空标题 fallback 和长 prompt 截断行为保持不变。
- 2026-06-12: `HomePageClient.js` 本地浏览器状态读写 helper 已拆到 `apps/web/app/lib/local-browser-state.js`，并补充 Node 单测；local thread id 和 AssistantRun id 的 localStorage key、SSR fallback、浏览器存储失败 fallback 保持不变。
- 2026-06-12: `HomePageClient.js` 本地对话列表缓存读写已并入 `apps/web/app/lib/local-chat-sessions.js`，并补充 Node 单测；local chat sessions storage key、normalize 后写入、无 window/坏 JSON/存储失败 fallback 保持不变。
- 2026-06-12: `HomePageClient.js` 本地消息缓存读写已并入 `apps/web/app/lib/local-chat-sessions.js`，并补充 Node 单测；local chat messages storage key、只保留最后 40 条、消息对象 shape 不额外 normalize、无 window/坏 JSON/存储失败 fallback 保持不变。
- 2026-06-12: `HomePageClient.js` activity events 缓存读写已拆到 `apps/web/app/lib/local-activity-events.js`，并补充 Node 单测；activity storage key、只保留前 20 条、事件对象 shape 不额外 normalize、无 window/坏 JSON/存储失败 fallback 保持不变。
- 2026-06-12: `HomePageClient.js` 本地 secret/account 缓存 helper 已拆到 `apps/web/app/lib/local-account-state.js`，并补充 Node 单测；secret binding ids、local key value、account email storage key、邮箱 normalize、读 fallback 和现有写入/清理语义保持不变。
- 2026-06-12: `HomePageClient.js` 自动数据集 key/title 生成 helper 已拆到 `apps/web/app/lib/dataset-identity.js`，并补充 Node 单测；默认时间、随机后缀、`zh-CN` 标题时间格式和现有调用语义保持不变。
- 2026-06-12: `HomePageClient.js` 本地密钥 SHA-256 指纹 helper 已拆到 `apps/web/app/lib/local-secret-fingerprint.js`，并补充 Node 单测；trim、空值 fallback、`crypto.subtle` 能力错误提示和 hex 输出语义保持不变。
- 2026-06-12: `HomePageClient.js` JSON API 请求 helper 已拆到 `apps/web/app/lib/home-api-client.js`，并补充 Node 单测；JSON body 序列化、FormData 透传、DataMax browser-scope headers、timeout ApiError、JSON/text 错误解析和现有调用语义保持不变。
- 2026-06-12: `HomePageClient.js` SSE 流式请求和事件块解析 helper 已并入 `apps/web/app/lib/home-api-client.js`，并补充 Node 单测；SSE headers、JSON body 序列化、delta/completed/error 事件处理、plain text fallback 和无 stream body 错误提示保持不变。
- 2026-06-12: `HomePageClient.js` assistant_run 流式展示文案、生成产物链接提取、内部 JSON 截断和重复链接清洗 helper 已拆到 `apps/web/app/lib/assistant-stream-content.js`，并补充 Node 单测；流式进度展示、最终报表链接只补一次、内部 payload 不进入用户可见消息的语义保持不变。
- 2026-06-12: `HomePageClient.js` 数据集 ID normalize、文档/报表/静态页草稿归属提取、数据集范围筛选和排序 helper 已拆到 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；数据集范围授权、报表模板命中和静态页筛选的现有顺序/去重语义保持不变。
- 2026-06-12: `HomePageClient.js` 已发布报表和报表模板标题/候选 ID/产物 URL 解析 helper 已拆到 `apps/web/app/lib/report-template-utils.js`，并补充 Node 单测；模板命中、标题优先级和产物链接来源优先级保持不变。
- 2026-06-12: `HomePageClient.js` 静态页报表 shelf/default、artifact key、baseline status 和默认模板写回 helper 已拆到 `apps/web/app/lib/static-page-report-shelf.js`，并补充 Node 单测；相同数据集组合默认模板识别、启停默认模板和 retired 排除语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页成品 HTML artifact、report render 摘要 artifact、预览路径白名单、artifact 合并去重 helper 已拆到 `apps/web/app/lib/html-artifact-utils.js`，并补充 Node 单测；成品页卡片、预览链接安全白名单和 report render artifact 替换语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页草稿异步状态、自动续接渲染判断、可复用模板选择、report shelf 可见性和草稿排序 helper 已拆到 `apps/web/app/lib/static-page-draft-workspace.js`，并补充 Node 单测；预览完成提示、渲染完成提示、data-report 静默规则和模板复用优先级保持不变。
- 2026-06-12: `HomePageClient.js` 静态页规划交接 HTML artifact、模板参考摘要、缺失证据摘要和结构信号摘要 helper 已并入 `apps/web/app/lib/html-artifact-utils.js`，并补充 Node 单测；交接卡片 payload、Image2 bridge 文案和结构信号截断规则保持不变。
- 2026-06-12: `HomePageClient.js` 后端 selected scope/dataset preselection 解析和候选范围提示 helper 已并入 `apps/web/app/lib/scope-planner.js`，并补充 Node 单测；UI 选中数据集恢复、all-visible preselection 策略和提示文案保持不变。
- 2026-06-12: `HomePageClient.js` assistant continuation、CC 转发、静态页编辑、静态页/报表触发和显式拒绝输出判断 helper 已拆到 `apps/web/app/lib/home-chat-intents.js`，并补充 Node 单测；`取高是什么意思？`、`风险识别系统有哪些项目经历？` 等解释型问题不误触发报表的语义保持不变。
- 2026-06-12: `HomePageClient.js` Codex 客户产物/任务聊天展示文案 helper 已并入 `apps/web/app/lib/codex-customer-artifacts.js`，并补充 Node 单测；产物主链接去重、待发布校验提示、终态任务摘要和列表截断语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页后端草稿、渲染输出、image job 和 prompt-only 队列操作 helper 已并入 `apps/web/app/lib/static-page-draft.js`，并补充 Node 单测；backend draft 状态合并、dataset scope 去重、最终页字段保留、排队提示和 prompt alias 语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页规划 summary、字段候选和 selected scope helper 已拆到 `apps/web/app/lib/static-page-conversation-context.js`，并补充 Node 单测；数据集/文档标题线索、最近消息截断、普通对话 fallback、conversation memory 和 selected scope payload 语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页 source refs 和预览进度文案 helper 已并入 `apps/web/app/lib/static-page-conversation-context.js`，并补充 Node 单测；local thread id 传递、Image2 固定任务 source refs、预览链接优先级和非 URL asset 不出链接语义保持不变。
- 2026-06-12: `HomePageClient.js` 本地消息构造 helper 已并入 `apps/web/app/lib/local-chat-sessions.js`，并补充 Node 单测；本地消息 id、role、content、created_at shape 和原有调用语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页 draft 查找、HTML artifact owner scope 和成品 artifact 匹配 helper 已并入 `apps/web/app/lib/html-artifact-utils.js`，并补充 Node 单测；local/backend draft id 查找、`ownerScope`/`owner_scope` 兼容和 `static_page_published_preview` 模板过滤语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页 draft map 替换 reducer 已并入 `apps/web/app/lib/static-page-draft-workspace.js`，并补充 Node 单测；旧 id 删除、本地仅 upsert、无效 draft copy fallback 和不 mutate 原 map 语义保持不变。
- 2026-06-12: `HomePageClient.js` UI notice descriptor 和本地消息构造 helper 已拆到 `apps/web/app/lib/ui-notice-message.js`，并补充 Node 单测；空消息/后台发送提示抑制、错误前缀、稳定 key、metadata 和 40 条截断语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页进度消息 descriptor 和本地消息构造 helper 已并入 `apps/web/app/lib/static-page-conversation-context.js`，并补充 Node 单测；进度消息 stable key、content、`final` metadata 和 40 条截断语义保持不变。
- 2026-06-12: `HomePageClient.js` Codex 客户产物/任务聊天 message descriptor 和本地消息构造 helper 已并入 `apps/web/app/lib/codex-customer-artifacts.js`，并补充 Node 单测；产物/任务 stable key、source、content、metadata 和 40 条截断语义保持不变。
- 2026-06-12: `HomePageClient.js` 报表 shelf 选择提示文案和本地消息构造 helper 已并入 `apps/web/app/lib/report-template-utils.js`，并补充 Node 单测；标题清洗、报表后缀补全、最后一条去重、metadata 和 40 条截断语义保持不变。
- 2026-06-12: `HomePageClient.js` assistant-run 流式占位消息、流式内容和 final 内容合成 helper 已并入 `apps/web/app/lib/assistant-stream-content.js`，并补充 Node 单测；占位文案、空 final fallback、流式文本优先级、status fallback 和生成产物链接只补一次语义保持不变。
- 2026-06-12: `HomePageClient.js` 本地消息追加、内容替换和 40 条截断 helper 已并入 `apps/web/app/lib/local-chat-sessions.js`，并补充 Node 单测；本地消息 shape、空追加引用、流式中间更新不截断、final 更新截断和缓存读写截断语义保持不变。
- 2026-06-12: `HomePageClient.js` 本地对话 snapshot 构造 helper 已并入 `apps/web/app/lib/local-chat-sessions.js`，并补充 Node 单测；thread fallback、标题 fallback、首条用户消息标题、started/updated 时间和 assistantRunId 传递语义保持不变。
- 2026-06-12: `HomePageClient.js` 对话菜单本地会话选项构造 helper 已并入 `apps/web/app/lib/local-chat-sessions.js`，并补充 Node 单测；当前本地线程隐藏规则、backend session 选中时展示规则、标题 fallback、更新时间 meta 和 localOnly 标记保持不变。
- 2026-06-12: `HomePageClient.js` 当前对话标题计算 helper 已并入 `apps/web/app/lib/conversation-title.js`，并补充 Node 单测；已选后端会话标题、空标题 fallback、草稿标题 trim、输入优先、首条用户消息 fallback 和默认新对话标题语义保持不变。
- 2026-06-12: `HomePageClient.js` 新建本地对话 draft helper 已并入 `apps/web/app/lib/local-chat-sessions.js`，并补充 Node 单测；thread id 生成、开始时间生成、已选数据集提示和普通新对话提示语义保持不变。
- 2026-06-12: `HomePageClient.js` 本地用户语句记忆 payload helper 已并入 `apps/web/app/lib/local-activity-events.js`，并补充 Node 单测；后台 conversation-memory 写入路径、字段 shape、空内容跳过和 best-effort 语义保持不变。
- 2026-06-12: `HomePageClient.js` 文档选择时的数据集范围恢复 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；有归属文档更新选中数据集、无归属文档不清空现有范围的语义保持不变。
- 2026-06-12: `HomePageClient.js` 数据集多选切换 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；当前选中范围规范化、已有项移除、不存在项追加和顺序保持语义不变。
- 2026-06-12: `HomePageClient.js` 文档数据集归属响应范围解析 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；`dataset_ids`、`datasetIds`、`document.dataset_ids`、`document.datasetIds` 的现有优先级和空数组阻断 fallback 语义保持不变。
- 2026-06-12: `HomePageClient.js` 文档数据集归属当前范围 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；文档已有归属优先、无归属时退回当前选中数据集的语义保持不变。
- 2026-06-12: `HomePageClient.js` 文档数据集归属切换意图 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；归属命中判断、PUT/DELETE 方法和完成提示文案语义保持不变。
- 2026-06-12: `HomePageClient.js` 数据集选择状态 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；普通数据集切换、文档归属响应后的首选数据集和空响应不更新语义保持不变。
- 2026-06-12: `HomePageClient.js` 目录刷新后的数据集选择保留 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；仅保留仍存在的数据集、合法 preferred 优先和失效 active 清空语义保持不变。
- 2026-06-12: `HomePageClient.js` 选中数据集列表增删 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；归档移除、创建数据集前置、上传/后端范围回填尾部追加的顺序和去重语义保持不变。
- 2026-06-12: `HomePageClient.js` 报表 shelf 数据集范围 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；显式多选优先、单选 fallback、可见数据集 ID 提取和 fetch 范围合并去重语义保持不变。
- 2026-06-12: `HomePageClient.js` 静态页草稿 shelf 数据集查询范围 helper 已并入 `apps/web/app/lib/dataset-record-scope.js`，并补充 Node 单测；优先数据集、目标数据集和查询顺序语义保持不变。
- 2026-06-12: `crates/platform-api/src/lib.rs` resource owner 可见性和 owner-managed resource 404 masking helper 已拆到 `resource_access` 模块并补模块单测；public 资源可见、owner 匹配可见、非 owner 仍用 404 mask 的语义保持不变。
- 2026-06-12: `resource_access` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` dataset/document lifecycle update parser 已拆到 `lifecycle_updates` 模块并补模块单测；trim、小写归一、unsupported lifecycle 的 `validation_error` 语义保持不变。
- 2026-06-12: `lifecycle_updates` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` 404/not_found error 构造 helper 已拆到 `not_found_errors` 模块并补模块单测；dataset/document/output/chat/assistant/workflow/report/static-page draft/image job 的 error code、status 和 message 语义保持不变。
- 2026-06-12: `not_found_errors` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` 请求范围 header helper 已拆到 `request_scope_headers` 模块并补模块单测；active secret binding header 解析、local thread id trim 和 secret binding id merge 去重顺序保持不变。

**Regression commands:**

```bash
cargo fmt --check
cargo test -p platform-api gateway_limiter --lib
cargo test -p platform-api model_gateway_profile --lib
cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib
npm --prefix apps/web run build
```

**Done when:**
- 行为不变。
- 回归集通过。
- 每个提交可单独回退。

## 10. 当前下一步

1. 若拿到合法 operator 凭证或脱敏回执，继续做 P1-1 authenticated operator live。
2. 若没有 operator 凭证，继续 P5：再选一个 `platform-api` 或主站前端小切片做行为保持重构，测试先行。
3. 若出现新客户失败样例，按 P2-1 做 summary-only dry-run，不重复跑已覆盖类型。
4. 若准备发布，先跑 P0 固定回归，再按 P0 部署边界同步 8 服务器。
5. GitHub Actions 账号额度恢复后，重跑 DataMax CI 并把结果补回 validation。
