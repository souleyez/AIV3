# AI Data Platform V3 Development Summary

## Purpose

这份文档用于把 `2026-04-19` 交接文档之后继续完成的一轮收口工作压缩成稳定说明，避免后续继续依赖聊天历史回忆状态。

适用场景：

- 回顾当前 V3 Rust 重构到底已经做到哪里
- 判断主计划中的 Phase 6 现状，而不是只看最初 skeleton 描述
- 在新线程继续开发前快速恢复对“已完成 / 未完成 / 为什么这样做”的理解

## One-Page Status

截至 `2026-04-21`，V3 已经不是“Phase 6 skeleton 刚打通”的状态，而是进入了“Phase 6 已完成最小闭环，并把 runtime / typed view / CLI inspect 面进一步收口，同时开始把 model-facing capability spec 落成真实 host 协议”的状态。

当前可视为已经稳定的部分：

- Phase 1 到 Phase 5 的统一 workflow / task / worker / signal 骨架已可复用
- Phase 6 的五条最小垂直切片已完整打通
  - `upload_ingest_workflow`
  - `retrieval artifact`
  - `memory_directory_workflow`
  - `dataset_output_workflow`
  - `chat_session_workflow`
- PostgreSQL 继续作为 execution / event / task / artifact 主状态
- NATS JetStream 继续只是唤醒加速层，不承载唯一真相

这一轮新增并收紧的重点，不是再多打一条业务链，而是把已有链路上的“结构化运行语义”补齐：

- retrieval evidence 从占位 metadata 进一步收紧成稳定 typed view
- memory directory 顶层和节点级 version/scope 语义更明确
- dataset output / chat message 从“原始文本 + raw manifest”收紧成 typed output sections
- LLM 调用与 tool 调用从 manifest 内隐信息收紧成 durable artifact 行
- chat session 可直接聚合最新 assistant message，而不必只依赖 session manifest
- runtime inspect 能力正式通过 API + CLI 暴露出来
- `runtime.inspect` 已开始暴露第一层 `model_facing` mapping，用于把 runtime truth 收口成 `capability_class / service_lane / report_entry_state / evidence_state / continuation_state / recommended_next_action / allowed_next_actions`
- `DatasetOutputView`、`ChatSessionView`、`ChatMessageView`、`ReportPlanSummary` 与 `ReportRenderOutputView` 也已开始直接暴露这层 `model_facing` 只读摘要，避免模型侧协议只停留在 inspect 调试面
- `WorkflowRuntimeInspectView` 顶层现在也已显式挂出 `report_plan` 与 `report_render_output`，不再让 report workflow 的 inspect 只能靠 `execution.kind` 侧推
- `model_facing` 判定已从粗粒度默认值收紧到更细的保守信号：`answer_content_present`、`document_focus`、`distinct_document_count`、`indexed_document_count`
- report side 的 `model_facing` 也已开始依据 `report_plan_status`、`current_ast_version_id`、`report_render_status`、`surface` 与 `asset_manifest` 做保守判定，而不再完全依赖 `runtime.inspect` 顶层泛化分类
- `model_facing` 现在还会直接给出 `continuation_state` 与 `recommended_next_action`，把“可继续的平台动作集合”进一步收成更稳定的 bounded continuation 协议
- report side 当前最主要的 host continuation 也已从泛化 `continue_report_generation` 收成 route 对齐的 `generate_report_output`
- `model_facing` 现在还会显式区分 `service_lane` 与 `report_entry_state`，先把“当前仍在 Material Service，还是已经明确进入 Report Service”从隐式约定收成协议事实
- `chat_session` 已落第一版真实 host-side `2选1` gate：`request_confirmation / stay_material_service / enter_report_service`
- 这条 `chat_session report_entry` gate 已不只存在于 HTTP route，当前也有 library 入口与本地 CLI，可被调试脚本或后续 host 直接复用
- 这条 `chat_session report_entry` gate 现在也已经进入默认 `tool registry`，不再只是 API/CLI 私有入口
- report side 的 `generate_report_output` 现在也已有对应 library 入口、本地 CLI 与默认 `tool registry: report.render`，不再只是 inspect / `model_facing` 里的推荐动作名
- `workflow.retry` 现在也已有对应 library 入口、本地 CLI 与默认 `tool registry` 条目，并会自动串联 `retry_requested + start`，不再要求 host 手工补第二次 restart
- `report.plan` 现在也已有对应 library 入口、本地 CLI 与默认 `tool registry` 条目，可对已有 `draft` plan 重开 planning execution
- `report.publish` 现在也已有对应 library 入口、本地 CLI 与默认 `tool registry` 条目，会复用最近一次成功 render 的资产并持久化 `published_reports / published_report_versions`
- `report.read_published` 现在也已有对应 library 入口、本地 CLI 与默认 `tool registry` 条目，可按 `plan_id` 回读 `published report detail`，并同时补上通用 list/detail route
- `apps/web` 现在也已有第一版 V3 智能助手壳子：复用老版助手的页面结构和视觉方向，但控制器改成直接消费 V3 host surface，并已显式支持左侧数据集切换
- `WorkflowModelFacingSummaryView` 现在也会直接派生 `recommended_tool_key / allowed_tool_keys`，当前已先覆盖 `request_report_entry_confirmation -> chat_session.report_entry`、`read_document_detail -> document.read_detail`、`compare_documents -> document.compare`、`retry_execution -> workflow.retry`、`continue_report_planning -> report.plan`、`refresh_directory -> memory_directory.refresh`、`generate_report_output -> report.render` 与 `publish_report -> report.publish`
- `memory_directory.refresh` 现在也已有对应 library 入口、本地 CLI 与默认 tool registry 条目，因此目录感知场景不再只给动作枚举，不给可执行 host surface
- `document.read_detail` 现在也已有对应 library 入口、本地 CLI、默认 tool registry 条目和聚合 detail route，因此 retrieval/detail 场景不再只暴露 `ReadDocumentDetail` 动作名
- `confirmation_required`、`needs_user_confirmation` 与 `request_report_entry_confirmation` 已不再只是 spec 词汇，而是会在 `ChatSessionView` / `runtime.inspect` / `pretty summaries` 中稳定产出
- `chat_message` 现在也能显式透传最小 `service_handoff` 视图，不再只让 message-level `model_facing` 从回答内容反推自己是否已经进入 `report_service`
- `dataset_output` 现在也能在创建时显式绑定 `chat_session_id`，并由 worker 主动透传同构 `service_handoff`，让 output-level `model_facing` 不再只靠回答内容反推是否已经进入 `report_service`
- report side 也已开始复用这套 provenance：`ReportPlanSummary` / `ReportRenderOutputView` 会从 workflow execution context 回填同构 `service_handoff`，并把 `service_handoff_source` / `confirmed_report_plan_id` 作为稳定 `model_facing.signals`
- `runtime.inspect --pretty` 当前也已开始为 report workflows 打印专门摘要块：`Report Plan Runtime` 与 `Report Render Runtime`

## How To Treat The 2026-04-21 Capability Spec

`C:/Users/soulzyn/Desktop/codex/ai-data-platform/docs/plans/2026-04-21-model-facing-platform-capability-spec.md` 对当前 V3 的价值主要在于“重新校正模型侧能力面应该长什么样”，但它不应被当成 Rust V3 当前实现的冻结真相。

这里需要明确三层关系：

- Rust V3 当前已经在做的 runtime / artifact / recovery / inspect 收口，是平台真实执行面的建设
- capability spec 主要定义“模型应该看到怎样的服务语言、证据级别和连续执行边界”
- 两者之间需要的是 mapping layer，而不是让任一侧直接吞并另一侧

因此这份 spec 在当前阶段应被视为：

- 有约束力的方向输入
- 可回写、可调整的 draft
- 用来指导后续 `capability_class` / `evidence_state` / `bounded continuation` 收口的上游文档

而不应被视为：

- 立即要求 Rust 代码逐字段照搬的硬实现合同
- 可以推翻当前 runtime truth / artifact truth 的更高优先级真相源

## What Is Now Implemented

### 1. Retrieval / Memory / Output / Chat Typed Views Have Been Tightened

当前 contracts 已不再只是返回原始 JSON manifest，而是补上了可稳定消费的 typed view：

- `RetrievalEvidenceManifestView`
- `MemoryDirectoryManifestView`
- `DatasetOutputManifestView`
- `ChatSessionManifestView`
- `ChatMessageManifestView`

其中最关键的结构化收口包括：

- retrieval evidence 明确区分 `embedding`、`recall`、`evidence locator`
- memory directory 顶层 manifest 带 `version_no`，节点也有 `scope` / `version_no`
- `DocumentLifecycle` 已从松散字符串消费收紧成专门 enum 视图
- dataset output section 显式带 `retrieval_evidence_ids`
- chat message section 也显式带 `retrieval_evidence_ids`
- turn runtime、finish reason、stream mode 都已有 typed enum / view
- chat turn 已能显式区分 provider phase 与 tool loop phase，而不只剩计数摘要
- chat turn 已开始显式暴露 streaming phase 的状态与关键时间点，而不只剩 `stream_mode`
- chat turn 已开始显式暴露 artifact commit phase 的状态与关键时间点，而不再只能靠 `assistant_message_persisted_at` 反推

这一步的意义是把“prompt 看到什么”与“系统事实上产出了什么”拆开。后面无论接真实 provider、真实工具执行、还是更强的 UI，都不需要重新从 raw JSON 猜语义。

### 2. Dataset Output And Chat Message Already Carry Durable Runtime Facts

当前 `dataset_outputs` 与 `chat_messages` 不只是有文本结果，还能直接挂出结构化运行事实：

- `retrieval_evidences`
- `llm_invocations`
- `tool_executions`

这意味着：

- dataset output 不再只是“生成了一段文本”
- chat assistant message 也不再只是“回复了一段文本”
- 调用过哪些工具、用了哪次 LLM 请求、绑定了哪些 retrieval evidence，都能作为独立事实读取
- `output_manifest_view.tool_trace` / `message_manifest_view.tool_trace` 已可优先从 durable `tool_executions` hydrate，而不是继续信 raw manifest
- worker 写回 durable `llm_invocations` / `tool_executions` 时，也不再需要先把 `tool_trace` 明细落进 raw manifest 再反解
- 如果 `dataset_output` 在创建时显式绑定了 `chat_session_id`，当前 output manifest 也会主动写回最小 `service_handoff`，把 `report_entry` gate 的结果继续往 output artifact 透传

不过 chat assistant message 的 raw manifest 现在仍会稳定携带 `tool_trace`，而 session manifest 的 response-ready / failed turn、当前 workflow task payload，以及 append-only workflow event checkpoint 都会稳定携带 recovery payload。这里不是回退到 manifest-first，而是为了支持 replay / recovery：

- 如果同一 `chat_turn_id` 在 assistant message 已创建后、但 durable rows 或 session context 还没补齐时失败
- worker 可以直接从已存在的 assistant artifact 解析 runtime 与 tool trace，继续补齐 commit，而不是再次调用 provider
- 如果 provider 已响应、response-ready session manifest 已写回，但 assistant message 还没创建成功
- worker 也可以直接从 session manifest 的 recovery payload 重建 assistant artifact，而不是再次调用 provider
- 如果 provider 已响应，但 response-ready session manifest 还没成功写回
- worker 仍可以直接从 workflow task payload 里的 recovery checkpoint 重建 assistant artifact，而不是再次调用 provider
- 如果 task payload recovery checkpoint 写回失败，但 append-only workflow event checkpoint 已经成功落库
- worker 仍可以直接从这条 workflow event recovery checkpoint 重建 assistant artifact，而不是再次调用 provider

这符合当前固定下来的架构原则：

- artifact first, prompt second
- schema and typed view evolve together

### 3. LLM Invocations And Tool Executions Are Durable First-Class Artifacts

本轮已经完成两条 durable artifact 线：

- `llm_invocations`
- `tool_executions`

它们已经具备：

- PostgreSQL 表与索引
- storage repository 读写接口
- manifest runtime parser
- worker 持久化写回
- contracts typed view
- API 读取聚合
- output/chat worker 已能直接用 runtime/tool call 结果写 durable rows，而不是只依赖 manifest 反解

已落地的能力面：

- `GET /v1/workflow-executions/{execution_id}/llm-invocations`
- `GET /v1/workflow-executions/{execution_id}/tool-executions`

这一步的意义不是多两个表，而是把“模型实际怎么执行”和“工具实际怎么执行”从 prompt 内隐历史中剥离出来，变成可查询、可回放、可验证的 durable state。

### 4. Chat Session Read Model Is No Longer Forced To Rely On Session Manifest Runtime

`ChatSessionView` 当前已经补上：

- `latest_assistant_message_id`
- `latest_assistant_message`

这让 session 读模型可以直接聚合真正的 assistant message artifact，而不是只读 `session_manifest.runtime` 里重复表达的一份摘要。

直接后果：

- 客户端可以直接拿到最新 assistant message 的 typed runtime 语义
- assistant message 上的 `llm_invocations` / `tool_executions` / evidence 也可顺着聚合面直接消费
- 后续继续削弱 `session_manifest.runtime` 的重复职责会更自然
- `chat-session-worker` 在重跑同一 `chat_turn_id` 时，如果 session 中已经存在该 turn 的 assistant message，会优先恢复这条已有 artifact，再继续做 manifest finalize、runtime rows 持久化和 session context 收口，而不是再次打 provider
- 如果 assistant message 还不存在，但 session manifest 的 `last_turn.recovery` 已经落库，也会直接从这份 recovery payload 重建 assistant artifact，而不是重新打 provider
- 如果 session manifest 里也还没有 recovery payload，但当前 workflow task payload 已经写下 `_chat_turn_recovery` checkpoint，也会直接从这份 task-local checkpoint 重建 assistant artifact
- 如果 task payload checkpoint 也不存在，但 execution 已经写下 `chat_turn_recovery_checkpoint` workflow event，也会直接从这条 append-only event checkpoint 重建 assistant artifact

### 5. Runtime Inspect Has Become A Formal API And CLI Surface

这一轮最重要的“能力面收口”是 `runtime.inspect`。

当前已经具备：

- API:
  - `GET /v1/workflow-executions/{execution_id}/runtime-inspect`
- contracts:
  - `WorkflowRuntimeInspectView`
- library:
  - `load_workflow_runtime_inspect(...)`
- CLI:
  - `runtime-inspect-cli <execution_id> [--pretty]`
- tool registry:
  - `runtime.inspect`

`runtime.inspect` 统一聚合：

- `execution`
- `dataset_output`
- `chat_session`
- `chat_messages`
- `llm_invocations`
- `tool_executions`
- `pretty_summaries`

这件事很关键，因为它把“人工调试入口”和“模型可调用入口”统一到了同一条 CLI-first surface 上，而不是继续散落在若干临时脚本和若干 API 片段里。

### 6. Chat Session Now Has A Minimal Host-Side Report Entry Gate

这一轮已经把 capability spec 里最关键、最容易失真的一段先落成真实宿主协议：`chat_session` 进入报表链不再只能靠隐式 prompt 约定，而是有了第一版可写回的 host gate。

当前已落地的最小合同包括：

- `ChatSessionManifestView.report_entry`
- `UpdateChatSessionReportEntryRequest / Response`
- `POST /v1/chat-sessions/{session_id}/report-entry`
- `apply_chat_session_report_entry_update(...)`
- `chat-session-report-entry-cli <session_id> <action> [--title ...] [--objective ...] [--pretty]`
- `tool registry: chat_session.report_entry`
- `ChatSessionReportEntryActionView`
  - `request_confirmation`
  - `stay_material_service`
  - `enter_report_service`
- `ModelFacingNextActionView::RequestReportEntryConfirmation`
- `ModelFacingContinuationStateView::NeedsUserConfirmation`

当前行为边界也已经固定：

- host 选择 `request_confirmation` 后，会在 `session_manifest.report_entry` 写入 `confirmation_required`
- `ChatSessionView.model_facing` 会把这类 session 收成：
  - `report_entry_state=confirmation_required`
  - `continuation_state=needs_user_confirmation`
  - `recommended_next_action=request_report_entry_confirmation`
- host 选择 `enter_report_service` 后，会直接创建 `report_plan` 和对应 workflow execution，再把 `report_entry_state` 推进到 `confirmed`
- host 选择 `stay_material_service` 后，不再删除这条 gate，而是把它写成：
  - `report_entry_state=not_applicable`
  - `resolved_action=stay_material_service`
  - `resolved_at=...`
  这样主线回到 Material Service，但 decline history 仍然可追踪
- `update_chat_session_report_entry` 内部的“请求校验 / 默认值推导 / manifest 状态迁移”已经抽成纯 helper，并补了单元测试，避免这段 host gate 逻辑继续埋在 handler 分支里
- `report_entry` 的宿主写回逻辑已经抽成 library 入口，可由 route 与 CLI 复用，而不是继续只埋在 axum handler 内
- `model_facing.signals` 里的 `report_entry_state` / `chat_turn_status` / `artifact_commit_status` 也已经从 Rust `Debug` 字符串收成稳定协议字符串，并补上了 `report_entry_resolved_action`
- `chat-session-worker` 现在会在生成 assistant message manifest 时，把 session-level `report_entry` 收成显式 `service_handoff`
- `ChatMessageManifestView.service_handoff` 当前会透传：
  - `source`
  - `service_lane`
  - `report_entry_state`
  - `resolved_action`
  - `suggested_title / suggested_objective`
  - `confirmed_report_plan_id`
- `ChatMessageView.model_facing` 当前会优先消费这条显式 handoff：
  - `confirmation_required` 时，message 级 summary 会稳定收成 `needs_user_confirmation`
  - `confirmed + report_service` 时，message 级 summary 会稳定切到 `report_planning`
- `CreateDatasetOutputRequest` 当前已支持可选 `chat_session_id`
- `platform-api` 在创建 `dataset_output` 时会校验这条 session 是否存在且属于同一 dataset，再把 `chat_session_id` 写入 execution context
- `dataset-output-worker` 当前会在绑定 session 的情况下，从 `session_manifest.report_entry` 提取同构 `service_handoff` 并写回 output manifest
- `DatasetOutputView.model_facing` 当前也会优先消费这条显式 handoff：
  - `confirmation_required` 时，output 级 summary 会稳定收成 `needs_user_confirmation`
  - `confirmed + report_service` 时，output 级 summary 会稳定切到 `report_planning`
- `ReportPlanSummary` 与 `ReportRenderOutputView` 当前也已支持可选 `service_handoff`
- `chat_session report_entry -> report_plan` 这条入口现在会把 confirmed handoff 写进 `report_plan` workflow execution context
- `report_render` 创建时也会沿用这条 provenance，把同构 handoff 从 `report_plan` execution context 继续透传到 `report_render` execution context
- report side 自身的 `model_facing` 虽然默认已经处于 `report_service / confirmed`，但当前还会额外暴露：
  - `service_handoff_source=chat_session_report_entry`
  - `confirmed_report_plan_id=...`
  - `service_handoff_suggested_title_present=true|false`
  - `service_handoff_suggested_objective_present=true|false`

这一步的意义不是“chat 已经变成重业务编排器”，而是先把最关键的服务切换点从隐式语言约定收成协议事实。

### 7. Initial Web Assistant Shell Now Consumes V3 Host Surfaces

`apps/web` 当前已经不再是空目录，而是有了第一版可运行的 V3 Web 壳子。这里刻意没有照搬旧版整套 controller，而是保留旧版智能助手的 shell / sidebar / chat / insight 结构，再把控制器和 API 对接改成直接消费 V3 已经落地的 host surface。

当前这一版前端已覆盖：

- 左侧 sidebar 显式支持数据集选择
- sidebar 内可直接创建新的 dataset，并自动切换过去
- 主会话区可在当前 dataset 下发起新的 `chat_session`
- 会话消息区会轮询 `chat_messages`，用于等待 worker 回写 assistant message
- 右侧 insight 面会同时展示当前 dataset 的 `chat_sessions`、`dataset_outputs` 与 `published_reports`
- `chat_session.report_entry` 的 host-side `2 选 1` gate 已接进 UI，可直接在页面里选择 `stay_material_service / enter_report_service`
- Web 层通过 Next route proxy 直接转发到 `platform-api /v1/*`，避免前端直接依赖跨端口 CORS

这条前端线当前仍然是“薄宿主 / 读模型优先”的实现，不假设尚未暴露的多轮追加接口已经存在：

- 当前发送问题仍然是“每次创建新会话”，而不是在旧会话上追加 turn
- 更重的 report planning / render / publish UI 还没铺开
- 前端的主要作用先是把现有 host surfaces 接起来，而不是重新发明一层业务编排协议

## Verified Baseline

本轮收口后的已验证基线：

- `cargo fmt --all`
- `cargo test -p dataset-output-worker`
- `cargo test -p platform-api`
- `cargo test`
- provider runtime API smoke

实际验证通过的重点不只包括 workflow 成功，还包括结构化 runtime 语义：

- `dataset_output.llm_invocations = 1`
- `dataset_output.tool_executions = 1`
- `chat assistant message.llm_invocations = 1`
- `chat assistant message.tool_executions = 1`
- `chat_session.latest_assistant_message_id` 与 assistant message 实际 ID 对齐
- `chat turn.artifact_commit_status` / `artifact_commit_ready_at` 能区分“可提交”与“已持久化”
- `chat turn.artifact_commit_status=failed` 能区分“provider 已响应但 artifact commit 失败”的终态
- `chat turn.artifact_commit_failure_source` 能指出失败落在 workflow event recovery persist / response-ready session update / assistant message create / manifest update / runtime rows persist / session context update 的哪一段
- `chat-session-worker` 的 replay helper 已验证会优先恢复同一 `turn_id` 的最新 assistant message
- `chat-session-worker` 的 session-turn recovery helper 已验证会优先消费 `last_turn.recovery` 中的 assistant content / runtime / tool trace，并在 `tool_trace_count` 与 `tool_trace` 明细不一致时拒绝继续恢复
- `chat-session-worker` 的 task-payload recovery helper 已验证会优先消费 `_chat_turn_recovery` checkpoint，并在 `tool_trace_count` 与 `tool_trace` 明细不一致时拒绝继续恢复
- `chat-session-worker` 的 workflow-event recovery helper 已验证会优先消费 `chat_turn_recovery_checkpoint`，并在 `tool_trace_count` 与 `tool_trace` 明细不一致时拒绝继续恢复
- `runtime-inspect` API 返回的聚合结果与 execution 真实状态对齐
- `runtime-inspect-cli` 返回的聚合结果与 API 对齐
- `platform-api` 默认测试集当前保持绿色，4 个 `report_entry_route_*` API 级 route test 已重新接回默认基线
- `report_entry_route_*` 当前通过 shared local Postgres fixture 复用同一 pool，并在每个 test 前重置数据库状态后重新同步 workflow definitions

本轮还顺手修复了一个真实联调问题：

- `platform-api` crate 在新增 `runtime-inspect-cli` 与 `chat-session-report-entry-cli` 之后已经不再只有单一 binary
- 因此 smoke 脚本启动 API 时必须显式写成：
  - `cargo run -p platform-api --bin platform-api`
- 如果仍写成 `cargo run -p platform-api`，会因为 binary 选择歧义而启动失败

## Why This Direction Is Coherent

这轮收口并不是“又多做了几层包装”，而是在兑现已经固定下来的架构约束：

- chat 默认弱编排，只按需供料
- artifact 优先于 prompt
- capability 默认 CLI-first，而不是 CLI-only
- PostgreSQL 继续作为主状态
- workflow / task / worker 骨架继续统一复用

因此这一轮没有去做的事情也是有意的：

- 没继续把 chat 扩成重业务编排器
- 没把 tool 执行语义重新埋回 prompt
- 没把 inspect 做成只在某个脚本里能跑的调试能力
- 没把 NATS 推成唯一执行通道

## What Is Still Not Finished

当前仍然没有完成的部分，主要集中在“真实 runtime 语义”，而不是 schema 外形：

- retrieval 仍未接真实向量索引与真实 embedding 生产链
- dataset output 仍未进入更完整的 provider-driven generation 流程
- chat session 仍未完成真正多轮、稳定 streaming、复杂 tool loop 的 runtime contract
- chat session 已经能追踪 `artifact_commit_failure_source`，但还没有更细的底层数据库错误码 / retry 分类
- session manifest 与 assistant message artifact 之间仍存在部分重复表达
- 第一版 host-side `2选1` gate 目前只落在 `chat_session`
- `chat_message`、`dataset_output`、`report_plan` 与 `report_render_output` 都已接入最小 `service_handoff` provenance
- `dataset_output` 当前这条 handoff 主链仍只覆盖“创建时显式绑定 `chat_session_id`”的入口，还没有延展到更多自动衔接场景
- `runtime.inspect` / `model_facing` 现在已经把 `read_document_detail`、`compare_documents`、`refresh_directory` 与 `generate_report_output` 收口到默认 tool key，其中 `compare_documents` 也已有同等级真实 host surface
- 如果失败发生在 provider 响应已经返回、但 task payload / workflow event recovery checkpoint 都还没成功写回之前，系统仍然没有可恢复 payload，只能按 fresh generate 重新执行

换句话说，当前已经把“结构化壳子”和“可查询事实面”搭好了。下一步不应直接跳成“继续堆 runtime 字段”，而是先把这些事实面整理成模型侧可稳定消费的协议，再继续补真实 provider / tool / streaming 语义。

## Recommended Next Slice

如果继续顺着当前这条线收口，最自然的下一步已经变成：

- 在 `compare_documents`、`workflow.retry`、`report.plan` 与 `report.publish` 都已经落成真实 host surface 之后，下一段重点不再是“补最后一个 report-side continuation”，而是继续把发布后消费面和 host 复用面接完整

更具体地说，建议优先做：

1. 把这层已完成的 publish/read host surface 接到真正会消费它的 surface：
   - CLI / 调试脚本
   - 后续 UI/agent host
   - `runtime.inspect` 之外的调用入口
2. 再决定是否把当前“复用 render 资产直接发布”的实现，继续推进成真正的 `report-publisher` 抽象与消费链路
   其中 CLI / 调试脚本与 tool-registry 这一层已经有了第一版入口，后续主要还剩 UI/agent host 对接
3. 在 `runtime.inspect` 顶层已经显式挂出 report provenance 之后，继续判断是否要把 report-side host continuation 再单独收成 inspect-level 专门动作，而不是只停留在 `ContinueReportPlanning / GenerateReportOutput`
4. 并行继续推进 provider 响应返回后、task payload / workflow event recovery checkpoint 写回之前那段最前沿失败窗口的 retry / idempotency 分类
5. 然后再回到更重的真实 provider / tool / streaming 语义补全

如果暂时不想碰更重的 provider 语义，次优先的补强点是：

- 继续把 `runtime.inspect` 的 CLI 使用方式、输出稳定性和运维说明补成更正式的操作文档

## Reference Files

主计划：

- [2026-04-18-v3-development-execution-plan.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-18-v3-development-execution-plan.md>)

架构约束：

- [2026-04-19-v3-architecture-constraints.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-19-v3-architecture-constraints.md>)

上一轮交接：

- [2026-04-19-v3-phase6-handoff.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-19-v3-phase6-handoff.md>)

本轮总结：

- [2026-04-20-v3-development-summary.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-20-v3-development-summary.md>)
