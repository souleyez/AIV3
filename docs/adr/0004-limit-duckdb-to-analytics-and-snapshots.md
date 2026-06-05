# ADR-0004: 将 DuckDB 限制为分析与快照层，而非平台主库

## Status
Accepted

## Context

DuckDB 很适合：

- 单机分析
- 快照
- Parquet 读取
- 报表事实层

但 DataMax 的核心需求是可扩展的平台状态治理，而不是把所有东西都塞进一个嵌入式库。

## Decision

DataMax 中 DuckDB 只用于：

- 开发与验证快照
- 离线分析包
- 报表数据打包
- Parquet 辅助分析

不得用于：

- 用户、会话、密钥、任务状态主存储
- 多进程主写路径

## Consequences

### Positive

- 充分发挥 DuckDB 在分析场景的优势
- 避免把平台状态压到不适合的数据库上
- 与 DataFusion + Parquet 的分析层组合更加清晰

### Negative

- 平台数据分层更多，需要更清楚的 ownership 设计

### Neutral

- DuckDB 仍是重要组成部分，但角色从“主库候选”变成“分析引擎成员”

## Alternatives Considered

### DuckDB 兼任分析与事务主库

- 放弃：不符合并发与治理边界

### 完全移除 DuckDB

- 放弃：会失去开发验证和快照分析的高性价比能力

## References

- DuckDB concurrency: https://duckdb.org/docs/current/connect/concurrency
