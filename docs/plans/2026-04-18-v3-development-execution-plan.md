# AI Data Platform V3 Development Execution Plan

## Purpose

本计划不是替代架构蓝图或 ADR，而是把 V3 的目标架构拆成可以连续执行、连续验收的开发切片。

当前执行时还应同时遵守：

- [2026-04-19-v3-architecture-constraints.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-19-v3-architecture-constraints.md>)

当前执行时还应同时参考，但不机械照搬：

- `C:/Users/soulzyn/Desktop/codex/ai-data-platform/docs/plans/2026-04-21-model-facing-platform-capability-spec.md`

适用范围：

- V3 Rust 平台核心
- PostgreSQL 主状态建设
- workflow engine 到 worker runtime 的执行链
- 报表规划、渲染、发布主链
- 后续 ingest / retrieval / memory 的复用骨架

## System Goal Recap

V3 不是旧系统代码翻译，而是保留已验证业务闭环后重建新的平台核心。目标能力保持一致：

- 多数据集、多文档、多用户的 AI 问答与知识检索
- 数据集 / 文档 / 会话作用域下的密钥与权限控制
- 长期记忆目录问答、证据召回、按数据集输出
- 报表规划、编辑、渲染、发布、版本回看
- 明确可观测、可审计、可重放的后台任务执行链

## Execution Principles

- 先打通垂直切片，再横向扩展
- PostgreSQL 先成为唯一主状态承载
- workflow transition、event persistence、task persistence 必须原子一致
- worker 先轮询可跑，再接 NATS JetStream 做加速层
- 每一阶段都保留 smoke test 和回归测试
- chat 默认弱编排，只按需供料
- capability 默认 CLI-first，而不是 CLI-only
- 外部规约先作为上游方向输入，不直接冻结为 Rust 侧不可调整契约
- 先把“平台真实运行事实”做稳，再决定模型侧协议最终如何收口

## Planning Inputs

`2026-04-21-model-facing-platform-capability-spec.md` 对当前 V3 有帮助，但它的角色应被明确为：

- 上游产品与模型侧协议方向
- 对 V3 capability surface / evidence protocol / bounded continuation 的设计输入
- 可迭代、可回写、可调整的 draft，而不是直接要求 Rust V3 逐字段硬编码对齐的冻结规范

因此当前计划的执行原则是：

- V3 先稳定主状态、artifact、runtime、recovery、inspect 真相面
- 再把这些真相面映射成模型可消费的 capability class / evidence state
- 如果 Rust 真实执行链与上游文档存在偏差，应优先修改 spec 或补充 mapping，而不是为了追文档表述而破坏当前稳定实现

## Current Baseline

截至 `2026-04-18`，已经具备：

- `platform-api` 可启动，可连本地 Postgres
- schema migration、workflow definition sync、local tenant bootstrap 已打通
- `dataset` 和 `report plan` 的最小读写接口已落地
- `workflow execution` / `workflow event` 查询接口已落地
- `start` 与通用 `signals` 接口已落地
- `report_plan_workflow` 已可从 `Pending -> Running -> Succeeded`

当前最重要的缺口：

- `enqueued_tasks` 还没有形成完整 worker 执行面
- workflow 和后台任务之间缺少独立的持久化任务表与 claim 机制
- report plan / report render 还没有真正的 worker crate 闭环

## Phase 1: Persisted Workflow Tasks

目标：把 workflow transition 产生的 `enqueued_tasks` 变成主状态库中的可追踪任务。

交付物：

- `workflow_tasks` PostgreSQL table
- `domain-model::WorkflowTask` 和 `WorkflowTaskStatus`
- `storage` 中的 workflow task repository
- execution advance 与 task enqueue 的事务化持久化
- API 返回已持久化任务视图

验收标准：

- `POST /v1/workflow-executions/{id}/start` 后能在 `workflow_tasks` 中看到任务
- execution event 和 task insert 在同一事务中提交
- `cargo check`、`cargo test`、本地 smoke test 均通过

## Phase 2: Worker Runtime Skeleton

目标：让任务不只是落库，而是能被最小 worker 消费并回传 signal。

交付物：

- `report-planner-worker` crate
- Postgres 轮询式 task claim / ack / fail 骨架
- `plan_report_ast` 占位执行逻辑
- worker 调用 platform API signal endpoint 回写 `step_completed` / `step_failed`

验收标准：

- 创建 report plan 后，worker 能自动消费任务
- `report_plan_workflow` 可自动从 `Running` 进入 `Succeeded`
- 失败任务可写回 `workflow.step_failed`

## Phase 3: Report Plan Artifacts

目标：把 planner 执行结果从“状态变化”升级成“有产物”的状态变化。

交付物：

- `report_plan_ast_versions` 或等价 artifact 表
- planner 产出占位 AST
- report plan status 与 workflow status 对齐
- execution context 中保存最新 artifact 引用

验收标准：

- planner 完成后能查询到持久化 AST 版本
- `report_plans` 从 `draft` 进入 `planned`

## Phase 4: Report Render Vertical Slice

目标：在同一套 workflow/task/worker 骨架上打通 report render。

交付物：

- `report-render-worker` crate
- `report_render_workflow` 的任务消费逻辑
- `report_outputs` / `published asset` 最小占位表
- render 完成后的 output artifact 持久化

验收标准：

- `report_render_workflow` 可完整完成
- 输出记录可查询、可追踪到执行与计划版本

## Phase 5: Event Bus Acceleration Layer

目标：在 Postgres 轮询已可用的基础上接入 NATS JetStream。

交付物：

- `event-bus` crate 的最小 publish / subscribe 封装
- task enqueue 后发 lightweight event
- worker 订阅加速唤醒，同时保留 DB polling 兜底

验收标准：

- 即使没有 NATS，系统仍能依赖 Postgres 跑通
- 接入 NATS 后 worker 触发更及时，但不改变主状态语义

## Phase 6: Ingest / Retrieval / Memory Expansion

目标：在稳定 workflow/task/worker 骨架上接入其它业务链路。

优先顺序：

- upload ingest
- memory directory
- dataset output
- chat session

要求：

- 不允许每条链路重新自造任务系统
- 必须复用统一的 workflow/task 状态模型

截至 `2026-04-19`，本阶段已经完成的切片：

- `upload_ingest_workflow` API + worker skeleton
- 文档注册、ingest workflow execution 创建与启动
- 两步 `upload_ingest_workflow`：`ingest -> retrieval`
- 文档 lifecycle 从 `received -> extracted -> indexed / failed`
- `document_chunks` 最小 artifact 持久化
- `retrieval-worker` skeleton 与 retrieval metadata 占位写回
- NATS 唤醒 + Postgres polling fallback 验证
- `memory_directory_workflow` API + worker skeleton
- `memory_directories` 最小 artifact 持久化
- `dataset_output_workflow` API + worker skeleton
- `dataset_outputs` 最小 artifact 持久化
- 数据集输出可关联最新 memory directory 快照
- `chat_session_workflow` API + worker skeleton
- `chat_sessions` / `chat_messages` transcript 持久化
- 会话可引用最新 memory directory 与 dataset output 上下文

截至 `2026-04-20`，本阶段又进一步完成了运行语义与读模型收口：

- retrieval evidence manifest / typed view 已稳定，`embedding / recall / evidence locator` 语义已拆开
- memory directory 顶层 manifest 与节点级 `version_no` / `scope` 语义已补齐
- `DocumentLifecycle` 消费面已收紧为专门 enum 视图
- dataset output / chat message 已带结构化 output sections 与 `retrieval_evidence_ids`
- `llm_invocations` 与 `tool_executions` 已成为 durable artifact，并聚合进 dataset output / chat message
- `ChatSessionView` 已可直接返回 `latest_assistant_message_id` 与 `latest_assistant_message`
- 已提供统一 runtime inspect 面：
  - `GET /v1/workflow-executions/{execution_id}/runtime-inspect`
  - `GET /v1/workflow-executions/{execution_id}/llm-invocations`
  - `GET /v1/workflow-executions/{execution_id}/tool-executions`
  - `runtime.inspect` CLI / `runtime-inspect-cli`

本阶段接下来建议按以下顺序推进：

1. 先把“模型侧 capability spec 如何映射到 V3 真实执行面”固定成内部计划，而不是直接进入更多行为重构
2. 在现有 runtime / artifact / inspect 基础上补 `capability_class` 与 `evidence_state` 一类模型侧协议字段
3. 再把 dataset output / chat session 从“typed view 已收紧”推进到“真实 provider / tool / streaming runtime 语义已稳定”
4. 继续减少对 `session_manifest.runtime` 一类重复摘要字段的依赖，让 artifact 聚合读模型成为主消费面
5. 继续把 `runtime.inspect` 打磨成更稳定的 CLI-first 运维 / 调试入口

## Phase 6 Detailed Breakdown

### Slice 6.1: Upload Ingest Skeleton

目标：

- 文档可注册
- 可创建并启动 `upload_ingest_workflow`
- worker 可把文档推进到 `indexed`

交付物：

- `POST /v1/documents`
- `POST /v1/documents/{id}/ingest`
- `ingest-worker`
- 文档 ingest metadata 占位写回

### Slice 6.2: Memory Directory Skeleton

目标：

- 数据集可创建并启动 `memory_directory_workflow`
- worker 可生成最小目录 artifact
- API 可查询 memory directory 产物

交付物：

- `POST /v1/datasets/{dataset_id}/memory-directory-refresh`
- `GET /v1/datasets/{dataset_id}/memory-directories`
- `memory-worker`
- `memory_directories` table

### Slice 6.3: Retrieval Artifact Skeleton

目标：

- 把文档 chunk、embedding 占位结果从 metadata 提升为独立 artifact
- 为后续 hybrid retrieval、evidence recall 留接口

交付物：

- `document_chunks` 或等价表
- retrieval 占位落库
- `retrieval-worker` 可运行 skeleton

当前状态：

- 已完成 `document_chunks` table
- 已完成 `retrieval-worker` runnable skeleton
- 已完成 retrieval evidence manifest / typed view 收口，已能稳定表达 `embedding / recall / evidence locator`
- 当前仍是 placeholder embedding / payload filter，尚未接真实向量索引

### Slice 6.4: Dataset Output Skeleton

目标：

- 以 dataset 为输入产生结构化输出 artifact
- 复用统一 workflow/task/worker 骨架，不与 report workflow 混淆

交付物：

- dataset output API
- `dataset_output_workflow` worker skeleton
- output artifact 占位表

当前状态：

- 已完成 `POST /v1/datasets/{dataset_id}/outputs`
- 已完成 `GET /v1/datasets/{dataset_id}/outputs`
- 已完成 `dataset-output-worker`
- 当前输出已带 typed sections、retrieval evidence 引用、LLM / tool durable traces
- 当前仍未完成更真实的 provider-driven generation 语义

### Slice 6.5: Chat Session Skeleton

目标：

- 明确 chat session workflow 的状态推进
- 为后续 LLM gateway / retrieval / tool traces 预留执行壳

交付物：

- chat session API skeleton
- workflow execution context 约定
- chat worker placeholder

当前状态：

- 已完成 `POST /v1/datasets/{dataset_id}/chat-sessions`
- 已完成 `GET /v1/datasets/{dataset_id}/chat-sessions`
- 已完成 `GET /v1/chat-sessions/{session_id}/messages`
- 已完成 `chat-session-worker`
- 当前会话已带 typed turn runtime、assistant message durable LLM / tool traces、latest assistant 聚合
- 当前仍未完成真正多轮、稳定 streaming、复杂 tool loop 的 runtime contract

## Near-Term Task Breakdown

当前建议的近端执行顺序：

1. 先在 V3 内部固定一份“可调整的模型侧映射计划”：
   - 哪些现有 runtime truth 对应上游 spec 里的 `Material Service / Report Service`
   - 哪些现有运行事实可先映射到 `catalog_memory / supply_only / live_detail / mixed / degraded`
   - 哪些字段应只是读模型映射，哪些才值得进入持久化 contract
2. 优先补“runtime truth -> model-facing protocol”最小映射层，而不是先扩更多业务分支：
   - `capability_class`
   - `evidence_state`
   - 当前允许的下一步动作面
3. 再继续推进 dataset output / chat session 的真实 provider / tool / streaming contract
4. 继续让 session 读模型直接依赖 assistant message artifact 聚合，而不是重复读取 session manifest 摘要
5. 保持 CLI-first 路线，继续完善 `runtime.inspect` 一类统一 inspect / replay 入口

## Model-Facing Reset For Phase 6

截至 `2026-04-21`，Phase 6 的后半段不再只是“补更多 runtime 字段”，而是进入一个更明确的双层收口阶段：

- 底层继续保证 runtime / artifact / recovery / inspect 是真实、可恢复、可查询的执行事实
- 上层开始把这些事实整理成模型侧更稳定的服务协议，而不是继续让模型直接消费底层 domain 细节

当前建议的执行顺序应理解为：

1. 不把外部 capability spec 当成冻结实现，而是当成可调整的对齐目标
2. 先在 V3 内部做最小映射与命名统一
3. 再补真实 provider / tool / streaming 语义
4. 最后再根据真实运行结果，回头收紧 capability surface 和连续执行协议

## Validation Checklist

每个阶段至少保留以下验证：

- `cargo fmt`
- `cargo check`
- `cargo test`
- 本地 HTTP smoke test
- Postgres 查询校验 execution / event / task / artifact 状态

## Not In Scope Yet

以下内容暂不优先：

- 复杂调度策略
- 多 worker lease 竞争优化
- 完整前端工作台
- NATS-only 执行模型
- 生产级发布流水线

先把主状态、任务执行、报表主链做实，再扩展外圈能力。
