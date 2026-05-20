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
| `dataset_id` | V3 中承接解析文档的目标数据集 UUID。必须传真实 UUID，不是第三方自定义数据集 key 或 slug |

8 服务器当前联调参数：第三方数据集 key `dataset-third-party-main` 对应的 V3 数据集 UUID 是 `7aaab2f7-1daa-4058-b9a1-76d3e54a1e44`。请求体里的 `dataset_id` 必须填这个 UUID，而不是 key 或文档 ID。

请求统一带：

```http
Authorization: Bearer <由我方提供的 token>
Content-Type: application/json
```

通用请求头字段说明：

| 字段 | 场景 | 说明 |
| --- | --- | --- |
| `Authorization` | 所有 V3 第三方接口 | Bearer Token 鉴权头，格式固定为 `Bearer <token>`。不要把 token 暴露到浏览器前端页面。 |
| `Content-Type` | `POST` JSON 请求 | 固定传 `application/json`，否则服务端可能无法按 JSON 解析请求体。 |
| `Accept` | 仅 `/events/stream` | 流式接口建议传 `text/event-stream`，普通 `/events` 和解析接口不用传。 |

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
  "dataset_id": "7aaab2f7-1daa-4058-b9a1-76d3e54a1e44",
  "document_external_id": "doc-20260518-0001",
  "revision_external_id": "v1",
  "title": "采购审批制度.pdf",
  "content_type": "application/pdf",
  "content_url": "https://third-party.example.com/files/doc-20260518-0001.pdf?signature=short-lived",
  "idempotency_key": "third-party-source-main:doc-20260518-0001:v1"
}
```

解析请求字段说明：

| 字段 | 必填 | 类型 | 说明 |
| --- | --- | --- | --- |
| `source_id` | 是 | string | V3 中配置的第三方资料源 ID。本次联调使用 `third-party-source-main`。 |
| `dataset_id` | 是 | UUID string | V3 目标数据集 UUID。本次 8 服务器联调使用 `7aaab2f7-1daa-4058-b9a1-76d3e54a1e44`。 |
| `document_external_id` | 是 | string | 第三方自己的文档 ID，后续查询解析状态和对话供料都使用这个 ID。 |
| `revision_external_id` | 否 | string | 第三方文档版本 ID。重传同一文档的新版本时建议递增，例如 `v1`、`v2`。 |
| `title` | 否 | string | 展示用文件名或标题。不传时 V3 会使用 `document_external_id` 作为标题。 |
| `content_type` | 否 | string | 文件 MIME 类型。不传时 V3 优先使用下载响应里的 `Content-Type`。 |
| `content_url` | 是 | string | V3 拉取文件的短时效 HTTPS 下载地址。不能指向私网、本机、未指定地址或组播地址。 |
| `metadata` | 否 | object | 第三方排查用扩展信息，只放非敏感业务字段；不传时按空对象处理。 |
| `idempotency_key` | 否 | string | 幂等键，建议固定为 `source_id:document_external_id:revision_external_id`，便于重试和排查。 |
| `allow_http_loopback` | 否 | boolean | 仅本机 loopback 冒烟测试使用。生产联调不要传 `true`。 |

兼容说明：请求体同时接受 Java 常用驼峰字段名，例如 `sourceId`、`datasetId`、`documentExternalId`、`revisionExternalId`、`contentType`、`contentUrl`、`idempotencyKey`、`allowHttpLoopback`。文档示例仍使用 V3 标准 snake_case。

注意：`dataset_id` / `datasetId` 的值必须是 V3 数据集 UUID。类似 `dataset-third-party-main` 这样的第三方数据集 key 不能直接放到 `dataset_id`，否则会返回 `422 Unprocessable Entity`。

成功响应关键字段示例：

```json
{
  "accepted": true,
  "source_id": "third-party-source-main",
  "document_external_id": "doc-20260518-0001",
  "revision_external_id": "v1",
  "document": {
    "id": "00000000-0000-0000-0000-000000000101",
    "dataset_id": "7aaab2f7-1daa-4058-b9a1-76d3e54a1e44",
    "title": "采购审批制度.pdf",
    "content_type": "application/pdf",
    "lifecycle": "received",
    "parse_status": "received",
    "parseStatus": "received"
  },
  "workflow_execution": {
    "id": "00000000-0000-0000-0000-000000000201",
    "kind": "upload_ingest_workflow",
    "stage": "queued",
    "status": "pending"
  }
}
```

解析响应字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `accepted` | boolean | V3 是否已接收解析请求。`true` 代表进入后台解析流程，不代表已经可问答。 |
| `source_id` | string | 本次请求使用的第三方资料源 ID。 |
| `document_external_id` | string | 本次请求绑定的第三方文档 ID。 |
| `revision_external_id` | string/null | 本次请求绑定的第三方文档版本 ID；请求未传时可能为空。 |
| `document` | object | V3 内部文档记录摘要。 |
| `document.id` | UUID string | V3 内部文档 ID。第三方通常不用保存，排查时有用。 |
| `document.dataset_id` | UUID string | 文档写入的 V3 数据集 UUID。 |
| `document.title` | string | V3 保存的文档标题。 |
| `document.content_type` | string | V3 保存的文件 MIME 类型。 |
| `document.lifecycle` | string | V3 文档生命周期；刚提交通常是 `received`。 |
| `document.parse_status` | string | snake_case 解析状态字段，供非 Java 调用方读取。 |
| `document.parseStatus` | string | camelCase 解析状态字段，供 Java/前端调用方读取。 |
| `workflow_execution` | object | 本次解析对应的后台工作流摘要。 |
| `workflow_execution.id` | UUID string | 后台工作流 ID，排查队列和失败原因时使用。 |
| `workflow_execution.kind` | string | 工作流类型；解析上传文档通常是 `upload_ingest_workflow`。 |
| `workflow_execution.stage` | string | 当前处理阶段，例如 `queued`、`running`、`completed`。 |
| `workflow_execution.status` | string | 工作流状态，例如 `pending`、`running`、`succeeded`、`failed`。 |

关键要求：

- `content_url` 必须使用 HTTPS 短时效签名下载地址；HTTP 只允许本机 loopback 冒烟测试，并且需要显式传 `allow_http_loopback=true` 或 `allowHttpLoopback=true`。
- `content_url` 不能指向私网、本地网段、未指定地址或组播地址，避免 V3 访问第三方内网资源。
- V3 拉取文档默认超时 30 秒，默认最大 50MB；超过限制会返回下载失败或文件过大错误。
- `document_external_id` 使用第三方自己的文档 ID，后续查询状态和对话问答都继续传这个 ID。
- `content_type` 可选；不传时 V3 会优先使用下载响应里的 `Content-Type`。
- `metadata` 可选，建议只放第三方侧排查需要的非敏感字段。
- 同一份文档重试时，建议保持稳定的 `idempotency_key`，方便排查；如果一次请求已经成功接收，不要用新的幂等键盲目重复提交同一版本。
- V3 不会把下载 URL 传给模型，也不会在响应里暴露 V3 本地对象路径。

成功响应会返回 V3 接收记录、内部文档 ID 和当前解析状态。解析是后台流程，刚提交后可能还在 `received` 或处理中。

## 3. 查询解析详情

第三方可以按自己的文档 ID 查询解析进度和解析结果摘要。

```http
GET https://v3.elepcloud.com/v1/external/channels/{connection_id}/documents/{document_external_id 或 V3 document_id}/parse-detail?source_id={source_id}
Authorization: Bearer <由我方提供的 token>
```

如果同一个 `document_external_id` 有多个版本，可以额外加 `revision_external_id={revision_external_id}` 只查指定版本；查询参数也兼容 `sourceId`、`revisionExternalId`。

解析详情路径和查询字段说明：

| 字段 | 位置 | 必填 | 说明 |
| --- | --- | --- | --- |
| `connection_id` | path | 是 | V3 第三方通道连接 ID，本次联调为 `generic-chat-main`。 |
| `document_external_id` | path | 是 | 推荐传第三方文档 ID；如果只保存了 V3 内部 `document.id`，也可以传 V3 文档 UUID。 |
| `source_id` | query | 建议传 | 第三方资料源 ID。通道配置了默认资料源时可省略，但联调建议显式传。 |
| `revision_external_id` | query | 否 | 指定第三方文档版本；不传时返回该外部文档 ID 的最新记录和历史记录。 |

响应重点看：

- `latest.lifecycle`：最近一次文档状态。常见值包括 `received`、`parsing`、`extracted`、`indexed`、`failed`；其中 `indexed` 表示 V3 已完成解析、切片和检索索引，通常可以进入问答。
- `latest.parse_status` / `latest.parseStatus`：解析器侧状态，通常和生命周期一起用于排查。
- `latest.model_status` / `latest.modelStatus`：模型或结构化解析侧状态；为空时代表本轮没有额外模型解析状态。
- `latest.parse_quality_status` / `latest.parseQualityStatus`：解析质量状态；为空时代表尚未给出质量判断。
- `latest.parse_quality_summary` / `latest.parseQualitySummary`：解析质量摘要，通常用于人工排查，不建议直接展示给终端用户。
- `latest.chunk_count`：已生成的文档切片数量。
- `latest.retrieval_evidence_count`：已进入检索证据的数量。
- `lifecycle` / `chunk_count` / `retrieval_evidence_count`：兼容字段，等同于 `latest` 里的同名信息，方便旧 SDK 直接读取。
- `workflow`：最近一次解析工作流摘要，用于定位队列、阶段和失败原因。
- `documents`：同一外部文档 ID 的历史解析记录。
- `ingest`：解析摘要，不包含原始下载 URL。

解析详情响应字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `source_id` | string | 查询命中的第三方资料源 ID。 |
| `document_external_id` | string | 查询命中的第三方文档 ID。 |
| `lifecycle` | string/null | 兼容字段，等同于 `latest.lifecycle`。 |
| `chunk_count` / `chunkCount` | number/null | 兼容字段，等同于 `latest.chunk_count`。camelCase 用于 Java/前端兼容。 |
| `retrieval_evidence_count` / `retrievalEvidenceCount` | number/null | 兼容字段，等同于 `latest.retrieval_evidence_count`。 |
| `parse_status` / `parseStatus` | string/null | 兼容字段，等同于 `latest.parse_status`。 |
| `model_status` / `modelStatus` | string/null | 兼容字段，等同于 `latest.model_status`。 |
| `parse_quality_status` / `parseQualityStatus` | string/null | 兼容字段，等同于 `latest.parse_quality_status`。 |
| `parse_quality_summary` / `parseQualitySummary` | object/null | 兼容字段，等同于 `latest.parse_quality_summary`。 |
| `latest` | object/null | 最新一条解析记录；判断能否问答时优先看这个对象。 |
| `latest.document_id` | UUID string | V3 内部文档 ID。 |
| `latest.dataset_id` | UUID string | 文档所在 V3 数据集 UUID。 |
| `latest.title` | string | 文档标题。 |
| `latest.content_type` | string | 文件 MIME 类型。 |
| `latest.lifecycle` | string | 文档生命周期；`indexed` 代表通常可以进入问答。 |
| `latest.source_id` | string | 第三方资料源 ID。 |
| `latest.document_external_id` | string | 第三方文档 ID。 |
| `latest.revision_external_id` | string/null | 第三方文档版本 ID。 |
| `latest.parse_status` / `latest.parseStatus` | string | 解析器侧状态。 |
| `latest.model_status` / `latest.modelStatus` | string | 模型或结构化解析侧状态；为空表示没有额外模型解析状态。 |
| `latest.chunk_count` | number | 已生成的文档切片数量。 |
| `latest.retrieval_evidence_count` | number | 已进入检索证据的数量。 |
| `latest.ingest` | object | 解析摘要，不包含原始下载 URL。 |
| `latest.workflow` | object/null | 最近解析工作流摘要，排查队列、阶段和失败原因时使用。 |
| `latest.created_at` | ISO datetime | V3 创建文档记录的时间。 |
| `latest.updated_at` | ISO datetime | V3 最近更新文档记录的时间。 |
| `documents` | array | 同一外部文档 ID 的历史解析记录，元素结构与 `latest` 相同。 |
| `ingest` | object/null | 兼容字段，等同于 `latest.ingest`。 |
| `workflow` | object/null | 兼容字段，等同于 `latest.workflow`。 |

推荐路径参数继续使用第三方自己的 `document_external_id`；如果调用方保存的是解析响应里的 V3 内部 `document.id`，也可以传这个 UUID 查询同一条解析详情。

建议以 `latest.lifecycle=indexed` 作为“可问答”的完成态；同时确认 `chunk_count` 和 `retrieval_evidence_count` 已经有值，说明文档已切片并进入检索证据。联调页面可以每 2-5 秒轮询一次，直到状态进入 `indexed` 或 `failed`；超过业务等待时间后提示用户稍后重试。

## 4. 对话时传入可用文档 ID，并接收 V3 回复

第三方页面发起对话时，在消息体里带上本轮允许使用的文档 ID 列表。需要一次性拿到完整回复时调用 `/events`；需要边生成边展示时调用 `/events/stream`。两个接口使用同一套鉴权、请求体和幂等规则。

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

对话请求字段说明：

| 字段 | 必填 | 类型 | 说明 |
| --- | --- | --- | --- |
| `platform` | 建议传 | string | 第三方平台类型，本次通用聊天联调建议传 `generic_chat`。未传时使用通道配置。 |
| `tenant_external_id` | 是 | string | 第三方租户、客户或组织 ID，用于隔离第三方侧上下文。 |
| `bot_external_id` | 是 | string | 第三方侧机器人或应用 ID，用于区分不同入口。 |
| `conversation_external_id` | 是 | string | 第三方会话 ID。同一聊天窗口必须保持一致，V3 用它串联多轮上下文。 |
| `thread_external_id` | 否 | string | 第三方消息线程 ID。没有线程概念时不用传。 |
| `sender_external_id` | 是 | string | 第三方用户 ID，代表本轮消息发送人。 |
| `message_external_id` | 是 | string | 第三方消息 ID。每条消息建议稳定唯一。 |
| `message_type` | 建议传 | string | 消息类型；默认 `text`。支持 `text`、`image`、`file`、`audio`、`video`、`card`、`event`、`unknown`。 |
| `text` | 文本消息必填 | string | 用户输入文本。非文本消息可为空，但第一阶段问答建议使用文本。 |
| `available_document_source_id` | 文档问答建议传 | string | 本轮允许供料的第三方资料源 ID。传文档列表时建议显式传。 |
| `available_document_external_ids` | 文档问答建议传 | string[] | 本轮允许 V3 使用的第三方文档 ID 列表。只传本轮可见文档，不要传全库。 |
| `mention_external_user_ids` | 否 | string[] | 被 @ 的第三方用户 ID 列表；无 @ 时传空数组或省略。 |
| `attachment_refs` | 否 | object[] | 第三方附件引用列表；第一阶段文档问答通常不用传。 |
| `attachment_refs[].attachment_external_id` | 附件存在时必填 | string | 第三方附件 ID。 |
| `attachment_refs[].filename` | 否 | string | 附件文件名。 |
| `attachment_refs[].content_type` | 否 | string | 附件 MIME 类型。 |
| `attachment_refs[].size_bytes` | 否 | number | 附件大小，单位字节。 |
| `attachment_refs[].download_url_redacted` | 否 | string | 脱敏后的下载地址或排查标记；不要传真实长期可用下载 URL。 |
| `idempotency_key` | 建议传 | string | 消息幂等键。第三方重试同一条消息时必须保持不变。 |
| `received_at` | 建议传 | ISO datetime | 第三方侧消息接收时间；不传或格式非法时 V3 使用服务端接收时间。 |

兼容说明：请求体同时接受 Java 常用驼峰字段名，例如 `tenantExternalId`、`botExternalId`、`conversationExternalId`、`threadExternalId`、`senderExternalId`、`messageExternalId`、`messageType`、`mentionExternalUserIds`、`attachmentRefs`、`availableDocumentSourceId`、`availableDocumentExternalIds`、`idempotencyKey`、`receivedAt`。文档示例仍使用 V3 标准 snake_case。

字段补充：

- `platform` 未传时，V3 会使用通道配置的平台值；生产联调仍建议显式传 `generic_chat` 或双方约定的平台值。
- `message_type` 未传时默认为 `text`；支持 `text`、`image`、`file`、`audio`、`video`、`card`、`event`、`unknown`。
- `received_at` 未传或格式非法时，V3 会使用服务端接收时间；生产联调建议传第三方侧消息时间。
- `idempotency_key` 未传时，V3 会按平台、租户和 `message_external_id` 生成兜底值；生产联调建议第三方显式传稳定值，便于重试和排查。
- 传 `available_document_external_ids` 时，必须同时传 `available_document_source_id`，或由 V3 通道配置默认资料源；未解析到的外部文档 ID 会进入 `unresolved_document_external_ids`，不会供料给模型。

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

对话响应字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `accepted` | boolean | V3 是否已接收并处理本轮消息。 |
| `assistant_run_id` | UUID string/null | 本轮助手运行 ID，排查模型调用和产物生成时使用。 |
| `idempotency_key` | string | V3 实际采用的幂等键。 |
| `reply` | object | 第三方页面需要展示或处理的回复对象。 |
| `reply.target_conversation_external_id` | string | 回复应回填到的第三方会话 ID。 |
| `reply.reply_type` | string | 回复类型，决定第三方页面如何渲染。 |
| `reply.text` | string/null | 文本回复或状态说明。 |
| `reply.card` | object/null | 卡片结构数据，仅 `reply_type=card` 时重点使用。 |
| `reply.artifact_links` | string[] | 产物下载链接列表，例如 HTML 下载地址。第三方服务器端带 token 下载后再分发。 |
| `reply.task_status` | string/null | 任务状态，例如 `answered`、`processing`、`failed`。 |
| `reply.requires_confirmation` | boolean | 是否需要用户确认动作。第一阶段不自动执行写动作。 |
| `reply.action_id` | string/null | 待确认动作 ID；对接确认闭环时使用。 |
| `reply.confirmation_id` | string/null | 确认请求 ID；对接确认闭环时使用。 |

常见 `reply_type`：

| `reply_type` | 页面处理方式 |
| --- | --- |
| `text` | 普通助手文本回复，展示 `text`。 |
| `task_status` | 任务状态，例如处理中、失败或等待证据；不要渲染成最终助手回答。 |
| `card` | 卡片结构，展示 `card` 中的摘要和操作提示。 |
| `artifact_link` | 产物链接，通常从 `artifact_links` 里取下载地址，由第三方服务端下载后再分发。 |
| `requires_confirmation` | 需要用户确认动作；第一阶段如果不接写动作闭环，可以展示为待人工确认或转人工处理。 |

### 流式响应接口

如果第三方聊天页面希望像常见 AI 对话一样逐字或分段展示 V3 回复，改用 SSE 流式接口。请求体与上面的 `/events` 完全一致，只需要把地址换成 `/events/stream`，并在请求头声明可接收 `text/event-stream`。

```http
POST https://v3.elepcloud.com/v1/external/channels/{connection_id}/events/stream
Authorization: Bearer <由我方提供的 token>
Content-Type: application/json
Accept: text/event-stream
```

第三方前端收到 `external_channel.delta` 后，把 `delta` 追加到同一个助手气泡；收到 `external_channel.completed` 后，以 `response.reply` 作为最终结果。如果业务暂时不需要流式展示，继续使用 `/events` 即可。

| 事件 | 含义 |
| --- | --- |
| `external_channel.started` | V3 已通过鉴权、连接和幂等校验，并开始处理本轮消息；这是传输状态，不是助手回复。 |
| `external_channel.delta` | 本轮回复的增量文本，第三方页面按顺序追加展示。 |
| `external_channel.completed` | 本轮处理完成；其中 `response` 的结构与 `/events` 的响应体一致。 |
| `error` | 请求失败或处理中断，通常包含 `status`、`error.code`、`error.message` 或 `message`。 |
| `done` | 流结束标记，`ok=true/false`。 |

SSE 事件数据字段说明：

| 事件 | 字段 | 类型 | 说明 |
| --- | --- | --- | --- |
| `external_channel.started` | `connection_id` | string | 当前第三方通道连接 ID。 |
| `external_channel.started` | `conversation_external_id` | string | 当前第三方会话 ID。 |
| `external_channel.started` | `message_external_id` | string | 当前第三方消息 ID。 |
| `external_channel.started` | `idempotency_key` | string | 当前消息幂等键。 |
| `external_channel.started` | `stream` | string | 固定表示流式传输类型，通常为 `sse`。 |
| `external_channel.started` | `status` | string | 流式处理启动状态，通常为 `started`。 |
| `external_channel.delta` | `index` | number | 增量片段序号，第三方按收到顺序追加即可。 |
| `external_channel.delta` | `delta` | string | 本次新增文本片段。 |
| `external_channel.completed` | `assistant_run_id` | UUID string | 本轮助手运行 ID。 |
| `external_channel.completed` | `response` | object | 最终响应体，结构与普通 `/events` 响应一致。 |
| `error` | `status` | number/string | 失败状态码或阶段状态。 |
| `error` | `error.code` | string | V3 错误码。 |
| `error` | `error.message` / `message` | string | 失败说明。 |
| `done` | `ok` | boolean | 流是否正常结束。 |

SSE 响应示例：

```text
event: external_channel.started
data: {"connection_id":"generic-chat-main","conversation_external_id":"conv-20260518-0001","message_external_id":"msg-20260518-0001","idempotency_key":"third-party:tenant-ext-001:msg-20260518-0001","stream":"sse","status":"started"}

event: external_channel.delta
data: {"index":0,"delta":"根据这份采购审批制度，"}

event: external_channel.delta
data: {"index":1,"delta":"本周建议重点关注审批超时、授权边界和供应商变更风险。"}

event: external_channel.completed
data: {"assistant_run_id":"00000000-0000-0000-0000-000000000001","response":{"accepted":true,"reply":{"target_conversation_external_id":"conv-20260518-0001","reply_type":"text","text":"根据这份采购审批制度，本周建议重点关注审批超时、授权边界和供应商变更风险。","task_status":"answered","requires_confirmation":false}}}

event: done
data: {"ok":true}
```

浏览器侧建议用 `fetch` 读取 `ReadableStream`，因为该接口是 `POST` 且需要 JSON 请求体，原生 `EventSource` 只适合 `GET`。不要把 `external_channel.started` 渲染成助手消息；页面只需要展示 `delta`，并在完成时用 `completed.response.reply` 收口。

如果 V3 已接收但暂时无法立即给出最终文本，会返回 `reply_type=task_status`；如果需要用户确认动作，会返回 `reply_type=requires_confirmation`。第三方页面按 `reply.target_conversation_external_id` 把回复展示回原会话即可。

如果后续要把确认动作做成闭环，再对接确认接口：

```http
POST https://v3.elepcloud.com/v1/external/channels/{connection_id}/confirmations
Authorization: Bearer <由我方提供的 token>
Content-Type: application/json
```

第一阶段文档问答和 HTML 交付不强制要求第三方实现确认接口；收到 `requires_confirmation` 时先展示动作摘要，避免自动执行写动作。

确认接口字段说明：

| 字段 | 必填 | 类型 | 说明 |
| --- | --- | --- | --- |
| `assistant_run_id` | 是 | UUID string | 需要确认的助手运行 ID，来自前序回复。 |
| `action_id` | 否 | string | 需要确认的动作 ID；前序回复给出时建议带上。 |
| `confirmation_external_id` | 是 | string | 第三方侧确认记录 ID，便于幂等和审计。 |
| `sender_external_user_id` | 是 | string | 做出确认的第三方用户 ID。 |
| `decision` | 是 | string | 确认结果，取值 `approved` 或 `rejected`。 |
| `comment` | 否 | string | 用户确认或拒绝时填写的备注。 |
| `idempotency_key` | 是 | string | 确认请求幂等键；第三方重试同一次确认时保持不变。 |
| `confirmed_at` | 是 | ISO datetime | 第三方侧完成确认的时间。 |

确认响应字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `accepted` | boolean | V3 是否接收该确认。 |
| `assistant_run_id` | UUID string | 对应的助手运行 ID。 |
| `action_id` | string | 被确认的动作 ID。 |
| `confirmation_state` | string | V3 侧确认状态。 |
| `idempotency_key` | string | V3 实际采用的确认幂等键。 |

注意：

- 同一轮、同一页面会话或同一聊天窗口请保持同一个 `conversation_external_id`；V3 会把它映射为同一个对话上下文。
- 如果本轮只允许问某几份文档，就只传这些 `document_external_id`。
- 未传入、未解析完成、找不到或无权使用的文档，不会进入本轮模型上下文。
- `message_external_id` 和 `idempotency_key` 建议每条消息稳定唯一，方便重试和排查。

## 5. 快速生成 HTML 交付件

本次第三方对接如果用户要“生成页面、说明页、报告页或一页 HTML”，优先走 V3 快速 HTML 交付模式：

- `html-anything` 作为模板来源和设计参考，不作为第三方需要单独对接的系统。
- V3 直接生成浏览器可打开的 `index.html`，跳过调试页、效果图和截图确认。
- HTML 仍走 V3 的模型路由、权限、数据集、证据供料和产物审计边界。
- 生成完成后，V3 可在消息响应 `reply.artifact_links` 中返回下载地址。
- 第三方服务器端使用同一个 Bearer Token 下载 HTML 后，再转存到第三方自己的文件库或下载中心；不要把 V3 token 放进浏览器页面。
- 下载响应是 `text/html; charset=utf-8` 附件，V3 会返回 `Cache-Control: no-store`；`render_output_id` 必须属于当前 `connection_id` 对应的外部通道运行。

V3 内部创建最终 HTML 时使用：

```json
{
  "direct_html": true,
  "background": false
}
```

V3 内部 HTML 渲染字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `direct_html` | boolean | `true` 表示直接生成最终可下载 HTML，不走截图确认或中间调试页。 |
| `background` | boolean | `false` 表示按当前请求同步推进；后台长任务模式由 V3 内部调度控制。 |

第三方下载 HTML：

```http
GET https://v3.elepcloud.com/v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/download
Authorization: Bearer <由我方提供的 token>
```

返回内容是 `text/html; charset=utf-8`，并以附件形式下载，例如 `v3-static-page-{render_output_id}.html`。

HTML 下载字段说明：

| 字段 | 位置 | 说明 |
| --- | --- | --- |
| `connection_id` | path | 第三方通道连接 ID；下载链接必须和产物所属通道一致。 |
| `render_output_id` | path | V3 HTML 渲染产物 ID，通常来自 `reply.artifact_links`。 |
| `Authorization` | header | 第三方服务器端下载时带 Bearer Token。不要在浏览器前端暴露。 |
| `Content-Type` | response header | 固定为 `text/html; charset=utf-8`。 |
| `Content-Disposition` | response header | 附件下载文件名，例如 `v3-static-page-{render_output_id}.html`。 |
| `Cache-Control` | response header | 固定 `no-store`，避免第三方或浏览器缓存敏感产物。 |

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

产物链接响应字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `reply.target_conversation_external_id` | string | HTML 产物所属第三方会话 ID。 |
| `reply.reply_type` | string | 产物场景通常为 `artifact_link`。 |
| `reply.text` | string | 面向第三方页面或服务端的交付说明。 |
| `reply.artifact_links` | string[] | V3 产物下载地址列表；需要第三方服务器端带 token 下载。 |
| `reply.task_status` | string | 任务状态，`answered` 表示本轮产物回复已完成。 |
| `reply.requires_confirmation` | boolean | 是否还需要用户确认。HTML 直接交付通常为 `false`。 |

## 6. 第一阶段验收口径

建议按以下顺序验收：

1. 文档上传后，第三方能调用解析接口，V3 返回 `accepted=true`、V3 内部 `document.id` 和解析工作流信息。
2. 查询解析详情能看到该 `document_external_id`，并最终进入 `latest.lifecycle=indexed`。
3. `latest.chunk_count` 和 `latest.retrieval_evidence_count` 有值后，对话请求带 `available_document_external_ids`，V3 能围绕指定文档回答。
4. 同一 `conversation_external_id` 的连续消息会作为同一个对话上下文进入模型。
5. 第三方发消息到 `/events` 后，能从响应体 `reply` 中拿到文本回复、任务状态或产物链接。
6. 第三方发消息到 `/events/stream` 后，能按 `delta` 追加展示，并以 `completed.response.reply` 收口。
7. 需要页面交付时，V3 能跳过效果图确认直接生成 HTML，并由第三方服务器端下载。
8. 换一个未传入、未解析完成、找不到或无权使用的文档 ID，V3 不应把该文档作为本轮回答依据。

## 7. 常见问题

| 现象 | 优先检查 |
| --- | --- |
| `401 Unauthorized` | `Authorization: Bearer ...` 是否使用我方提供的 token，是否有多余空格或过期 |
| `422 Unprocessable Entity` | 请求体字段类型是否正确；`dataset_id` 必须是 V3 数据集 UUID，不能传 `dataset-third-party-main` 这类外部 key |
| `400 Bad Request` | 请求体字段名、JSON 格式、`content_url`、`source_id`、`dataset_id` 或 `available_document_*` 是否符合要求 |
| `400 external_document_content_url_insecure` | `content_url` 是否为 HTTPS；HTTP 只允许 loopback 冒烟测试 |
| `400 external_document_content_url_private_host` | `content_url` 是否指向私网、本机、未指定地址或组播地址 |
| `400 external_document_too_large` | 文件是否超过默认 50MB，或下载响应 `Content-Length` 是否过大 |
| `404 Not Found` | `connection_id`、`source_id`、`document_external_id` 是否和联调参数一致 |
| `403 Forbidden` | 第三方通道或资料源是否被禁用，目标 `dataset_id` 当前 token 是否有权访问 |
| `405 Method Not Allowed` | 是否误打了 `http://`、是否被重定向后从 `POST` 变成 `GET` |
| 解析一直没进入问答可用 | `latest.lifecycle` 是否到 `indexed`，`chunk_count` 和 `retrieval_evidence_count` 是否有值，`workflow` 是否有失败原因 |
| 能提交但问答没有引用文档 | 文档是否解析完成，`available_document_external_ids` 是否传了正确的第三方文档 ID，`available_document_source_id` 是否传对 |
| 多轮上下文接不上 | 同一轮对话是否保持相同 `conversation_external_id`，每条消息是否使用新的 `message_external_id` |
| 流式页面没有逐段展示 | 是否调用 `/events/stream`，请求头是否带 `Accept: text/event-stream`，浏览器侧是否用 `fetch` 读取 `ReadableStream` |
| HTML 链接打不开 | 第三方是否由服务器端带 Bearer Token 下载，不要让浏览器直接带 V3 token |
| `static_page_html_not_ready` | HTML 产物尚未渲染完成，稍后用同一下载地址重试 |
| 重复提交文档 | 检查 `idempotency_key` 是否按文档 ID 和版本稳定生成 |

## 8. 双方职责边界

- 第三方负责：上传入口、文档下载地址、第三方文档 ID、用户页面、对话请求发起、HTML 下载后的本地转存或分发。
- V3 负责：拉取文档、解析入库、按文档 ID 查询解析详情、按本轮可用文档范围生成回答、快速 HTML 生成和下载出口。
- 第一阶段暂不要求第三方开放完整资料库批量同步接口；后续如果要做批量资料源同步，再对接资料源参数卡。
