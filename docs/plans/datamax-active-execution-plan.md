# DataMax 当前唯一执行计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**目标：** 保持 DataMax 只有一份可执行计划，并优先完成当前不依赖外部凭据、生产风险批准或业务决策的事项。当前优先级先收敛“视频/页面材料提取为 PPT 交付物”的通路，再继续处理后台深化解析、去重、问答质量、报表和第三方对接加固。

**架构：** DataMax 继续作为权限、文档/数据入库、企业记忆、模型路由、媒体/视频提取交付物、静态页/报表产物和第三方契约的系统事实源。本计划优先安排可以本地完成、只读验证或低风险闭环的任务；需要 operator 凭据、业务拍板、外部 provider 额度或生产写入的事项统一挂起。

**技术栈：** Rust workspace（`platform-api`、`storage`、`media-worker`、`assistant-run-worker`、其他 worker、`codex-host-agent`）、PostgreSQL、workflow tasks、Next.js Web、第三方文档生成器、本地/8 服务器 smoke、Markdown validation ledger。

---

## 单计划原则

- `docs/plans/datamax-active-execution-plan.md` 是唯一 active plan。
- 历史计划文件已经汇总到本文件，并归档在仓库外备份区。
- 后续新任务只更新本文件，不再新建日期计划。
- 验证证据继续写入 `docs/validation/**`。
- 运维/业务决策继续写入 `docs/operations/**`。
- 未经明确批准，不改第三方公开 URL、鉴权、必填请求字段、已有响应字段。
- 本计划不触碰 120 服务器。
- 未经 operator 明确批准，不运行真实历史 backfill、不改生产行级 identity 映射、不启用生产低负载模板预热。

## 当前基线

- 历史计划归档提交：`f911862`。
- 远端新增视频/PPT 优先通道提交：`88a03ed`，已整合进本中文版计划。
- 8 服务器已知仓库漂移：未跟踪文件 `mode`，不要动。
- 当前 completion audit：`docs/validation/datamax-main-gap-closure-completion-audit.md`。
- 历史计划归档包：`C:\Users\soulzyn\Desktop\codex-backups\datamax-plan-consolidation-20260607-093057.zip`。
- Task 2 需要按执行时的真实 HEAD 重新记录本地、GitHub、8 服务器和服务状态。

## 外部资源/外部决策挂起项

这些事项不是下面独立执行队列的阻塞项。

| 挂起项 | 挂起原因 | 恢复条件 |
| --- | --- | --- |
| 模型池认证 smoke | 需要合法 operator cookie 或批准的 local-key 登录 | operator 提供凭据后运行 `npm run smoke:model-gateway-operator` |
| `bi_contract_warning` / `bi_rentsales_detail` 生产行级映射 | 需要业务决定：实体/最新快照语义，还是行级明细语义 | 业务确认需要行级明细后，先做 staging-only discriminator 验证 |
| 真实历史 enrichment/backfill | 会写历史记录或入队真实任务 | operator 批准一个极小单文档真实批次，并确认回滚/队列监控 |
| 生产低负载模板预热 | 会产生后台 Image2/静态页任务 | operator 批准短时低负载窗口和事后清理检查 |
| 完整第三方数据库注册/同步公开 API | 这是新的公开 API/鉴权契约 | 产品确认开放，并完成接口评审 |
| Cloudflare Codex 生产兜底依赖 | 依赖外部账号、额度、配置状态 | provider/host readiness 与付费/额度状态确认 |
| 真实登录态视频提取 | 需要 cookie、扫码、私有 host、浏览器播放或录屏 | operator 提供已批准的可访问视频源，或明确批准 reviewed jump-host/provider smoke |

---

## 当前执行顺序

1. 已完成：计划文档整理。
2. 已完成：当前 head 基线回执。
3. 已优先完成：视频/页面材料提取为 PPT 交付物优先通道。
4. 已完成：后台 enrichment / 去重诊断。
5. 已完成：duplicate / canonical read-through smoke。
6. 已完成：被动回答质量离线语料。
7. 下一步：数据源 row identity staging 自测。
8. 静态页/报表回归语料强化。
9. 第三方数据库只读状态强化。
10. operator 观测页小幅打磨。
11. 最终 validation 和可选部署。

每个任务通过测试后独立提交。

---

## Task 1：计划文档整理

**状态：已完成，2026-06-07。**

**文件：**

- 保留：`docs/plans/datamax-active-execution-plan.md`
- 已用 backup-first 方式归档：其他所有 `docs/plans/*.md`
- 已更新：`docs/validation/datamax-main-gap-closure.md`
- 已更新：`docs/validation/datamax-main-gap-closure-completion-audit.md`

**结果：**

- 31 份历史计划文件已用 `Safe-RemoveToBackup.ps1` 归档。
- 归档包：`C:\Users\soulzyn\Desktop\codex-backups\datamax-plan-consolidation-20260607-093057.zip`。
- `docs/plans` 目录只剩本文件。
- 已提交并同步到 GitHub/8 服务器：`f911862`。

**后续规则：**

- 不再新增日期计划。
- 如果需要新增任务，直接加到本文件对应任务区或追加新的 Task。
- 历史 validation 里引用旧计划路径的内容保留为审计上下文，不批量改写。

---

## Task 2：当前 Head 基线回执

**状态：已完成，2026-06-07，提交 `26afa6e`。**

**目标：** 记录当前本地、GitHub、8 服务器和只读运行状态，作为后续任务的起点。

**结果：**

- 本地、GitHub 和 8 服务器已同步到当时基线 head。
- 8 服务器只保留已知未跟踪文件 `mode`，未触碰。
- 只读队列状态和文档检查结果已写入 `docs/validation/datamax-main-gap-closure-completion-audit.md`。
- `npm run check:pure-third-party-guide-html` 已通过。

**文件：**

- 修改：`docs/validation/datamax-main-gap-closure-completion-audit.md`

**Step 1：记录本地和 8 服务器状态**

运行：

```powershell
git status --short --branch
git rev-parse --short HEAD
ssh 8服务器 'cd /srv/aiv3/repo && git status --short --branch && git rev-parse --short HEAD && systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-codex-host-agent.service aiv3-document-enrichment-worker.service aiv3-ingest-worker.service aiv3-retrieval-worker.service aiv3-static-page-worker.service aiv3-media-worker.service aiv3-assistant-run-worker.service'
```

期望：

- 本地和 8 服务器 head 一致。
- 8 服务器只有已知 `?? mode`。
- 核心服务都是 `active`；如 `media-worker` 或 `assistant-run-worker` 未部署，需要记录真实状态，不做话术遮盖。

**Step 2：运行只读队列和文档检查**

运行：

```powershell
ssh 8服务器 'curl -sS --max-time 8 http://127.0.0.1:3000/v1/workflow-tasks/queue-stats >/tmp/datamax-active-plan-qstats.json && python3 -m json.tool /tmp/datamax-active-plan-qstats.json >/dev/null'
npm run check:pure-third-party-guide-html
```

期望：

- queue stats 是合法 JSON。
- 第三方文档生成结果是最新的。

**Step 3：追加回执**

在 `docs/validation/datamax-main-gap-closure-completion-audit.md` 追加短节，记录：

- 本地/GitHub head。
- 8 服务器 head。
- active 服务列表。
- `media-worker` 与 `assistant-run-worker` 状态。
- queue stats 是否可达。
- 已知 `mode` 未触碰。
- 没有记录密钥、token、原始任务 payload、原始客户内容。

**Step 4：提交**

```powershell
git add docs/validation/datamax-main-gap-closure-completion-audit.md
git commit -m "Record DataMax active baseline"
```

---

## Task 3：视频/页面材料提取为 PPT 交付物优先通道

**状态：已优先完成，2026-06-07，提交 `29561a1`。**

**目标：** 先收敛支持范围内的视频/PPT 交付路径：上传的视频文件、直连视频 URL、公开页面中可解析出的直连视频资源，应该产出完整且 redacted 的交付包，包括 transcript/material notes、选中页证据、`video_slides_screenshot_based.pptx`、`video_slides.md`、必要 manifest、可持久追踪的 published-version 元数据、前端可见下载/打开动作，以及 AssistantRun 后续消息告知用户 PPT 包已准备好。

**结果：**

- 本地确定性交付物契约已完成并验证。
- 支持来源边界已固定为上传视频文件、直连视频 URL、公开页面中可解析出的直连视频资源。
- 完整交付包要求已覆盖 PPTX、Markdown、slide notes、subtitle/page map、slide rectangles、extraction/final/published manifests 和 published-version history。
- AssistantRun follow-up、API/UI 下载/打开 surface、durable published-version 元数据、manifest redaction 已纳入验证。
- `docs/validation/video-ppt-deliverable-smoke.md` 和 `docs/validation/datamax-main-gap-closure.md` 已记录测试回执。
- 真实登录态/私有视频 smoke 仍是 operator-only 挂起项，不作为后续开发阻塞。

**边界：**

- 不绕过登录态页面、扫码登录、cookie、私有 host、播放限制、平台反爬。
- 不把浏览器录屏或屏幕抓取当成替代方案。
- 不在公开 manifest、assistant follow-up、validation 文档或下载产物中持久化原始 source URL、本地路径、cookie、provider payload、token-like 字符串。
- 不运行真实 jump-host/provider 视频 smoke，除非 operator 提供批准过的可访问源和明确范围。

**文件：**

- 检查/修改：`crates/media-worker/src/lib.rs`
- 检查/修改：`crates/media-worker/src/main.rs`
- 检查/修改：`crates/platform-api/src/lib.rs`
- 检查/修改：`crates/platform-api/src/react_agent_tools.rs`
- 检查/修改：`crates/storage/src/lib.rs`
- 检查/修改：`crates/domain-model/src/lib.rs`
- 检查/修改：`apps/web/app/HomePageClient.js`
- 检查/修改：`tools/validate-video-deliverables.mjs`
- 检查/修改：`scripts/run-assistant-run-worker-smoke.sh`
- 检查/修改：`scripts/run-jump-host-video-deliverable-smoke.ps1`
- 更新：`docs/validation/video-ppt-deliverable-smoke.md`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：审计当前视频/PPT 通路**

运行：

```powershell
rg -n "video_extraction|VideoExtraction|extract_video_ppt|video_slides|pptx|PublishedVideoPpt|wechat_video|media-worker|assistant_run_model_completion" crates apps scripts docs -g "*.rs" -g "*.js" -g "*.mjs" -g "*.md" -g "*.sh" -g "*.ps1"
```

期望：定位 workflow 注册、media-worker 任务步骤、交付物写入、公开 validator、durable published-version 表、AssistantRun follow-up、API 下载/打开 surface、Web UI 状态。

**Step 2：补齐本地确定性交付物契约**

确认 controlled sample 和代码路径覆盖：

- 支持 source kind：上传视频文件、直连视频 URL、公开页面解析到直连视频资源。
- 登录态来源进入 handoff/unsupported 状态，不索要 cookie、扫码或录屏。
- 最终包包含 `video_slides_screenshot_based.pptx`、`video_slides.md`、`slide_notes.md`、`subtitle_page_map.json`、`slide_rectangles_manifest.json`、`extraction_artifacts_manifest.json`、`final_deliverables_manifest.json`、`published_deliverable_manifest.json`、`published_version_history.json`。
- PPTX 是真实 OOXML ZIP，包含 `[Content_Types].xml`、`ppt/presentation.xml` 和 slide entries。
- 选中页 duplicate/crop 状态可见；需要人工复核时有 review-required 标记。
- manifest 和 assistant 可见 payload 已 redacted。
- API/UI 暴露稳定 summary page 与 PPTX/Markdown 下载动作，不泄露私有文件路径。

**Step 3：运行本地目标测试**

运行：

```powershell
npm run test:video-deliverables
cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete
cargo test -p media-worker slide_rectangle --lib
cargo test -p media-worker durable_published_version --lib
cargo test -p media-worker model_completion --lib
cargo test -p storage auth_migrations_are_registered_in_order --lib
cargo test -p domain-model workflow_kind_roundtrips_video_extraction
cargo test -p platform-api video_extraction --lib
cargo test -p platform-api video_ppt --lib
bash scripts/run-assistant-run-worker-smoke.sh
```

期望：确定性 contract 测试通过，不需要真实 provider 凭据、不使用原始客户媒体、不写生产。DB-backed assistant-run-worker smoke 如果没有明确 disposable test database，记录 skip。

**Step 4：运行 jump-host 脚本自测**

运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-jump-host-video-deliverable-smoke.ps1 -SelfTest
```

期望：脚本自检可运行。若本机缺 PowerShell/jump-host 访问，记录 skip 原因；不要替换为真实登录态视频。

**Step 5：更新验证记录**

更新 `docs/validation/video-ppt-deliverable-smoke.md` 和 `docs/validation/datamax-main-gap-closure.md`，记录：

- 支持来源边界。
- 已完成交付包形状。
- 已运行测试，以及 jump-host self-test 是运行还是跳过。
- 如果检查 8 服务器，记录 `aiv3-media-worker.service` 与 `aiv3-assistant-run-worker.service` 当前状态。
- 剩余 operator-only real-video smoke 决策。

**Step 6：提交**

```powershell
git add crates apps tools scripts docs/validation
git commit -m "Prioritize video PPT deliverable completion"
```

---

## Task 4：后台 Enrichment 与去重诊断

**状态：已完成，2026-06-07。**

**目标：** operator 能判断文档是否 canonical、duplicate、已解析、已索引、已 enrichment、等待中或被阻塞，同时不暴露原始文档内容。

**结果：**

- 外部 source summary 增加 bounded `document_diagnostics` / `documentDiagnostics`，最多返回最近 8 个源文档的安全状态摘要。
- 诊断摘要只包含 document id、external id、dataset ids、canonical id、dedup 状态、parse/index/enrichment 状态、enrichment status counts、最新 workflow task 状态字段和 redacted waiting/blocked/failure reason。
- 观测页 operations summary 增加文档诊断 helper 和紧凑 UI，最多显示 4 个文档诊断。
- helper 对 reason 文本执行 URL、路径、Bearer、`v3in_` token、cookie/password/API key 类字符串 redaction。
- 不返回或展示原始标题、正文、chunk、object path、source URL、provider payload、cookie、token、数据库连接串或 secret env。
- 验证回执已写入 `docs/validation/datamax-main-gap-closure.md`。

**文件：**

- 检查/修改：`crates/platform-api/src/lib.rs`
- 检查/修改：`crates/storage/src/lib.rs`
- 检查/修改：`apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- 检查/修改：`apps/web/app/lib/external-integrations.js`
- 测试：`apps/web/app/lib/external-integrations.test.mjs`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：定位当前诊断能力**

运行：

```powershell
rg -n "dedup_state|canonical_document_id|document_enrichment|enrichment_runs|fingerprint|parse_status|index_status" crates apps docs -g "*.rs" -g "*.js" -g "*.mjs" -g "*.md"
```

期望：找到当前 storage 字段、enrichment run helper 和观测页 helper。

**Step 2：补齐 redacted summary helper**

如果当前 summary 不够完整，新增或扩展一个只返回以下字段的 helper：

- document id / external id / dataset ids。
- canonical id。
- duplicate 状态。
- parse/index/enrichment 状态。
- 最新 task id、task kind、task status、updated_at。
- failure reason 的 redacted 短摘要。

不返回完整 chunk、原始文档文本、COS/object path、provider payload。

**Step 3：接到观测页**

观测页需要能显示：

- canonical / duplicate badge。
- enrichment 状态。
- 最新 task 状态。
- 可区分“已停用/重复归档”和“真正卡住”。

**Step 4：测试**

运行：

```powershell
cargo test -p storage enrichment --lib
cargo test -p platform-api external_document --lib
npm test -- apps/web/app/lib/external-integrations.test.mjs
```

如某个 test target 不存在，记录实际可运行 target，并补最小单测。

**Step 5：记录验证**

更新 `docs/validation/datamax-main-gap-closure.md`，说明 operator 现在如何看 duplicate/canonical/enrichment 状态。

**Step 6：提交**

```powershell
git add crates apps docs/validation
git commit -m "Expose document enrichment dedup diagnostics"
```

---

## Task 5：Duplicate 与 Canonical Read-Through Smoke

**状态：已完成，2026-06-07。**

**目标：** 确认 8 服务器本地重复文档不会破坏第三方 dataset/file 授权，也不会导致“数据集下有文件但问不到”的问题。

**结果：**

- 新增 `scripts/smoke/document-dedup-readthrough.mjs --self-test`，确定性覆盖 duplicate document ref、duplicate dataset ref、多 `dataset_external_ids`、document + dataset 混合授权去重、同 `conversation_external_id` 范围继承和不同会话隔离。
- 本地 canonical read-through Rust 覆盖已补跑，确认 duplicate document 能读取 canonical chunks、retrieval evidence 和 facts。
- 计划中的 `third_party_authorization` / `document_dedup` 过滤词当前没有命中测试；已改用实际测试名补证。
- 8 服务器只读聚合显示 `hy-sql-traffic-area` 当前 `documents=2407`、`datasets=4`、`duplicates=0`；全库聚合显示 `documents=2624`、`duplicates=0`、`canonical=22`、`unknown=2602`。
- 当前 8 服务器没有 duplicate/canonical 现场样本可解释“已停用/问不到”；后续若出现 duplicate rows，可用本 smoke 和 read-through 聚合重新验证。
- 验证回执已写入 `docs/validation/datamax-main-gap-closure.md`。

**文件：**

- 新建/修改：`scripts/smoke/document-dedup-readthrough.mjs`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：梳理授权读取路径**

运行：

```powershell
rg -n "available_document_external_ids|dataset_external_ids|conversation_external_id|canonical_document_id|document_scope|authorized" crates scripts docs -g "*.rs" -g "*.mjs" -g "*.md"
```

期望：确认 dataset-level 授权、document-level 授权、conversation 持续授权和 canonical read-through 使用同一查询语义。

**Step 2：写 smoke**

smoke 场景：

- 同一个 external document 上传两次，第二次成为 duplicate。
- dataset 授权传入 canonical 或 duplicate 任一侧，都能读到 canonical chunks。
- `dataset_external_ids` 可包含多个分组。
- 文档授权和分组授权可同时传；如果文档已在分组内，去重后不重复供料。
- 后续同 `conversation_external_id` 不重复传授权，仍继承会话可见范围。

**Step 3：本地测试**

运行：

```powershell
node scripts/smoke/document-dedup-readthrough.mjs --self-test
cargo test -p platform-api third_party_authorization --lib
cargo test -p storage document_dedup --lib
```

**Step 4：8 服务器只读验证**

只读查询指定已知第三方 dataset/document，确认：

- dataset 下文件数量符合预期。
- duplicate/canonical 关系能解释“已停用”。
- 不记录原始客户文本和完整 payload。

**Step 5：提交**

```powershell
git add scripts/smoke docs/validation crates
git commit -m "Add canonical document readthrough smoke"
```

---

## Task 6：被动回答质量离线语料

**状态：已完成，2026-06-07。**

**目标：** 在不恢复 live hard gate、不拦截正常客户回答的前提下，找出低质量回答和漏触发动作，并形成可回放语料。

**结果：**

- 新增 `scripts/smoke/answer-quality-offline-corpus.mjs --self-test`，只运行本地确定性 fixture，不调用 DataMax、不入队 Codex、不影响客户回答。
- 离线语料覆盖 10 类已知问题标签：邓工、一字 PDF、doc 问邓工、简历公司统计、多维简历排序、14 份简历项目经历、考勤缺勤/工时/日期格式、养老护理流程、新百取高/风险/销售缺口、临时简历附件范围。
- 判定 label 覆盖 7 类：资料充足但回答不足、检索供料不相关、应生成报表但缺 artifact、报表链接缺失、报表链接重复、临时附件未进范围、第三方远程 fallback。
- self-test 生成 redacted report，只记录 case id、覆盖标签、label/count、runtime/redaction flags，不记录原始问题、答案、证据、客户 payload、凭据或 provider payload。
- 8 服务器只读门禁检查确认 hard gate 和 `answer_quality_autofix` 专用开关未设置，`answer_quality_autofix` 不在 task/capability allowlist 中；当前不会创建 live Codex task。
- `node --check scripts/smoke/answer-quality-offline-corpus.mjs`、`node scripts/smoke/answer-quality-offline-corpus.mjs --self-test --pretty` 和 `CC=clang CXX=clang++ cargo test -p platform-api answer_quality --lib` 已通过。

**文件：**

- 检查/修改：`crates/platform-api/src/lib.rs`
- 检查/修改：`crates/assistant-run-worker/src/lib.rs`
- 新建/修改：`scripts/smoke/answer-quality-offline-corpus.mjs`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：确认生产硬门禁关闭**

运行：

```powershell
rg -n "QUALITY_GATE|quality_gate|ANSWER_QUALITY|React|react_recovery|insufficient" crates apps docs scripts
```

期望：线上默认不挡正常回答；如果有质量判断，只能是观测/离线回放或明确启用的实验开关。

**Step 2：新增离线判定器**

判定器只输出 label，不修改代码、不入队 Codex、不影响客户响应：

- 资料不足类话术。
- 检索供料明显不相关。
- 应触发报表但没触发。
- 报表链接缺失或重复。
- 临时附件未纳入问答范围。
- 第三方远程接口不可用 fallback。

**Step 3：加入已知客户问题 fixture**

至少覆盖：

- “邓工是谁”。
- 一字 PDF。
- doc 问“邓工是谁”。
- 简历公司名统计。
- 多维简历排序出表。
- 简历项目经历跨 14 份材料。
- 考勤缺勤/工时长短/日期格式。
- 养老护理翻身、发药核对、交接班。
- 新百取高、风险、经营状况、销售缺口/助推。
- 临时上传简历加入问答范围。

**Step 4：测试**

运行：

```powershell
node scripts/smoke/answer-quality-offline-corpus.mjs --self-test
cargo test -p platform-api answer_quality --lib
```

**Step 5：记录验证**

更新 `docs/validation/datamax-main-gap-closure.md`：

- 离线语料覆盖了哪些问题。
- 生产质量门禁仍未默认生效。
- 后续如要自动优化，必须另走受控计划和审核。

**Step 6：提交**

```powershell
git add crates scripts/smoke docs/validation
git commit -m "Add passive answer quality corpus"
```

---

## Task 7：数据源 Row Identity Staging 自测

**目标：** 对数据库接入、数据源 staging、row identity、`staging_plan` 做自测，不直接改生产 row 语义。

**文件：**

- 修改：`scripts/run-data-ingestion-staging-live-smoke.sh`
- 检查/修改：`crates/platform-api/src/lib.rs`
- 检查/修改：`crates/storage/src/lib.rs`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：定位当前 staging plan 路径**

运行：

```powershell
rg -n "staging_plan|data_ingestion_analysis|row_identity|database_sync|data_source" crates scripts docs -g "*.rs" -g "*.sh" -g "*.md"
```

**Step 2：补自测用例**

至少覆盖：

- 没有 row identity 时给出安全摘要和 staging plan。
- 候选字段变化时不自动写 production。
- 只在 staging/analysis 层输出建议。
- 不泄露原始连接串、密码、token。

**Step 3：测试**

运行：

```powershell
bash scripts/run-data-ingestion-staging-live-smoke.sh --self-test
cargo test -p platform-api data_ingestion --lib
cargo test -p storage data_source --lib
```

**Step 4：记录验证**

更新 `docs/validation/datamax-main-gap-closure.md`，说明 row identity 仍处于 staging self-test，不自动改生产。

**Step 5：提交**

```powershell
git add scripts crates docs/validation
git commit -m "Harden data ingestion staging self test"
```

---

## Task 8：静态页与报表回归语料强化

**目标：** 确认新世界/新百经营月报模板是默认主模板，命中模板时可以基于已有模板变更，报表链接只出现一次，导出文件可访问，且普通问答不被报表动作截断。

**文件：**

- 修改：`scripts/smoke/external-report-export.mjs`
- 修改：`scripts/smoke/external-report-focus.mjs`
- 检查/修改：`crates/platform-api/src/lib.rs`
- 检查/修改：`crates/codex-host-agent/src/lib.rs`
- 检查/修改：`apps/web/app/HomePageClient.js`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：确认触发词范围仅限报表上下文**

应触发：

- 取高。
- 经营状况。
- 经营健康度。
- 风险识别。
- 销售缺口。
- 哪些门店需要助推。
- 统计经营报表。

不应误触发：

- “取高是什么意思？”
- “风险识别系统有哪些项目经历？”
- 与新百经营数据无关的普通知识问答。

**Step 2：确认产物契约**

产物必须包含：

- 一个主要报表链接。
- 报表名称：`新世界百货经营管理月报表`。
- `table-data.csv`。
- `report.ppt`。
- `report.md`。
- URL 保留正确 focus。
- 命中模板时使用主模板，不选旧 fallback/prewarm/smoke 模板。

**Step 3：运行本地与公网检查**

运行：

```powershell
node scripts/smoke/external-report-export.mjs --self-test
node scripts/smoke/external-report-focus.mjs --self-test
cargo test -p platform-api report_trigger --lib
npm test -- apps/web/app/lib/external-integrations.test.mjs
```

如果需要公网 URL 只读检查，使用已发布报表 URL 验证可访问，不改第三方接口。

**Step 4：更新验证**

记录：

- 命中模板后的回答仍能正常继续。
- 报表地址只出现一次。
- 三个导出文件可访问。
- 触发/不触发案例。

**Step 5：提交**

```powershell
git add scripts crates apps docs/validation
git commit -m "Harden report trigger regression corpus"
```

---

## Task 9：第三方数据库只读状态强化

**目标：** 让第三方“已挂接数据库/数据集”状态清楚可见，并减少误判为“接口不可用”或“没收到数据”。

**文件：**

- 检查/修改：`crates/platform-api/src/lib.rs`
- 检查/修改：`crates/storage/src/lib.rs`
- 检查/修改：`apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：定位第三方数据库状态**

运行：

```powershell
rg -n "external database|third_party_database|database_only|data_source|dataset_external_id|source_id" crates apps docs -g "*.rs" -g "*.js" -g "*.md"
```

**Step 2：补只读状态摘要**

摘要字段：

- source/system user。
- tenant/bot。
- dataset_external_ids。
- database/data-source 是否存在。
- 最近 sync/analysis 状态。
- 是否只读挂接。
- 最近错误 redacted summary。

不新增或改变第三方公开字段，除非用户另行批准。

**Step 3：观测页展示**

观测页显示：

- 已挂接。
- 未同步。
- 分析中。
- 只读可用。
- 需要 operator 处理。

**Step 4：测试**

运行：

```powershell
cargo test -p platform-api third_party_database --lib
npm test -- apps/web/app/lib/external-integrations.test.mjs
```

**Step 5：提交**

```powershell
git add crates apps docs/validation
git commit -m "Expose third party database read only status"
```

---

## Task 10：Operator 观测页小幅打磨

**目标：** 在不大改 UI 的前提下，让 operator 更容易看任务状态、第三方会话、文档入库、报表产物和异常原因。

**文件：**

- 修改：`apps/web/app/external-integrations/ExternalIntegrationsPageClient.js`
- 修改：`apps/web/app/lib/external-integrations.js`
- 测试：`apps/web/app/lib/external-integrations.test.mjs`
- 更新：`docs/validation/datamax-main-gap-closure.md`

**Step 1：确认现有页面能力**

运行：

```powershell
rg -n "selected|lazy|workflow|queue|artifact|conversation|external" apps/web/app/external-integrations apps/web/app/lib -g "*.js" -g "*.mjs"
```

**Step 2：小幅补齐**

优先做：

- 只看选中项。
- 懒加载任务明细。
- 第三方对话测试保护。
- 任务失败 redacted reason。
- 报表产物链接和导出文件可见。

不做大规模页面重构。

**Step 3：测试**

运行：

```powershell
npm test -- apps/web/app/lib/external-integrations.test.mjs
npm run lint -- --file apps/web/app/external-integrations/ExternalIntegrationsPageClient.js
```

如果 lint 命令格式与项目不匹配，运行项目现有等价命令并记录。

**Step 4：提交**

```powershell
git add apps/web/app docs/validation
git commit -m "Polish external integration observability"
```

---

## Task 11：最终验证与可选部署

**目标：** 汇总以上独立任务结果。只有用户明确要求部署时才部署；否则只提交、推 GitHub、做 8 服务器只读检查。

**文件：**

- 修改：`docs/validation/datamax-main-gap-closure.md`
- 修改：`docs/validation/datamax-main-gap-closure-completion-audit.md`

**Step 1：本地总检查**

运行：

```powershell
git status --short --branch
git diff --check
npm run check:pure-third-party-guide-html
```

按实际改动补充对应 targeted tests。

**Step 2：8 服务器只读 smoke**

运行：

```powershell
ssh 8服务器 'cd /srv/aiv3/repo && git status --short --branch && git rev-parse --short HEAD'
ssh 8服务器 'curl -sS --max-time 8 http://127.0.0.1:3000/v1/workflow-tasks/queue-stats >/tmp/datamax-active-plan-final-qstats.json'
```

**Step 3：如用户明确要求部署**

运行：

```powershell
ssh 8服务器 'cd /srv/aiv3/repo && git pull --ff-only'
ssh 8服务器 'cd /srv/aiv3/repo && cargo build -p platform-api --release'
ssh 8服务器 'sudo systemctl restart aiv3-platform-api.service'
ssh 8服务器 'systemctl is-active aiv3-platform-api.service'
```

如果没有代码改动，不重启服务。

**Step 4：最终记录**

记录：

- 已完成的独立任务。
- 已运行测试与 smoke。
- 外部资源/外部决策仍挂起的事项。
- 未改第三方公开 URL、鉴权、必填请求字段、已有响应字段。
- 未触碰 120 服务器。

**Step 5：提交**

```powershell
git add docs/validation
git commit -m "Record DataMax active plan validation"
```

---

## 当前完成定义

本计划达到当前完成态时，需要满足：

- `docs/plans` 只包含 `datamax-active-execution-plan.md`。
- 上述可独立处理的任务已完成，或明确标注为当前不需要。
- 变更涉及的本地测试通过。
- 8 服务器只读检查通过。
- 只有用户明确要求时才部署。
- 外部资源/外部决策项继续有清晰恢复条件。
- 不记录原始凭据、原始客户行、完整文档、provider payload、object path、token。
