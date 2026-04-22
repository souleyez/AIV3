# AI Data Platform V3 Phase 6 Handoff

## Purpose

这份文档用于在长时间开发过程中做线程切换交接，避免继续消耗聊天上下文。

适用场景：

- 新开线程继续执行 V3 Rust 重构
- 回顾当前已经完成的垂直切片
- 快速恢复本地可运行环境
- 明确下一步应该从哪里接着做

## Current Status

当前工作目录：

- `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3`

当前阶段：

- 已进入 V3 代码骨架落地阶段，不再是纯文档规划
- Phase 1 到 Phase 5 的基础骨架已可用
- Phase 6 的五条最小垂直切片已打通
- Phase 6 已进一步完成 typed runtime / inspect capability 收口

已完成的 Phase 6 切片：

- `upload_ingest_workflow`
- `retrieval artifact skeleton`
- `memory_directory_workflow`
- `dataset_output_workflow`
- `chat_session_workflow`

## What Is Implemented

目前仓库已经具备以下能力：

- `platform-api` 可启动，可自动 migrate schema、sync workflow definitions、bootstrap 本地 tenant
- `workflow_executions`、`workflow_events`、`workflow_tasks` 主状态链完整可查
- Worker 统一复用 Postgres claim + workflow signal 回写模式
- NATS JetStream 作为唤醒加速层已经接入，但主状态仍然以 PostgreSQL 为准
- retrieval / memory / dataset output / chat manifest 已有稳定 typed view
- `llm_invocations`、`tool_executions` 已作为 durable artifact 落库
- `ChatSessionView` 已能直接聚合 `latest_assistant_message`
- 已提供统一 runtime inspect API + CLI 能力面
- dataset output / chat message 的 typed `tool_trace` 已可优先从 durable `tool_executions` hydrate
- output/chat worker 写 durable rows 时已可直接使用 runtime/tool call 结果，不再需要先把 `tool_trace` 明细写进 raw manifest 再反解

已经落地的 artifact / 业务表：

- `documents`
- `document_chunks`
- `retrieval_evidences`
- `memory_directories`
- `dataset_outputs`
- `chat_sessions`
- `chat_messages`
- `llm_invocations`
- `tool_executions`

已经落地的 worker crate：

- `ingest-worker`
- `retrieval-worker`
- `memory-worker`
- `dataset-output-worker`
- `chat-session-worker`

## Verified Baseline

本轮最后一次验证已经通过：

- `cargo fmt --all`
- `cargo test`
- provider runtime API smoke
- PostgreSQL 状态核对

注意：

- 当前 Windows PowerShell 里 `cargo` 不在 PATH
- Rust 工具链应通过 WSL 调用

稳定做法：

```bash
wsl bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check'
```

本轮实际跑通的端到端链路：

```text
ingest -> retrieval -> memory_directory -> dataset_output -> chat_session
```

最后一次 smoke 的关键结果：

- `chat_status = Succeeded`
- `chat_stage = chat_session_completed`
- `message_count = 2`
- `message_roles = [user, assistant]`
- `chat_session` 已关联最新 `memory_directory_id`
- `chat_session` 已关联最新 `dataset_output_id`
- `dataset_output_runtime_inspect_llm_invocation_count = 1`
- `dataset_output_runtime_inspect_tool_execution_count = 1`
- `chat_session_runtime_inspect_llm_invocation_count = 1`
- `chat_session_runtime_inspect_tool_execution_count = 1`
- `chat_session_runtime_inspect_cli_message_count = 2`
- `chat_session_latest_assistant_message_id` 与 assistant message 实际 ID 对齐

补充说明：

- 当前代码已进一步收口到 raw `output_manifest` / `message_manifest` 默认不再写 `tool_trace` 明细
- provider smoke 基线脚本本身已不依赖 raw `tool_trace`，主要校验 typed view 与 durable artifact 是否对齐

额外注意：

- `platform-api` crate 现在有两个 binary：
  - `platform-api`
  - `runtime-inspect-cli`
- 因此启动 API 时必须显式指定：

```bash
wsl bash -lc 'cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo run -p platform-api --bin platform-api'
```

## Important Files

主计划文档：

- [2026-04-18-v3-development-execution-plan.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-18-v3-development-execution-plan.md>)

本次交接文档：

- [2026-04-19-v3-phase6-handoff.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-19-v3-phase6-handoff.md>)
- [2026-04-19-v3-architecture-constraints.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-19-v3-architecture-constraints.md>)
- [2026-04-20-v3-development-summary.md](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/docs/plans/2026-04-20-v3-development-summary.md>)

关键实现入口：

- [Cargo.toml](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/Cargo.toml>)
- [crates/storage/migrations/0001_initial_schema.sql](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/storage/migrations/0001_initial_schema.sql>)
- [crates/contracts/src/lib.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/contracts/src/lib.rs>)
- [crates/storage/src/lib.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/storage/src/lib.rs>)
- [crates/platform-api/src/lib.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/platform-api/src/lib.rs>)
- [crates/platform-api/src/bin/runtime-inspect-cli.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/platform-api/src/bin/runtime-inspect-cli.rs>)
- [crates/tool-registry/src/lib.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/tool-registry/src/lib.rs>)
- [crates/workflow-definitions/src/lib.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/workflow-definitions/src/lib.rs>)
- [crates/chat-session-worker/src/lib.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/chat-session-worker/src/lib.rs>)
- [crates/chat-session-worker/src/main.rs](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/crates/chat-session-worker/src/main.rs>)
- [tmp/phase6-validation/provider-runtime-api-smoke.ps1](</C:/Users/soulzyn/Desktop/codex/ai-data-platform-v3/tmp/phase6-validation/provider-runtime-api-smoke.ps1>)

## Local Environment Notes

本地 Docker 容器已经验证可用：

- `compose-postgres-1`
- `compose-nats-1`
- `compose-redis-1`
- `compose-qdrant-1`
- `compose-minio-1`

当前建议：

- 保持 Docker 容器运行
- Worker / API 进程按需拉起，避免长期挂着

临时联调脚本位于：

- `tmp/phase6-validation/start-platform-api.sh`
- `tmp/phase6-validation/start-ingest-worker-nats.sh`
- `tmp/phase6-validation/start-retrieval-worker-nats.sh`
- `tmp/phase6-validation/start-memory-worker-nats.sh`
- `tmp/phase6-validation/start-dataset-output-worker-nats.sh`
- `tmp/phase6-validation/start-chat-session-worker-nats.sh`

本轮结束前，临时启动的 API 和 worker 进程已停止；Docker 容器未关闭。

## Recommended Next Slice

下一步不要再继续堆新表或新垂直切片，先把已有 output / chat 线的真实 runtime 语义做实。

继续实现前，先遵守：

- chat 默认弱编排，只按需供料
- capability 默认 CLI-first，不把能力埋进 prompt
- PostgreSQL 继续作为 execution / event / task / artifact 主状态

推荐顺序：

1. 继续削弱 `session_manifest.runtime` 的重复职责，让 assistant message artifact 成为更直接的事实来源
2. 把 chat turn runtime 从 placeholder/provider smoke 推进到更明确的 provider / tool / streaming 契约
3. 让 dataset output 围绕 durable `llm_invocations` / `tool_executions` 继续补真实执行语义
4. 如需补运维面，再把 `runtime.inspect` CLI 的使用方式和输出约束补成正式操作文档

原因：

- retrieval evidence、typed view、inspect capability 这一层已经基本收口
- 现在真正还松的，是 output / chat 的真实 provider / tool / streaming 语义
- 继续在这一层收口，比再开新 schema 支线更自然

## Exact Next Task

新线程建议直接从这条开始：

- 把 dataset output / chat session 从“typed view 已收紧”推进到“真实 runtime 语义已稳定”

具体落地方向：

- 继续减少 `session_manifest.runtime` 与 assistant message artifact 之间的重复表达
- 明确 chat turn 的 provider / tool / streaming contract，并优先沉到 typed runtime / durable artifact
- 让 dataset output 的执行语义更多由 durable `llm_invocations` / `tool_executions` 支撑，而不是只停在占位样本
- 保持现有 smoke 可跑，并继续验证 API / CLI inspect 聚合结果一致

边界要求：

- 不重造任务系统
- 不改掉现有 workflow signal 契约
- 不把 NATS 变成唯一执行路径
- 不把复杂业务推进重新塞回 chat prompt

## Suggested New Thread Prompt

如果要切新线程，建议直接用下面这段作为开场：

```text
继续 AI Data Platform V3 Rust 重构。先读 docs/plans/2026-04-20-v3-development-summary.md、docs/plans/2026-04-19-v3-phase6-handoff.md 和 docs/plans/2026-04-18-v3-development-execution-plan.md。当前 upload_ingest、retrieval、memory_directory、dataset_output、chat_session 五条最小垂直切片都已完成，并且 retrieval evidence typed view、durable llm_invocations/tool_executions、latest assistant aggregate、runtime.inspect API/CLI 都已通过 smoke。下一步按交接文档继续把 dataset_output / chat_session 从 typed view 收紧推进到真实 provider / tool / streaming runtime 语义，保持现有 workflow/task/worker 骨架不变。
```

## Decision

建议在当前这个里程碑点切新线程继续。

原因很直接：

- 现在正好是一个完整切片结束点
- 已有清晰交接文档，不会丢上下文
- 下一步是 output / chat runtime 语义收口，属于新的实现批次
- 把新批次放进新线程，后续回溯更干净
