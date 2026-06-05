# DataMax Repository and Module Layout

## Purpose

本文件定义 DataMax 全量重构后的仓库结构、Rust crate 边界、前端应用边界，以及共享契约的组织方式。原则是：

- 仓库结构直接映射架构层
- 核心领域与框架实现分离
- 契约优先于具体 handler
- 任何模块都不允许“顺手越层”

## Monorepo Layout

```text
ai-data-platform-v3/
├── apps/
│   ├── web/
│   └── docs-site/
├── crates/
│   ├── domain-model/
│   ├── contracts/
│   ├── platform-api/
│   ├── auth-scope/
│   ├── workflow-engine/
│   ├── workflow-definitions/
│   ├── prompt-registry/
│   ├── tool-registry/
│   ├── mcp-gateway/
│   ├── llm-gateway/
│   ├── ingest-worker/
│   ├── retrieval-worker/
│   ├── memory-worker/
│   ├── report-runtime/
│   ├── report-compiler/
│   ├── report-publisher/
│   ├── storage/
│   ├── analytics/
│   ├── event-bus/
│   ├── observability/
│   └── test-fixtures/
├── schemas/
│   ├── openapi/
│   ├── protobuf/
│   └── jsonschema/
├── infra/
│   ├── docker/
│   ├── compose/
│   ├── kubernetes/
│   └── terraform/
├── scripts/
├── docs/
│   ├── architecture/
│   ├── adr/
│   └── validation/
└── Cargo.toml
```

## Apps

### `apps/web`

- `Next.js + TypeScript`
只负责：

- UI
- session 体验
- 页面状态
- 流式展示
- 移动端适配

不负责：

- 业务编排
- 检索裁剪
- 密钥权限决策
- 报表核心生成逻辑

### `apps/docs-site`

- 内部技术文档站点
- 用于 ADR、架构、契约、运行手册可视化

## Core Crates

### `crates/domain-model`

唯一领域对象定义来源：

- User
- Tenant
- Dataset
- Document
- SecretBinding
- SecretGrant
- Workflow
- ReportPlan
- ReportModule
- PublishedReport
- EvidenceChunk

要求：

- 不依赖 axum、sqlx、qdrant 客户端等框架实现
- 只定义领域模型和核心值对象

### `crates/contracts`

统一请求、响应、事件、DTO：

- API DTO
- event payload
- MCP tool schema mapping
- JSON schema / OpenAPI bridge

要求：

- 契约变更先于实现
- 不允许 handler 自己临时定义 ad-hoc JSON

### `crates/platform-api`

平台主 HTTP 服务：

- dataset API
- document API
- chat API
- report API
- publish API
- task status API

内部只依赖服务层接口，不直接写 SQL。

### `crates/auth-scope`

统一授权和作用域：

- 身份认证
- grant 验证
- dataset 可见性
- secretProtected gate
- document scope expansion

### `crates/workflow-engine`

工作流引擎核心：

- workflow instance
- state transition
- retry policy
- compensation hooks
- dead-letter integration
- event persistence

### `crates/workflow-definitions`

各业务流程定义：

- chat_session_workflow
- dataset_output_workflow
- memory_directory_workflow
- upload_ingest_workflow
- report_plan_workflow
- report_render_workflow

### `crates/prompt-registry`

- prompt definitions
- prompt versioning
- active prompt resolution
- prompt rendering context

### `crates/tool-registry`

- internal tool definitions
- MCP tool metadata
- HTTP adapter metadata
- tool authorization policy hooks

### `crates/mcp-gateway`

- MCP client/server integration
- remote server registration
- internal tool to MCP exposure
- transport abstraction

### `crates/llm-gateway`

- provider abstraction
- Responses API integration
- OpenClaw adapter
- streaming normalization
- tool call normalization
- response persistence hooks

## Worker Crates

### `crates/ingest-worker`

- file intake
- OCR/extraction
- title inference
- chunking
- grouping
- metadata extraction

### `crates/retrieval-worker`

- embedding generation
- vector upsert
- hybrid retrieval preparation
- payload filter shaping

### `crates/memory-worker`

- long-term memory indexing
- directory aggregation
- recall optimization

### `crates/report-runtime`

- report plan execution
- data binding orchestration
- chart materialization
- html composition pipeline

### `crates/report-compiler`

- theme compiler
- layout compiler
- PC/mobile target generation

### `crates/report-publisher`

- published asset generation
- version registry update
- rollback
- cache invalidation

## Infrastructure Crates

### `crates/storage`

统一存储抽象：

- PostgreSQL repositories
- Redis adapters
- Object storage adapters
- Qdrant adapters
- DuckDB snapshot adapters

### `crates/analytics`

- DataFusion logical query layer
- Parquet dataset registration
- metric query compiler
- report query assembly

### `crates/event-bus`

- NATS JetStream integration
- publisher/subscriber API
- delivery policy
- replay and dead-letter support

### `crates/observability`

- tracing setup
- metrics instruments
- log correlation
- OTel exporter wiring

### `crates/test-fixtures`

- golden scenarios
- sample datasets
- mock tool adapters
- workflow replay fixtures

## Schema Directory

### `schemas/openapi`

- Platform API public contract

### `schemas/protobuf`

- internal event schemas
- background task contracts

### `schemas/jsonschema`

- prompt context schema
- tool contract schema
- report plan schema

## Dependency Rules

### Allowed Direction

- `apps/web -> platform-api`
- `platform-api -> contracts + auth-scope + workflow-engine + prompt-registry + tool-registry`
- `workflow-engine -> domain-model + contracts + event-bus`
- `workers -> domain-model + contracts + storage + analytics + event-bus`
- `storage -> domain-model`

### Forbidden Direction

- `domain-model -> framework crates`
- `apps/web -> database or queue directly`
- `platform-api -> direct qdrant / duckdb business logic`
- `report editor UI -> published assets directly`
- `worker -> read web-only DTOs`

## Data Ownership

| Module | Owns |
|---|---|
| `platform-api` | external write surface |
| `auth-scope` | policy decisions |
| `workflow-engine` | execution state |
| `storage` | persistence adapters |
| `ingest-worker` | document structural facts |
| `retrieval-worker` | vector/index updates |
| `report-runtime` | report execution semantics |
| `report-publisher` | published asset lifecycle |
| `analytics` | analytical query execution |

## Build and Release Units

- `web`
- `platform-api`
- `worker-bundle`
- `mcp-gateway`
- `analytics-service`

尽量避免每个 crate 都变成独立部署单元。crate 是代码边界，不等于运行进程边界。

## Suggested Runtime Processes

### Process Set A: Web

- `apps/web`

### Process Set B: Core API

- `platform-api`

### Process Set C: Workflow + Tool Gateway

- `workflow-engine`
- `mcp-gateway`
- `llm-gateway`

### Process Set D: Workers

- `ingest-worker`
- `retrieval-worker`
- `memory-worker`
- `report-runtime`
- `report-compiler`
- `report-publisher`

### Process Set E: Analytics

- `analytics`

## Validation Gates

DataMax 仓库必须自带以下门禁：

- Rust format + clippy + deny warnings
- OpenAPI contract diff
- protobuf schema compatibility check
- workflow replay tests
- golden scenario tests
- report render snapshot tests
- mobile/desktop playwright smoke tests

## Definition of Done

- 任一新能力都先落 `contract`
- 任一新工作流都先落 `workflow definition`
- 任一新工具都先落 `tool registry`
- 任一新报表能力都先落 `report AST`
- 任一新页面都不能直接依赖未稳定接口
