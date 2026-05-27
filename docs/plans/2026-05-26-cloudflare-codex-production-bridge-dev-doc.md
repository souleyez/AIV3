# Cloudflare Codex Production Bridge Development Doc

**目标：** 让 8 服务器上的 V3 在演示环境中稳定调用 1 服务器 `codex-web` / Cloudflare Codex，完成“第三方或本地指令 -> Image2 效果图 -> Codex 生成最终 HTML -> V3 发布产物链接”的闭环。

**原则：** V3 仍然是系统事实源。8 服务器负责对话、数据集、文档范围、任务编排、状态、发布和第三方响应；1 服务器 / Cloudflare Codex 只作为受控执行器，不直接成为第三方入口、权限中心或业务状态中心。

---

## 当前判断

1. 代码路径已经基本具备。
   - `crates/static-page-worker` 会把 `static-page-visual` 任务提交到 Codex Web orchestrator。
   - `crates/codex-host-agent` 支持 `cloudflare_orchestrator` 模式，可执行固定模板 `static_page_image2_data_publish`。
   - `crates/platform-api` 的 readiness 已允许 `cloudflare_orchestrator + cloudflare_codex`，不要求 `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=true`。

2. 生产缺口主要是配置和 smoke。
   - 8 服务器需要配置 `CODEX_ORCHESTRATOR_BASE_URL`、`CODEX_ORCHESTRATOR_RUNTIME_TARGET` 和访问密钥。
   - 密钥推荐走 `CODEX_ORCHESTRATOR_KEY_FILE`，不要把明文 key 写入 env。
   - `aiv3-static-page-worker` 和 `aiv3-codex-host-agent` 必须使用同一个 orchestrator 配置。
   - 配完后需要用真实第三方请求做端到端 smoke。

3. 最终 HTML 生成不应只停留在 8 侧本地 renderer。
   - 演示优先阶段，效果图和最终 HTML 都应优先走 Cloudflare Codex 固定执行器。
   - 8 服务器负责把 Cloudflare 返回的结构化结果或 `artifact.html` 发布到 V3 的 `/generated-artifacts/`。

---

## 目标链路

```text
第三方 / V3 本地对话
  -> platform-api 识别静态页 / 报表意图
  -> V3 选择数据集、文档范围、数据库来源、输出格式和模板 skill
  -> static-page-worker 提交 static-page-visual 到 souleye.cc
  -> Cloudflare Codex / Image2 返回 HTTPS 图片 artifact
  -> V3 写回 preview_asset_key 和 image job 状态
  -> codex-host-agent 提交 static_page_image2_data_publish 固定任务
  -> Cloudflare Codex 生成最终 HTML 或 public_url
  -> V3 校验、发布到 /generated-artifacts/
  -> 第三方响应拿到最终页面链接
```

---

## 8 服务器生产配置

推荐写入 `/etc/aiv3/aiv3.env`：

```text
CODEX_ORCHESTRATOR_BASE_URL=https://souleye.cc
CODEX_ORCHESTRATOR_RUNTIME_TARGET=cloudflare
CODEX_ORCHESTRATOR_KEY_FILE=/etc/aiv3/secrets/codex-orchestrator.key

CODEX_HOST_TASK_ENABLED=true
CODEX_HOST_TASK_ALLOWLIST=static_page_image2_data_publish
CODEX_HOST_AGENT_EXECUTION_MODE=cloudflare_orchestrator
CODEX_HOST_AGENT_HOST_KIND=cloudflare_codex
CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES=static_page_image2_data_publish
CODEX_HOST_AGENT_TASK_TIMEOUT_MS=1800000
```

可选配置：

```text
CODEX_ORCHESTRATOR_API_PATH=/api/codex/orchestrator/v1
CODEX_ORCHESTRATOR_PROJECT_ID=<optional-project-id>
CODEX_ORCHESTRATOR_POLL_INTERVAL_MS=5000
STATIC_PAGE_ORCHESTRATOR_POLL_INTERVAL_MS=5000
STATIC_PAGE_ORCHESTRATOR_MAX_POLLS=240
```

说明：

- `CODEX_ORCHESTRATOR_ACCESS_KEY` 和 `CODEX_ORCHESTRATOR_KEY_FILE` 二选一。
- key file 可以是一行 raw token，也可以是包含 `keys.rawKey`、`rawKey`、`accessKey`、`token` 等字段的 JSON。
- `CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC=false` 可以保留；它只限制旧的本机 `codex_exec`，不阻塞 `cloudflare_orchestrator`。
- 8 服务器不需要让前端、第三方或 `codex-web` 在请求体里传长期凭证。

配置后重启：

```text
systemctl restart aiv3-platform-api
systemctl restart aiv3-static-page-worker
systemctl restart aiv3-codex-host-agent
```

---

## 1 服务器 / codex-web 要求

1. `https://souleye.cc` 的 orchestrator API 可访问。
2. 已创建绑定 Cloudflare runtime 的 access key。
3. runtime target id 与 8 服务器一致，默认 `cloudflare`。
4. 支持任务类型：
   - `static-page-visual`
   - 固定模板生成 HTML 所需的 Codex task。
5. 任务结果可以返回：
   - Image2 图片 artifact；
   - 最终 HTML；
   - 或 V3 可校验的 generated-artifact public URL。

---

## 开发任务拆解

### P0：配置闭环和可观测性

- 确认 8 服务器 env 中 `CODEX_ORCHESTRATOR_*` 对 `platform-api`、`static-page-worker`、`codex-host-agent` 都可见。
- 确认 8 服务器日志不再出现默认 `127.0.0.1:3003 connection refused`。
- 确认日志错误文案是当前代码版本：`CODEX_ORCHESTRATOR_ACCESS_KEY or CODEX_ORCHESTRATOR_KEY_FILE is required`。
- 集中观测页展示每次第三方测试对应的：
  - assistant run；
  - image job；
  - orchestrator task id；
  - codex host task；
  - generated artifact link。

### P1：效果图任务 smoke

- 从第三方接口或本地对话发起静态页请求。
- 断言 `static-page-worker` 提交：

```text
kind=static-page-visual
source=ai-data-platform-static-pages
runtimeTargetId=cloudflare
metadata.output=image-artifact
```

- 在 1 服务器看到对应任务。
- Cloudflare 返回图片 artifact。
- 8 服务器写回：
  - `static_page_image_jobs.status=preview_ready`
  - `preview_asset_key=https://...`
  - `assistant_run_events.name=static_page_image_job.preview_ready`

### P2：最终 HTML 发布 smoke

- V3 在效果图 ready 后提交 `static_page_image2_data_publish`。
- Cloudflare Codex 返回以下任一结果：
  - `artifact.public_url`
  - `artifact.html`
- 若返回 `artifact.html`，8 服务器负责写入 V3 generated-artifacts 目录并生成 public URL。
- 校验最终 URL：
  - HTTP 200；
  - HTML 完整；
  - 不暴露 prompt、密钥、原始数据库 URL；
  - 有口径说明或 validation report。

### P3：慢任务状态策略

当前风险：慢任务可能被短轮询判为失败。

目标：

- `queued`、`submitted`、`running`、`pending`、`processing`、`in_progress`、`retry_wait`、`retrying`、`waking_runtime`、`waiting` 都保持 processing。
- transient poll error 只写 `poll_retry` 快照，不马上失败。
- 超过短轮询时优先保留 running/pending 状态，供第三方后续查状态。
- 只有 Cloudflare 明确 `failed/cancelled` 或 V3 校验不通过才标 failed。

建议：

- 演示期将 `STATIC_PAGE_ORCHESTRATOR_MAX_POLLS` 调到至少 240，按 5 秒轮询约 20 分钟。
- 演示期将 `CODEX_HOST_AGENT_TASK_TIMEOUT_MS` 调到至少 1800000，避免 Cloudflare Codex 生成静态页超过 15 分钟时被 V3 侧提前判失败。
- 后续把长任务改为“提交后异步跟踪”，接口先返回任务卡，完成后通过状态查询或事件流补链接。

### P4：文档问答演示升级

引入可切换策略：

```text
DOC_QA_MODE=model_pool|cloudflare_codex|auto
```

演示环境可设：

```text
DOC_QA_MODE=auto
DOC_QA_AUTO_CODEX_BIAS=1
```

建议升级到 Cloudflare Codex 的场景：

- 多文档复杂分析；
- 需要排序、统计、出表；
- 需要按模板 skill 输出；
- 普通 RAG 低置信；
- 数据库 + 文档混合分析；
- 需要生成报表或静态页。

保留回退：

- Cloudflare Codex 超时或失败时，模型池直接回答；
- 对用户和第三方不暴露内部失败态；
- 状态供料给模型，让模型能说“正在深度分析/已完成/已降级回答”。

---

## 验收清单

### 配置验收

- 8 服务器 `aiv3.env` 包含 `CODEX_ORCHESTRATOR_BASE_URL=https://souleye.cc`。
- 8 服务器使用 `CODEX_ORCHESTRATOR_KEY_FILE`，文件存在且服务用户可读。
- `aiv3-static-page-worker` 和 `aiv3-codex-host-agent` 均已重启。
- `platform-api` readiness 显示 `codex_auto_publish_ready=true` 或等价状态。

### 任务验收

- 1 服务器能看到来自 8 服务器的 `static-page-visual` 任务。
- 8 服务器能拿到 HTTPS 图片 artifact。
- 8 服务器能提交 `static_page_image2_data_publish`。
- 最终生成 V3 public URL。
- 第三方响应或状态查询能拿到最终链接。

### 质量验收

- 最终页面能打开，HTTP 200。
- 页面不是 placeholder。
- 页面包含真实 V3 数据或明确演示数据说明。
- 页面有数据口径/时间/范围说明。
- 不泄露密钥、内部路径、原始数据库连接串、完整 prompt。

### 回退验收

- Cloudflare 失败时不影响普通聊天。
- 静态页可回退到内置 HTML fallback。
- 慢任务不会误报失败。
- 观测页能定位失败在哪一段：V3 编排、Image2、Cloudflare Codex、发布。

---

## 不做的事

- 不让第三方直接访问 Cloudflare Codex。
- 不把 Cloudflare Codex 作为 V3 权限中心。
- 不允许用户输入控制 Codex CLI 参数、密钥、工作目录或部署命令。
- 不让 Codex 覆盖已有客户稳定 URL。
- 不把数据库密码、原始连接串或服务端密钥放进 prompt。

---

## 后续代码关注点

1. `static-page-worker` 长轮询超时后不要把任务立即标 failed。
2. `codex-host-agent` 的 Cloudflare output 继续保持“HTML 由 V3 发布”的路径。
3. `platform-api` 对第三方响应补齐最终 artifact link 和 processing 状态。
4. 集中观测页继续保持“打开才加载、选中才详情”的懒加载策略。
5. 文档问答的 Cloudflare Codex 模式必须做环境开关和模型池回退。
