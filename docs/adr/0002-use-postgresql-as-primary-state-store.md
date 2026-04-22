# ADR-0002: 使用 PostgreSQL 作为唯一主事务状态库

## Status
Accepted

## Context

当前系统状态分散在 JSON、本地文件、缓存和 DuckDB 中，边界不清晰。V3 需要：

- 强事务的业务状态中心
- 清晰的审计与回放基础
- 可承载用户、数据集、文档元数据、密钥、任务状态、报表资产定义

DuckDB 适合分析场景，但不适合作为平台主事务库。

## Decision

使用 `PostgreSQL` 作为唯一主事务状态库。

以下内容必须进入 PostgreSQL：

- 用户、租户、会话
- 数据集、文档元数据
- 密钥绑定与 grants
- workflow state
- prompt/tool registry
- 报表定义与发布版本

## Consequences

### Positive

- 主状态拥有清晰的一致性边界
- 更适合做审计、回放、权限和资产治理
- 未来多实例扩展时不会受嵌入式数据库限制

### Negative

- 相比本地文件和嵌入式数据库，基础设施复杂度上升
- 需要认真设计 schema、索引和迁移体系

### Neutral

- Redis、Qdrant、DuckDB 仍然保留，但只承担各自专用角色

## Alternatives Considered

### DuckDB 作为主库

- 放弃：官方并发文档明确指出其 native database format 不支持多进程并发写

### 继续使用 JSON + 本地文件

- 放弃：不可治理，不可审计，不适合 V3

## References

- DuckDB concurrency: https://duckdb.org/docs/current/connect/concurrency
