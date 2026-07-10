> **ARCHIVED 2026-07-10:** R4 交接与 PostgreSQL 18.4 升级完成凭据。本文不再是活动计划；唯一活动入口是 `docs/plans/datamax-active-execution-plan.md`。

# DataMax V3 盘点后收版、暗发布与资产链路 Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Do not skip task gates or combine live-write tasks without explicit user approval.

**Goal:** 在保护当前大体量 dirty worktree 的前提下，先形成可审计、可回滚、默认关闭新写能力的 DataMax V3 发布候选，再通过测试租户逐步打通主站资产导入、异步解析、统一检索、第三方私有入口和 Codex 客户端联合验收。

**Architecture:** V3 维持“PostgreSQL 主状态 + 大型模块化 API + 独立 worker 进程”的现实部署形态，继续承载企业数据、租户权限、数据集、资产库、任务卡、产物和审计。当前本地能力先以 feature-off 方式暗发布；主站、parser、retrieval 和第三方入口分别使用独立门禁、独立验证和独立回退点。资产画像使用独立 evidence 存储，不伪装成 document chunk；首版解析复用现有 `ingest-worker` 的任务机制，不新增 parser 二进制或新基础设施。

**Tech Stack:** Rust workspace、Axum、Next.js 16、Node test/smoke scripts、PostgreSQL、现有 Redis/NATS 唤醒链路、systemd workers、文档/数据库/多模态资产解析、静态页与企业产物、可选 Cloudflare/Codex 执行链路。

**Plan Date:** 2026-07-10

**Revision:** R4，PostgreSQL 18.4 停机冷迁移已完成并记录验证、备份与回滚凭据。

---

## 1. 文档权威与执行契约

### 1.1 唯一入口

1. 本文是 DataMax V3 后续开发的唯一活动计划。
2. `docs/plans/datamax-active-execution-plan.md` 保持冻结，只作历史执行账本，不再追加流水记录。
3. `docs/validation/datamax-main-gap-closure.md` 是本计划所有本地验证、提交、部署和 live 回执的唯一记录入口。
4. `docs/archive/plans/2026-06-17-multimodal-asset-library-fashion-gallery.md`、`docs/archive/plans/2026-06-17-fashion-postchain-absorption-plan.md` 只作设计参考，不能覆盖本文任务顺序和门禁。
5. `docs/plans/2026-07-07-server10-aiv3-deployment-plan.md` 不属于本 release train；不得把 10 服务器或 120 服务器混入本计划。

### 1.2 授权边界

| 动作 | 本计划是否自动授权 | 说明 |
| --- | --- | --- |
| 读取、归类、格式检查、本地无副作用测试 | 是 | 用户要求执行对应 Task 后可进行 |
| 修改本地代码和本文列明的文档 | 是 | 仅限当前 Task 的明确文件和行为 |
| 创建本地测试数据库 schema、写入本地 fixture | 条件授权 | 必须使用明确的本地测试库，不得连接生产库 |
| Commit | 否 | 必须先报告候选差异和验证结果，获得用户明确确认 |
| Push GitHub | 否 | 必须有明确 push 指令 |
| 部署 8 服务器、修改 `/etc/aiv3/*.env`、重启服务 | 否 | 必须有明确部署指令 |
| 真实资产导入、parser live、retrieval live、第三方 pilot | 否 | 必须有测试身份、受控数据集和 `approval_id` |
| backfill、对象清理、source sync、生产数据同步 | 否 | 必须单独审批，不得由本计划隐含授权 |

### 1.3 执行报告状态

每个 Task 完成后必须使用以下状态之一：

- `PASS`：代码、验证、记录和停止条件全部满足。
- `PENDING`：当前阶段可安全暂停，不是失败；必须列出未完成条件。
- `AUTH_REQUIRED`：需要 commit、push、部署、凭证或 live-write 授权。
- `FAIL`：出现实际门禁失败、未知差异、契约漂移、权限泄漏或不可归因错误。

每次报告至少包含：

```text
Task:
Status:
HEAD / worktree:
Changed files:
Validation:
Known failures:
Writes performed:
Commit / push / deploy:
Next gate:
```

### 1.4 硬停止条件

出现以下任一情况立即停止当前 Task，不得顺手扩修：

1. 发现无法归属的用户改动、意外大块删除或工作区内容被外部进程改变。
2. 第三方公开 URL、鉴权、必填字段或既有响应字段发生变化。
3. 新增行出现 token、cookie、数据库 URL、原始 provider payload、客户原文、对象路径或本机绝对路径。
4. migration 包含删除、重命名、不可逆类型变更或生产 backfill。
5. 普通问答、流式、静态页或既有第三方能力被新资产逻辑误触发或阻断。
6. 8 服务器不是预期 HEAD、远端工作区不干净、服务配置与上次启动不一致。
7. live smoke 缺少受控身份、数据集、资产库、连接 allowlist 或明确 `approval_id`。

---

## 2. 当前可信基线

### 2.1 仓库与线上

截至 2026-07-10 的交接基线：

| 项目 | 状态 |
| --- | --- |
| 本地仓库 | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3` |
| 分支 | `main` |
| 本地 HEAD | `e71e1ef75d9ee54745c96b8864595a193521a583` |
| `origin/main` | 与本地 HEAD 一致 |
| 8 服务器 | `8.155.8.7`，仓库 `/srv/aiv3/repo` |
| 交接时线上 HEAD | 与 `origin/main` 一致，工作区干净 |
| 交接时核心服务 | `aiv3-platform-api`、`aiv3-web`、`aiv3-assistant-run-worker`、`aiv3-chat-session-worker`、`aiv3-static-page-worker` 为 `active` |
| 交接时健康状态 | `http://127.0.0.1:3000/healthz` 为 `ok`，`readyz` 为 `ready` |

这些线上状态是交接快照，不是永久事实。执行任何服务器 Task 前必须重新读取远端 HEAD、工作区、服务和 health/ready。

### 2.2 本地工作区快照

当前已核对：

- 28 个 tracked 文件存在差异，约 `8416` 行新增、`1018` 行删除。
- 另有 45 个未跟踪文件；`git status --short` 因目录聚合显示 41 个条目。
- 仓库共有 36 个 Rust package、404 个 Rust 源文件，约 32.6 万行 Rust；`platform-api` 约 23.3 万行，占 Rust 代码约 72%。
- 当前工作树有 140 个 API route，较 HEAD 新增 2 个本地资产导入 route；Web `app` 目录约 5 万行，Node scripts 约 3.25 万行。
- 主要维护热点是 `crates/platform-api/src/lib.rs`、`apps/web/app/HomePageClient.js`、`apps/web/app/globals.css` 和超大 smoke 脚本；本 release train 不顺手拆分这些热点。
- 核心范围包括资产库、主站 UI、资产画像供料、第三方图片结构化识别、报表规划和验证文档。
- `external_aigolf_skill_support.rs`、`decks/`、10 服务器计划不属于核心 RC，默认排除。
- `docs/plans/datamax-active-execution-plan.md` 的冻结标记和 `docs/validation/datamax-main-gap-closure.md` 的既有记录必须保留。

### 2.3 已知验证

本轮盘点后已通过：

- `cargo test -q -p platform-api asset_import --lib`：56/56。
- `cargo test -q -p platform-api fashion_postchain_adapter --lib`：8/8。
- `cargo check --workspace`。
- 在 `apps/web` 正确工作目录执行全部 38 个 Web test 文件：当前 400/400。
- `npm --prefix apps/web run build`；Next.js production build 通过。
- `node --check scripts/smoke/fashion-design-asset-import.mjs`。
- `npm run test:third-party-public-contract-guard`：1/1。
- `npm run smoke:fashion-design-asset-import -- --self-test --pretty`。
- `npm run smoke:fashion-design-asset-import -- --preflight --pretty`。
- `git diff --check`。

已关闭的已知门禁：

- `cargo fmt --check` 已通过；原 `asset_import_support.rs` 四处测试断言排版已由 rustfmt 收口。

当前非阻断工程告警：

- 从仓库根目录直接运行全部 Web test 会因一个测试依赖 `process.cwd()` 得到 398/399；从 `apps/web` 运行则 399/399。必须用 package script 固化正确 cwd。
- Web test 会出现 `MODULE_TYPELESS_PACKAGE_JSON` 告警；本 release train 先记录，不为消警贸然改变整个 Web package 的模块语义。
- Next build 提示 `middleware` 文件约定已弃用；不影响当前 build，迁移到 `proxy` 放入独立 P5 工程治理任务。

尚未形成验收证据：

- 全量 `cargo test --workspace`；当前仅执行 workspace check 和受影响模块测试。
- 当前大工作区的线上业务回归。
- 真实 V3 session 资产导入。
- parser、asset evidence、第三方 asset-imports live。
- 本地 RC 对 8 服务器的暗发布和业务回归。

### 2.4 已在线能力边界

| 能力 | 当前可信口径 |
| --- | --- |
| 主站问答 | 支持普通问答、数据集选择、流式过程和后续动作入口 |
| 数据集与权限 | 文档、数据库和会话按 tenant、用户、数据集和第三方通道隔离；主站选择用于聚焦，不把全局可见权限误变成硬限制 |
| 文档知识库 | 支持上传、ZIP 展开、解析、OCR/VLM 兜底、索引、状态查询和重解析供料 |
| 数据库接入 | 支持数据源登记、staging analysis、目标数据集、同步状态和报表问答；生产同步仍需确认 |
| 第三方集成 | 支持通道鉴权、用户/会话、文档/数据集范围、requested skills、输出格式、流式事件、异步状态和静态页链接 |
| 报表与静态页 | 支持模板、数据绑定、Image2 视觉链路、HTML、发布链接、既有产物继续修改和 fallback |
| 产物任务卡 | 主站已有任务卡、状态、文件、打开和继续编辑基础链路 |
| Worker 与观测 | model gateway、assistant/chat/static-page 等 worker、队列状态、provider fallback、401 和脱敏聚合观测路径已存在 |

### 2.5 本地已具备但尚未上线

1. 企业资产库挂接多个数据集，并按当前用户可见范围汇总。
2. 通用 `asset_items`、`dataset_asset_memberships`、`asset_profiles` 和本地新增 `asset_parse_runs` 底座。
3. 图片、PPT、视频和服装设计图画像摘要，以及 `FashionDesignImageProfileV1` 归一化。
4. 设计图 URL、文件、ZIP、batch/package 导入入口和主站图库 UI。
5. 图库任务卡 stable id、状态、详情、字段账本和不制造无关产物的 dry-run。
6. 扩大候选文档范围、资产画像供料、供料压缩和不足时继续检索的本地改动。
7. 第三方图片结构化识别的独立 runtime/provider/prompt/reply 和换图幂等逻辑。
8. 静态页与 report planner 的资产画像和解析状态上下文。
9. 高尔夫 skill 和演示 deck 也在同一 worktree，但不属于本 release train 核心 RC。

### 2.6 尚未闭环

1. 生产 parser worker 完成真实 OCR/VLM 画像并持久化 accepted profile。
2. asset profile 物化为独立 retrieval evidence，并经 membership 守卫进入统一检索。
3. 主站真实任务卡稳定经历 pending/running/partial/completed/failed/retrying。
4. 第三方私有 `asset-imports` endpoint、私有文档和单 connection pilot。
5. 8 服务器测试 tenant 的真实图片/ZIP 端到端验证。
6. V3 与 Codex 企业客户端的 config、执行、上传、挂接、发布和继续编辑联合 smoke。
7. authenticated operator live 和受控并发 live。
8. CI 尚未执行 `cargo check --workspace`、完整 Web unit tests、新资产 smoke 和第三方公开契约 guard。
9. migration runner 没有 ledger/checksum，启动时会顺序重放全部 migration；当前 release train 只做 append-only 修复，不在同一批次改造 runner。
10. OpenAPI、docs-site、mcp-gateway、analytics、report compiler/runtime/publisher 与 Redis/Qdrant/MinIO/DataFusion 等仍有接口或脚手架成分，不能作为已闭环生产能力排期。

### 2.7 原 P0–P5 工作归并

| 原优先级 | 归并结果 | 本计划任务 |
| --- | --- | --- |
| P0 当前工作区收版和 8 服务器回归 | 先做 feature-off RC、固化 CI，再暗发布关闭 P0 | Task 1–7 |
| P1 authenticated operator、20 路问答和重任务 live | 功能链路稳定后在受控窗口执行 | Task 12 |
| P2 异步深解析 | 资产 parser 纳入当前 release train | Task 9 |
| P2 fingerprint、重复文档和对象治理 | 继续只读，真实 backfill/cleanup 延后并单独审批 | 不在本 release train |
| P3 报表焦点、客户失败样例、主站任务卡 | 既有能力随暗发布回归；资产任务卡在测试 tenant pilot | Task 7–8 |
| P3 模板复用和低负载预热 | 默认关闭，仅在独立 reviewed 窗口执行 | 不在本 release train |
| P4 数据接入生产同步 | 继续停在 staging/确认边界 | 不在本 release train |
| P4 企业资产库和图库 | 主站、parser、evidence、第三方依次 canary | Task 8–11 |
| P4 Codex 客户端企业配置器 | 在资产链路稳定后做联合验收 | Task 13 |
| P5 工程治理与拆分 | Task 7 关闭 P0 前暂停；之后只做行为保持切片 | 后续独立安排 |

---

## 3. 目标架构与数据流

```mermaid
flowchart LR
    UI["主站图库 UI"] --> CAP["authenticated capability / feature gate"]
    CAP --> API["platform-api asset import"]
    API --> PG["PostgreSQL asset items / memberships / parse runs"]
    API --> TASK["workflow task: parse_asset_profile"]
    TASK --> INGEST["existing ingest-worker"]
    INGEST --> PROVIDER["OCR / VLM adapter"]
    PROVIDER --> PROFILE["accepted asset profile"]
    PROFILE --> EVIDENCE["asset_retrieval_evidences"]
    EVIDENCE --> RETRIEVAL["retrieval-worker union search"]
    RETRIEVAL --> ASSISTANT["assistant / report planner safe supply"]
    THIRD["private third-party connection"] --> EXTGATE["connection allowlist"]
    EXTGATE --> API
```

核心边界：

1. `platform-api` 负责身份、tenant、dataset ownership、asset library scope、幂等导入和任务创建。
2. `ingest-worker` 首版新增 `parse_asset_profile` 任务分支，复用现有 worker、队列和重试机制，不在 API 请求内同步调用 provider。
3. parser 只持久化安全画像、状态和受控错误码，不保存 raw provider payload。
4. asset evidence 使用独立表和 source kind，不伪造成 `documents` 或 `document_chunks`。
5. retrieval 在查询前再次验证 tenant、dataset membership 和资产库可见范围。
6. 主站和第三方分别有独立开关；一个开关不得隐式打开另一个入口。

---

## 4. 架构决策记录

### ADR-01: 采用暗发布 release train

**Status:** Accepted

**Context:** 当前本地改动规模大，若继续完成 parser、retrieval 和第三方入口后再首次部署，会扩大回归面并延迟真实反馈。

**Decision:** 当前工作区先形成 feature-off RC。Task 7 完成首次暗发布并关闭 P0；Task 8–11 分别形成独立 canary 和回退点。

**Consequences:**

- Positive：能先验证已有主站、问答、静态页和第三方公开契约没有回归。
- Negative：会增加部署次数和 validation 记录量。
- Alternative rejected：一次性完成全部 P4 再部署，风险和定位成本过高。

### ADR-02: Migration 从 `0017` 起保持 append-only

**Status:** Accepted

**Context:** `0015_asset_libraries.sql` 已在当前基线提交，仓库已有 `0016_v3_client_artifacts.sql`；当前 migration runner 没有 ledger/checksum，而是启动时顺序重放 `MIGRATIONS`。

**Decision:** 把本地新增的 `asset_parse_runs` SQL 从 `0015` 移到 `0017_asset_parse_runs.sql`；后续 asset evidence 使用 `0018`。已提交 migration 不再追加新对象。

**Consequences:**

- Positive：版本归因、审计和回滚说明清晰。
- Negative：需要调整现有 unit tests 和 migration 注册列表。
- Alternative rejected：继续修改 `0015` 虽可能因 `if not exists` 生效，但无法准确证明哪个发布引入了表。

### ADR-03: 当前 RC 的新行为全部 default-off

**Status:** Accepted

**Context:** 当前导入路由直接挂载，只检查登录态、可见数据集和 owner 权限；同时本地还改变了 asset profile 模型供料、静态页/报表上下文和第三方图片结构化识别。只关闭导入入口仍不足以证明暗发布不改变线上行为。

**Decision:** 新增 `MAIN_SITE_ASSET_IMPORT_ENABLED`、`ASSET_PROFILE_SUPPLY_ENABLED` 和 `EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED`，全部默认 `false`。主站写入口另加 `MAIN_SITE_ASSET_IMPORT_TENANT_ALLOWLIST`；服务端 handler 是最终门禁，UI 通过 authenticated readiness 响应决定是否显示导入控件。asset profile 供料关闭时，assistant、静态页和 report planner 保持旧的无资产画像行为；第三方图片结构化识别关闭时保持既有图片消息链路。

**Consequences:**

- Positive：可按测试 tenant 灰度，不依赖 UI 隐藏保护后端。
- Negative：需要一个只返回安全 capability 的内部响应，并在 assistant/report/third-party 路径增加回归测试。
- Alternative rejected：仅使用 `NEXT_PUBLIC_*` 前端开关，无法阻止直接调用 API。

### ADR-04: 首版 parser 复用 `ingest-worker`

**Status:** Accepted

**Context:** 系统已有 workflow task、NATS wakeup、状态和重试基础；新增 parser 二进制和 systemd unit 会扩大当前 release train。

**Decision:** 使用现有 `ingest` queue，增加 `parse_asset_profile` task key 和显式分支。provider 调用、状态迁移和 accepted profile 写入由 `ingest-worker` 完成。

**Consequences:**

- Positive：不新增部署单元，复用现有观测和 retry。
- Negative：provider 慢调用可能影响 ingest queue；首轮 pilot 因此限制为单并发和小批量。
- Follow-up：若真实队列等待超过门禁，再把相同二进制按独立 queue/service 运行，而不是提前新建服务。

### ADR-05: Asset evidence 独立于 document evidence

**Status:** Accepted

**Context:** 现有 `retrieval_evidences` 强依赖 document/chunk 外键。把图片画像伪装成文档会破坏来源解释和权限语义。

**Decision:** 使用 `0018_asset_retrieval_evidences.sql` 建立独立 evidence 表；retrieval-worker 在授权后合并 document、database 和 asset 结果。

**Consequences:**

- Positive：来源、幂等、权限和 parser version 可独立治理。
- Negative：union ranking 和 storage API 增加一种 source kind。
- Alternative rejected：创建虚假 document/chunk，短期省表但长期污染数据模型。

### ADR-06: 本 release train 保持模块化单体与既有 worker 拓扑

**Status:** Accepted

**Context:** 全仓盘点显示生产实现集中在 `platform-api`、PostgreSQL 和多个 worker；`analytics`、`mcp-gateway`、report compiler/runtime/publisher 等 crate 主要仍是接口或占位实现，Qdrant/MinIO/DataFusion/DuckDB 也未形成当前主链路。此时按“目标微服务架构”排期会把脚手架误当成能力，并扩大运维面。

**Decision:** 当前 release train 只在既有 API、PostgreSQL、workflow task 和 worker 拓扑内完成收版。不得为资产链路新增微服务、数据库、向量库、对象存储或队列；只有 canary 数据证明现有拓扑无法满足门禁时，才另开 ADR 和独立计划。

**Consequences:**

- Positive：部署、回滚和故障归因保持在现有运维能力内。
- Negative：`platform-api` 热点继续存在，新增 helper 仍需严格限制依赖方向。
- Alternative rejected：在当前 RC 同时落地控制面/数据面拆分，无法与业务变更安全隔离。

### ADR-07: CI 是 RC 前置产物，不是发布后的补充检查

**Status:** Accepted

**Context:** 当前 CI 只覆盖 rustfmt、少量 model-gateway/static-page tests、smoke self-tests 和 Web build；本地盘点证明 workspace check、399 个 Web unit tests、资产 smoke 和公开契约 guard 都可在无凭证条件下稳定执行。

**Decision:** 在形成本地 RC 前，新增独立 CI 任务固化这些无凭证门禁。CI 先保持 targeted tests + workspace check，不在当前大工作区直接引入全量 `cargo test --workspace`。

**Consequences:**

- Positive：后续提交和暗发布共享同一套可重复门禁，避免依赖人工记忆。
- Negative：self-hosted runner 时间增加，需要观察 45 分钟 job timeout。
- Follow-up：若 targeted CI 稳定，再用独立矩阵评估全量 Rust tests，不阻塞当前 RC。

---

## 5. 非功能与安全门禁

### 5.1 安全

1. 所有写入口必须同时满足 authenticated session、tenant scope、dataset visibility、dataset owner/manage 权限和 feature allowlist。
2. 第三方入口额外满足 connection allowlist；主站 tenant allowlist 不可替代第三方 connection allowlist。
3. 公开第三方接口、URL、鉴权、请求字段和既有响应字段保持不变。
4. URL 资产在 SSRF 防护完成前只登记 locator，不允许 worker 主动下载。Task 9 必须阻止私网/loopback/link-local、重定向越权、DNS 重绑定和超限下载后才能开启远程 URL parser。
5. ZIP 使用现有默认上限：最多 100 张图片、单 entry 80 MiB、总展开 300 MiB；首轮 pilot 进一步限制为 1 个 ZIP、最多 10 张、总展开不超过 20 MiB。
6. 回执、日志、任务卡和 validation 不记录 cookie、bearer、raw provider payload、对象 key、URL、客户文件名或本地路径。

### 5.2 可靠性

1. 导入幂等键覆盖 tenant、source kind、safe source id、dataset 和 parser version。
2. parse 状态只允许：`pending -> processing -> completed|partial|retrying|failed`；未知状态必须拒绝写入。
3. pilot 默认 parser 总尝试次数为 2 次，即首次执行加 1 次重试；重试必须可观测且不得覆盖已 accepted 的旧 profile。
4. provider 超时初始门禁为 120 秒；单个 pilot worker 并发为 1。
5. failed/partial 任务保留安全错误码和 manual retry 入口，不自动循环重试。
6. 关闭 feature flag 不删除已写入的测试资产；清理必须使用 reviewed manifest 和单独审批。

### 5.3 性能与容量

以下是 pilot 门禁，不是正式 SLA：

| 指标 | 初始门禁 |
| --- | --- |
| 同步导入 API，不含对象上传 | p95 不高于 2 秒 |
| 单张图片解析 | 180 秒内进入 completed/partial/failed |
| pilot 并发 | 1 |
| 主站 pilot | 1 张图片 + 1 个小 ZIP |
| 第三方 pilot | 单 connection、single/batch/ZIP 各 1 次 |
| ingest queue 等待 | 不持续超过 5 分钟 |

超出门禁先记录 provider、队列、数据库或对象读取瓶颈，不直接扩容或更换技术。

### 5.4 可用性与回退

1. 当前 release train 不允许 destructive migration；既有生产数据目标 RPO 为 0。
2. 新能力异常时首先关闭对应 flag，目标 15 分钟内停止新写入。
3. 代码回滚使用 forward revert commit，再由 8 服务器 fast-forward；禁止在 dirty 或未知远端执行 `reset --hard`。
4. schema 为 additive，旧服务必须能够在新表存在时继续运行。
5. 每次部署后至少观察 10 分钟错误日志；首个即时 health 连接拒绝可重试，但最终 health/ready 必须恢复。

### 5.5 可观测性

至少记录以下脱敏事件或等价状态：

```text
asset_import.created
asset_parse.queued
asset_parse.started
asset_parse.partial
asset_parse.retrying
asset_parse.failed
asset_parse.completed
asset_evidence.materialized
asset_retrieval.supplied
asset_import.access_denied
```

事件只记录 tenant/asset/task 的内部 ID、状态、计数、耗时、parser version 和安全错误码，不记录原始输入或凭证。

### 5.6 可维护性与 CI

1. `cargo check --workspace`、完整 Web unit tests、资产导入 self-test/preflight 和第三方公开契约 guard 必须在无凭证 CI 中运行。
2. Web tests 必须通过 `apps/web` package script 启动，避免依赖调用者 cwd；当前预期为 38 个文件、400 个测试，新增测试后只允许数量上升。
3. CI 失败必须能归属为格式、编译、unit、contract、smoke 或 build；不得把多个高风险动作折叠成一个不可定位步骤。
4. 当前不强制全量 `cargo test --workspace`，但任何受影响 crate 必须运行对应 tests；全量测试矩阵作为 P5 独立任务。
5. 不为清除 `MODULE_TYPELESS_PACKAGE_JSON` 或 middleware 弃用告警而改变 RC 行为；告警治理在 P0 关闭后单独执行。

---

## 6. Release train 与依赖

```text
Task 1 工作区盘点
  -> Task 2 #1015 / rustfmt 收口
  -> Task 3 append-only migration
  -> Task 4 三类 default-off feature gates
  -> Task 5 固化无凭证 CI 门禁
  -> Task 6 本地 RC + 提交审批
  -> Task 14 PostgreSQL 18.4 停机冷迁移
  -> Task 7 8服务器暗发布，关闭 P0
  -> Task 8 主站测试租户 pilot
  -> Task 9 ingest-worker parser live
  -> Task 10 asset evidence + unified retrieval
  -> Task 11 第三方私有 pilot
  -> Task 12 operator / 受控并发门禁
  -> Task 13 V3 与 Codex 客户端联合验收
```

执行规则：

- Task 1–5 只允许本地开发和无副作用验证。
- Task 6 的 commit、push 分别需要用户授权。
- Task 7 是首次部署关口；Task 8–13 不得提前混入 Task 7。
- Task 8、9、10、11 每完成一个阶段，都必须有独立部署、回执和 flag rollback 证明。
- Task 14 必须在已批准收版提交同步 GitHub 后执行；数据库切换窗口不得混入 schema、feature 或业务数据变更。
- 任一 Task 为 `FAIL` 时，不得继续后续 Task；`PENDING` 和 `AUTH_REQUIRED` 可安全交接。

建议里程碑（按 1 名主开发 + 1 名 reviewer 估算，不含审批等待、provider 不稳定和远端窗口排队）：

| 里程碑 | Tasks | 交付结果 | 估算工程时间 |
| --- | --- | --- | --- |
| M0 本地可重复基线 | 1–5 | dirty worktree 归属、格式/migration/gates/CI 全部可重复 | 1–2 天 |
| M1 feature-off RC | 6 | 可回退 commit 并同步 GitHub | 0.5–1 天 |
| M1B 数据库主版本升级 | 14 | PostgreSQL 18.4、checksums、冷迁移和旧数据目录保留 | 0.5–1 天 |
| M1C feature-off 暗发布 | 7 | 8 服务器应用暗发布、P0 关闭 | 0.5–1 天 |
| M2 主站写入 canary | 8 | 测试 tenant 真实导入、幂等、任务卡和回退 | 0.5–1 天 |
| M3 解析与检索闭环 | 9–10 | accepted profile、独立 evidence、统一检索 | 3–5 天 |
| M4 第三方私有 canary | 11 | 单 connection 私有入口，公开契约不变 | 1–2 天 |
| M5 运营与客户端验收 | 12–13 | 受控并发结论、V3/Codex 联合回执 | 1–2 天 |

任一里程碑可在前一里程碑 `PASS` 后独立暂停；不能用总工期承诺替代每个 live gate 的授权。

---

## 7. 可执行任务

### Task 1: 冻结并归属当前工作区

**Purpose:** 在任何格式化或代码修改前，确认每个差异的来源、候选版本归属和排除项。

**Files:**

- Review: 全部 tracked diff。
- Review: 全部 `??` 文件。
- Modify: `docs/validation/datamax-main-gap-closure.md`，新增 `RC-01 Worktree Inventory` 小节。

**Steps:**

1. 记录本地、远端分支和工作区：

```powershell
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
git diff --name-status
git diff --numstat
git ls-files --others --exclude-standard
git diff --check
```

2. 按以下组建立清单：

- A：storage/contracts/migration。
- B：资产导入 API、画像和主站 UI。
- C：assistant supply、静态页和 report planner。
- D：第三方图片结构化识别。
- E：validation/plan。
- X：高尔夫、deck、10 服务器计划和其他不入版内容。

3. 检查 `crates/platform-api/src/lib.rs` 的删除是否都有对应 module/helper 承接。
4. 检查 `crates/storage/migrations/0015_asset_libraries.sql` 相对 HEAD 的差异只包含待迁移的 `asset_parse_runs`。
5. 检查 `docs/plans/datamax-active-execution-plan.md` 只保留冻结标记和交接链接，不继续追加执行流水。
6. 把文件归属、未知项、排除项和下一步写入 validation。

**Expected:**

- HEAD 与 `origin/main` 均为交接基线，或明确说明为什么已变化。
- 没有未知来源的大块差异。
- `git diff --check` 为 0。

**Stop:** 发现未知用户改动、意外丢路由、意外删除模块或工作区被外部改写。

**Done:** 每个 tracked/untracked 项都有 A–E 或 X 归属；状态 `PASS` 才能进入 Task 2。

---

### Task 2: 收口 #1015 和 rustfmt 门禁

**Purpose:** 只修已知格式问题，并把主站图库任务卡 dry-run 的安全口径写入 validation。

**Files:**

- Modify: `crates/platform-api/src/asset_import_support.rs`。
- Review: `apps/web/app/lib/asset-library-view-model.js`。
- Test: `apps/web/app/lib/asset-library-view-model.test.mjs`。
- Review: `scripts/smoke/fashion-design-asset-import.mjs`。
- Modify: `docs/validation/datamax-main-gap-closure.md`。

**Steps:**

1. 先确认失败仍只有已知断言：

```powershell
cargo fmt --check
```

Expected: 当前收版状态直接 PASS；若再次出现超出 `asset_import_support.rs` 的格式 diff，停止检查来源。

2. 执行 rustfmt：

```powershell
cargo fmt --all
git diff --name-only
```

3. 如果 rustfmt 修改了 `asset_import_support.rs` 之外的 Rust 文件，逐个确认是纯机械格式；出现逻辑变化立即停止。
4. 运行定向回归：

```powershell
cargo fmt --check
cargo test -q -p platform-api asset_import --lib
cargo test -q -p platform-api fashion_postchain_adapter --lib
cargo check -q -p platform-api
node --test apps/web/app/lib/asset-library-view-model.test.mjs
node --check scripts/smoke/fashion-design-asset-import.mjs
npm run test:third-party-public-contract-guard
npm run smoke:fashion-design-asset-import -- --self-test --pretty
npm run smoke:fashion-design-asset-import -- --preflight --pretty
git diff --check
```

5. 在 validation 的 `RC-02 #1015 Closure` 记录：无网络、无生产写入、无任务创建、无公开契约变化、任务卡仍为 dry-run。

**Expected:** 56+ asset import tests、8+ adapter tests、19+ Web tests 和 contract guard 通过。

**Stop:** 测试数量下降、回执出现 locator/auth/raw payload，或 helper 实际创建任务卡/写生产数据。

**Done:** rustfmt 和全部定向门禁为 `PASS`；不 commit、不 push。

---

### Task 3: 把 `asset_parse_runs` 移到 append-only migration

**Purpose:** 修复 migration 版本归因，并验证 migration 可重复执行。

**Files:**

- Modify: `crates/storage/migrations/0015_asset_libraries.sql`。
- Create: `crates/storage/migrations/0017_asset_parse_runs.sql`。
- Modify: `crates/storage/src/lib.rs`。
- Create: `crates/test-fixtures/tests/migration_replay.rs`。
- Modify: `docs/validation/datamax-main-gap-closure.md`。

**Steps:**

1. 在 `crates/storage/src/lib.rs` 先写失败测试：

```rust
assert_eq!(MIGRATIONS.last().map(|migration| migration.version), Some("0017"));
assert!(!ASSET_LIBRARIES_SCHEMA.sql.contains("create table if not exists asset_parse_runs"));
assert!(ASSET_PARSE_RUNS_SCHEMA.sql.contains("create table if not exists asset_parse_runs"));
```

2. 运行并确认先失败：

```powershell
cargo test -q -p storage migrations_include_asset_parse_runs_schema --lib
cargo test -q -p storage auth_migrations_are_registered_in_order --lib
```

Expected: `ASSET_PARSE_RUNS_SCHEMA` 或 `0017` 尚不存在导致 FAIL。

3. 从 `0015` 移出 `asset_parse_runs` 表和三个索引，原样加入 `0017_asset_parse_runs.sql`。
4. 在 `storage/src/lib.rs` 注册：

```rust
pub const ASSET_PARSE_RUNS_SCHEMA: Migration = Migration {
    version: "0017",
    description: "asset parse runs",
    sql: include_str!("../migrations/0017_asset_parse_runs.sql"),
};
```

5. 把 `ASSET_PARSE_RUNS_SCHEMA` 追加到 `MIGRATIONS`，更新顺序断言和资产库 schema 测试。
6. 新增本地 PostgreSQL replay test：连接明确的本地测试 URL，连续调用两次 `storage.migrate()`，然后只查询 `to_regclass('public.asset_parse_runs')` 和必要索引是否存在；不插入业务数据。
7. 启动本地 PostgreSQL 并运行：

```powershell
docker compose -f infra/compose/docker-compose.local.yml up -d postgres
$env:PLATFORM_DATABASE_URL='postgres://ai_platform:ai_platform@127.0.0.1:5432/ai_data_platform_v3'
cargo test -q -p storage migrations --lib
cargo test -q -p test-fixtures --test migration_replay -- --nocapture
$env:PLATFORM_DATABASE_URL=$null
```

8. 检查 SQL 只有 additive `create table/index if not exists`，无 drop、alter destructive 或 backfill。

**Expected:** `0015` 恢复已发布内容；`0017` 可连续执行两次；表、唯一约束和索引存在。

**Stop:** replay 需要连接非本地数据库、出现 destructive SQL、或旧 schema 不能升级。

**Done:** migration unit/replay test `PASS`；validation 记录 migration 边界和 forward-revert 回滚说明。

---

### Task 4: 为当前 RC 的新行为增加 default-off gates

**Purpose:** 确保首次部署既不开放真实资产写入口，也不改变普通问答、静态页、报表规划和既有第三方图片消息行为。

**Files:**

- Create: `crates/platform-api/src/asset_import_access_support.rs`。
- Modify: `crates/platform-api/src/lib.rs`。
- Modify: `crates/platform-api/src/asset_import_support.rs`。
- Modify: `crates/platform-api/src/assistant_run_model_supply_item_support.rs`。
- Modify: `crates/platform-api/src/static_page_template_reference_support.rs`。
- Modify: `crates/platform-api/src/external_channel_sse_support.rs`。
- Modify: `crates/platform-api/src/external_image_structured_extract_runtime_support.rs`。
- Modify: `crates/contracts/src/lib.rs`。
- Modify: `crates/report-planner-worker/src/main.rs`。
- Modify: `apps/web/app/HomePageClient.js`。
- Modify: `apps/web/app/components/WorkspaceDirectoryPanel.js`。
- Modify: `apps/web/app/lib/asset-library-view-model.js`。
- Test: `apps/web/app/lib/asset-library-view-model.test.mjs`。
- Modify: `scripts/smoke/fashion-design-asset-import.mjs`。

**Required configuration:**

```text
MAIN_SITE_ASSET_IMPORT_ENABLED=false
MAIN_SITE_ASSET_IMPORT_TENANT_ALLOWLIST=
ASSET_PROFILE_SUPPLY_ENABLED=false
EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED=false
```

**Steps:**

1. 先写 Rust 失败测试，覆盖：三个 feature env 缺失时全部 disabled；主站 enabled 但 tenant 未 allowlist；主站 enabled 且 tenant allowlisted；空 tenant 不通过。
2. 新增 `asset_import_access_support`，复用 `platform_env_flag` 和安全 CSV 匹配语义；返回稳定 reason code，不回显 env 内容。
3. 新增 authenticated readiness 路径：

```text
GET /v1/asset-imports/fashion-design-images/readiness
```

响应只允许：

```json
{
  "enabled": false,
  "reason_code": "feature_disabled",
  "single_supported": true,
  "batch_supported": true,
  "zip_supported": true
}
```

4. 在 single 和 batch POST handler 的任何数据库写入前执行相同 server-side gate。
5. 先验证默认关闭：POST 返回稳定的 disabled 错误码，asset_items/parse_runs/task 数量不增加。
6. 前端打开资产库视图时读取 readiness；`enabled=false` 时不渲染导入按钮，也不发 POST。
7. Web view-model 测试覆盖 capability 缺失、disabled、enabled 和未知字段忽略。
8. `ASSET_PROFILE_SUPPLY_ENABLED=false` 时 assistant supply 不添加 `asset_profile_hint`，静态页/report planner 不加入 asset profile 模块；开启时才保持当前本地实现。
9. `EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED=false` 时图片消息不得进入新 provider/runtime；开启时仍须通过独立 provider、换图幂等和 redaction 测试。
10. smoke self-test/preflight 增加：`defaultDisabled=true`、`nonAllowlistedDenied=true`、`assetProfileSupplyDisabled=true`、`externalImageStructuredExtractDisabled=true`、`publicContractUnchanged=true`。

**Validation:**

```powershell
cargo test -q -p platform-api asset_import_access --lib
cargo test -q -p platform-api asset_import --lib
cargo test -q -p platform-api asset_profile_supply --lib
cargo test -q -p platform-api external_image_structured_extract --lib
cargo test -q -p report-planner-worker asset_profile
cargo check -q -p platform-api
node --test apps/web/app/lib/asset-library-view-model.test.mjs
node --check scripts/smoke/fashion-design-asset-import.mjs
npm run smoke:fashion-design-asset-import -- --self-test --pretty
npm run smoke:fashion-design-asset-import -- --preflight --pretty
npm run test:third-party-public-contract-guard
```

**Stop:** UI 隐藏但 API 仍可写、allowlist 读取失败时默认放行、feature-off 时 assistant/report/third-party 行为仍变化，或 readiness 暴露 tenant/env 值。

**Done:** 默认配置下主站没有真实资产写路径，asset profile 不进入模型/报表供料，第三方图片不进入新结构化识别；每条能力可独立开启和回退。

---

### Task 5: 固化无凭证 CI 门禁

**Purpose:** 把本轮已在本地证明可重复的 workspace check、完整 Web tests、资产 smoke 和公开契约 guard 固化到 self-hosted CI，先消除“本地通过但提交后无人复验”的发布风险。

**Files:**

- Modify: `apps/web/package.json`。
- Modify: `package.json`。
- Modify: `.github/workflows/datamax-ci.yml`。
- Modify: `docs/validation/datamax-main-gap-closure.md`。

**Steps:**

1. 先记录当前缺口：`apps/web/package.json` 没有 `test` script，CI 没有 workspace check、完整 Web tests、资产 smoke 和公开契约 guard。
2. 在 `apps/web/package.json` 新增：

```json
"test": "node --test"
```

3. 在根 `package.json` 新增：

```json
"test:web": "npm --prefix apps/web test"
```

4. 从仓库根运行并确认 package script 固定了正确 cwd：

```powershell
npm run test:web
```

Expected: 38 个 test files、当前 400/400 通过；不得再出现因 `process.cwd()` 指向仓库根导致的单测失败。

5. 在 `no-credential-smoke` 增加独立步骤，不与 Web build 合并：

```yaml
- name: Run web unit tests
  run: npm run test:web

- name: Guard asset and public integration contracts
  run: |
    npm run test:third-party-public-contract-guard
    npm run smoke:fashion-design-asset-import -- --self-test --pretty
    npm run smoke:fashion-design-asset-import -- --preflight --pretty
```

6. 在 smoke syntax step 增加 `node --check scripts/smoke/fashion-design-asset-import.mjs`。
7. 在 `rust-minimal` 增加两个可归因步骤：

```yaml
- name: Rust workspace check
  run: cargo check --workspace

- name: Guard asset import modules
  run: |
    cargo test -p platform-api asset_import --lib
    cargo test -p platform-api fashion_postchain_adapter --lib
```

8. 不在本 Task 引入全量 `cargo test --workspace`，也不设置任何 live credential、database URL 或 execute flag。
9. 本地复跑 CI 等价命令：

```powershell
cargo fmt --check
cargo check --workspace
cargo test -q -p platform-api asset_import --lib
cargo test -q -p platform-api fashion_postchain_adapter --lib
npm run test:web
npm run test:third-party-public-contract-guard
npm run smoke:fashion-design-asset-import -- --self-test --pretty
npm run smoke:fashion-design-asset-import -- --preflight --pretty
npm --prefix apps/web run build
git diff --check
```

10. 在 validation 记录每个 CI step 的输入边界、预期输出和 timeout；不得写入 runner 路径、token 或环境变量值。

**Stop:** CI 需要 live 凭证、Web tests 仍依赖调用者 cwd、job 超出既有 timeout、测试数量下降，或公开契约 guard 与资产私有能力发生耦合。

**Done:** 本地 CI 等价命令全部 `PASS`，workflow 中每类失败可单独定位；不 commit、不 push。

---

### Task 6: 形成可暗发布的本地 RC

**Purpose:** 对当前大工作区做完整发布候选审核，确认 Task 5 的 CI 门禁已进入候选，排除 off-scope 文件，并在获得授权后形成可 forward-revert 的提交。

**Files:**

- Review: Task 1 的 A–E 文件。
- Exclude: `crates/platform-api/src/external_aigolf_skill_support.rs`，除非差异被证明是核心依赖。
- Exclude: `decks/`。
- Exclude: `docs/plans/2026-07-07-server10-aiv3-deployment-plan.md`。
- Modify: `docs/validation/datamax-main-gap-closure.md`。

**Steps:**

1. 先运行完整本地门禁：

```powershell
cargo fmt --check
cargo test -q -p storage migrations --lib
cargo test -q -p platform-api asset_import --lib
cargo test -q -p platform-api fashion_postchain_adapter --lib
cargo test -q -p platform-api asset_profile_supply --lib
cargo test -q -p platform-api assistant_run_model_supply --lib
cargo test -q -p platform-api static_page_template_reference --lib
cargo test -q -p platform-api external_image_structured_extract --lib
cargo check --workspace
npm run test:web
npm --prefix apps/web run build
npm run test:third-party-public-contract-guard
npm run smoke:fashion-design-asset-import -- --self-test --pretty
npm run smoke:fashion-design-asset-import -- --preflight --pretty
npm run smoke:main-assistant-streaming -- --self-test
npm run smoke:static-page-5way -- --self-test
git diff --check
```

2. 对新增行做敏感信息审核，只记录命中文件和结论，不把可能的秘密值打印进 validation。
3. 审核 `0015`/`0017`、tenant isolation、索引、唯一约束、additive rollback。
4. 审核 `lib.rs` 路由、handler 和 module 拆分没有丢失既有行为。
5. 推荐提交切片：

- `feat(storage): add append-only asset parse schema`
- `feat(assets): add guarded fashion gallery import`
- `feat(retrieval): supply safe asset profiles`
- `feat(integrations): isolate image structured extraction`
- `docs: record DataMax release candidate`

6. 如果 `lib.rs` 等文件的跨能力耦合导致文件级切片不安全，不做脆弱的部分暂存；改用一个明确的 `feat: stage guarded DataMax asset release candidate`，但 validation 仍按模块列出测试结果。
7. 暂存后运行：

```powershell
git diff --cached --stat
git diff --cached --check
git status --short
```

8. 向用户报告候选文件、排除文件、门禁结果和建议 commit message，状态设为 `AUTH_REQUIRED`。
9. 只有用户明确确认后才 commit；commit 后重新运行 `git status --short --branch`，不得自动 push。

**Stop:** Web build、workspace check、普通问答/静态页 smoke 或公开契约 guard 失败；候选仍含 X 组内容。

**Done:** 本地 RC 有明确 commit、可回退边界和完整 validation；尚未 push/deploy 时状态为 `AUTH_REQUIRED`。

---

### Task 7: 8 服务器 feature-off 暗发布并关闭 P0

**Purpose:** 在不开放资产写能力的情况下部署 RC，先证明既有线上能力没有回归。

**Requires explicit approval:** push、8 服务器部署、数据库 additive migration 和服务重启。

**Preconditions:**

1. Task 6 为 `PASS`，用户已批准 commit 和 push。
2. `origin/main` 包含唯一批准的 release commit。
3. `MAIN_SITE_ASSET_IMPORT_ENABLED`、`ASSET_PROFILE_SUPPLY_ENABLED`、`EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED` 在 8 服务器缺失或明确为 false。
4. 不启用第三方 asset-imports、parser live 或 asset evidence write。

**Steps:**

1. 本地确认：

```powershell
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
```

2. 远端只读 preflight：

```text
cd /srv/aiv3/repo
git status --short --branch
git rev-parse HEAD
systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-assistant-run-worker.service aiv3-chat-session-worker.service aiv3-static-page-worker.service aiv3-report-planner-worker.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz
```

3. 若远端工作区非空或 HEAD 非预期，停止，不覆盖远端。
4. 确认 feature 没有意外开启，命令只返回成功/失败，不打印 env：

```bash
if grep -Eqi '^(MAIN_SITE_ASSET_IMPORT_ENABLED|ASSET_PROFILE_SUPPLY_ENABLED|EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED)=(1|true|yes|on)$' /etc/aiv3/*.env 2>/dev/null \
  || systemctl cat aiv3-platform-api.service aiv3-report-planner-worker.service 2>/dev/null \
    | grep -Eqi '^(Environment=)?(MAIN_SITE_ASSET_IMPORT_ENABLED|ASSET_PROFILE_SUPPLY_ENABLED|EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED)=(1|true|yes|on)$'; then
  echo 'a dark-release feature is unexpectedly enabled' >&2
  exit 1
fi
```

5. fast-forward 和构建：

```bash
cd /srv/aiv3/repo
git fetch origin main
git merge --ff-only origin/main
cargo fmt --check
CC=clang CXX=clang++ cargo test -q -p storage migrations --lib
CC=clang CXX=clang++ cargo test -q -p platform-api asset_import --lib
CC=clang CXX=clang++ cargo check -q -p platform-api
npm --prefix apps/web run build
CC=clang CXX=clang++ cargo build --release \
  -p platform-api \
  -p assistant-run-worker \
  -p chat-session-worker \
  -p static-page-worker \
  -p report-planner-worker
```

6. 重启受影响服务：

```bash
sudo systemctl restart \
  aiv3-platform-api.service \
  aiv3-web.service \
  aiv3-assistant-run-worker.service \
  aiv3-chat-session-worker.service \
  aiv3-static-page-worker.service \
  aiv3-report-planner-worker.service
```

7. 等待启动完成后验证：

```bash
systemctl is-active \
  aiv3-platform-api.service \
  aiv3-web.service \
  aiv3-assistant-run-worker.service \
  aiv3-chat-session-worker.service \
  aiv3-static-page-worker.service \
  aiv3-report-planner-worker.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz
git status --short
git rev-parse HEAD
```

8. 验证 `asset_parse_runs` 表存在，但 readiness 为 disabled、未 allowlist POST 不产生写入、asset profile 不进入 assistant/report supply、第三方图片不进入新结构化 provider。
9. 使用服务器侧环境变量读取凭证，不把 bearer/cookie 放在可见 CLI 参数中，运行：

```bash
npm run smoke:main-assistant-streaming -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:fashion-design-asset-import -- --preflight --pretty
npm run test:third-party-public-contract-guard
```

10. 在有受控 private smoke 凭证时再运行主站流式和 static-page targeted live；无凭证时状态为 `AUTH_REQUIRED`，不能伪装成通过。
11. 观察 10 分钟错误日志，只记录脱敏错误类型和计数。

**Rollback:**

- 新能力异常：确认 feature flag 为 false 并重启 platform-api。
- 既有能力回归：本地创建 forward revert commit，经批准 push，远端再次 ff-only、build、restart。
- 不删除 `0017` 表，不执行 `reset --hard`。

**Done:** 线上 HEAD 与 release commit 一致；目标服务 active；health/ready、Web build、普通问答/静态页/公开契约回归通过；资产写、资产画像供料和第三方新图片识别均关闭。此时 P0 关闭。

---

### Task 8: 主站测试租户资产导入 pilot

**Purpose:** 只验证真实写入、任务卡和读取闭环，不宣称 parser 或 unified retrieval 已完成。

**Requires explicit approval:** 测试身份、测试 tenant、测试 dataset、测试 asset library、env 修改、服务重启、真实写入。

**Files:**

- Modify if needed: `crates/platform-api/src/asset_import_support.rs`。
- Modify if needed: `apps/web/app/HomePageClient.js`。
- Modify if needed: `apps/web/app/components/WorkspaceDirectoryPanel.js`。
- Modify: `scripts/smoke/fashion-design-asset-import.mjs`。
- Modify: `docs/validation/datamax-main-gap-closure.md`。

**Steps:**

1. 记录批准的 tenant UUID、dataset UUID、asset library UUID 和 `approval_id`；validation 只写安全占位和计数，不写凭证。
2. 在 8 服务器配置：

```text
MAIN_SITE_ASSET_IMPORT_ENABLED=true
MAIN_SITE_ASSET_IMPORT_TENANT_ALLOWLIST=<approved test tenant uuid>
```

3. 重启 `aiv3-platform-api`，确认非 allowlisted tenant 仍被拒绝。
4. 通过环境变量运行 smoke，避免凭证出现在命令参数：

```powershell
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_BASE_URL='https://v3.elepcloud.com'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_COOKIE='<read locally; do not print>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_DATASET_ID='<approved dataset uuid>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_ASSET_LIBRARY_ID='<approved asset library uuid>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_APPROVAL_ID='<approved id>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_EXECUTE='true'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_ACK_LIVE_WRITE='true'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_PRETTY='true'
npm run smoke:fashion-design-asset-import
```

5. 只导入 1 张测试 PNG 和 1 个最多 10 张图、总展开不超过 20 MiB 的 ZIP。
6. 人工验证：任务卡 stable id、卡片不重复、只更新自身状态、详情脱敏、字段账本、失败原因和重试入口。
7. 数据库只查安全聚合：asset 数量、membership 数量、parse run 状态数量、profile 数量；不打印 locator 和 metadata。
8. 重复执行同一幂等输入，确认不重复创建 asset/membership/parse run。
9. 关闭 flag，重启 platform-api，证明新写入停止；测试数据保留，生成 reviewed cleanup manifest，但不执行删除。
10. 清除本地 smoke 环境变量：

```powershell
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_COOKIE=$null
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_BEARER=$null
```

**Expected:** 导入、任务卡、状态和详情闭环通过；parse run 可保持 `pending`，不得将 seed profile 宣称为真实 parser 结果。

**Stop:** 其他 tenant 可见、幂等失败、任务卡泄露 locator、出现无关产物或普通问答受阻。

**Done:** 主站写路径 pilot `PASS`，feature 已重新关闭，测试资产和 cleanup manifest 已记录。

---

### Task 9: 接通 `ingest-worker` 真实 parser

**Purpose:** 把 `pending` parse run 交给异步 worker，产生可复核的 accepted profile 和稳定状态。

**Files:**

- Modify: `crates/platform-api/src/asset_import_support.rs`。
- Modify: `crates/platform-api/src/fashion_postchain_adapter_support.rs`。
- Modify: `crates/platform-api/src/lib.rs`，仅导出 worker 所需安全 helper。
- Modify: `crates/ingest-worker/src/main.rs`。
- Modify: `crates/storage/src/lib.rs`。
- Modify: `crates/contracts/src/lib.rs`。
- Modify: `scripts/smoke/fashion-design-asset-import.mjs`。
- Modify: `docs/operations/v3-system-manual.zh-CN.md`。

**Required configuration:**

```text
ASSET_PARSE_ENABLED=false
ASSET_PARSE_MAX_ATTEMPTS=2
ASSET_PARSE_PROVIDER_TIMEOUT_MS=120000
```

**Steps:**

1. 先写失败测试：导入成功后 enqueue `queue=ingest`、`task_key=parse_asset_profile`；feature disabled 时不 enqueue；同 asset/parser version 只存在一个可运行任务。
2. 在 `ingest-worker` 的 `process_task` 增加显式 `parse_asset_profile` 分支；未知 task key 不允许落入 document ingest。
3. worker 加载 asset、parse run 和受控 locator，先更新 `processing`，再调用现有 fashion adapter/provider。
4. provider 输入只包含当前 tenant/asset 获准的数据；禁止把 raw provider response 写入 metadata 或日志。
5. 成功时写 accepted profile 和 `completed`；字段不足写 `partial`；可重试错误写 `retrying` 并最多再入队一次；永久错误写 `failed`。
6. parser version 变化时创建新 parse run；旧 accepted profile 保留，只有显式 accept 决策才能成为当前 profile。
7. URL locator 在完成 SSRF、redirect、DNS、MIME、字节和 timeout 守卫前返回 `remote_source_disabled`，不主动抓取。
8. 运行本地 worker 定向测试和无网络 adapter 测试：

```powershell
cargo test -q -p platform-api asset_import --lib
cargo test -q -p platform-api fashion_postchain_adapter --lib
cargo test -q -p ingest-worker parse_asset_profile
cargo check -q -p ingest-worker
cargo check --workspace
npm run smoke:fashion-design-asset-import -- --self-test --pretty
```

9. 形成独立 commit、经授权 push/deploy；构建并重启 `aiv3-platform-api.service` 和 `aiv3-ingest-worker.service`。
10. 先以 `ASSET_PARSE_ENABLED=false` 验证旧 ingest 任务；再在测试 tenant 窗口开启，单并发解析 1 张图。
11. 验证状态链、parser version、耗时、安全错误码和普通文档 ingest 不受影响。
12. pilot 结束后关闭 `ASSET_PARSE_ENABLED`；保留结果，不自动清理。

**Expected:** 单图在 180 秒内进入 completed/partial/failed；worker failure 可归因，普通 document ingest smoke 通过。

**Stop:** provider 调用发生在 API 请求内、无限重试、覆盖旧 accepted profile、queue 持续等待超过 5 分钟或文档 ingest 回归。

**Done:** parser live 有独立 commit、部署、状态回执和 flag rollback 证明。

---

### Task 10: 写入 asset evidence 并进入统一检索

**Purpose:** 让同一 dataset 在统一权限守卫下检索文档、数据库和图片资产。

**Files:**

- Create: `crates/storage/migrations/0018_asset_retrieval_evidences.sql`。
- Modify: `crates/storage/src/lib.rs`。
- Modify: `crates/platform-api/src/asset_profile_supply_support.rs`。
- Modify: `crates/platform-api/src/assistant_run_model_supply_item_support.rs`。
- Modify: `crates/platform-api/src/assistant_run_model_supply_budget_support.rs`。
- Modify: `crates/retrieval-worker/src/main.rs`。
- Modify: `crates/contracts/src/lib.rs`。
- Test: retrieval and membership tests in affected modules。

**Required configuration:**

```text
ASSET_RETRIEVAL_EVIDENCE_WRITE_ENABLED=false
```

**Steps:**

1. 先写 failing migration/storage tests，要求 `0018` 为最后 migration，asset evidence 不含 document/chunk 外键。
2. 创建 `asset_retrieval_evidences`，至少包含 tenant、dataset、asset、profile kind/version、materialized text、safe metadata、content hash、created/updated 时间和稳定唯一键。
3. 唯一键覆盖 tenant + dataset + asset + profile kind + profile version + content hash；重复 materialize 不新增行。
4. 新增 materializer：只使用 accepted profile、OCR、安全 caption 和标签，不保存 raw provider JSON 或 locator。
5. retrieval-worker 查询 asset evidence 前 join dataset membership 和 tenant；没有 membership 的 evidence 不进入候选。
6. union ranking 保留 `source_kind=asset_profile`、asset kind、时间权重和安全来源解释。
7. assistant/report supply 只输出紧凑摘要和 evidence ref，不输出完整 attributes JSON。
8. 测试：跨 tenant 隔离、跨 dataset 隔离、多 membership、重复解析幂等、document/asset 混合排序、普通问答无资产时行为不变。

**Validation:**

```powershell
cargo test -q -p storage migrations --lib
cargo test -q -p storage asset_retrieval --lib
cargo test -q -p retrieval-worker asset
cargo test -q -p platform-api asset_profile_supply --lib
cargo test -q -p platform-api assistant_run_model_supply --lib
cargo check --workspace
npm run smoke:main-assistant-streaming -- --self-test
```

9. 独立 commit、授权后部署 `platform-api`、`retrieval-worker` 和受影响 assistant/report worker。
10. 测试 tenant 使用一张已 completed/partial 的图片提问，验证命中和来源解释；对照普通文档问答，确认没有误召回。
11. 关闭 `ASSET_RETRIEVAL_EVIDENCE_WRITE_ENABLED`，证明新 evidence 写入可停止而旧查询仍可读取。

**Expected:** 统一检索可命中资产，权限、幂等、排序和来源解释均通过。

**Stop:** 伪造 document/chunk、跨 scope 命中、locator 泄露或普通问答质量明显回退。

**Done:** asset evidence 和 retrieval 有独立 migration、commit、部署和 canary 回执。

---

### Task 11: 第三方私有 `asset-imports` pilot

**Purpose:** 在不改变公开第三方契约的前提下，只向一个 reviewed connection 开放私有资产导入。

**Files:**

- Create: `crates/platform-api/src/external_asset_import_support.rs`。
- Modify: `crates/platform-api/src/lib.rs`。
- Modify: `crates/platform-api/src/asset_import_support.rs`。
- Modify: `crates/platform-api/src/external_channel_image_idempotency_support.rs`。
- Test: `tools/third-party-public-contract-guard.test.mjs`。
- Create: `docs/integrations/v3-third-party-asset-imports-private.md`。
- Modify: `scripts/smoke/fashion-design-asset-import.mjs` 或新增 focused private smoke。

**Required configuration:**

```text
EXTERNAL_ASSET_IMPORT_ENABLED=false
EXTERNAL_ASSET_IMPORT_CONNECTION_ALLOWLIST=
```

**Steps:**

1. 先写失败测试：flag false 拒绝；flag true 但 connection 未 allowlist 拒绝；tenant/source/dataset/asset library scope 不匹配拒绝。
2. 新入口复用既有第三方鉴权、connection、tenant、source 和 dataset scope；不接受客户端自行指定其他 tenant。
3. 返回稳定 JSON：request id、task ref、parse status、asset safe summary；不返回 locator、raw metadata 或 provider payload。
4. 复用 single/batch/ZIP、换图和幂等逻辑；相同 message/idempotency key 不重复写入。
5. 公开接口文档保持不变；私有文档标明 pilot、feature flag 和连接 allowlist。
6. contract guard 增加断言：公开 schema/URL/required fields 未出现 `asset-imports` 或 `asset_library_external_ids`。
7. 本地 self-test/preflight 全部通过后形成独立 commit。
8. 经授权部署，先 feature-off 回归公开第三方问答和静态页，再只开启一个测试 connection。
9. single、batch、ZIP、换图、重复请求和跨 connection 隔离各运行一次。
10. pilot 完成后关闭 flag，保留脱敏聚合和 cleanup manifest。

**Validation:**

```powershell
npm run test:third-party-public-contract-guard
npm run smoke:fashion-design-asset-import -- --self-test --pretty
npm run smoke:fashion-design-asset-import -- --preflight --pretty
cargo test -q -p platform-api external_asset_import --lib
cargo test -q -p platform-api external_channel_image_idempotency --lib
cargo check -q -p platform-api
```

**Stop:** 公开契约漂移、其他 connection 可见、asset 默认非 private、幂等失败或响应泄露内部字段。

**Done:** 单 connection pilot `PASS`，公开契约保持不变，flag 已重新关闭。

---

### Task 12: 生产级 operator 与受控并发门禁

**Purpose:** 在功能链路稳定后验证观测、队列、provider 和并发瓶颈，不把压测与功能开发混在一起。

**Requires explicit approval:** operator 凭证、受控窗口和生产 live。

**Steps:**

1. 使用合法 operator 身份验证脱敏 provider/queue/fallback 聚合；不得输出凭证或客户数据。
2. 主站和第三方共 20 路问答、流式链路分批执行，先 5 路再递增；任何异常先停止递增。
3. 运行 5 路本地重任务和 2 路 fallback；区分 provider failure、queue backlog、worker crash 和业务校验失败。
4. 资产 parser 仍保持单并发；只有单并发稳定且 queue wait 低于 5 分钟才讨论增加并发。
5. 记录 p50/p95、成功/partial/failed 数、queue wait、provider timeout、fallback count 和用户可见错误。
6. 不自动修复 provider 瞬时错误；若 targeted smoke 全部通过，正确结论可以是“不改代码”。

**Stop:** 凭证缺失、窗口未批准、服务错误率上升、普通问答延迟显著恶化或客户数据可能泄漏。

**Done:** 有可复核的脱敏聚合、瓶颈归因和是否扩容/改代码的明确结论。

---

### Task 13: V3 与 Codex 客户端联合验收

**Purpose:** 验证企业配置包、本地执行、产物上传、挂接、发布和继续编辑完整闭环。

**Reference:** `docs/integrations/v3-codex-client-boundary-contract.md`

**Flow:**

```text
V3 config package
-> 企业客户端读取授权范围
-> 本地 Codex 执行
-> 上传 manifest + files
-> V3 挂接 dataset / asset library
-> 任务卡显示结果
-> private 或 sandbox publish
-> 用户继续“修改报表”
```

**Steps:**

1. 先运行 self-test：

```powershell
npm run smoke:v3-client-artifact-joint -- --self-test
```

2. 使用正确 platform-api base 做 preflight；若公网域名返回前端 HTML 而不是 API health，必须失败，不能把 HTTP 200 当通过。
3. 校验 config package 不包含 activation token、cookie、数据库 URL 或超出授权的数据集/资产库。
4. 真实 execute 必须使用测试身份、受控 workspace、私有或 sandbox 发布和显式批准。
5. 验证 manifest、文件 hash、artifact attach、任务卡、发布链接和继续编辑。
6. 验证客户端不能修改 V3 产品源码、服务、migration、auth、公开 API 或 deploy state。
7. 记录客户端版本、V3 commit、测试数据集、产物计数和安全边界；不记录凭证和客户文件内容。

**Validation:**

```powershell
npm run smoke:v3-client-artifact-joint -- --self-test
npm run smoke:v3-client-artifact-joint -- --preflight --base-url <approved-platform-api-base>
```

**Stop:** base URL 指向错误服务、授权范围漂移、manifest 泄密、产物覆盖稳定 URL 或客户端尝试修改 V3 产品代码。

**Done:** config -> execute -> upload -> attach -> publish -> edit 全链路有测试身份下的联合回执。

---

### Task 14: PostgreSQL 18.4 停机冷迁移

**Purpose:** 在系统当前未使用的前提下，停止全部 AIV3 服务，将 8 服务器主状态库从 PostgreSQL 17.9 冷迁移到 18.4，不采用蓝绿或逻辑复制。

**Reference:** `docs/adr/0003-upgrade-postgresql-18.md`

**Preconditions:**

1. 当前收版提交已经 commit 并 push 到 GitHub。
2. `postgresql-17.service` active，数据库只读预检通过。
3. PGDG 仓库可提供 `postgresql18-server` 和 `postgresql18-contrib` 18.4。
4. 备份目录不在 PostgreSQL 17 数据目录内，磁盘空间足够。

**Steps:**

1. 记录 PostgreSQL 版本、扩展、数据库大小、表数、连接数、locale、checksums 和 15 个 AIV3 unit 的数据库依赖。
2. 停止全部 `aiv3-*.service` 和 Web 服务，确认没有应用连接或写入。
3. 使用版本化 PostgreSQL 17 工具生成 globals 和数据库 custom-format 逻辑备份，并校验备份文件非空、`pg_restore --list` 可读。
4. 对 `/var/lib/pgsql/17/data` 创建只读归档或快照，记录 checksum；不删除原目录。
5. 安装 PostgreSQL 18.4 server/contrib，初始化 checksums-on 新集群，迁移 reviewed `postgresql.conf` 和 `pg_hba.conf` 设置。
6. 在 5432 启动 PostgreSQL 18，恢复 globals、`ai_data_platform_v3`、`aidp_client` 和 `home_platform`，运行 `ANALYZE`。
7. 验证版本 18.4、checksums on、SCRAM、locale、pgcrypto、64+ 表、关键行数、GIN/全文检索索引和 migration replay。
8. 将 15 个 AIV3 unit 和 `home-db-backup.service` 的依赖从 `postgresql-17.service` 改为 `postgresql-18.service`，同步更新备份脚本，daemon-reload 后启动服务。
9. 验证全部目标服务 active、health/ready、Web、普通问答、文档、检索、静态页和 feature-off 状态。
10. 禁用 PostgreSQL 17 自启动，但保留 17 数据目录和全部备份；清理另行审批。

**Rollback:** 在重新开放业务写入前，停止 AIV3/PG18，恢复 unit 依赖并重新启动 PostgreSQL 17；禁止删除或覆盖 17 数据目录。

**Stop:** 任一备份不可读、恢复对象/行数不一致、pgcrypto/索引缺失、系统 unit 未全部切换、health/ready 失败或出现未知写入。

**Done:** PostgreSQL 18.4 active、checksums on、AIV3 全部使用新服务且业务回归通过；PG17 数据目录和备份仍保留。

**Completion receipt (2026-07-10):**

- Release gate: `f9463861ced34022fb98ed959bccda015c8b7800` 已推送到 `origin/main`；GitHub Actions `29075435644` 为 `success`。
- Backup: `/srv/aiv3/backups/postgresql-17-to-18-20260710T071104Z`，约 199 MB；globals、三个 custom-format 数据库 dump、restore list 和 PG17 物理归档均通过 SHA-256 校验。
- Cluster: PostgreSQL `18.4` active/enabled，data checksums `on`，UTF-8、`en_US.UTF-8`、`Asia/Shanghai` 和 SCRAM 设置已核对；PostgreSQL 17 inactive/disabled。
- Data parity: `ai_data_platform_v3` 64 表 / 106,974 行，`aidp_client` 42 表 / 237 行，`home_platform` 55 表 / 3,258 行；三库逐表行数均与 PG17 基线完全一致。
- Database objects: 主库 205 个索引、9 个 GIN 索引、0 个 invalid index、0 个 unvalidated constraint；`pgcrypto` 1.4 摘要函数验证通过。
- Runtime: 17 个维护前 active 的 AIV3 服务全部恢复 active；本机 API `healthz`、`readyz` 和 Web 均为 HTTP 200，公网 `https://v3.elepcloud.com/` 为 HTTP 200，切换后服务 priority-error 日志为 0。
- Operations: 15 个 AIV3 unit 加 `home-db-backup.service` 已改为 PG18 依赖；备份任务实际执行成功，新 dump 可由 PG18 `pg_restore --list` 读取。
- Rollback retained: `/var/lib/pgsql/17/data`、PG17 RPM、systemd 原件和上述备份均未删除；后续清理必须另行审批。

---

## 8. 本 release train 明确不做

1. 不继续 P5 小拆分，直到 Task 7 关闭 P0。
2. 不把高尔夫 skill、演示 deck 或 10 服务器部署混入核心 RC。
3. 不开放第三方公开 `asset-imports` 或 `asset_library_external_ids`。
4. 不执行 P2 fingerprint backfill、对象清理、source sync 或生产数据库同步。
5. 不恢复会阻断正常回答的硬质量门禁；回答不足继续检索或提出可执行追问。
6. 不把 Cloudflare/Codex fallback 当主链路成功；主链路失败必须可见、可重试、可归因。
7. 不为了假设中的高并发提前增加 parser 微服务、Kubernetes 或新消息系统。

### 8.1 P0 关闭后的独立工程治理队列

以下项目来自全仓盘点，但不与本 release train 混做：

1. 为 migration runner 增加 ledger、checksum 和已执行版本审计，替代启动时无记录重放。
2. 建立全量 Rust test CI 矩阵，并按 crate 历史时长拆分 runner，不直接塞进当前 `rust-minimal`。
3. 让 OpenAPI 与 140 个实际 route 收敛，先覆盖 auth、datasets、documents、assistant、external 和 assets 六个域。
4. 对 `platform-api/src/lib.rs` 做行为保持切片；先按 route domain 抽 handler，不在同一提交改变契约。
5. 对 Web 热点做行为保持治理：修复 cwd 依赖、模块类型告警、middleware 约定、`HomePageClient.js` 和 `globals.css` 拆分。
6. 对 `mcp-gateway`、analytics、report compiler/runtime/publisher 和未接通基础设施逐项写保留/实现/删除 ADR；没有真实调用方的脚手架不得继续被当成线上能力。

---

## 9. 每阶段通用发布门禁

任何候选提交或部署至少满足：

1. `cargo fmt --check`。
2. `cargo check --workspace`，或明确记录无法覆盖的 crate 和原因。
3. 受影响 Rust module tests。
4. migration unit test；有新 schema 时追加本地 replay test。
5. `npm run test:web` 全量 Web unit tests 和 Web production build。
6. 主站普通问答和流式 self-test；部署后有凭证再跑 targeted live。
7. 第三方公开契约 guard。
8. 静态页 targeted smoke。
9. 资产导入 self-test/preflight；execute 仅在批准后。
10. `git diff --check` / `git diff --cached --check`。
11. 新增行敏感信息审核，不把疑似秘密值打印进回执。
12. 8 服务器 HEAD、工作区、服务、health/ready 和 10 分钟错误日志抽样。
13. 对应 feature flag 的 default-off 和 rollback 证明。

---

## 10. 新线程启动指令

首轮只执行 Task 1 和 Task 2：

```text
接手 DataMax V3 主线。完整阅读 docs/plans/datamax-active-execution-plan.md。

只执行 Task 1 和 Task 2：保护当前 dirty worktree，不 reset、不覆盖用户改动，不 commit、不 push、不部署、不真实写入、不改第三方公开契约。

完成差异归属、#1015 validation、rustfmt 和定向回归后，按本文报告模板返回 PASS/PENDING/AUTH_REQUIRED/FAIL，并明确是否具备进入 Task 3 的条件。
```

进入 Task 3–6 的启动指令：

```text
继续执行 DataMax V3 可执行计划 Task 3 到 Task 6。先复核 Task 1/2 的 HEAD、工作区和 validation。完成 append-only 0017、三类 default-off capability gate、无凭证 CI 固化和本地 RC 审核；不得 commit、push 或部署，直到用户明确批准对应动作。
```

进入 Task 7 的启动指令：

```text
只执行 DataMax V3 计划 Task 7。先确认用户已明确批准 push 和 8 服务器暗发布。新资产写能力必须保持关闭；远端非 clean 或 HEAD 不匹配立即停止。完成 build、服务、health/ready、既有业务 smoke、公开契约 guard 和 feature-off 证明后报告，不执行 Task 8。
```

---

## 11. 关键参考文件

- `docs/validation/datamax-main-gap-closure.md`：唯一验证和部署回执。
- `docs/plans/datamax-active-execution-plan.md`：冻结历史账本。
- `docs/archive/plans/2026-06-17-multimodal-asset-library-fashion-gallery.md`：资产库原始设计参考。
- `docs/integrations/v3-codex-client-boundary-contract.md`：V3 与 Codex 客户端边界。
- `docs/operations/v3-system-manual.zh-CN.md`：服务和运行说明。
- `docs/architecture/v3-rust-architecture.md`：总体架构。
- `docs/architecture/v3-repository-and-module-layout.md`：模块边界。
- `docs/adr/0002-use-postgresql-as-primary-state-store.md`：PostgreSQL 主状态决策。
- `infra/compose/docker-compose.local.yml`：本地 PostgreSQL/Redis/NATS/Qdrant/MinIO。
- `.github/workflows/datamax-ci.yml`：当前无凭证 CI 和本计划 Task 5 的固化入口。
