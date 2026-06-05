# ADR Index

本目录保存 DataMax 全量 Rust 重构的关键架构决策记录。

## ADR List

- [ADR-0001: 采用 Next.js 前端加 Rust 平台核心的双语言架构](./0001-adopt-nextjs-plus-rust-platform-core.md)
- [ADR-0002: 使用 PostgreSQL 作为唯一主事务状态库](./0002-use-postgresql-as-primary-state-store.md)
- [ADR-0003: 使用 MCP 作为外部工具统一边界](./0003-use-mcp-as-external-tool-boundary.md)
- [ADR-0004: 将 DuckDB 限制为分析与快照层，而非平台主库](./0004-limit-duckdb-to-analytics-and-snapshots.md)
- [ADR-0005: 先构建显式工作流引擎，不将 Temporal Rust SDK 作为 DataMax 核心依赖](./0005-build-explicit-workflow-engine-before-temporal-rust.md)

## Rules

- 重要架构决策必须先形成 ADR，再落代码
- 任何推翻已接受 ADR 的变更都必须新增 superseding ADR
- ADR 是执行边界，不是讨论记录
