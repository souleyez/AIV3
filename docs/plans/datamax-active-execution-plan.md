# DataMax 主线执行计划

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.
>
> 本文件是 `docs/plans/` 下唯一有效计划。历史计划保留在 `docs/archive/plans/`；长日志、回执和失败定位统一写入 `docs/validation/datamax-main-gap-closure.md`。

**目标:** 让 DataMax 在生产环境持续满足主站问答、第三方问答、范围授权、报表/静态页生成、数据接入分析和异步文档理解这几条主线。

**架构:** 企业记忆、文档权限、报表模板、任务编排和产物发布都由 DataMax 平台侧管理。模型调用保持无状态，只接收本轮由平台裁剪后的上下文、检索证据和工具结果。重任务通过 worker、smoke 脚本或固定 Codex/Cloudflare 执行器链路处理；涉及生产写入的动作按本计划标注必须人工确认。

**技术栈:** Rust workspace services、Next.js `apps/web`、Node smoke scripts、PostgreSQL、8 服务器 systemd workers、静态页/报表产物、本地模板复用和可选 Cloudflare/Codex fallback。

**重建日期:** 2026-06-12。

---

## 1. 执行规则

- `docs/plans/` 只保留本文件。
- 本文件只保留当前状态、执行队列、验收命令和完成标准；详细证据写入 `docs/validation/datamax-main-gap-closure.md`。
- 不默默修改第三方公开 API URL、鉴权、必填请求字段或已有响应字段；确需变更先问。
- 不记录密钥、bearer、cookie、provider key、数据库 URL、原始客户行、原始 provider payload、完整文档内容、私有对象路径或 content hash。
- 不动 120 服务器。
- 质量门禁保持禁用或被动采样；不要恢复会拦截正常回答的硬门禁。
- 数据接入可以产出分析和 `staging_plan`；生产 schema/data 写入必须人工确认。
- P2 解析和 fingerprint 默认只走 `--dry-run --summary-only`；真实 backfill、入队、对象清理或源同步必须单独确认。
- 纯文档或纯 smoke 脚本更新不需要重启 8 服务器服务。

## 2. 当前基线

详细证据入口：`docs/validation/datamax-main-gap-closure.md`。

| 模块 | 当前状态 | 剩余缺口 |
| --- | --- | --- |
| 主站 | `https://doc.elepcloud.com/` 是无需登录直接问答入口。8 服务器主站 20 路 live smoke 已通过。 | 真实流式 delta 仍需在具备受控上下文时补一次生产验证。 |
| 管理台/文档 | `https://v3.elepcloud.com/` 是管理台和第三方接口文档入口。 | 保持域名分工稳定；不做未经确认的公开接口变更。 |
| 第三方问答 | 第三方 20 路 live smoke 已通过；使用服务器侧 active bearer，未打印 token。数据集/文档范围授权已实现。 | 遇到客户新失败样例时继续做范围相关 live smoke。 |
| 报表/静态页 | 新百默认模板是 `xinbai-functional-modular-template-20260604`；第三方静态页 5 路 live smoke 已复用模板并返回 HTTP 200 产物。 | queue-stats 和低负载预热 live 观测需要合法 operator cookie/bearer 或运维侧安全回执。 |
| 报表导出 | 标准产物为 `index.html`、`table-data.csv`、`report.ppt`、`report.md`。 | 用户可见文案里链接只出现一次；已有结构字段中继续承载产物 URL。 |
| 数据源/CC 模式 | 数据源页和 staging analysis 链路已存在。CC/Codex 可协助 DB/API 接入，但必须落到明确目标数据集。 | 生产写入仍需人工确认。 |
| P2 解析 | 8 服务器已对 DOC/DOCX/PDF/XLSX/PPTX/MP4 类小样本完成 summary-only dry-run；未写入、未入队。 | 继续扩样，并在真实 backfill 前确认 fact 类型用途策略。 |
| 文档 fingerprint 治理 | 8 服务器只读聚合 inventory 已通过：`document_count=2637`、`fingerprinted_document_count=32`、`canonical=32`、`unknown=2605`、重复 hash 分组 `0`，并已增加缺 fingerprint 原因聚合。对象清理 dry-run 计划脚本已提供聚合影响和回滚要求；本地对象可达性 preflight 已提供 stat-only 聚合检查。 | 真实对象清理仍禁用；当前不删除文件。 |
| CI | 本地和 8 服务器等价命令覆盖当前主要缺口。GitHub Actions 因账号付款/额度状态在 job 启动前失败。 | 账号状态恢复后重跑 DataMax CI；当前 Actions 失败不按代码失败处理。 |

## 3. 当前执行队列

### P0-1 已完成：收尾当前 P2 Fingerprint Smoke 批次

**原因:** 当前工作区已有安全的 P2-2 文档 fingerprint inventory smoke 改动，应该先验证、提交、推送、同步 8 服务器，再开新功能。

**状态:** 已提交 `2b34dff`，已推送 GitHub，8 服务器已 fast-forward 到 `2b34dfffc` 并完成 post-sync smoke。后续只在修改 P2 fingerprint/inventory 逻辑时重跑本节。

**涉及文件:**
- 新增：`scripts/smoke/document-fingerprint-inventory.mjs`
- 修改：`package.json`
- 修改：`scripts/README.md`
- 修改：`.github/workflows/datamax-ci.yml`
- 修改：`docs/plans/datamax-active-execution-plan.md`
- 修改：`docs/validation/datamax-main-gap-closure.md`

**本地验证:**

```bash
git diff --check
node --check scripts/smoke/document-fingerprint-inventory.mjs
npm run smoke:document-fingerprint-inventory -- --self-test
npm run smoke:p2-summary-only-dry-run -- --self-test
```

**推送后 8 服务器验证:**

```bash
cd /srv/aiv3/repo
git pull --ff-only origin main
npm run smoke:document-fingerprint-inventory -- --self-test
npm run smoke:document-fingerprint-inventory -- --env-file /etc/aiv3/aiv3.env --dataset-limit 20 --pretty --output-dir target/document-fingerprint-inventory-smoke-p2-20260612
systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-chat-session-worker.service aiv3-static-page-worker.service
```

**完成标准:**
- 本地 self-test 通过。
- 8 服务器 live inventory 只输出聚合计数。
- 回执不含 DB URL、token、原始 URL、content hash、文档标题、对象路径或正文。
- validation 记录 post-sync run id 和服务器 commit。

### P0-2 已完成自测：稳定第三方报表触发

**原因:** 客户可见问题集中在“要求报表但没有触发”和“触发报表后截断正常回答”。

**状态:** 本地和 8 服务器自测已通过；8 服务器 Rust 路由测试、触发/误触发 guard、导出字段和 public artifact 校验均通过。后续遇到新的客户失败样例时按本节重跑针对性 live smoke。

**范围:**
- 触发词放宽只限新百经营报表域。
- 触发报表不能截断正常回答。
- 用户可见文案中报表链接只出现一次。
- 已有结构字段继续携带产物 URL，不新增公开字段。

**应触发报表:**
- `取高`
- `经营状况`
- `经营情况`
- `经营健康度`
- `风险识别`
- `低活跃品牌`
- `销售缺口`
- `需要助推`
- 其他明显的新百经营管理表述

**不应触发报表:**
- `取高是什么意思？`
- `风险识别系统有哪些项目经历？`
- 新百经营数据集之外的一般概念问题

**验证命令:**

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
CC=clang CXX=clang++ cargo test -p platform-api external_channel_static_page_artifact --lib
npm run validate:xinbai-report-template
npm run validate:xinbai-report-template -- --artifact-dir /srv/aiv3/shared/objects/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604 --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
```

**完成标准:**
- 当前关注焦点对应模块前置。
- 正常回答不被报表任务截断。
- 报表链接可点击且不重复。
- 导出文件可访问。

### P0-3 已完成本地回归：主站聊天体验

**原因:** DataMax 主站是直接可见入口，之前出现过新对话、自动滚动、进度展示相关回归。

**状态:** 本地 assistant-run 进度摘要测试、本地会话测试和 Web build 已通过。生产浏览器级直接问答/自动滚动/新建对话基线已有 validation 记录；后续修改主站 chat UI 时重跑本节。

**重点文件:**
- `apps/web/app/HomePageClient.js`
- `apps/web/app/components/WorkspaceDirectoryPanel.js`
- `apps/web/app/lib/assistant-run-progress.test.mjs`
- `apps/web/app/lib/local-chat-sessions.test.mjs`

**验证命令:**

```bash
node --test apps/web/app/lib/assistant-run-progress.test.mjs
node --test apps/web/app/lib/local-chat-sessions.test.mjs
npm --prefix apps/web run build
```

**完成标准:**
- 未登录直接问答可用。
- 长回复或流式进度能自动滚动到最新内容。
- 新建对话不导致当前会话丢失。
- 进度/思考展示为安全摘要，不暴露原始 provider payload。

## 4. P1 生产观测与并发

### P1-1 Model Gateway Operator Smoke

**前置:** 需要合法 operator cookie/bearer，或运维侧提供安全脱敏回执。

**验证命令:**

```bash
node scripts/smoke/model-gateway-operator.mjs --base-url https://v3.elepcloud.com
```

**完成标准:**
- 能看到 provider lane、限流、fallback 状态，且不泄密。
- RightCode 主力不可用或耗尽时切 MiniMax fallback 的状态可观测。
- 未授权访问仍返回 HTTP 401。

### P1-2 Queue Stats 与静态页预热观测

**前置:** 需要合法 operator cookie/bearer，或运维侧提供安全脱敏回执。

**验证命令:**

```bash
npm run smoke:static-page-prewarm-observability -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
```

**live 目标:** 观测 queued、running、published、failed、skipped-existing-template、waiting-for-low-load，不输出任务 payload。

**完成标准:**
- 低负载预热可安全观测。
- 模板复用仍是优先路径。
- Cloudflare/Codex fallback 不通时，本地模板复用不受影响。

### P1-3 20 路并发保活

**已验证 live:** 主站 20 路、第三方 20 路、静态页 5 路。

**大版本前重复:**

```bash
npm run smoke:main-chat-20way -- --self-test
npm run smoke:external-channel-20way -- --self-test
npm run smoke:static-page-5way -- --self-test
```

**live 重跑条件:** 只有在修改 runtime、model gateway、worker 或第三方 channel 相关逻辑时重跑。

## 5. P2 企业记忆与文档理解

### P2-1 解析/事实 dry-run 扩样

**目标:** 文档入库后可在空闲时异步深化：实体、别名、岗位、组织、项目、操作步骤、表格事实、合同指标、面积、客流、租售比、有效期和证据来源。

**当前规则:** 只做 dry-run。

**当前状态:** 已完成多类型、小型混合媒体和 doc-heavy 数据集 summary-only 扩样。最新 8 服务器 doc-heavy 样本 `826fb514-2e71-4a7b-9976-43f0199c6d61` limit5 dry-run 通过，派生 facts 137，未知 fact 类型 0，未写入、未入队。

**验证命令:**

```bash
npm run smoke:p2-summary-only-dry-run -- --self-test
npm run smoke:p2-summary-only-dry-run -- --dataset-id <dataset-uuid> --limit 5 --env-file /etc/aiv3/aiv3.env
```

**下一批样本:**
- 客户手册或制度流程类数据集。
- 简历类数据集。
- 经营表格类数据集。
- 混合媒体数据集只做 dry-run。

**完成标准:**
- 已知 fact 类型归入 `report_aggregation`、`retrieval_enhancement` 或 `evidence_index_only`。
- 未知 fact 类型固定进入 `review_required`。
- 未经确认不写真实 facts、fingerprints、snapshots，也不入队。

### P2-2 重复文档与对象治理

**目标:** 降低重复存储，让同一文档可安全归属多个数据集，同时不破坏授权语义。

**当前安全入口:**

```bash
npm run smoke:document-fingerprint-inventory -- --env-file /etc/aiv3/aiv3.env --dataset-limit 20
npm run smoke:document-object-cleanup-plan -- --env-file /etc/aiv3/aiv3.env --dataset-limit 20
npm run smoke:document-object-filesystem-preflight -- --env-file /etc/aiv3/aiv3.env --probe-limit 200
```

**当前状态:** inventory 已输出 `object_locator_reason_counts` 和 `fingerprint_gap_reason_counts` 聚合字段。8 服务器正式脚本 post-sync 已验证，当前缺 fingerprint 原因聚合显示 `local_locator_requires_filesystem_probe=2602`、`remote_locator_requires_fetch=3`；该统计未读取文件系统、未下载远程对象、未输出路径。对象清理计划脚本已在 8 服务器正式 post-sync 通过，只输出聚合影响和 rollback 要求，`object_deletes_enabled=false`；当前 dry-run 显示 `review_object_cleanup_candidate_count=0`、`review_index_mapping_candidate_count=0`。本地对象可达性 preflight 已在 8 服务器正式 post-sync 通过，只对有界样本执行 `stat()`，不读文件内容、不输出路径；200 条样本中 `file_found=57`、`file_missing=143`。

**下一步:**
1. 真实对象清理必须另行人工确认，并先产出 operator-reviewed manifest。
2. 若后续要修复缺 fingerprint 覆盖，先基于 preflight 聚合结果选择恢复对象、调整 root 配置或受控 backfill 范围。

**完成标准:**
- 不删除源对象。
- 不改变数据集权限语义。
- 清理决策可审计、可回滚。

## 6. P3 工程治理

**优先顺序:**
1. 继续拆分 `crates/platform-api/src/lib.rs`，每次只做一个行为保持的小切片。
2. 继续拆分 `apps/web/app/HomePageClient.js`，先有测试再抽 hook/component。
3. smoke 脚本持续标注 self-test、preflight、live 边界。
4. 生成的 integration HTML 如果只是行尾/stat 噪声，不要纳入提交。
5. GitHub Actions 账号额度问题修复后再重跑 DataMax CI。

**源码重构最小回归集:**

```bash
cargo fmt --check
CC=clang CXX=clang++ cargo test -p platform-api gateway_limiter --lib
CC=clang CXX=clang++ cargo test -p platform-api model_gateway_profile --lib
CC=clang CXX=clang++ cargo test -p platform-api gateway_status_exposes_lane_and_provider_counts_without_secrets --lib
npm --prefix apps/web run build
```

## 7. 8 服务器部署流程

只在用户明确要求部署时执行。

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

按改动范围追加对应 worker build/restart。纯文档或纯 smoke 脚本更新不重启服务。

**部署后 smoke 菜单:**

```bash
npm run smoke:external-report-focus -- --self-test
npm run smoke:external-report-export -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:cloudflare-fallback-2way -- --self-test
npm run smoke:production-placeholder-readiness -- --env-file /etc/aiv3/aiv3.env --env-file /etc/aiv3/minimax.env --allow-not-ready --json-stdout
```

## 8. 下一步建议

下一步优先处理 P1-1/P1-2 的 operator 观测缺口；如果拿不到合法 operator 凭证或运维脱敏回执，就继续做 P2 summary-only 扩样，因为这条线安全、可独立推进，也不依赖外部权限。
