# ADR-0001: 采用 Next.js 前端加 Rust 平台核心的双语言架构

## Status
Accepted

## Context

DataMax 目标是一次性全量重构 AI Data Platform。系统同时存在两类需求：

- 前端体验需要高频迭代，尤其是聊天 UI、报表编辑器、移动端交互
- 平台核心需要承载复杂状态、长链路编排、检索、权限、报表运行时和后台任务

如果全量使用 TypeScript，平台核心的状态复杂度、并发安全性、长期维护成本较高。
如果全量使用 Rust，则前端产品迭代效率会明显下降，同时 AI 供应商接入生态不占优势。

## Decision

采用双语言架构：

- 前端与 BFF 使用 `Next.js + TypeScript`
- 平台核心、编排、数据处理、报表运行时、后台任务全面使用 `Rust`

## Consequences

### Positive

- 前端继续保持产品开发效率
- 平台核心获得更强的类型安全、并发安全和长期可维护性
- 可以在不牺牲 UI 迭代速度的前提下完成系统级重构

### Negative

- 系统存在双语言工程栈，团队需要维护两套主工具链
- 需要额外定义跨层契约，避免 BFF 与 Rust API 演化失控

### Neutral

- Python 仅保留为边缘实验性适配脚本，不再是主干语言

## Alternatives Considered

### 全量 TypeScript

- 放弃：平台核心状态复杂度过高，无法从根上解决当前运行时问题

### 全量 Rust

- 放弃：前端和产品层开发效率显著下降，不符合体验层高频迭代需求

## References

- axum: https://docs.rs/axum/latest/axum/
- tokio runtime: https://docs.rs/tokio/latest/tokio/runtime/
