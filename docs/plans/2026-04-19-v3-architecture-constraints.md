# DataMax Architecture Constraints

## Purpose

这份文档不是替代主计划，而是把 DataMax 重构过程中必须持续遵守的架构约束单独固定下来。

适用范围：

- `platform-api`
- workflow / task / worker 主执行链
- retrieval / memory / dataset output / chat session
- 后续 tool execution、CLI 能力暴露与模型调用面

这份文档解决的核心问题只有一个：

- 避免系统在后续实现中重新滑回“重聊天编排、弱 artifact、能力埋在 prompt 里”的旧形态

## Constraint 1: Chat Default Weak Orchestration

默认约束：

- `chat_session` 是薄执行壳，不是系统总控中心
- 对话默认只做按需供料、上下文绑定、turn runtime 记录、answer artifact 产出
- 不把复杂业务状态推进塞进 chat prompt 或 chat worker

允许的编排：

- turn 生命周期推进
- tool 调用边界控制
- timeout / retry / idempotency
- runtime / tool trace / artifact 引用持久化

禁止的倾向：

- 让 chat 负责“替系统决定一切下一步”
- 让 retrieval / memory / output 的事实边界退化成 prompt 拼接
- 把多步骤业务流程写成 prompt 内隐逻辑，而不是显式 workflow / task / artifact

设计判断标准：

- 如果某段逻辑属于“执行语义”，应该落在 workflow / worker / tool contract
- 如果某段逻辑属于“事实结果”，应该落在独立 artifact
- 如果某段逻辑只是“本轮回答组织方式”，才允许留在 chat runtime

## Constraint 2: Artifact First, Prompt Second

默认约束：

- evidence、memory snapshot、dataset output、chat answer 都优先建模为可查询 artifact
- prompt 只消费 artifact，不拥有 artifact
- manifest 必须可单独解析、可版本化、可回放

直接后果：

- retrieval 先稳定 `embedding / recall / evidence` schema
- dataset output 显式引用 retrieval evidence
- chat answer 显式引用 dataset output / evidence，而不是只保留自由文本
- memory directory 继续作为独立版本产物，而不是隐式上下文

禁止的倾向：

- 用 prompt 文本替代结构化 evidence
- 用“最新状态默认推断”替代 artifact 引用
- 让 API 只能返回原始 JSON，而没有 typed view

## Constraint 3: Capability Surface Is CLI-First

默认约束：

- 系统能力尽量以 CLI 形式暴露给模型与人工执行
- CLI 是统一操作面，不是唯一实现面
- 核心业务逻辑应该沉在 library / service 层，CLI 只是稳定入口

推荐分层：

1. domain / service / library
2. worker / API / runtime
3. CLI / agent callable surface

CLI 约束：

- 输入输出尽量稳定、结构化、可 smoke
- 人工可复现
- 模型可理解调用边界
- 不依赖 prompt 隐含知识才能正确使用

禁止的倾向：

- 只把逻辑写进脚本，不保留库层实现
- 让系统能力只能通过某个 prompt“间接触发”
- 把 CLI 当成临时调试脚本而不是正式能力入口

## Constraint 4: PostgreSQL Is the Source of Truth

默认约束：

- execution、event、task、artifact 主状态以 PostgreSQL 为准
- NATS JetStream 只负责唤醒加速，不承载唯一真相
- 任一 worker 都必须能在没有 NATS 的情况下依赖 Postgres claim 跑通

禁止的倾向：

- NATS-only 执行模型
- 内存态任务队列成为唯一任务来源
- signal 已发出但数据库无主状态记录

## Constraint 5: Reuse One Workflow / Task Model

默认约束：

- 新链路必须复用现有 workflow / task / signal 骨架
- 不允许针对 retrieval、chat、tool execution 单独再造一套调度系统
- worker 之间只允许在 artifact / signal / durable state 层耦合

禁止的倾向：

- 每条链路自定义不同状态机语义
- 为了“快一点”绕开 workflow task persistence
- 让 worker 直接依赖另一个 worker 的内存态结果

## Constraint 6: Schema and Typed View Must Evolve Together

默认约束：

- manifest schema 升级时，typed view 与 API parser 一起升级
- 新增结构化字段必须补测试与 smoke，不允许只写 raw JSON
- 对旧 manifest 保持必要 fallback，但新语义以新 schema 为准

直接后果：

- schema 版本升级不是装饰字段
- typed view 是稳定消费面
- smoke 不只验 workflow 成功，还要验 manifest 语义对齐

## Near-Term Implications

按当前 Phase 6 状态，最近几步的优先顺序应受这些约束直接约束：

1. 继续把 retrieval / evidence / output / chat answer 的 artifact 链收紧
2. 不继续把 chat 扩成重业务编排器
3. tool execution 先定义稳定 contract 与 CLI 暴露面，再考虑复杂自动执行
4. streaming 先做 turn runtime / event contract，再做更重的传输实现

## Decision Summary

对后续实现的简化判断可以压成四句话：

- chat 默认弱编排，只按需供料
- artifact 优先于 prompt
- capability 默认 CLI-first，而不是 CLI-only
- PostgreSQL 主状态和统一 workflow/task 骨架不能被绕开
