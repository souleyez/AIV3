# DataMax 主线缺口补齐 Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.
>
> 本文件是 `docs/plans/` 下唯一有效计划。旧计划只作为历史材料保留在 `docs/archive/plans/`。验证记录、回执摘要和失败定位统一写入 `docs/validation/datamax-main-gap-closure.md`。

**目标:** 让 DataMax 稳定支撑主站问答、第三方问答、范围授权、报表/静态页生成、数据接入分析和异步文档理解，并把后续工作压缩成可执行、可验证、可回滚的队列。

**架构:** DataMax 平台侧负责企业记忆、会话授权、文档范围、报表模板、任务编排和产物发布。模型调用保持无状态，只接收平台临时裁剪后的上下文、检索证据和工具结果。重任务由 worker、smoke 脚本、固定 Codex/Cloudflare 执行器或本地模板复用链路承接；生产写入和第三方公开契约变更必须显式确认。

**Tech Stack:** Rust workspace services、Next.js `apps/web`、Node smoke scripts、PostgreSQL、8 服务器 systemd workers、静态页/报表产物、DataMax 模板库、本地生页链路和可选 Cloudflare/Codex fallback。

**重建日期:** 2026-06-12

---

## 1. 维护规则

- `docs/plans/` 只保留本文件。
- 本文件只写当前基线、执行队列、验收命令和阻塞项；不要再写长日志。
- 详细验证证据写入 `docs/validation/datamax-main-gap-closure.md`。
- 不记录密钥、bearer、cookie、provider key、数据库 URL、原始客户行、原始 provider payload、完整文档内容、私有对象路径、对象 key 或 content hash。
- 不默默修改第三方公开 API URL、鉴权、必填请求字段或已有响应字段；必须变更时先问。
- 不动 120 服务器。
- 质量门禁保持禁用或被动采样；不要恢复会拦截正常回答的硬门禁。
- 数据接入可以产出分析和 `staging_plan`；生产 schema/data 写入必须人工确认。
- P2 解析、fingerprint、事实抽取和对象治理默认只跑 `--dry-run --summary-only`；真实 backfill、入队、对象清理或源同步必须单独确认。
- 纯文档或纯 smoke 脚本更新不重启 8 服务器服务。

## 2. 当前生产基线

| 模块 | 已确认状态 | 当前缺口 |
| --- | --- | --- |
| 主站 | `https://doc.elepcloud.com/` 是无需登录直接问答入口；主站 20 路 self-test 已具备，8 服务器 live 基线曾通过。 | 真流式 delta 和自动滚动仍需在下一次 UI/运行时改动后做生产浏览器回归。 |
| 管理台/文档 | `https://v3.elepcloud.com/` 是管理台和第三方接口文档入口。 | 继续保持域名分工稳定；不做未经确认的公开接口变更。 |
| 第三方问答 | 20 路并发 self-test 已具备；数据集/文档范围授权已实现。 | 新失败样例需要按客户实际范围做 targeted live smoke。 |
| 报表/静态页 | 新百默认模板是 `xinbai-functional-modular-template-20260604`；报表产物标准包含 `index.html`、`table-data.csv`、`report.ppt`、`report.md`。 | 报表链接用户文案只出现一次；第三方结构字段继续承载产物 URL；focus 排序需要随客户新需求回归。 |
| 数据源/CC 模式 | 数据源页和 staging analysis 链路已存在；CC/Codex 可协助 DB/API 接入。 | 所有接入必须落到明确目标数据集；生产写入仍需确认。 |
| P2 企业记忆 | DOC/DOCX/PDF/XLSX/PPTX/MP4 小样本、doc-heavy、经营表格类数据集已完成 summary-only dry-run 扩样；最新经营表格样本派生 facts 1204，未知 fact 类型 0，未写入、未入队。 | 继续扩制度手册、简历、考勤/经营明细样本；真实写入前先审 fact 类型用途策略。 |
| fingerprint/对象治理 | inventory、原因聚合、对象清理 dry-run plan、文件系统 preflight 已具备；当前不删除源对象。 | 缺 fingerprint 覆盖和 missing object 处理仍停留在只读分析阶段。 |
| CI | 本地和 8 服务器 smoke 覆盖当前主要缺口。 | GitHub Actions 因账号付款/额度状态在 job 启动前失败，账号恢复后重跑。 |

## 3. 当前执行顺序

### P0-1 保持发布前固定回归

**目标:** 每次准备发布 8 服务器前，先确认第三方问答、报表触发、主站 UI、静态页和导出字段没有回归。

**Files:**
- Validate: `scripts/smoke/external-report-focus.mjs`
- Validate: `scripts/smoke/external-report-export.mjs`
- Validate: `scripts/smoke/external-scoped-document-chat.mjs`
- Validate: `scripts/smoke/static-page-5way.mjs`
- Validate: `apps/web/app/lib/assistant-run-progress.test.mjs`
- Validate: `apps/web/app/lib/local-chat-sessions.test.mjs`
- Record: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Run local non-credential gates**

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

**Step 2: Run build gates when frontend or Rust changed**

```bash
npm --prefix apps/web run build
CC=clang CXX=clang++ cargo test -p platform-api external_channel_static_page_artifact --lib
```

**Step 3: Record result**

Update `docs/validation/datamax-main-gap-closure.md` with command, result, commit, and safety notes.

**Done when:**
- 报表触发不截断正常回答。
- 报表链接可点且用户文案只出现一次。
- `table-data.csv`、`report.ppt`、`report.md` 可访问。
- `取高是什么意思？`、`风险识别系统有哪些项目经历？` 不误触发报表。

### P0-2 8 服务器同步和部署边界

**目标:** 区分 docs-only 同步、代码发布、服务重启，避免无必要重启。

**Files:**
- Validate: `docs/validation/datamax-main-gap-closure.md`
- Deploy target: `/srv/aiv3/repo`

**Docs-only sync command:**

```bash
cd /srv/aiv3/repo
git pull --ff-only origin main
git rev-parse --short HEAD
git status --short --branch
systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-chat-session-worker.service aiv3-static-page-worker.service
```

**Code deploy command:**

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
- code deploy 后 services 全 active。
- validation 记录服务器 head 和 smoke 结果。

## 4. P1 生产观测与并发

### P1-1 Operator 观测闭环

**目标:** 能看清 model gateway、provider fallback、queue stats 和静态页预热状态，同时不暴露密钥或任务 payload。

**Blocked by:** 合法 operator cookie/bearer/local-key，或运维侧提供脱敏回执。

**Files:**
- Validate: `scripts/smoke/model-gateway-operator.mjs`
- Validate: `scripts/smoke/static-page-prewarm-observability.mjs`
- Record: `docs/validation/datamax-main-gap-closure.md`

**Commands:**

```bash
node scripts/smoke/model-gateway-operator.mjs --base-url https://v3.elepcloud.com
npm run smoke:static-page-prewarm-observability -- --self-test
```

**Done when:**
- 未授权访问仍为 HTTP 401。
- 已授权回执只含 provider lane、限流、fallback、queued/running/published/failed/skipped 状态。
- 回执不含 token、cookie、provider key、任务原文或客户数据。

### P1-2 20 路并发和重任务并发

**目标:** 主站、第三方、静态页和 fallback 都有明确并发验收口径。

**Files:**
- Validate: `scripts/smoke/main-chat-20way.mjs`
- Validate: `scripts/smoke/external-channel-20way.mjs`
- Validate: `scripts/smoke/static-page-5way.mjs`
- Validate: `scripts/smoke/cloudflare-fallback-2way.mjs`

**Self-test commands:**

```bash
npm run smoke:main-chat-20way -- --self-test
npm run smoke:external-channel-20way -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
```

**Live target:**
- 主站问答：20 路。
- 第三方问答：20 路。
- 本地 image2/html 重任务：5 路。
- Cloudflare/Codex fallback：2 路。

**Done when:**
- live smoke 在受控窗口通过。
- 失败时能区分模型供应商、队列拥塞、权限、模板命中和产物发布问题。

## 5. P2 企业记忆与文档理解

### P2-1 异步深解析 dry-run 扩样

**目标:** 文档入库后，空闲时可持续深化实体、别名、组织、项目、操作步骤、表格事实、合同指标、面积、客流、租售比、有效期和证据来源。

**Files:**
- Validate: `scripts/smoke/p2-summary-only-dry-run.mjs`
- Record: `docs/validation/datamax-main-gap-closure.md`

**Current coverage:**
- 小型混合媒体样本。
- doc-heavy 样本。
- 经营表格类 XLSX 样本，limit5 派生 facts 1204，未知 fact 类型 0。
- 制度手册/护理流程类 Word 旧格式样本，limit5 派生 facts 256，未知 fact 类型 0。
- 简历类 14 份 PDF 样本，limit14 派生 facts 3010，包含组织、岗位、项目系统、时间段等维度，未知 fact 类型 0。

**Next samples:**
- 考勤/缺勤/工时表格类数据集。

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

**Files:**
- Validate: `scripts/smoke/document-fingerprint-inventory.mjs`
- Validate: `scripts/smoke/document-object-cleanup-plan.mjs`
- Validate: `scripts/smoke/document-object-filesystem-preflight.mjs`
- Record: `docs/validation/datamax-main-gap-closure.md`

**Commands:**

```bash
npm run smoke:document-fingerprint-inventory -- --env-file /etc/aiv3/aiv3.env --dataset-limit 20
npm run smoke:document-object-cleanup-plan -- --env-file /etc/aiv3/aiv3.env --dataset-limit 20
npm run smoke:document-object-filesystem-preflight -- --env-file /etc/aiv3/aiv3.env --probe-limit 200
```

**Done when:**
- 只输出聚合计数和原因分布。
- 不输出对象路径、对象 key、hash、文档标题或正文。
- 真实对象清理前有 operator-reviewed manifest 和回滚说明。

## 6. P3 报表模板和第三方触发

### P3-1 新百默认模板保持唯一主模板

**目标:** 相同或有交集的数据集组合优先复用新百模块化月报模板，按用户关注焦点调整模块顺序，不重复走高消耗 image2 设计链路。

**Files:**
- Validate: `scripts/validate-xinbai-report-template.mjs`
- Validate: `scripts/smoke/external-report-focus.mjs`
- Validate: `scripts/smoke/external-report-export.mjs`

**Command:**

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

**Files:**
- Validate: `scripts/smoke/external-scoped-document-chat.mjs`
- Validate: `scripts/smoke/external-video-ppt.mjs`

**Command:**

```bash
npm run smoke:external-scoped-document-chat -- --self-test
npm run smoke:external-video-ppt -- --self-test
```

**Done when:**
- 同一 conversation 的授权范围可延续。
- 传 `dataset_external_ids` 与额外 document refs 时，分组内重复文档不重复供料，分组外显式文档可补充生效。
- 模板参考文档不污染事实回答。

## 7. P4 数据接入与 CC/Codex 模式

### P4-1 Staging-only 数据接入闭环

**目标:** 用户通过主站或第三方提出数据库/API/ERP/MCP 接入需求时，系统可让 CC/Codex 辅助分析，但必须落到明确目标数据集，并先产出 staging plan。

**Files:**
- Validate: `scripts/run-data-ingestion-staging-live-smoke.sh`
- Validate: `scripts/run-data-ingestion-staging-sync-smoke.sh`
- Validate: `crates/platform-api/src/*`
- Record: `docs/validation/datamax-main-gap-closure.md`

**Commands:**

```bash
bash scripts/run-data-ingestion-staging-live-smoke.sh --self-test
bash scripts/run-data-ingestion-staging-sync-smoke.sh
```

**Done when:**
- 无目标数据集时拒绝生产同步。
- 有目标数据集时只输出分析和 `staging_plan`。
- confirm 前不创建/更新生产数据集。

## 8. P5 工程治理

### P5-1 大文件拆分和最小回归

**目标:** 继续降低 `platform-api` 和主站前端核心文件复杂度，但每次只做一个行为保持的小切片。

**Preferred order:**
1. `crates/platform-api/src/lib.rs` 按已有模块边界继续拆。
2. `apps/web/app/HomePageClient.js` 先补测试，再抽 hook/component。
3. smoke 脚本持续标注 self-test、preflight、live 边界。
4. integration HTML 若只是行尾/stat 噪声，不纳入提交。

**Regression commands:**

```bash
cargo fmt --check
CC=clang CXX=clang++ cargo test -p platform-api gateway_limiter --lib
CC=clang CXX=clang++ cargo test -p platform-api model_gateway_profile --lib
CC=clang CXX=clang++ cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib
npm --prefix apps/web run build
```

**Done when:**
- 行为不变。
- 回归集通过。
- 每个提交可单独回退。

## 9. 当前下一步

1. 下一个可独立推进项是 P2-1：考勤/缺勤/工时表格类 summary-only dry-run 扩样。
2. 若考勤类样本没有合适候选，转入 P1-1 operator 观测前置准备或 P5 小切片重构。
3. 若拿到合法 operator 凭证或脱敏回执，优先做 P1-1 operator 观测闭环。
4. 若进入发布窗口，按 P0-1 和 P0-2 跑固定回归后再部署。
5. GitHub Actions 账号额度恢复后，重跑 DataMax CI 并把结果补回 validation。
