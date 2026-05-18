# V3 第三方文档解析与问答对接说明

**版本：** 第一阶段联调版  
**V3 基础地址：** `https://v3.elepcloud.com`  
**适用方式：** 第三方保留自己的页面、资料上传入口和用户体系；V3 提供文档解析、入库、解析状态查询和按指定文档回答能力。

## 1. 联调前由我方提供

请先确认以下参数，样例中的占位值需要替换成正式联调值：

| 参数 | 说明 |
| --- | --- |
| `connection_id` | V3 为本次第三方通道分配的连接 ID |
| `token` | 第三方请求 V3 时使用的 Bearer Token |
| `source_id` | V3 中配置的第三方资料源 ID |
| `dataset_id` | V3 中承接解析文档的目标数据集 ID |

请求统一带：

```http
Authorization: Bearer <由我方提供的 token>
Content-Type: application/json
```

生产联调请直接使用 `https://v3.elepcloud.com/v1/...`。不要先请求 `http://` 再依赖重定向，避免调试工具把 `POST` 改成 `GET` 后出现 `405 Method Not Allowed`。

## 2. 第三方上传文档后发起解析

第三方完成文档上传后，向 V3 发起解析请求。V3 会按 `content_url` 拉取文档，写入指定数据集，并关联第三方自己的 `document_external_id`。

```http
POST https://v3.elepcloud.com/v1/external/channels/{connection_id}/documents/parse
Authorization: Bearer <由我方提供的 token>
Content-Type: application/json
```

示例请求：

```json
{
  "source_id": "third-party-source-main",
  "dataset_id": "dataset-third-party-main",
  "document_external_id": "doc-20260518-0001",
  "revision_external_id": "v1",
  "title": "采购审批制度.pdf",
  "content_type": "application/pdf",
  "content_url": "https://third-party.example.com/files/doc-20260518-0001.pdf?signature=short-lived",
  "idempotency_key": "third-party-source-main:doc-20260518-0001:v1"
}
```

关键要求：

- `content_url` 建议使用 HTTPS 短时效签名下载地址。
- `document_external_id` 使用第三方自己的文档 ID，后续查询状态和对话问答都继续传这个 ID。
- 同一份文档重复请求时，建议保持稳定的 `idempotency_key`，方便排查和去重。
- V3 不会把下载 URL 传给模型，也不会在响应里暴露 V3 本地对象路径。

成功响应会返回 V3 接收记录、内部文档 ID 和当前解析状态。解析是后台流程，刚提交后可能还在 `received` 或处理中。

## 3. 查询解析详情

第三方可以按自己的文档 ID 查询解析进度和解析结果摘要。

```http
GET https://v3.elepcloud.com/v1/external/channels/{connection_id}/documents/{document_external_id}/parse-detail?source_id={source_id}
Authorization: Bearer <由我方提供的 token>
```

响应重点看：

- `latest.lifecycle`：最近一次文档状态，例如 `received`、`extracted`、`failed`。
- `latest.chunk_count`：已生成的文档切片数量。
- `latest.retrieval_evidence_count`：已进入检索证据的数量。
- `documents`：同一外部文档 ID 的历史解析记录。
- `ingest`：解析摘要，不包含原始下载 URL。

当 `chunk_count` 和 `retrieval_evidence_count` 已经有值后，再进入问答验收更稳。

## 4. 对话时传入可用文档 ID，并接收 V3 回复

第三方页面发起对话时，仍调用原来的消息事件接口，并在消息体里带上本轮允许使用的文档 ID 列表。这个接口同时承担两个职责：第三方把用户消息发给 V3，V3 把本次生成回复或处理状态放在响应体 `reply` 中返回给第三方。

```http
POST https://v3.elepcloud.com/v1/external/channels/{connection_id}/events
Authorization: Bearer <由我方提供的 token>
Content-Type: application/json
```

示例请求：

```json
{
  "platform": "generic_chat",
  "tenant_external_id": "tenant-ext-001",
  "bot_external_id": "bot-v3",
  "conversation_external_id": "conv-20260518-0001",
  "sender_external_id": "user-10001",
  "message_external_id": "msg-20260518-0001",
  "message_type": "text",
  "text": "帮我总结这份采购审批制度，并指出本周需要处理的风险。",
  "available_document_source_id": "third-party-source-main",
  "available_document_external_ids": [
    "doc-20260518-0001"
  ],
  "mention_external_user_ids": [],
  "attachment_refs": [],
  "idempotency_key": "third-party:tenant-ext-001:msg-20260518-0001",
  "received_at": "2026-05-18T10:00:00Z"
}
```

生成回复响应示例：

```json
{
  "accepted": true,
  "assistant_run_id": "00000000-0000-0000-0000-000000000001",
  "idempotency_key": "third-party:tenant-ext-001:msg-20260518-0001",
  "reply": {
    "target_conversation_external_id": "conv-20260518-0001",
    "reply_type": "text",
    "text": "根据这份采购审批制度，本周建议重点关注审批超时、授权边界和供应商变更风险。",
    "task_status": "answered",
    "requires_confirmation": false
  }
}
```

如果 V3 已接收但暂时无法立即给出最终文本，会返回 `reply_type=task_status`；如果需要用户确认动作，会返回 `reply_type=requires_confirmation`。第三方页面按 `reply.target_conversation_external_id` 把回复展示回原会话即可。

注意：

- 同一轮、同一页面会话或同一聊天窗口请保持同一个 `conversation_external_id`；V3 会把它映射为同一个对话上下文。
- 如果本轮只允许问某几份文档，就只传这些 `document_external_id`。
- 未传入、未解析完成或无权使用的文档，不会进入本轮模型上下文。
- `message_external_id` 和 `idempotency_key` 建议每条消息稳定唯一，方便重试和排查。

## 5. 快速生成 HTML 交付件

本次第三方对接如果用户要“生成页面、说明页、报告页或一页 HTML”，优先走 V3 快速 HTML 交付模式：

- `html-anything` 作为模板来源和设计参考，不作为第三方需要单独对接的系统。
- V3 直接生成浏览器可打开的 `index.html`，跳过调试页、效果图和截图确认。
- HTML 仍走 V3 的模型路由、权限、数据集、证据供料和产物审计边界。
- 生成完成后，V3 可在消息响应 `reply.artifact_links` 中返回下载地址。
- 第三方服务器端使用同一个 Bearer Token 下载 HTML 后，再转存到第三方自己的文件库或下载中心；不要把 V3 token 放进浏览器页面。

V3 内部创建最终 HTML 时使用：

```json
{
  "direct_html": true,
  "background": false
}
```

第三方下载 HTML：

```http
GET https://v3.elepcloud.com/v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/download
Authorization: Bearer <由我方提供的 token>
```

返回内容是 `text/html; charset=utf-8`，并以附件形式下载，例如 `v3-static-page-{render_output_id}.html`。

响应中的产物链接示例：

```json
{
  "reply": {
    "target_conversation_external_id": "conv-20260518-0001",
    "reply_type": "artifact_link",
    "text": "HTML 已生成，可由第三方服务器下载后交付给用户。",
    "artifact_links": [
      "https://v3.elepcloud.com/v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/download"
    ],
    "task_status": "answered",
    "requires_confirmation": false
  }
}
```

## 6. 第一阶段验收口径

建议按以下顺序验收：

1. 文档上传后，第三方能调用解析接口，V3 返回成功接收。
2. 查询解析详情能看到该 `document_external_id` 的状态、切片数量和检索证据数量。
3. 对话请求带 `available_document_external_ids` 后，V3 能围绕指定文档回答。
4. 同一 `conversation_external_id` 的连续消息会作为同一个对话上下文进入模型。
5. 第三方发消息到 `/events` 后，能从响应体 `reply` 中拿到文本回复或处理状态。
6. 需要页面交付时，V3 能跳过效果图确认直接生成 HTML，并由第三方服务器端下载。
7. 换一个未传入的文档 ID 或不传文档 ID，V3 不应把该文档作为本轮回答依据。

## 7. 常见问题

| 现象 | 优先检查 |
| --- | --- |
| `401 Unauthorized` | `Authorization: Bearer ...` 是否使用我方提供的 token，是否有多余空格或过期 |
| `404 Not Found` | `connection_id`、`source_id`、`document_external_id` 是否和联调参数一致 |
| `405 Method Not Allowed` | 是否误打了 `http://`、是否被重定向后从 `POST` 变成 `GET` |
| 能提交但问答没有引用文档 | 文档是否解析完成，`available_document_external_ids` 是否传了正确的第三方文档 ID |
| 多轮上下文接不上 | 同一轮对话是否保持相同 `conversation_external_id`，每条消息是否使用新的 `message_external_id` |
| HTML 链接打不开 | 第三方是否由服务器端带 Bearer Token 下载，不要让浏览器直接带 V3 token |
| 重复提交文档 | 检查 `idempotency_key` 是否按文档 ID 和版本稳定生成 |

## 8. 双方职责边界

- 第三方负责：上传入口、文档下载地址、第三方文档 ID、用户页面、对话请求发起、HTML 下载后的本地转存或分发。
- V3 负责：拉取文档、解析入库、按文档 ID 查询解析详情、按本轮可用文档范围生成回答、快速 HTML 生成和下载出口。
- 第一阶段暂不要求第三方开放完整资料库批量同步接口；后续如果要做批量资料源同步，再对接资料源参数卡。
