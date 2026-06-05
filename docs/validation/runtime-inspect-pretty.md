# `runtime.inspect --pretty`

`runtime-inspect-cli <execution_id> --pretty` 现在会先打印高信号摘要，再输出完整 JSON。

同一组摘要现在也会通过 `GET /v1/workflow-executions/{execution_id}/runtime-inspect` 暴露在 `pretty_summaries` 字段里，保证 API / CLI / 后续 UI 读取的是同一份聚合结果。

当前 `WorkflowRuntimeInspectView` 顶层除了 `dataset_output` / `chat_session` / `chat_messages`，也已经显式带出：

- `report_plan`
- `report_render_output`

因此 report workflows 的 inspect 已经不再只能靠 `execution.kind` 和顶层 `model_facing` 间接判断。

与这组只读 inspect surface 配套，当前 `chat_session report_entry` 这条 host gate 也已经有可脚本化入口：

- `chat-session-report-entry-cli <session_id> <request_confirmation|stay_material_service|enter_report_service> [--title <title>] [--objective <objective>] [--pretty]`
- `tool registry: chat_session.report_entry`

当前 `platform-api` 里对应的 API 级 route test 也已经接回默认测试基线，并改成走 shared local Postgres fixture：

- `cargo test -p platform-api report_entry_route`

当前 fixture 行为边界：

- 会复用同一份 local Postgres pool，避免每个 test 都重新建连接
- 每个 test 开始前会 `truncate ... cascade` 重置表状态，再重新写入 workflow definitions
- route test 之间仍串行执行，避免共享数据库上的相互污染

当前摘要块顺序：

1. `Model-facing Runtime`
2. `Execution-scope Runtime`
3. `Dataset Output Runtime`
4. `Report Plan Runtime`
5. `Report Render Runtime`
6. `Latest Assistant Turn`

说明：

- 不存在的块不会输出
- chat 主链通常仍只会看到前 1、2、3、6 块里的相关子集
- report workflow 现在通常会看到 1、2，再加上 4 或 5

## 看什么

### `Model-facing Runtime`

适合先判断“这条 execution 当前更像哪种模型侧服务能力”，以及宿主当前证据级别大致到了哪里。

重点字段：

- `capability_class`
- `service_lane`
- `report_entry_state`
- `evidence_state`
- `continuation_state`
- `recommended_next_action`
- `allowed_next_actions`
- `recommended_tool_key`
- `allowed_tool_keys`
- `signals`

当前这层是只读 mapping，不是新的持久化真相源。它的作用是把现有 runtime / artifact / inspect 真相面，先整理成一层更适合模型侧消费的协议摘要。

当前 `capability_class` 会先收口到较少的稳定类别：

- `dataset_directory_awareness`
- `evidence_retrieval`
- `material_explanation_and_synthesis`
- `report_planning`
- `report_generation_and_editing`
- `controlled_platform_action`

当前 `service_lane` 会先把能力面压到三条主线：

- `material_service`
- `report_service`
- `controlled_platform_action`

当前 `report_entry_state` 是对“是否已经显式进入报表链”的协议表达：

- `not_applicable`
- `confirmation_required`
- `confirmed`

当前 DataMax 已经在 `chat_session` 上落了第一版最小 host-side `2选1` gate，因此当前更常见的是：

- 资料链路：`report_entry_state=not_applicable`
- chat session 已触发进入报表链确认：`report_entry_state=confirmation_required`
- 已进入 `report_plan` / `report_render` 的报表链路：`report_entry_state=confirmed`

当前阶段要注意边界：

- `confirmation_required` 的真相源仍然是 host 写入的 `session_manifest.report_entry`，不是 worker 自动推导
- `chat_message` 当前已经会显式透传一层最小 `service_handoff`，因此 message-level `model_facing` 不再只靠回答内容猜自己是否已经进入 `report_service`
- `dataset_output` 当前在创建时如果显式绑定了 `chat_session_id`，worker 也会主动透传同构 `service_handoff`，因此 output-level `model_facing` 也能稳定参与这条 handoff 判定
- `report_plan` / `report_render_output` 自身的 `model_facing` 现在也已开始带上同构 provenance signals，但 `runtime.inspect --pretty` 顶层对 report workflows 目前仍主要按 execution kind 聚合，没有把这层 provenance 单独抬出来
- `generate_report_output` 现在不只是一条 inspect 推荐动作；默认 host surface 已补出 `report.render`，并同时暴露为 library 入口、本地 CLI 与默认 tool registry 条目
- `publish_report` 现在也不只是一条 inspect 推荐动作；默认 host surface 已补出 `report.publish`，并会复用最近一次成功 render 的资产生成 `published_reports / published_report_versions`
- `read_document_detail` 现在也不只是一条 inspect 推荐动作；默认 host surface 已补出 `document.read_detail`，并同时暴露为 library 入口、本地 CLI、默认 tool registry 条目和聚合 detail route
- `compare_documents` 现在也不只是一条 inspect 推荐动作；默认 host surface 已补出 `document.compare`，并同时暴露为 library 入口、本地 CLI 与默认 tool registry 条目
- `retry_execution` 现在也不只是一条 inspect 推荐动作；默认 host surface 已补出 `workflow.retry`，并自动串联 `retry_requested + start` 两段 transition
- `continue_report_planning` 现在也不只是一条 inspect 推荐动作；默认 host surface 已补出 `report.plan`，并会对已有 `draft` plan 重开 planning execution
- `request_report_entry_confirmation` / `read_document_detail` / `compare_documents` / `retry_execution` / `continue_report_planning` / `refresh_directory` / `generate_report_output` / `publish_report` 这几类动作现在还会直接派生 `recommended_tool_key / allowed_tool_keys`，host 不必再自己手写动作到 tool key 的第一层映射
- 用户确认进入报表链后，host 会直接创建 `report_plan` 和对应 execution，再把 `report_entry_state` 推进到 `confirmed`
- 用户明确选择继续停留在资料主线后，host 会把 `report_entry_state` 收回到 `not_applicable`，但 `chat_session.session_manifest_view.report_entry` 会保留 `resolved_action=stay_material_service` 与 `resolved_at`，作为 durable decline history

当前 `evidence_state` 会先保守映射：

- `catalog_memory`
- `supply_only`
- `live_detail`
- `mixed`
- `degraded`

当前 `live_detail` 已开始有保守判定，不再只是预留名词。当前主要依赖：

- 已有 retrieval evidence
- 当前回答内容已经生成
- `document_focus=single_document`

也就是说，只有当系统当前看起来更像“围绕单文档证据做详情回答”时，才会把证据级别提升到 `live_detail`。

当前阶段仍然更常见的是：

- `catalog_memory`：只有目录态上下文
- `supply_only`：已有 retrieval evidence，但更像“供料 / 检索结果”而不是详情回答
- `mixed`：当前更像多文档综合，或目录态与 evidence 供料态同时存在
- `degraded`：执行或持久化链进入失败态

`allowed_next_actions` 用来表达这条 execution 在当前状态下更合理的下一步，例如：

- `answer_directly`
- `read_document_detail`
- `compare_documents`
- `request_report_entry_confirmation`
- `continue_report_planning`
- `generate_report_output`
- `publish_report`
- `wait_for_tool_loop`
- `finalize_artifact_commit`
- `retry_execution`

`continuation_state` 用来把这组 action 进一步收成更稳定的连续执行判断，当前会先收口到：

- `ready_to_answer`
- `needs_user_confirmation`
- `needs_platform_continuation`
- `waiting_for_runtime`
- `retry_required`

这层的重点不是替代 `allowed_next_actions`，而是帮助 host / model 快速判断“现在应该停下来直接回答，还是继续调平台能力，还是先等 runtime 自己收口”。

`recommended_next_action` 则是在当前 `continuation_state` 下更保守的一步建议：

- 如果已经可直接回答，会优先给 `answer_directly`
- 如果当前卡在报表链入口确认，会优先给 `request_report_entry_confirmation`
- 如果仍需详情 / 对比 / 报表推进，会优先给第一条必要平台动作
- 如果 turn 还在 pending tool loop / artifact commit，会优先给 `wait_for_tool_loop` 或 `finalize_artifact_commit`
- 如果已进入失败态，会优先给 `retry_execution`

`signals` 是这层 mapping 当前依赖的底层事实摘要，例如：

- `workflow_kind=...`
- `retrieval_evidence_count=...`
- `answer_content_present=...`
- `document_focus=single_document|multi_document|unknown`
- `distinct_document_count=...`
- `indexed_document_count=...`
- `has_memory_directory=...`
- `chat_turn_status=...`
- `artifact_commit_status=...`
- `service_handoff_source=...`
- `service_handoff_lane=...`
- `service_handoff_suggested_title_present=true|false`
- `service_handoff_suggested_objective_present=true|false`
- `report_entry_resolved_action=stay_material_service|enter_report_service`

### `Execution-scope Runtime`

适合排查“provider 报错但 artifact 还没落出来”的 execution 级失败。

重点字段：

- `llm_invocation_count`
- `tool_execution_count`
- `failed_tool_execution_count`
- `latest_provider`
- `latest_model`
- `latest_request_id`
- `latest_finish_reason`

如果这里已经出现 `latest_finish_reason=error`，但 dataset output / chat artifact 还不存在，说明失败已经被 execution-scope durable runtime 记录下来了。

### `Dataset Output Runtime`

适合排查报表 / 摘要生成链。

重点字段：

- `dataset_output_id`
- `llm_invocation_count`
- `tool_execution_count`
- `retrieval_evidence_count`
- `output_section_count`
- `provider`
- `model`
- `request_id`
- `finish_reason`
- `provider_failure_kind`
- `provider_failure_message`
- `latency_ms`
- `token_usage`
- `tool_status_summary`

`tool_status_summary` 会直接告诉你 tool loop 当前是：

- `requested > 0`：还有工具调用没完成
- `failed > 0`：至少有工具调用失败
- `completed > 0`：已有工具调用成功返回

如果 provider 在请求阶段或响应解析阶段失败，当前还会额外给出：

- `provider_failure_kind`
- `provider_failure_message`

常见 `provider_failure_kind`：

- `request_failed`：请求没发出去或网络层直接失败
- `request_timeout`：请求超时
- `http_status`：provider 已返回 HTTP 非 2xx
- `response_body_read_failed`：HTTP 已返回，但 body 读取失败
- `invalid_json`：body 读取成功，但 JSON 无法解析
- `invalid_response`：JSON 结构不符合当前协议预期
- `finish_reason_error`：provider 返回了可解析响应，但 `finish_reason=error`

### `Report Plan Runtime`

适合排查报表规划链，尤其是“这份 plan 是不是明确从 chat/report-entry gate 进入的”。

重点字段：

- `report_plan_id`
- `title`
- `status`
- `theme_key`
- `current_ast_version_id`
- `service_handoff_source`
- `report_entry_state`
- `confirmed_report_plan_id`

当前如果 plan 是从 `chat_session report_entry` 确认后创建出来的，常见信号会是：

- `service_handoff_source=chat_session_report_entry`
- `report_entry_state=confirmed`
- `confirmed_report_plan_id=<same plan id>`

当前这类 plan 一旦已经有 `current_ast_version_id`，更常见的下一步动作会是：

- `generate_report_output`

### `Report Render Runtime`

适合排查报表渲染链，尤其是“render output 是不是沿用了同一条 report-entry provenance”。

重点字段：

- `report_render_output_id`
- `report_plan_id`
- `surface`
- `status`
- `has_asset_path`
- `asset_kind`
- `service_handoff_source`
- `report_entry_state`
- `confirmed_report_plan_id`

当前如果 render output 是沿用同一条 report-side provenance 继续生成的，常见信号会是：

- `service_handoff_source=chat_session_report_entry`
- `report_entry_state=confirmed`
- `confirmed_report_plan_id=<parent plan id>`

当前 render 成功且已有 `asset path` 时，更常见的下一步动作会回到：

- `answer_directly`

### `Latest Assistant Turn`

适合排查 chat session 主链，尤其是“provider 已响应，但 assistant message 还没真正完成持久化”的窗口。

重点字段：

- `assistant_message_id`
- `turn_id`
- `status`
- `stream_mode`
- `stream_status`
- `provider_status`
- `artifact_commit_status`
- `tool_loop_status`
- `provider_request_id`
- `finish_reason`
- `provider_failure_kind`
- `provider_failure_message`
- `provider_requested_at`
- `provider_responded_at`
- `first_token_at`
- `stream_completed_at`
- `artifact_commit_ready_at`
- `artifact_commit_failure_source`
- `tool_calls_emitted_at`
- `tool_loop_settled_at`
- `assistant_message_persisted_at`
- `completed_at`
- `tool_status_summary`

时间点解释：

- `provider_requested_at`：provider 请求发出
- `provider_responded_at`：provider 已返回；如果失败发生在请求发出前或超时，这里会保持为空
- `first_token_at`：streaming 模式下首个 token 对外可见的时间点
- `stream_completed_at`：streaming 模式下流本身已经结束；不等于 artifact 已持久化
- `artifact_commit_ready_at`：assistant artifact 已进入可提交窗口，接下来才可能真正落库
- `tool_calls_emitted_at`：tool call 集已经从 provider 响应中落出
- `tool_loop_settled_at`：当前这轮 tool loop 已进入终态；成功或失败都算 settled
- `assistant_message_persisted_at`：assistant message 已真正落库
- `completed_at`：turn 整体完成

`stream_status` 解释：

- `not_requested`：当前 turn 不是 streaming 模式
- `pending`：streaming 已进入请求阶段，但流还未结束
- `completed`：流已经结束
- `failed`：流以失败终止

`tool_loop_status` 解释：

- `not_requested`：这一轮没有 tool loop
- `pending`：tool call 已发出，但至少仍有一个请求未完成
- `completed`：tool loop 已成功收口，没有待执行工具
- `failed`：tool loop 已终态失败，没有待执行工具

`artifact_commit_status` 解释：

- `not_ready`：provider / stream 还没走到可以提交 assistant artifact 的阶段
- `pending`：artifact 已可提交，但 assistant message 还没真正持久化
- `failed`：artifact 已进入可提交窗口，但 turn 已终态失败且 assistant message 没有落库
- `completed`：assistant artifact 已完成持久化

如果当前 turn 的 provider 失败原因已被 durable runtime 记录，还会补充：

- `provider_failure_kind`
- `provider_failure_message`

这两项和 dataset output runtime 的含义一致，适合快速区分“请求压根没成功发出去”和“provider 已响应但 payload/finish_reason 有问题”。

如果 `artifact_commit_ready_at` 已有值，但 `assistant_message_persisted_at` 为空，说明当前正处在 artifact commit 窗口里，还没有真正完成落库。

如果 turn 已 `failed`，同时 `artifact_commit_status=failed`，说明 provider / stream 已经把可提交内容准备好了，但失败发生在 assistant artifact 持久化之后的最后一段窗口，而不是 provider 阶段本身。

`artifact_commit_failure_source` 当前会暴露少量稳定来源：

- `workflow_event_recovery_persist`
- `response_ready_session_update`
- `assistant_message_create`
- `assistant_message_manifest_update`
- `llm_invocation_persist`
- `tool_execution_persist`
- `session_context_update`

其中 `workflow_event_recovery_persist` 表示 provider 已响应，worker 已准备写第一条 append-only 的 recovery event checkpoint，但这条 event 没有成功落库。

其中 `response_ready_session_update` 表示 provider 已响应，但 response-ready session manifest 还没成功写回；这时 assistant artifact 还没创建，不过 failed session manifest 仍会尽量落下 recovery payload，供同一 turn 重放时直接恢复。

这组 source 主要覆盖 provider 已响应之后的持久化窗口；如果失败发生在更后面的 workflow signal / task bookkeeping，session turn 不会被回写成 artifact commit failed。

对于 `workflow_event_recovery_persist`、`response_ready_session_update`、`assistant_message_create`、`assistant_message_manifest_update`、`llm_invocation_persist`、`tool_execution_persist`、`session_context_update` 这几类失败，worker 现在都会优先尝试复用同一 `turn_id` 已经落下的恢复载体。因此重放同一 turn 时，目标行为是不再触发第二次 provider 调用，而是继续补齐 artifact commit 缺口。

如果失败发生在 `assistant_message_create` 这一步之后，但 response-ready / failed session manifest 已经写下 recovery payload，worker 也会直接从 `last_turn.recovery` 重建 assistant artifact，而不是重新打 provider。

当前仍然无法恢复的窗口，是 response-ready session manifest 写回之前的失败；那时系统既没有 assistant artifact，也没有 recovery payload，只能重新走 fresh generate 路径。

## 推荐排查顺序

1. 先看 `Execution-scope Runtime`，判断有没有 execution 级失败。
2. 再看 `Dataset Output Runtime` 或 `Latest Assistant Turn`，确认具体主链卡在哪。
3. 最后再下钻 JSON，查看 `llm_invocations`、`tool_executions`、`chat_messages`、`output_manifest_view` 等完整细节。

## 当前定位

这份 pretty 输出是给运维 / 联调用的高信号入口，不替代完整 JSON。摘要负责快速定位，完整 JSON 负责精确回放与字段核对。
