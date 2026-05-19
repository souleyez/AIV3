# V3 第三方文档解析与按文档问答第一阶段

本文定义第一阶段对接方式：第三方仍使用自己的页面和资料库，V3 只输出“解析、入库、按指定文档回答”的能力。

## 流程

1. 第三方完成文档上传后，调用 V3 文档解析接口。
2. V3 按 `content_url` 拉取文档，落入 V3 本地对象区，并创建关联 `source_id + document_external_id` 的文档记录。
3. V3 启动现有上传解析工作流，解析完成后写入文档切片和检索索引。
4. 第三方可按外部文档 ID 查询解析详情。
5. 第三方发起对话时传入本轮可用的外部文档 ID 列表，并保持同一轮对话使用同一个 `conversation_external_id`。
6. V3 只在这些文档 ID 映射出的内部文档范围内检索，并在进入模型前完成权限过滤。
7. V3 在同一个 `/events` 响应体的 `reply` 字段里返回生成回复、任务状态或确认卡片；第一阶段不需要第三方再调用单独的“取回复”接口。
8. 如果本轮要求生成页面或报告页，V3 可走快速 HTML 交付模式，跳过效果图确认，直接生成可下载 HTML。

## 解析请求

```http
POST /v1/external/channels/{connection_id}/documents/parse
Authorization: Bearer <channel-inbound-token>
Content-Type: application/json
```

请求体见 `third-party-document-parse-request.sample.json`。

关键字段：

- `source_id`：V3 中配置的第三方资料源 ID。
- `dataset_id`：V3 中承接这些资料的目标数据集 ID。
- `document_external_id`：第三方自己的文档 ID，后续对话也传这个 ID。
- `revision_external_id`：可选，第三方文档版本号。
- `content_url`：V3 拉取文档的临时下载地址。生产建议 HTTPS 和短时效签名 URL。
- `idempotency_key`：建议由第三方按“文档 ID + 版本”生成，便于排查。

安全边界：

- `content_url` 生产必须使用 HTTPS；HTTP 只允许本机 smoke。
- V3 不把下载 URL 传入模型，只保存脱敏 URL 和本地对象路径。
- 返回给第三方的解析响应不暴露本地对象路径。

## 解析详情查询

```http
GET /v1/external/channels/{connection_id}/documents/{document_external_id 或 V3 document_id}/parse-detail?source_id={source_id}
Authorization: Bearer <channel-inbound-token>
```

响应包含：

- `latest`：最近一次解析记录；
- `documents`：该外部文档 ID 对应的历史解析记录；
- `lifecycle`：顶层兼容字段，等同于 `latest.lifecycle`；
- `chunk_count`：顶层兼容字段，等同于 `latest.chunk_count`；
- `retrieval_evidence_count`：顶层兼容字段，等同于 `latest.retrieval_evidence_count`；
- `ingest`：顶层兼容字段，等同于 `latest.ingest`，不含原始下载 URL。

兼容说明：推荐路径参数继续使用第三方自己的 `document_external_id`；如果调用方已经保存了解析响应里的 V3 内部 `document.id`，也可以传这个 UUID 查询同一条解析详情。

## 对话按文档限定

第三方发起对话时仍调用原来的通道事件接口。这个接口同时承担两个职责：第三方把用户消息发给 V3，V3 把本次生成结果或处理状态放在响应体 `reply` 中返回给第三方。

```http
POST /v1/external/channels/{connection_id}/events
Authorization: Bearer <channel-inbound-token>
Content-Type: application/json
```

在消息体中增加：

- `conversation_external_id`：同一轮、同一页面会话或同一聊天窗口保持不变；V3 会把它映射为同一个对话上下文。
- `available_document_source_id`：资料源 ID；如果通道配置里已有 `default_source_id`，可省略。
- `available_document_external_ids`：本轮允许 V3 使用的第三方文档 ID 列表。

请求体见 `third-party-chat-with-document-ids.sample.json`。

生成回复响应示例：

```json
{
  "accepted": true,
  "assistant_run_id": "00000000-0000-0000-0000-000000000001",
  "idempotency_key": "third-party:tenant-ext-001:msg-20260518-0001",
  "reply": {
    "target_conversation_external_id": "conv-20260518-001",
    "reply_type": "text",
    "text": "根据这几份采购制度，本周建议重点关注审批超时、授权边界和供应商变更风险。",
    "task_status": "answered",
    "requires_confirmation": false
  }
}
```

如果 V3 已接收但暂时无法立即给出最终文本，会返回 `reply_type=task_status`；如果需要用户确认动作，会返回 `reply_type=requires_confirmation`。第三方页面按 `reply.target_conversation_external_id` 把回复展示回原会话即可。

## 快速 HTML 交付

本次第三方对接中，页面类产物采用快速 HTML 模式：

- `html-anything` 只作为 V3 的模板来源和设计参考；
- 不要求第三方对接调试页、效果图、截图预览或人工确认图；
- V3 直接产出浏览器可打开的 HTML；
- 第三方服务器端带入站 Bearer Token 下载 HTML，再转存到自己的文件库或下载中心。

V3 内部渲染请求使用：

```json
{
  "direct_html": true,
  "background": false
}
```

第三方下载接口：

```http
GET /v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/download
Authorization: Bearer <channel-inbound-token>
```

返回 `text/html; charset=utf-8` 附件。下载权限要求：该 `render_output_id` 必须来自同一个外部通道的 AssistantRun，否则 V3 会拒绝下载。

## 第一阶段验收

- 文档解析接口能下载第三方文档并启动 V3 解析工作流。
- 解析详情接口能按 `source_id + document_external_id` 查到状态。
- 对话请求带 `available_document_external_ids` 时，V3 只从这些文档供料。
- 同一 `conversation_external_id` 的连续消息会作为同一个对话上下文进入模型。
- 第三方发消息到 `/events` 后，能从响应体 `reply` 中拿到文本回复或处理状态。
- 页面类产物可用快速 HTML 模式直接生成，并通过外部通道下载接口交付给第三方。
- 未解析、未授权或未传入的文档不会进入模型上下文。
