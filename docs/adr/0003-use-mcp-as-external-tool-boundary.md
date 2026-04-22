# ADR-0003: 使用 MCP 作为外部工具统一边界

## Status
Accepted

## Context

V3 需要将工具调用层标准化。当前系统内部存在多种能力接入方式：

- 进程内函数调用
- HTTP API
- 供应商定制接入

如果继续维持这种混合方式，未来模型供应商、外部工具、内外部能力复用都会越来越散。

## Decision

使用 `MCP` 作为外部工具统一边界：

- 外部工具与第三方能力优先走 MCP
- 内部能力通过 adapter 暴露为 tool contract
- LLM 层只面向 tool registry 和 MCP gateway

## Consequences

### Positive

- 工具面标准化，便于替换模型供应商和外部工具
- 平台能力可以更容易对外暴露和复用
- 与行业主流 agent/tool 生态保持一致

### Negative

- 需要维护一层额外的 gateway 与 contract 映射
- Rust MCP SDK 当前为 Tier 2，需要避免与具体实现深度绑定

### Neutral

- 平台内部仍允许保留直接函数调用，但不允许直接暴露给模型层

## Alternatives Considered

### 全部保留 HTTP 工具调用

- 放弃：协议表达力不足，难以形成统一 agent/tool 生态

### 继续使用内部自定义 contract

- 放弃：平台未来会持续走向封闭和散乱

## References

- MCP architecture: https://modelcontextprotocol.io/docs/learn/architecture
- MCP SDK tiers: https://modelcontextprotocol.io/docs/sdk
- OpenAI Responses API remote MCP support: https://openai.com/index/new-tools-and-features-in-the-responses-api/
