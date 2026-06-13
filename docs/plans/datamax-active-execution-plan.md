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

## 2.1 待完成清单与完成标准

| 优先级 | 工作项 | 当前状态 | 下一步 | 完成标准 |
| --- | --- | --- | --- | --- |
| P0 | 发布前固定回归和部署边界 | self-test、8 服务器 code deploy、docs-only sync 口径已稳定。 | 每次发版继续执行 P0 命令并写 validation；docs-only 不重启服务。 | 本地和 8 服务器 P0 smoke 通过；服务全 `active`；`/readyz` 正常；validation 记录 head、命令、结果和安全边界。 |
| P1 | operator 观测闭环 | 未授权和观测 key 脱敏路径已验证；authenticated operator live 缺合法凭证或运维脱敏回执。 | 拿到 operator cookie/bearer/local-key 后跑 live；拿不到则不阻塞 P5。 | 未授权仍 401；授权回执只含脱敏 provider/queue/fallback 聚合；不泄露 token、cookie、任务原文、客户数据或对象路径。 |
| P1 | 20 路问答和重任务并发 live | self-test 已覆盖主站、第三方、静态页 5 路、Cloudflare fallback 2 路。 | 在受控窗口跑 live 20 路问答和 5/2 路重任务，并记录瓶颈分类。 | 主站和第三方 20 路问答成功；本地重任务 5 路、fallback 2 路不越限；失败能归因到模型供应商、队列、权限、模板命中或发布。 |
| P2 | 异步深解析和事实用途分类 | 多类型 summary-only dry-run 已覆盖；真实写入仍关闭。 | 继续用新失败样例或新类型扩样；不重复跑已覆盖类型。 | fact 类型均归入 `report_aggregation`、`retrieval_enhancement`、`evidence_index_only` 或 `review_required`；未知类型进 review；未经确认不写库、不入队。 |
| P2 | fingerprint、对象治理和重复文档 | inventory、repair plan、filesystem preflight 已是只读计划；缺真实 backfill/清理确认。 | 仅在用户确认后准备 operator-reviewed manifest、回滚说明和小批量执行方案。 | 真实 hash/backfill/对象清理前有 reviewed manifest；执行结果只记录聚合计数和原因分布；不输出对象 key、hash、标题或正文。 |
| P3 | 新百默认报表模板和第三方触发 | 默认模板、focus、导出字段和链接去重 smoke 已稳定。 | 继续用真实客户问题做 targeted smoke；发现 focus 错位时只改模板/focus 判断，不改公开接口。 | 取高、经营状况、风险识别、低活跃、销售缺口、助推门店均能前置正确模块；解释型问题不误触发；链接只出现一次且结构字段可识别。 |
| P3 | 数据集模板复用与低负载预热 | 模板复用、客户不可见 prewarm candidate、worker 低负载 requeue、local render/publish 路径和 observability self-test 已具备；生产默认仍保持关闭。 | 只有在 reviewed 低负载窗口显式启用 `STATIC_PAGE_TEMPLATE_PREWARM_ENABLED=true` 后才跑 live；常规发版继续跑 observability self-test 和未授权 guard。 | 相同或有交集的数据集组合和相同 `default_prompt` 优先复用已有模板；低负载预热不重复生成、不影响在线问答、不产生用户可见噪声；观测回执明确 `waitingForLowLoadCount`、`customerVisiblePrewarmLeakCount=0`、`prewarmCustomerVisibilityOk=true`。 |
| P4 | 数据接入与 CC/Codex 模式 | staging analysis 存在；生产同步必须人工确认。 | 保持“先分析、出 staging_plan、落目标数据集”的闭环；确认前不写生产。 | 无目标数据集时拒绝生产同步；有目标数据集时只输出分析和 `staging_plan`；confirm 前不创建/更新生产数据集或 schema。 |
| P5 | 工程治理和复杂文件拆分 | `platform-api` 与主站前端已持续小切片拆分；仍是当前可独立推进主线。 | 无 P1 凭证、无 P2/P4 写入确认、无新客户失败样例时，继续 P5 行为保持切片。 | 每个切片有定向测试、`cargo fmt --check`/`cargo check`/Web build/P0 smoke；提交可单独回退；不改变第三方公开契约。 |
| CI | GitHub Actions | job 启动前失败，`steps=[]`，本地和 8 服务器验证可替代但不能声称 CI 通过。 | 账号额度/runner 恢复后重跑 DataMax CI。 | `Rust Minimal` 和 `No-Credential Smoke` 有正常 steps 且通过；结果写入 validation。 |

## 2.2 当前执行队列

| 顺序 | 当前项 | 状态 | 下一步 | 关闭标准 |
| --- | --- | --- | --- | --- |
| 1 | P5 external artifact request helper 拆分 | 已完成行为保持拆分、提交 GitHub、部署 8 服务器并通过 P0 验证。 | 已写回远端验证；作为当前已关闭基线。 | `external_artifact_request_support` 模块单测、simple artifact template payload 回归、requested skills/AI Golf 回归、`external_channel_static_page_artifact`、`cargo check`、Web build、P0 smoke、8 服务器 release build、health/ready 和服务 active 全通过；不改变第三方公开契约。 |
| 2 | P5 external bot message parse orchestration 拆分 | 已完成行为保持拆分、提交 GitHub、部署 8 服务器并通过 P0 验证。 | 已写回远端验证；作为当前已关闭基线。 | `external_bot_message_parse_support` 模块单测、第三方 payload alias、artifact request、requested skills、answer policy、AI Golf、静态页触发、`cargo check`、Web build、P0 smoke、8 服务器 release build、health/ready 和服务 active 全通过；不改变第三方公开契约。 |
| 3 | P1 authenticated operator live | 阻塞于合法 operator cookie/bearer/local-key 或运维脱敏回执。 | 拿到凭证后只跑观测类 live smoke，不输出密钥、cookie、任务原文或客户数据。 | 未授权仍 401；授权回执只显示脱敏 queue/provider/fallback 聚合。 |
| 4 | P1 live 并发压测 | self-test 具备；生产 live 需要受控窗口。 | 在低风险窗口跑主站/第三方 20 路问答、本地重任务 5 路、Cloudflare/Codex fallback 2 路。 | 成功率、耗时、失败归因和限流表现写入 validation；不把 self-test 当生产压测结论。 |
| 5 | P2/P4 真实写入类工作 | 仍保持 dry-run/summary-only。 | 只有在用户明确确认 backfill、入队、对象清理、source sync 或生产数据接入后才准备执行 manifest。 | 执行前有 operator-reviewed manifest、回滚方案和小批量策略；执行后只记录脱敏聚合结果。 |
| 6 | P3 客户失败样例与新百模板焦点 | 常规 smoke 已稳定；真实失败样例按需补 targeted smoke。 | 遇到低活跃、风险、经营健康度、取高、助推、销售缺口等焦点错位时，只改模板/focus 判断，不改公开 API。 | 正确模块前置；解释型问题不误触发；报表链接只出现一次；导出字段可访问。 |
| 7 | P5 AI Golf requested skill support 拆分 | 已完成行为保持拆分、提交 GitHub、部署 8 服务器并通过 P0 验证。 | 已写回远端验证；作为当前已关闭基线。 | `external_aigolf_skill_support` 模块单测、AI Golf 旧端到端卡片/事件回归、第三方 payload/解析、静态页触发、`cargo check`、Web build、P0 smoke、8 服务器 release build、health/ready 和服务 active 全通过；不改变第三方公开契约。 |
| 8 | P5 external answer policy value helper 拆分 | 已完成行为保持拆分、提交 GitHub、部署 8 服务器并通过 P0 验证。 | 已写回远端验证；作为当前已关闭基线。 | `external_answer_policy_support` 模块单测、旧 answer policy 回归、第三方 payload/解析、静态页触发、`cargo check`、Web build、P0 smoke、8 服务器 release build、health/ready 和服务 active 全通过；不改变第三方公开契约。 |
| 9 | P5 external requested skills policy helper 拆分 | 已完成本地行为保持拆分和回归，待提交、部署 8 服务器并写回远端验证。 | 收版、推送 GitHub、部署 8 服务器，跑 release build、health/ready 和 P0 smoke。 | `external_requested_skills_support` policy helper 单测、第三方 requested-skills/answer-policy/AI Golf/静态页触发回归、`cargo check`、Web build、P0 smoke、8 服务器 release build、health/ready 和服务 active 全通过；不改变第三方公开契约。 |
| 10 | P5 下一行为保持切片 | 待选择。 | 若仍无 P1 凭证、P2/P4 写入确认或新客户失败样例，继续评估第三方接入或主站前端周边小函数拆分。 | 单切片可独立回退，有定向测试、P0 self-test 和 8 服务器验证；不改变第三方公开契约。 |

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
npm run smoke:static-page-prewarm-observability -- --self-test
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

### P3-3 数据集模板复用与低负载预热

**目标:** 客户未明确要求更换样式时，相同或有交集的数据集组合优先复用已发布模板；低负载时可静默补齐缺失模板，但不影响在线问答。

**Commands:**

```bash
npm run smoke:static-page-prewarm-observability -- --self-test
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
```

**Done when:**
- 已有模板的数据集组合不重复走 image2 设计链路。
- 无模板的数据集组合只在低负载、队列容量允许、无同类任务运行时预热。
- 预热任务失败不影响客户对话；客户未要求报表时不主动推送链接。
- 预热状态可在观测页或 smoke 回执中按 queued/running/published/failed/skipped 脱敏查看。

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
- 2026-06-14: `crates/platform-api/src/lib.rs` document chunk 和 document enrichment run view mapping helper 已拆到 `document_view_support` 模块；本地和 8 服务器 `cargo fmt --check`、`cargo test -p platform-api document_view_support --lib`、`cargo test -p platform-api to_document_chunk_view_exposes_typed_state --lib`、`cargo check -p platform-api`、第三方静态页触发单测、Web build 和 P0 smoke 均通过，已提交并同步 8 服务器。
- 2026-06-14: `crates/platform-api/src/lib.rs` conversation memory item 和 workflow event view mapping helper 已拆到 `basic_view_support` 模块；本地和 8 服务器 `cargo fmt --check`、`cargo test -p platform-api basic_view_support --lib`、`cargo check -p platform-api`、第三方静态页触发单测、Web build 和 P0 smoke 均通过，已提交并同步 8 服务器。
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
- 2026-06-12: `request_scope_headers` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` dataset visibility/local-thread scope/standard-list hiding helper 已并入 `resource_access` 模块并补模块单测；public/owner/secret binding 可见性、local-only thread bypass、external temporary 和 external document parse 隐藏规则保持不变。
- 2026-06-12: `resource_access` dataset visibility helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` 第三方系统账户邮箱/展示名 helper 已拆到 `external_system_user` 模块并补模块单测；scope kind/id slug、24 字符截断、12 位 hash 后缀、`aidp.local` 域名和展示名 fallback 保持不变。
- 2026-06-12: `external_system_user` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` external management control helper 已并入 `external_channel_support` 模块并补模块单测；reason 仅记录是否存在、integration kind 分类和 management control patch 字段保持不变。
- 2026-06-12: `external_channel_support` management control helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` external channel connection/platform validation helper 已并入 `external_channel_support` 模块并补模块单测；connection_id 长度/字符集、platform 长度/字符集、错误码和错误文案保持不变。
- 2026-06-12: `external_channel_support` validation helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-12: `crates/platform-api/src/lib.rs` external channel inbound token/config helper 已并入 `external_channel_support` 模块并补模块单测；token 前缀/长度、敏感配置清理 key 集合、token rotation/expiry 字段和 temporary access summary 字段保持不变。
- 2026-06-12: `external_channel_support` inbound token/config helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` external database source id 和 external channel allowed database source helper 已并入 `external_channel_support` 模块并补模块单测；source id trim/长度/路径字符规则、allowed/default source 判定、配置 key alias、错误码和错误文案保持不变。
- 2026-06-13: `external_channel_support` database source helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` external reply/action dispatch URL 与 secret 校验 helper 已并入 `external_channel_support` 模块并补模块单测；URL 长度/字符/协议/host 规则、reply secret printable 规则、action secret redacted/长度/control-char 规则、错误码和错误文案保持不变。
- 2026-06-13: `external_channel_support` dispatch validation helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` static-page template missing-evidence、reference intent 和 source-refs upsert helper 已并入 `static_page_template_reference_support` 模块并补模块单测；可见供料、图表样例行、文档章节线索、已生成模板引用跳过和模板引用去重语义保持不变。
- 2026-06-13: `static_page_template_reference_support` missing-evidence/reference helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` resend-after 倒计时和 local key fingerprint helper 已并入 `auth_session_support` 模块并补模块单测；验证码重发倒计时边界、本地密钥 trim 后 SHA-256 指纹和 auth session token hash 输出语义保持不变。
- 2026-06-13: `auth_session_support` auth helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` report/static-page owner visibility wrapper 已并入 `resource_access` 模块并补模块单测；public owner、owner match、未登录和非 owner 隐藏语义保持不变。
- 2026-06-13: `resource_access` report/static-page owner visibility helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` dataset summary 和 metadata 读取 helper 已拆到 `dataset_summary_support` 模块并补模块单测；DatasetSummary 字段映射、snake/camel alias、字符串 trim、数组/嵌套列表收集和 limit 语义保持不变。
- 2026-06-13: `dataset_summary_support` dataset summary helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant scope 文档摘要 helper 已拆到 `assistant_scope_summary` 模块并补模块单测；word count、标题 fallback、parse status 优先级、content kind、媒体 hint、结构理解 hint 和计数摘要语义保持不变。
- 2026-06-13: `assistant_scope_summary` assistant scope summary helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` memory directory source-document scope helper 已拆到 `memory_directory_scope` 模块并补模块单测；显式 source ids 优先、manifest 递归收集去重、owner 可见性和文档 ACL 过滤语义保持不变。
- 2026-06-13: `memory_directory_scope` memory directory scope helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` workflow context UUID 解析 helper 已拆到 `workflow_context_support` 模块并补模块单测；缺失 key、非字符串值、非法 UUID 和合法 UUID 解析语义保持不变。
- 2026-06-13: `workflow_context_support` workflow context UUID helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run React completed event payload 和 html artifact 汇总 helper 已拆到 `assistant_run_react_support` 模块并补模块单测；entrypoint/html_artifacts 可选字段、artifact id 去重、默认标题/模板/source 字段语义保持不变。
- 2026-06-13: `assistant_run_react_support` assistant-run React helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 通用 SSE JSON/error/delta 编码 helper 已拆到 `sse_support` 模块并补模块单测；event/data 格式、error+done 终态、24 字符 delta 分块和单 delta 语义保持不变。
- 2026-06-13: `sse_support` SSE helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run SSE envelope/accepted/completed helper 已拆到 `assistant_run_sse_support` 模块并补模块单测；public envelope、completed response 字段、done 终态和 live-stream 后跳过 final delta 的语义保持不变。
- 2026-06-13: `assistant_run_sse_support` assistant-run SSE helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方客户可见文本清洗 helper 已拆到 `external_channel_public_text` 模块并补模块单测；状态映射、内部技术名替换、内嵌 JSON 剥离、重复产物链接去重、空白压缩、内部上下文兜底文案和截断语义保持不变。
- 2026-06-13: `external_channel_public_text` 第三方客户可见文本 helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方公开产物链接 helper 已拆到 `external_channel_public_artifact` 模块并补模块单测；公开产物 URL 提取、reply 卡片 fallback、客户可见报表链接追加和 artifact link 去重语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_public_artifact` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方静态页焦点和客户可见提示 helper 已拆到 `external_channel_static_page_focus` 模块并补模块单测；URL focus 优先、template adaptation 焦点模块、user intent 焦点推断、默认模块文案和客户侧提示语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_static_page_focus` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方公开 card 状态/裁剪/清洗 helper 已拆到 `external_channel_public_card` 模块并补模块单测；公开状态映射、artifact/preview 链接保留规则、内部字段剥离、静态页类型脱敏和 accepted template baseline 链接允许语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_public_card` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方 SSE envelope/public payload、静态页 SSE sequence、card status/poll 和 background continuation helper 已拆到 `external_channel_sse_support` 模块并补模块单测；SSE schema、兼容字段展开、状态排序、poll 解析和 cancelled-but-background-continues 语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方公开流式 resume sequence、已发布事件判定和 dedupe hash helper 已并入 `external_channel_sse_support` 模块并补模块单测；`Last-Event-ID`/query/body 优先级、负数 sequence 归零、公开 schema 过滤和 hash 稳定性保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` 公开流式 resume/dedupe helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方公开流式 payload compact、card summary、events 和 replay body helper 已并入 `external_channel_sse_support` 模块并补模块单测；内部字段裁剪、公开产物链接释放、preview 链接保留、since sequence replay 和公开 schema 过滤语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` 公开流式 payload/replay helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方 completed stream data helper 已并入 `external_channel_sse_support` 模块并补模块单测；task status 优先、card status fallback、空文本默认文案、客户可见文本清洗、reply type 和 artifact links 字段语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` completed stream data helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方 needs_input SSE 判定和文案 helper 已并入 `external_channel_sse_support` 模块并补模块单测；task_status/card status 判定、空文本默认文案和客户可见文本清洗语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` needs_input SSE helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方静态页 SSE pipeline/status/progress key/terminal/event name helper 已并入 `external_channel_sse_support` 模块并补模块单测；Image2 pipeline 类型识别、公开 artifact URL 终态提升、provisional artifact 保持处理中、cancelled 后台续接、progress key 和事件名语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` 静态页 SSE status/progress helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方 SSE delta+event 编码 helper 已并入 `external_channel_sse_support` 模块并补模块单测；先发 `external_channel.delta`、再发命名事件、公开 payload 清洗和内部 dedupe key 裁剪语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` SSE delta+event helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方 SSE retry 文案、静态页 SSE poll/timeout 配置和 live answer stream flag helper 已并入 `external_channel_sse_support` 模块并补模块单测；retry reason 文案、毫秒 env fallback、flag 开关值和现有调用语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` SSE timing/flag helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方 retrieval_started SSE event helper 已并入 `external_channel_sse_support` 模块并补模块单测；retrieval phase、sequence、processing status、conversation/message/idempotency 字段和客户可见检索提示文案保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` retrieval_started SSE helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方静态页 SSE progress text helper 已并入 `external_channel_sse_support` 模块并补模块单测；公开产物链接就绪文案、focus 模块提示、公开链接追加、publish retrying 公开文案和内部状态隐藏语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` static-page progress text helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run live answer stream flag helper 已并入 `assistant_run_sse_support` 模块并补模块单测；`ASSISTANT_RUN_LIVE_ANSWER_STREAM_ENABLED` 解析值、默认 false 和现有调用语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_sse_support` live answer stream flag helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方静态页 preview public URL helper 已并入 `external_channel_sse_support` 模块并补模块单测；relative preview key、绝对 URL query/fragment 裁剪、`data:image`/`blob:` 拒绝和 500 字符截断语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` static-page preview public URL helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方静态页 continue-polling SSE payload/event helper 已并入 `external_channel_sse_support` 模块并补模块单测；公开 envelope、sequence=95、status_url/poll_after_seconds、内部字段清理、dedupe key 和 persisted 包装调用语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` static-page continue-polling SSE helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方静态页 progress SSE payload/event helper 已并入 `external_channel_sse_support` 模块并补模块单测；运行态 public_url 提升为 published、公开 envelope、event name、sequence、artifact_links、dedupe key 和 persisted 包装调用语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` static-page progress SSE helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 第三方非持久化 completion SSE 测试 helper 已并入 `external_channel_sse_support` 模块并补模块单测；queued/completed/done 事件、delta 输出、sequence 和客户可见文本语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_channel_sse_support` non-persisted completion SSE helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 静态页 report snapshot 构造、业务表清洗、图表点/KPI 和 evidence notes helper 已拆到 `static_page_report_snapshot` 模块并补模块单测；`v3.report_snapshot` schema、大小写兼容字段、业务表内部字段裁剪、evidence note 角色和表 ID 规范化语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `static_page_report_snapshot` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 静态页 data snapshot version、updated_at、validation summary 和 unit hints helper 已拆到 `static_page_data_snapshot_support` 模块并补模块单测；snapshotVersion 前缀、最新时间戳选择、模块/样本计数、unitHints 去重截断和 validation status 语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `static_page_data_snapshot_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 静态页 structure signals、heading candidate 和 docs-page heading binding helper 已拆到 `static_page_structure_signals` 模块并补模块单测；`structure_signals` schema、source structure policy、field/bound module 摘要、sectionTitleHints 去重截断和 docs-page 结构模块绑定语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `static_page_structure_signals` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 外部对话观测 timeline event view、payload 摘要、phase/status/display fallback 和 artifact link 提取 helper 已拆到 `external_conversation_timeline` 模块并补模块单测；SSE payload 脱敏、内部字段裁剪、状态推断、链接去重过滤和 debug payload 语义保持不变。本地 `cargo check`、关联 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_conversation_timeline` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、关联 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 外部文档 URL 安全校验、对象根路径、文件扩展名、路径段清洗和 URL 脱敏 helper 已拆到 `external_document_object_support` 模块并补模块单测；HTTPS/loopback/private IP 策略、content-type fallback、路径段截断和 query/credential 脱敏语义保持不变。本地 `cargo check`、外部文档解析 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_document_object_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、外部文档解析 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 外部数据库源 connector kind、表名、连接串库名推断和连接串脱敏 helper 已拆到 `external_database_source_config_support` 模块并补模块单测；MySQL alias、unsupported kind 错误、表名去重/长度/控制字符、JDBC/MySQL URL 解析和 raw credential 脱敏语义保持不变。本地 `cargo check`、数据库源 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `external_database_source_config_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、数据库源 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` ZIP 归档入库 entry 安全命名、标题、跳过规则、扩展名白名单、content-type 推断和 env 默认值 helper 已拆到 `zip_ingest_support` 模块并补模块单测；ZIP 展开输出命名、忽略目录、支持文件类型、子文档 content-type 和 env fallback 语义保持不变。本地 `cargo check`、ZIP 展开 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `zip_ingest_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、ZIP 展开 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` workflow/dataset/chat/assistant/static-page/report/document 等路径参数 UUID 解析 helper 已拆到 `id_parse_support` 模块并补模块单测；各 ID wrapper、invalid UUID 错误码和错误文案保持不变。本地 `cargo check`、ID 解析 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `id_parse_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、ID 解析 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 文档摘要 dataset id 去重和媒体 content-type 类型推断 helper 已拆到 `document_view_support` 模块并补模块单测；canonical dataset 优先、去重顺序、audio/video/unknown 推断语义保持不变。本地 `cargo check`、文档视图 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `document_view_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、文档视图 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 检索搜索 limit/backend 和静态页/HTML 产物列表 limit helper 已拆到 `retrieval_query_support` 模块并补模块单测；默认 limit、边界 clamp、legacy/postgres backend fallback、scan/candidate limit 和列表 limit 语义保持不变。本地 `cargo check`、检索 backend Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `retrieval_query_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、检索 backend Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 检索证据 relevance 排序和 recall rank hint helper 已拆到 `retrieval_evidence_ranking_support` 模块并补模块单测；recall score 降序、rank hint 升序、chunk index 升序和 created_at 新优先排序语义保持不变。本地 `cargo check`、检索排序 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `retrieval_evidence_ranking_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、检索排序 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 检索证据 lexical query score、manifest term weights 和 vector norm helper 已并入 `retrieval_evidence_ranking_support` 模块并补模块单测；cosine score、四位小数 rounding、非数值 term 跳过、空权重/零 norm 返回 0 语义保持不变。本地 `cargo check`、检索排序 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `retrieval_evidence_ranking_support` lexical score helper 拆分已提交并同步 8 服务器；远端 `cargo check`、检索排序 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 检索文本 lexical score 和 limited term weights helper 已并入 `retrieval_evidence_ranking_support` 模块并补模块单测；文本 cosine score、空权重/零 norm 返回 0、term weight 排序截断和 limit=0 语义保持不变。本地 `cargo check`、检索排序 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `retrieval_evidence_ranking_support` lexical text helper 拆分已提交并同步 8 服务器；远端 `cargo check`、检索排序 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 检索 evidence search text 和 document chunk TOC 判定 helper 已并入 `retrieval_evidence_ranking_support` 模块并补模块单测；summary/excerpt/source/payload/section hints 拼接、目录/页码尾/省略号 TOC 判定语义保持不变。本地 `cargo check`、检索排序/文档 chunk 搜索文本 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `retrieval_evidence_ranking_support` search text/TOC helper 拆分已提交并同步 8 服务器；远端 `cargo check`、检索排序/文档 chunk 搜索文本 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` HTML artifact 下载文件 path/filename/content-type helper 已拆到 `html_artifact_download_support` 模块并补模块单测；支持模板类型、generated_artifacts snake/camel alias、文件索引错误、绝对本地路径、允许根校验、文件名清洗和 content-type fallback 语义保持不变。本地 `cargo check`、HTML artifact 下载 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `html_artifact_download_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、HTML artifact 下载 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` HTML artifact manifest 从事件/记录递归收集、排序和去重 helper 已拆到 `html_artifact_collection_support` 模块并补模块单测；事件倒序扫描、嵌套 `html_artifacts` 递归、非法 manifest 跳过、limit 截断、created_at 倒序和同 id 保留最新语义保持不变。本地 `cargo check`、HTML artifact 事件收集 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `html_artifact_collection_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、HTML artifact 事件收集 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` HTML artifact 文本摘要、variant 序列化、action intent prompt 和 patch operations helper 已拆到 `html_artifact_summary_support` 模块并补模块单测；不安全文本拦截、字符截断、enum 字符串序列化、action prompt 字段优先级和 patch/operations alias 语义保持不变。本地 `cargo check`、HTML artifact event/patch Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `html_artifact_summary_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、HTML artifact summary/event/patch Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` HTML artifact event request、action intent payload、patch payload 和 payload safety 校验 helper 已拆到 `html_artifact_event_support` 模块并补模块单测；interaction mode 限制、event type 匹配、payload 大小限制、不安全内容拦截、patch op/path/from 校验和错误码/文案保持不变。本地 `cargo check`、HTML artifact event/patch Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `html_artifact_event_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、HTML artifact event/patch Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` HTML artifact patch 转静态页 operation 的 JSON pointer、module selector、nested field、layout 校验和 mobile order/style direction 转换 helper 已拆到 `html_artifact_static_page_patch_support` 模块并补模块单测；原入口继续调用 `validate_static_page_operations`，转换语义、错误码和错误文案保持不变。本地 `cargo check`、HTML artifact static-page patch/event Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `html_artifact_static_page_patch_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、HTML artifact static-page patch/event Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 文档 chunk 搜索文本、section title hints、noun terms、document structure block 和标题规范化 helper 已拆到 `document_chunk_support` 模块并补模块单测；搜索文本拼接、metadata alias、parse structure hints、标题推断、结构块类型判定和候选词提取语义保持不变。本地 `cargo check`、文档 chunk 搜索文本/检索排序 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `document_chunk_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、文档 chunk 搜索文本/检索排序 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 文档 chunk fallback supply count、source locator 和 summary helper 已并入 `document_chunk_support` 模块并补模块单测；`document_chunk_fallback` 计数、`object_key/title#chunk` locator、summary title/section/excerpt 形状保持不变。本地 `cargo check`、fallback supply/文档 chunk 搜索文本/检索排序 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `document_chunk_support` fallback helper 拆分已提交并同步 8 服务器；远端 `cargo check`、fallback supply/文档 chunk 搜索文本/检索排序 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 文档 chunk 检索供料 query-centered excerpt 和 prompt match helper 已并入 `document_chunk_support` 模块并补模块单测；文本空白规范化、问题词命中居中、无命中回退开头窗口、前后省略号和长 token 优先匹配语义保持不变。本地 `cargo check`、fallback supply/检索排序 Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `document_chunk_support` excerpt helper 拆分已提交并同步 8 服务器；远端 `cargo check`、fallback supply/检索排序 Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run detail target 构造和计数 helper 已拆到 `assistant_run_detail_support` 模块并补模块单测；detail-first gating、retrieval evidence 过滤、文档去重、media/timestamp reason、字段 alias 保留、limit 截断和 detail target 计数语义保持不变。本地 `cargo check`、附件标题/detail target/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_detail_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、附件标题/detail target/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run evidence supplied count 和 evidence status label helper 已拆到 `assistant_run_evidence_state_support` 模块并补模块单测；`supplied_items` 数组计数、`supplied/empty/not_requested/unknown` 状态文案、fallback supply count 和 detail target count 组合文案保持不变。本地 `cargo check`、Codex package/ReAct trace/supply quality Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_evidence_state_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、Codex package/ReAct trace/supply quality Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run placeholder user message 和 evidence trail label helper 已并入 `assistant_run_evidence_state_support` 模块并补模块单测；占位运行时用户提示、普通聊天/空供料/已供料边界文案和证据链标签语义保持不变。本地 `cargo check`、Codex package/ReAct trace/evidence trail Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_evidence_state_support` evidence text helper 拆分已提交并同步 8 服务器；远端 `cargo check`、Codex package/ReAct trace/evidence trail Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` conversation memory 供料值、外部会话 ID、source assistant run ids、limit 和供料资格 helper 已拆到 `assistant_run_conversation_memory_support` 模块并补模块单测；对话记忆供料 JSON 形状、用户态过滤、env limit clamp、assistant_run_id 去重和 metadata/source refs fallback 语义保持不变。本地 `cargo check`、conversation memory/ReAct recall/external user memory/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_conversation_memory_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、conversation memory/ReAct recall/external user memory/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` conversation memory scope ids、current-thread alias、local-thread/user-context/external-user scope 映射、候选项和全局用户记忆 key helper 已并入 `assistant_run_conversation_memory_support` 模块并补模块单测；scope 去重、空值过滤、当前线程恢复、候选项 JSON 形状和用户 key 语义保持不变。本地 `cargo check`、conversation memory scope/route/ReAct recall/external user memory/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_conversation_memory_support` scope helper 拆分已提交并同步 8 服务器；远端 `cargo check`、conversation memory scope/route/ReAct recall/external user memory/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run scope intent、supply policy、默认数据库供料抑制、policy string 和 prefer detail helper 已拆到 `assistant_run_scope_policy_support` 模块并补模块单测；intent alias、supply_policy/supplyPolicy、snake/camel policy 字段、detail_first 判定和默认值语义保持不变。本地 `cargo check`、scope policy/static-page scope/resume scan/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_scope_policy_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、scope policy/static-page scope/resume scan/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run selected dataset/document/template/temporary/attachment ids、scope action/context/candidate policy、recommended tool actions 和 entity-scan policy helper 已拆到 `assistant_run_scope_selection_support` 模块并补模块单测；scope id 顺序去重、模板文档不进证据、attachment title alias、policy 默认值/覆盖、recommendedActions 截断和 coverage/recommended scan 判定语义保持不变。本地 `cargo check`、scope selection/static-page scope/resume scan/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_scope_selection_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、scope selection/static-page scope/resume scan/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` prompt substring/token/metric matching helper 已拆到 `prompt_match_support` 模块并补模块单测；中文 substring、ASCII token、case-insensitive substring、metric token boundary 和 `markdown` 不误判 `down` 的语义保持不变。本地 `cargo check`、prompt match/database aggregate/entity scan/attendance/static-page scope Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `prompt_match_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、prompt match/database aggregate/entity scan/attendance/static-page scope Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` external answer policy 输出格式、evidence-state policy、default_prompt 强语气识别和 model-facing professional boundary helper 已拆到 `assistant_run_answer_policy_support` 模块并补模块单测；output_format object/string 解析、snake/camel policy path、强语气转专业边界和原 default_prompt 保留语义保持不变。本地 `cargo check`、answer policy/customer tone/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_answer_policy_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、answer policy/customer tone/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run model context/scope compact helper 已拆到 `assistant_run_model_context_support` 模块并补模块单测；nested answer_policy 转专业边界、scope document 采样预算、scope id 数组预算、候选 scope 嵌套压缩和 metadata/raw blob 不入模型上下文语义保持不变。本地 `cargo check`、model context/large document scope/answer policy/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_model_context_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、model context/large document scope/answer policy/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run model supplied-items bucket/budget helper 已拆到 `assistant_run_model_supply_budget_support` 模块并补模块单测；不同供料类型独立 quota、原始顺序保持、unknown type 归入 other、included/omitted budget 字段和 model_rule 语义保持不变。本地 `cargo check`、supply budget/model context/web search/large document scope/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_model_supply_budget_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、supply budget/model context/web search/large document scope/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run model supplied-item formatter helper 已拆到 `assistant_run_model_supply_item_support` 模块并补模块单测；retrieval evidence 安全 manifest 摘要、search evidence contract/model_rule、database aggregate 默认 guidance/rows 压缩和 conversation memory 摘要/ref 截断语义保持不变。本地 `cargo check`、supply item/model context/web search/large document scope/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_model_supply_item_support` helper 拆分已提交并同步 8 服务器；远端 `cargo check`、supply item/model context/web search/large document scope/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` structured fact/entity-scan compact helper 已拆到 `assistant_run_structured_fact_context_support` 模块并补模块单测；entity scan payload、fact snapshot payload、fact snapshot as scan payload、rows_by_type 压缩、include_fact_snapshots 开关和 resume/project delivery/profile rows 语义保持不变。本地 `cargo check`、structured fact/model context/fact snapshot/direct answer/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_structured_fact_context_support` helper 拆分已提交并随最新 `main` 同步 8 服务器；远端当前 HEAD `a3fe7402b` 包含本轮 `3a7c415`，`cargo check`、structured fact Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` resume profile 表格/字段/排序 helper 已拆到 `assistant_run_resume_profile_support` 模块并补模块单测；候选人名、数组字段展示、缺失值 `-`、Markdown 表格转义、年龄/年份/技能数排序、性别统计、工作年限和学历 rank 语义保持不变。本地 `cargo check`、resume profile/entity scan/fact snapshot/direct answer/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_resume_profile_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `16143fb69`，`cargo check`、resume profile/entity scan/fact snapshot/direct answer/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` resume project delivery 直答/HTML artifact helper 已拆到 `assistant_run_resume_project_delivery_support` 模块并补模块单测；逐份简历项目交付表、未识别项目交付段占位、artifact payload summary、safe generation policy、dataset data refs 去重/截断和 rapid HTML artifact 语义保持不变。本地 `cargo check`、resume project delivery/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_resume_project_delivery_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `d8b77086f`，`cargo check`、resume project delivery/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` resume profile 匹配条件、term 匹配和评分 helper 已拆到 `assistant_run_resume_profile_match_support` 模块并补模块单测；提示词优先字段、最长 term 优先、ASCII/company/中文 term 匹配、noise 过滤、匹配评分、匹配摘要和候选人筛选语义保持不变。本地 `cargo check`、resume profile match/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_resume_profile_match_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `3c10124fa`，`cargo check`、resume profile match/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` resume profile 匹配答案生成 helper 已并入 `assistant_run_resume_profile_match_support` 模块并补模块单测；匹配条件为空时不直答、全条件过滤、match score/最近年份/技能数/候选人排序、无匹配文案和原 Markdown 表格列顺序语义保持不变。本地 `cargo check`、resume profile match/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_resume_profile_match_support` 匹配答案 helper 拆分已提交并同步 8 服务器；远端 HEAD `a92df5c7a`，`cargo check`、resume profile match/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` resume prompt intent helper 已拆到 `assistant_run_resume_prompt_support` 模块并补模块单测；简历排序/排行/出表识别、项目交付明细识别、职位/地点/公司/教育/证书/年限排序、候选人匹配识别、频次/覆盖类问题保护和升序判断语义保持不变。本地 `cargo check`、resume prompt/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_resume_prompt_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `1d4aab509`，`cargo check`、resume prompt/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 通用维度 prompt intent helper 已拆到 `assistant_run_prompt_dimension_support` 模块并补模块单测；技能/项目/岗位/人员/地点/学校/学历/证书/关键词/年份/章节/段落/表格/年龄/性别/时间统计识别、简历信号和简历时间统计边界语义保持不变。本地 `cargo check`、prompt dimension/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_prompt_dimension_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `5de8deffa`，`cargo check`、prompt dimension/resume/entity scan/fact snapshot/Codex package/ReAct trace Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` static-page template reference helper 已拆到 `static_page_template_reference_support` 模块并补模块单测；模板 ID 解析、intent 推断、enabled/paused 模板校验、safe design reference JSON、guardrails/forbidden outputs 和后续模板适配调用语义保持不变。本地 `cargo check`、static-page template reference/static-page artifact Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `static_page_template_reference_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `48f409b86`，`cargo check`、static-page template reference/static-page artifact Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` static-page document-template reference/evidence summary helper 已并入 `static_page_template_reference_support` 模块并补模块单测；第三方上传/选择模板只作为格式/结构/字段参考、模板不扩展事实权限、evidence summary、临时合同面积/客流 supplemental metrics 和后续静态页模板调用语义保持不变。本地 `cargo check`、static-page template reference/document template/data snapshot/static-page artifact Rust 回归、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `static_page_template_reference_support` document-template helper 拆分已提交并同步 8 服务器；远端 HEAD `5b25d1405`，`cargo check`、static-page template reference/document template/data snapshot/static-page artifact Rust 回归、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` static-page template prewarm 候选、source refs、低负载策略、prompt 和 task payload helper 已拆到 `static_page_template_prewarm_support` 模块并补模块单测；同数据集组合/default_prompt 的 prewarm key、客户不可见 source refs、低负载 contract、导出字段和第三方报表触发语义保持不变。本地 `cargo check`、prewarm/template match/template baseline/static-page artifact Rust 回归、prewarm observability smoke、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `static_page_template_prewarm_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `e4f02add0`，`cargo check`、prewarm/template match/template baseline/static-page artifact Rust 回归、prewarm observability smoke、Web build、release build、health/ready、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` static-page payload、preview contract、payload field accessor 和通用 JSON merge/set helper 已拆到 `static_page_payload_support` 模块并补模块单测；设计指纹、preview stale 判定、移动端模块顺序、payload 字段 alias、递归 merge 和现有静态页操作状态机语义保持不变。本地 `cargo check`、payload/preview contract/operation/render guard/static-page artifact Rust 回归、prewarm observability smoke、Web build、第三方报表/导出/临时文档/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `static_page_payload_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `e32f40ee0`，`cargo fmt --check`、payload/render guard/static-page artifact Rust 回归、`cargo check`、Web build、release build、health/ready、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` 新百已发布报表链接直答、runtime manifest 和 output artifacts helper 已拆到 `assistant_run_xinbai_report_link_support` 模块并补模块单测；新百报表链接窄触发、修复/重生成请求不误复用、导出 URL、外部通道 artifact card 和质量补链路语义保持不变。本地 `cargo check`、xinbai report link/public response/SSE exports/static-page artifact Rust 回归、prewarm observability smoke、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底 smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `assistant_run_xinbai_report_link_support` helper 拆分已提交并同步 8 服务器；远端 HEAD `fd7b220cc`，`cargo fmt --check`、xinbai report link/public response/SSE exports/static-page artifact Rust 回归、`cargo check`、Web build、release build、health/ready、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` assistant-run provider retry attempts/backoff/delay/retryable-error helper 已拆到 `assistant_run_provider_retry_support` 模块并补模块单测；retry attempts/backoff env clamp、invalid env fallback、指数退避上限和 transient provider failure retryable 判定语义保持不变。本地 `cargo fmt --check`、provider retry/support/static-page artifact Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `709bf39ce`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` workflow task view、logical queue/key、remote poll metadata 和 queue stats 聚合 helper 已拆到 `workflow_task_view_support` 模块并补模块单测；显式 logical 字段优先、legacy static-page publish 推断、retrying 计数、duration percentile 和 next available 聚合语义保持不变。本地 `cargo fmt --check`、workflow task view/stats Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `ffb86d3aa`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-13: `crates/platform-api/src/lib.rs` static-page render output download/preview URL、外部通道 URL、retryable failure reason 和 URL path segment encoding helper 已拆到 `static_page_render_output_view_support` 模块并补模块单测；Rendered+non-empty HTML 才出链接、external channel scope 优先、internal fallback、失败原因字段优先级和 path segment 编码语义保持不变。本地 `cargo fmt --check`、static-page render output URL/retryable reason Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `13cae778f`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` 检索证据 view mapping 和 evidence manifest parser helper 已拆到 `retrieval_evidence_view_support` 模块并补模块单测；view 字段透传、manifest status/id/timestamp fallback、recall rank fallback 和 existing recall metadata regression 语义保持不变。本地 `cargo fmt --check`、retrieval evidence view Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `5453d097f`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` 工具定义 view、工具执行 view 和 manifest tool trace parser helper 已拆到 `tool_view_support` 模块并补模块单测；tool registry reference 展开、inline tool snapshot 解析、CLI contract、tool execution source/status 映射和 manifest tool trace 语义保持不变。本地 `cargo fmt --check`、tool view Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `ec079ef8e`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` LLM invocation domain-to-contract view mapping helper 已拆到 `llm_invocation_view_support` 模块并补模块单测；`source_kind`、`mode`、`finish_reason`、usage、provider metadata、system prompt 和 tool trace 字段映射语义保持不变。本地 `cargo fmt --check`、LLM invocation view Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `60e37eeb7`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` runtime manifest helper 已拆到 `runtime_manifest_support` 模块并补模块单测；LLM finish reason 到 manifest finish reason、最新 invocation runtime、tool trace 排序、tool status 计数和 workflow execution scope runtime summary 语义保持不变。本地 `cargo fmt --check`、runtime manifest Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `cedc0f786`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `scripts/smoke/static-page-prewarm-observability.mjs` 已补强低负载预热观测回执，新增 `waitingForLowLoadCount`、`customerVisiblePrewarmLeakCount` 和 `prewarmCustomerVisibilityOk`；self-test 现在明确验证 waiting-for-low-load 事件保持 `customer_visible=false` 且泄漏计数为 0。生产 prewarm 仍默认关闭，不启用后台生图。已提交并同步 8 服务器，远端 HEAD `1d55360ae`，脚本级 smoke、health/ready 和服务 active 检查均通过，未 build、未重启。
- 2026-06-14: `crates/platform-api/src/lib.rs` report plan/render/published report view helper 已拆到 `report_view_support` 模块并补模块单测；report plan summary、AST version、render output、published report/version 字段映射、model_facing 派生和导出 artifact manifest 透传语义保持不变。本地 `cargo fmt --check`、report view Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `bf62d4cbf`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` memory directory view 和 directory manifest parser helper 已拆到 `memory_directory_view_support` 模块并补模块单测；目录计数、source document ids、原始 manifest 透传、有效 directory tree hydrate、无效 manifest raw fallback、根节点 scope/version 默认值和文档 lifecycle/chunk count 解析语义保持不变。本地 `cargo fmt --check`、memory directory view Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `a14c9f8b9`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` manifest runtime/context binding/provider failure parser helper 已拆到 `manifest_runtime_view_support` 模块并补模块单测；context binding、runtime mode、finish reason、自定义 finish reason、provider failure kind/message、token usage、provider/model/request/system prompt/tool trace 字段解析语义保持不变。本地 `cargo fmt --check`、manifest runtime Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `f996ecc8e`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` dataset output manifest/view mapping helper 已拆到 `dataset_output_view_support` 模块并补模块单测；manifest content section、retrieval evidence ids、service handoff、generator placeholder runtime、LLM invocation runtime 覆盖、tool execution trace 覆盖、DatasetOutputView 字段映射语义保持不变。本地 `cargo fmt --check`、dataset output view Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `bb2aa3413`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` manifest service handoff、report entry state、resolution 和 timestamp parser helper 已拆到 `manifest_service_handoff_support` 模块并补模块单测；service lane、report entry state、resolved action、optional timestamp、suggested title/objective 和 confirmed report plan id 解析语义保持不变。本地 `cargo fmt --check`、manifest service handoff/dataset output/report/chat message Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `161a6e24a`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` chat session in-progress turn 判断和 pending turn manifest 构造 helper 已拆到 `chat_session_turn_manifest_support` 模块并补模块单测；pending/completed/failed 判断、prompt trim、initial prompt 保留、context binding 默认值、latest memory/output id 映射、turn_started 事件和 pending last_turn 字段语义保持不变。本地 `cargo fmt --check`、chat session turn manifest/pending chat session/chat message Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `bd88244bb`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` chat session/message manifest parser、chat turn runtime parser、chat turn event builder 和 stream/artifact/tool-loop inference helper 已拆到 `chat_session_manifest_view_support` 模块并补模块单测；session status、report entry、latest memory/output scope、message output/evidence ids、service handoff、placeholder runtime、stream/tool/artifact 状态推断和事件时间线语义保持不变。本地 `cargo fmt --check`、chat session manifest/pending chat session/chat message Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `940b78afa`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` chat message manifest hydrate 和 latest LLM/tool execution 补全 helper 已并入 `chat_session_manifest_view_support` 模块并补模块单测；latest LLM runtime 覆盖、tool trace 排序、runtime tool_trace_count 覆盖、provider status/finish reason 推断、tool status summary、tool loop settled time 和事件重建语义保持不变。本地 `cargo fmt --check`、chat session manifest/pending chat session/chat message Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `9d4987363`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` assistant turn message metadata hydrate helper 已并入 `chat_session_manifest_view_support` 模块并补模块单测；非 assistant 消息忽略、assistant message id/persisted_at/completed_at 补全、artifact commit ready/status 推断、failure source 清理和事件重建语义保持不变。本地 `cargo fmt --check`、chat session manifest/pending chat session/chat message Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `ca821f667`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` chat message view assembly helper 已并入 `chat_session_manifest_view_support` 模块并补模块单测；ChatMessageView 字段映射、message manifest hydrate、assistant turn metadata 补全和 model-facing summary 派生语义保持不变。本地 `cargo fmt --check`、chat session manifest/pending chat session/chat message Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `efd33ee0c`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` chat session manifest hydrate 和 session view assembly helper 已并入 `chat_session_manifest_view_support` 模块并补模块单测；ChatSessionView 字段映射、latest assistant message id、assistant replied 会话 last_turn 投影和 session runtime 清理语义保持不变。本地 `cargo fmt --check`、chat session manifest/session view/message view Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT/静态页并发/Cloudflare 兜底/prewarm observability smoke 和主站流式/本地会话 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `024ba3755`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。

**Regression commands:**

```bash
cargo fmt --check
cargo test -p platform-api <touched_module_or_behavior_filter> --lib
cargo test -p platform-api external_channel_static_page_artifact --lib
cargo check -p platform-api
npm --prefix apps/web run build
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
npm run smoke:external-scoped-document-chat -- --self-test
npm run smoke:external-video-ppt -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
npm run smoke:static-page-prewarm-observability -- --self-test
node --test apps/web/app/lib/assistant-run-progress.test.mjs
node --test apps/web/app/lib/local-chat-sessions.test.mjs
```

**Done when:**
- 行为不变。
- 回归集通过。
- 每个提交可单独回退。
- 不改第三方公开 API URL、鉴权、必填请求字段或已有响应字段。

- 2026-06-14: `crates/platform-api/src/lib.rs` 的 `parse_external_bot_message_payload` 解析编排已拆到 `external_bot_message_parse_support` 模块并补模块单测；payload alias、artifact request、requested skills、AI Golf schema guard 和 answer policy 的执行顺序保持不变。本地和 8 服务器 `cargo fmt --check`、解析模块、第三方 payload alias、artifact/requested-skills/answer-policy/AI Golf/静态页触发 Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT、主站/第三方并发、静态页 5 路、Cloudflare fallback、P2 dry-run、production readiness、prewarm observability self-test 和前端 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `3a413c636`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。`main-assistant-streaming` 与 `external-channel-streaming-10way` 当前无 `--self-test` 参数，真实流式验证仍归入 P1 live/8 服务器窗口。
- 2026-06-14: `crates/platform-api/src/lib.rs` 的 AI Golf requested skill kind、schema/image 入参校验、course-map segmentation skill 选择和 skill policy 生成已拆到 `external_aigolf_skill_support` 模块并补模块单测；AI Golf 执行器 payload/回复/事件恢复逻辑仍留在原路径，字段和行为保持不变。本地和 8 服务器 `cargo fmt --check`、AI Golf support 模块、AI Golf 旧端到端卡片/事件、第三方解析/payload alias、静态页触发 Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT、主站/第三方并发、静态页 5 路、Cloudflare fallback、P2 dry-run、production readiness、prewarm observability self-test 和前端 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `23acb867f`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` 的 external output format label/model rule 和 `external_answer_policy_value` 已拆到既有 `external_answer_policy_support` 模块并补模块单测；第三方 answer policy 结构、输出格式约束、render mode rule 和 assistant-run guidance fallback 行为保持不变。本地和 8 服务器 `cargo fmt --check`、answer policy support 模块、旧 answer policy、第三方解析/payload alias、静态页触发 Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT、主站/第三方并发、静态页 5 路、Cloudflare fallback、P2 dry-run、production readiness、prewarm observability self-test 和前端 Node 回归均通过；已提交并同步 8 服务器，远端 HEAD `de6edd652`，release build、服务重启、health/ready 和 8 服务器 P0 smoke 均通过。
- 2026-06-14: `crates/platform-api/src/lib.rs` 的 `external_requested_skill_mode` 和 `external_requested_skills_policy_value` 已拆到既有 `external_requested_skills_support` 模块并补模块单测；第三方 requested-skills policy payload 的 `source`、`default_mode`、`engine`、`enforcement`、`model_rule` 和 skills 字段映射语义保持不变。本地 `cargo fmt --check`、requested skills support、第三方解析/payload alias、artifact/answer-policy/AI Golf/静态页触发 Rust 回归、`cargo check`、Web build、第三方报表/导出/临时文档/视频 PPT、主站/第三方并发、静态页 5 路、Cloudflare fallback、P2 dry-run、production readiness、prewarm observability self-test 和前端 Node 回归均通过；待提交并同步 8 服务器。

## 10. 当前下一步

1. 默认继续 P5：选择下一个 `platform-api` 或主站前端行为保持小切片，先补定向测试，再做拆分和 P0 回归。
2. 若用户要求发布，再按 P0 code deploy 边界同步 8 服务器；若只是文档同步，则只 fast-forward，不 build、不重启。
3. 若拿到合法 operator 凭证或运维脱敏回执，优先切到 P1-1 authenticated operator live。
4. 若进入受控压测窗口，执行 P1-2 live 20 路问答和 5/2 路重任务并发验证。
5. 若出现新客户失败样例，按 P2-1 或 P3 做 targeted smoke；只扩新样例或新类型，不重复跑已覆盖类型。
6. 若用户明确确认生产写入、backfill、对象清理或 source sync，才进入 P2/P4 真实执行准备；否则保持 dry-run/summary-only。
7. GitHub Actions 账号额度恢复后，重跑 DataMax CI 并把结果补回 validation。
