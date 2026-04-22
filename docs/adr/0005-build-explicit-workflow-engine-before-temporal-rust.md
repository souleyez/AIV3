# ADR-0005: 先构建显式工作流引擎，不将 Temporal Rust SDK 作为 V3 核心依赖

## Status
Accepted

## Context

V3 需要 durable execution、任务恢复、显式状态机和可回放能力。Temporal 在 durable execution 领域是正确方向，但当前 Temporal Rust SDK 官方文档仍明确标记为 alpha-stage，workflow worker API 仍不稳定。

本次重构是一次性全量重建，不能把核心执行层建立在当前不稳定 SDK 上。

## Decision

V3 先自建显式工作流引擎：

- workflow definition
- state transition
- retry / compensation
- event persistence
- replay validation

保留未来接入 Temporal 的演化空间，但 V3 第一版不把 Temporal Rust SDK 作为硬依赖。

## Consequences

### Positive

- 可立即获得显式编排和状态治理能力
- 避免把关键执行层押在当前不稳定 Rust SDK 上
- 未来如 Rust SDK 成熟，可在抽象层下切换

### Negative

- 团队需要自行维护 workflow engine 基础能力
- 初期比直接套一个成熟 runtime 需要更多设计工作

### Neutral

- “不直接使用 Temporal Rust SDK”不等于否定 Temporal，而是承认当前生态成熟度边界

## Alternatives Considered

### 直接采用 Temporal Rust SDK

- 放弃：当前 alpha-stage，不适合作为一次性全量重构的主执行依赖

### 继续使用路由函数里的隐式编排

- 放弃：这正是当前系统的核心短板之一

## References

- Temporal docs: https://docs.temporal.io/
- Temporal Rust SDK docs: https://docs.rs/temporalio-sdk/latest/temporalio_sdk/
