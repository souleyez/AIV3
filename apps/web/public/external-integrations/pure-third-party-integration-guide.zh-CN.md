# V3 纯第三方简单版接口文档

**版本：** 2026-05-25  
**Base URL：** `https://v3.elepcloud.com`  
**鉴权：** `Authorization: Bearer <V3 inbound token>`  
**请求格式：** `Content-Type: application/json`

最小顺序：

1. 文档解析：把普通文档和模板文档解析入 V3。
2. 聊天同步：传用户 ID、会话 ID、文档范围、默认提示词和输出格式；文档范围首次传入后同一会话持续有效。
3. 生成产物：从模板列表选择模板，解析模板，按模板生成报表/HTML 产物。

可选补充：数据库源由 V3 先配置和同步，第三方只查状态并在聊天时传对应数据集范围。

## 1. 文档解析

### 1.1 发起解析

```http
POST /v1/external/channels/{connection_id}/documents/parse
Authorization: Bearer <V3 inbound token>
Content-Type: application/json
```

```jsonc
{
  "source_id": "third-party-source-main",          // 文档源 ID；连接未配置默认文档源时必填
  "dataset_external_id": "workspace-docs-main",    // 第三方稳定数据集/资料库 ID；没有可省略；UUID 也可以，只要是稳定业务分组
  "dataset_title": "默认资料库",                    // 数据集展示名；自动创建数据集时使用
  "document_external_id": "doc-20260520-0001",     // 第三方文档 ID；后续聊天用它指定文档范围
  "revision_external_id": "rev-20260520-01",       // 文档版本 ID；同一文档更新时传新版本
  "title": "采购审批制度.docx",                     // 文档标题；不传则使用 document_external_id
  "content_type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document", // 文件 MIME 类型
  "content_url": "https://third.example.com/files/doc-20260520-0001.docx", // V3 下载文件的 HTTPS 地址
  "metadata": {                                    // 业务元数据；只放非敏感字段
    "category": "policy",                          // 文档分类
    "owner": "采购部"                               // 文档归属部门/人
  },
  "idempotency_key": "parse:doc-20260520-0001:rev-20260520-01" // 幂等键；重复请求不应创建重复任务
}
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `source_id` | 条件必填 | 文档源 ID；连接没有默认文档源时必填 |
| `dataset_external_id` | 否 | 第三方稳定数据集/资料库 ID；用于自动归档。UUID 可以使用，只要它在第三方业务侧是长期复用的稳定分组 ID |
| `dataset_title` | 否 | 数据集名称；自动创建数据集时使用 |
| `document_external_id` | 是 | 第三方文档 ID；聊天时放入 `available_document_external_ids` |
| `revision_external_id` | 否 | 文档版本 ID；用于区分同一文档的不同版本 |
| `title` | 否 | 文档标题 |
| `content_type` | 否 | 文件 MIME 类型；不传时 V3 使用下载响应的 Content-Type |
| `content_url` | 是 | 文件下载地址；生产只接受 HTTPS |
| `metadata` | 否 | 非敏感业务元数据 |
| `idempotency_key` | 否 | 幂等键；建议包含文档 ID 和版本 ID |

`dataset_external_id` 是业务稳定分组，不是每次上传生成的任务 ID、文件 ID 或下载任务 ID。V3 会为每个第三方通道连接建立对应系统账户；该通道解析入库、文档分组移动和对话运行都归属同一个系统账户，普通资料库列表默认不展示系统解析源。多个第三方接入时，不同通道连接会落到不同系统账户，便于隔离和审计。

响应：

```jsonc
{
  "accepted": true,                                // 是否已接收解析任务
  "source_id": "third-party-source-main",          // 文档源 ID
  "document_external_id": "doc-20260520-0001",     // 第三方文档 ID
  "revision_external_id": "rev-20260520-01",       // 第三方文档版本 ID
  "document": {                                    // V3 文档摘要
    "id": "7f4b2c4f-3f64-4f8b-8b9d-111111111111", // V3 内部文档 ID
    "dataset_id": "5af2f8a6-0d3c-4a12-a77a-222222222222", // V3 内部数据集 ID
    "title": "采购审批制度.docx",                   // 文档标题
    "content_type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document", // 文件 MIME 类型
    "lifecycle": "created",                        // 文档生命周期；解析完成后通常变为 indexed
    "parse_status": "queued",                      // 解析状态
    "parse_quality_status": null,                  // 解析质量状态；可能为 ok/attention_required/failed/null
    "created_at": "2026-05-20T10:00:00Z",          // 创建时间
    "updated_at": "2026-05-20T10:00:00Z"           // 更新时间
  },
  "workflow_execution": {                          // 解析工作流摘要
    "id": "workflow-execution-id",                 // 工作流 ID
    "status": "running"                            // 工作流状态
  }
}
```

### 1.2 查询解析结果

```http
GET /v1/external/channels/{connection_id}/documents/{document_external_id}/parse-detail?source_id={source_id}&revision_external_id={revision_external_id}
Authorization: Bearer <V3 inbound token>
```

响应：

```jsonc
{
  "source_id": "third-party-source-main",          // 文档源 ID
  "document_external_id": "doc-20260520-0001",     // 第三方文档 ID
  "lifecycle": "indexed",                          // 文档生命周期；indexed 表示可用于问答
  "chunk_count": 12,                               // 已入库文本片段数
  "retrieval_evidence_count": 12,                  // 可检索证据数
  "parse_status": "succeeded",                     // 解析状态：queued/running/succeeded/failed
  "parse_quality_status": "ok",                    // 质量状态：ok/attention_required/failed/null
  "parse_quality_summary": {},                     // 质量诊断摘要
  "model_status": "ready",                         // 给模型供料状态：ready/processing/failed/unknown
  "ingest": {},                                    // 入库诊断信息
  "workflow": {},                                  // 工作流诊断信息
  "latest": {},                                    // 最新一次解析的完整文档快照
  "documents": []                                  // 匹配到的版本列表
}
```

可问答条件：

| 字段 | 期望值 | 注释 |
| --- | --- | --- |
| `lifecycle` | `indexed` | 文档已完成入库 |
| `chunk_count` | `> 0` | 有可供模型读取的文本片段 |
| `retrieval_evidence_count` | `> 0` | 有可检索证据 |
| `parse_quality_status` | `ok` 或为空 | `attention_required` 表示解析质量需要人工或 VLM 兜底 |
| `model_status` | `ready` | 可供模型使用 |

### 1.3 移动文档分组

第三方需要把已解析文档移动到另一个资料库/分组时调用。V3 不重新下载、不重新解析，只修改该 `document_external_id` 对应文档的归属数据集。

```http
PATCH /v1/external/channels/{connection_id}/documents/{document_external_id}/dataset
Authorization: Bearer <V3 inbound token>
Content-Type: application/json
```

```jsonc
{
  "source_id": "third-party-source-main",       // 文档源 ID；连接已配置默认源时可省略
  "dataset_external_id": "workspace-docs-new",  // 目标第三方稳定数据集/资料库 ID；不存在时 V3 自动创建
  "dataset_title": "新资料库",                   // 目标数据集展示名；自动创建时使用
  "revision_external_id": "rev-20260520-01"     // 可选；只移动指定版本。不传则移动同一文档 ID 下所有版本
}
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `source_id` | 条件必填 | 文档源 ID；V3 无法从连接或文档 ID 推断时必填 |
| `dataset_external_id` | 条件必填 | 目标第三方稳定数据集/资料库 ID；和 `dataset_id` 二选一。UUID 可以使用，只要它在第三方业务侧是长期复用的稳定分组 ID |
| `dataset_id` | 条件必填 | 目标 V3 数据集 UUID；和 `dataset_external_id` 二选一 |
| `dataset_title` | 否 | 目标数据集名称；自动创建数据集时使用 |
| `revision_external_id` | 否 | 第三方文档版本 ID；不传则移动同一外部文档 ID 的全部版本 |

移动目标也应是业务稳定分组。若第三方使用 UUID 作为稳定分组 ID，V3 会按该 UUID 建立或复用对应资料库；响应中的 `dataset_id` 会回显实际归属的数据集。

响应：

```jsonc
{
  "accepted": true,                              // 是否已完成移动
  "source_id": "third-party-source-main",        // 文档源 ID
  "document_external_id": "doc-20260520-0001",   // 第三方文档 ID
  "revision_external_id": "rev-20260520-01",     // 本次移动限定的版本 ID；未传则为空
  "dataset_id": "5af2f8a6-0d3c-4a12-a77a-333333333333", // 目标 V3 数据集 ID
  "dataset_external_id": "workspace-docs-new",   // 目标第三方稳定数据集/资料库 ID
  "moved_count": 1,                              // 移动的 V3 文档记录数量
  "previous_dataset_ids": [                      // 移动前的 V3 数据集 ID 列表
    "5af2f8a6-0d3c-4a12-a77a-222222222222"
  ],
  "documents": []                                // 移动后的文档摘要列表
}
```

### 1.4 查询数据库源状态

数据库账号、表映射和同步由 V3 侧配置。第三方只用通道 token 查询已授权数据库源的状态，不传数据库密码，不发 SQL。

```http
GET /v1/external/channels/{connection_id}/database-sources/{source_external_id}/status
Authorization: Bearer <V3 inbound token>
```

响应：

```jsonc
{
  "source_id": "hy-sql-main",                     // V3 数据库源 ID；路径中的 source_external_id
  "connector_kind": "mysql",                      // 数据库连接类型
  "redacted_summary": {                           // 脱敏配置摘要
    "kind": "mysql",                              // 数据库类型
    "database": "hy_sql",                         // 数据库名
    "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL", // 服务端密钥引用，不是密码
    "table_count": 1,                             // 已映射表数量
    "tables": ["bi_traffic_area"]                 // 已映射表名
  },
  "status": {
    "config_valid": true,                         // 配置是否可用
    "dataset": {},                                // 当前默认或显式目标数据集摘要
    "datasets": [],                               // 该数据库源已同步过的目标数据集列表
    "dataset_readiness": {},                      // 数据集问答可用状态
    "table_readiness": [],                        // 各表文档/索引/分块状态
    "recent_sync_runs": [],                       // 最近同步任务；可含 row_failure_groups
    "sync_readiness": {                           // 综合同步可用状态
      "row_failure_groups": []                    // 行转换失败按表/原因聚合
    },
    "semantic_profile": {},                       // 表字段/指标/维度语义摘要
    "health_findings": {}                         // 可给运维看的问题摘要
  }
}
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `connection_id` | 是 | V3 分配的第三方通道 ID |
| `source_external_id` | 是 | V3 已授权给该通道的数据库源 ID |
| `status.sync_readiness.signal` | 否 | `ready` 表示数据库同步数据可用于问答/报表；`sync_running`、`sync_failed`、`no_documents` 表示仍需等待或排查 |
| `status.sync_readiness.row_failure_groups` | 否 | 数据库行转换失败分组；包含 `table`、`reason`、`reported_failed_row_count`、`sample_count`、`sample_source_primary_keys` |
| `status.recent_sync_runs[].row_failure_groups` | 否 | 最近同步任务中的失败行分组，便于判断本次同步哪个表/原因失败较多 |
| `status.dataset_readiness.signal` | 否 | `ready` 表示目标数据集已具备可检索证据 |
| `status.health_findings.items` | 否 | 配置、同步、索引、行转换失败等问题列表 |

## 2. 聊天同步

### 2.1 普通聊天

```http
POST /v1/external/channels/{connection_id}/events
Authorization: Bearer <V3 inbound token>
Content-Type: application/json
```

```jsonc
{
  "platform": "generic_chat",                      // 平台类型；纯第三方默认 generic_chat
  "tenant_external_id": "tenant-ext-001",          // 第三方租户/客户 ID
  "bot_external_id": "bot-v3",                     // 第三方机器人/应用 ID
  "conversation_external_id": "conv-20260520-0001",// 会话 ID；同一会话保持不变
  "thread_external_id": "thread-optional",         // 子线程 ID；没有可省略
  "sender_external_id": "user-10001",              // 用户 ID；用于审计和用户历史上下文
  "sender_display_name": "张三",                   // 用户展示名；没有可省略
  "message_external_id": "msg-20260520-0001",      // 第三方消息 ID；用于幂等
  "message_type": "text",                          // 消息类型；文本用 text
  "text": "请总结本轮文档里的审批风险。",            // 用户问题
  "default_prompt": "请面向业务用户，优先基于本轮文档回答。", // 本轮默认提示词
  "output_format": "rich_text",                    // 输出格式：rich_text/image_text/markdown_table/json
  "render_mode": "normal",                         // 输出模式：normal 普通回答；artifact 产物生成
  "artifact_type": null,                            // 可选：产物类型；静态页推荐传 static_page
  "template": null,                                 // 可选：产物模板；见 3.3
  "available_document_source_id": "third-party-source-main", // 本轮文档源 ID
  "available_document_external_ids": [             // 本轮允许使用的文档 ID 列表
    "doc-20260520-0001"
  ],
  "dataset_external_id": null,                     // 可选：授权单个稳定业务分组；传入后同一会话持续有效
  "dataset_external_ids": [],                      // 可选：授权多个稳定业务分组；有多个分组时用数组
  "documentExternalId": "doc-20260520-0001",        // 可选兼容写法：单文档 ID；有数组时不用传
  "requested_skills": [],                          // 本轮指定 skill；没有传空数组或省略
  "mention_external_user_ids": [],                 // 本条消息 @ 的用户 ID；没有传空数组或省略
  "attachment_refs": [],                           // 附件引用；没有传空数组或省略
  "idempotency_key": "chat:tenant-ext-001:msg-20260520-0001", // 幂等键
  "received_at": "2026-05-20T10:00:00Z"            // 第三方收到消息的时间
}
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `platform` | 是 | 纯第三方默认 `generic_chat` |
| `tenant_external_id` | 是 | 第三方租户/客户 ID |
| `bot_external_id` | 是 | 第三方机器人/应用 ID |
| `conversation_external_id` | 是 | 会话 ID；同一聊天窗口保持不变，用于维持多轮上下文和文档范围授权 |
| `thread_external_id` | 否 | 子线程 ID |
| `sender_external_id` | 是 | 用户 ID；同一用户保持稳定 |
| `sender_display_name` | 否 | 用户展示名 |
| `message_external_id` | 是 | 消息 ID；同一消息重试时保持不变 |
| `message_type` | 是 | `text`、`file`、`image`、`audio`、`video`、`card`、`event`、`unknown` |
| `text` | 文本必填 | 用户输入内容 |
| `default_prompt` | 否 | 本轮默认提示词；会供给模型但不越过权限和证据规则 |
| `output_format` | 否 | `rich_text` 富文本；`image_text` 图文排版；`markdown_table` MD 表格；`json` JSON |
| `render_mode` | 否 | `normal` 普通回答；`artifact` 生成产物 |
| `artifact_type` | 产物生成建议填 | 推荐产物语义字段；静态页传 `static_page` 后，V3 会自动补齐产物模式和图文链路 |
| `template` | 使用模板时填 | 产物模板引用对象；用于结构、版式、字段组织和风格参考，不扩大事实证据范围 |
| `available_document_source_id` | 文档问答建议填 | 本次授权所属文档源 ID；连接配置默认文档源时可省略，但单独传此字段不会授权整源文档回答 |
| `available_document_external_ids` | 文档问答建议填 | 允许 V3 使用的文档 ID；也可用 `documentExternalId` 传单个文档；首次传入后同一 `conversation_external_id` 后续有效 |
| `dataset_external_id` | 分组文档问答建议填 | 第三方稳定业务分组/资料库 ID；传入后表示本会话可使用该分组下的全部文档，同一 `conversation_external_id` 后续有效；UUID 也可以使用，只要它在第三方业务侧是稳定分组 ID |
| `dataset_external_ids` | 多分组文档问答建议填 | 第三方稳定业务分组/资料库 ID 数组；一个工作区选择多个分组时使用。兼容别名：`datasetExternalIds`、`availableDatasetExternalIds` |
| `requested_skills` | 否 | 本轮 skill 列表 |
| `mention_external_user_ids` | 否 | 被 @ 的第三方用户 ID |
| `attachment_refs` | 否 | 附件引用列表 |
| `idempotency_key` | 是 | 幂等键 |
| `received_at` | 是 | ISO 8601 时间 |

`dataset_external_id` / `dataset_external_ids` 可以和 `available_document_external_ids` 同时传，V3 会按并集合并授权：分组内文档整组生效，分组外的显式文档也生效，已经包含在分组内的显式文档自动去重。如果只想授权具体少数文档，应只传 `available_document_external_ids` 或 `documentExternalId`，不传分组字段。第三方内部读权限由第三方在传入这些范围前完成判断；V3 按本轮/本会话传入的文档或分组范围供料，不会因为只传 `available_document_source_id` 自动扩大到整源文档。

响应：

```jsonc
{
  "accepted": true,                                // V3 是否接收本条消息
  "assistant_run_id": "assistant-run-id",          // V3 本次运行 ID
  "idempotency_key": "chat:tenant-ext-001:msg-20260520-0001", // 幂等键
  "reply": {                                      // 回复对象
    "target_conversation_external_id": "conv-20260520-0001", // 要回写的会话 ID
    "reply_type": "text",                         // 回复类型：text/task_status/card/artifact_link/requires_confirmation
    "text": "本轮文档显示...",                    // 回复正文
    "card": null,                                 // 卡片内容；没有为 null 或省略
    "artifact_links": [],                         // 产物链接；没有为空数组
    "task_status": null,                          // 任务状态；非任务回复为空
    "requires_confirmation": false,               // 是否需要用户确认
    "action_id": null,                            // 需要确认或回调的动作 ID
    "confirmation_id": null                       // 确认 ID
  }
}
```

### 2.2 流式聊天

```http
POST /v1/external/channels/{connection_id}/events/stream
Authorization: Bearer <V3 inbound token>
Content-Type: application/json
```

请求体与 `POST /events` 相同。

### 2.3 数据接入/入库分析请求

第三方接口无需新增字段。客户可以在普通聊天消息里直接提出数据接入、入库、建表、字段映射、清洗、schema、ETL、导入或数据库分析需求，例如：

```jsonc
{
  "platform": "generic_chat",
  "tenant_external_id": "tenant-ext-001",
  "bot_external_id": "bot-v3",
  "conversation_external_id": "conv-20260520-0001",
  "sender_external_id": "user-10001",
  "message_external_id": "msg-data-20260520-0001",
  "message_type": "text",
  "text": "帮我接入这份考勤表并入库分析字段，重点看缺勤和工时长短。",
  "available_document_source_id": "third-party-source-main",
  "available_document_external_ids": ["attendance-202605.xlsx"],
  "idempotency_key": "data-ingestion:tenant-ext-001:msg-data-20260520-0001",
  "received_at": "2026-05-20T10:20:00Z"
}
```

V3 只会把已由 V3 选中或已授权可见的文档、文件、数据集、数据库源预览交给固定 Cloudflare Codex 任务；不会把原始数据库 URL、凭据、完整表 dump 或无限制本地路径放进任务包。

当服务端启用 `data_ingestion_analysis` 固定能力时，符合条件的请求会返回现有 `task_status` 形态：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `task_status` |
| `reply.task_status` | `data_ingestion_analysis_queued` |
| `reply.card.type` | `v3_data_ingestion_analysis` |
| `reply.card.codex_host_workflow_execution_id` | 固定分析任务 ID |

若没有选中或上传可分析的数据源/表格/文档，V3 会返回 `data_ingestion_analysis_source_required`，提示第三方先补充资料范围。凭据请求、生产表写入、覆盖导入、schema 迁移、公开 API/auth/请求响应字段变更都会转人工确认，不会自动执行。

## 3. 生成产物（报表）

### 3.1 模板列表字段

第三方页面或第三方 AI 只需要保留下面这些模板字段；生成时把选中的模板带入 3.3。

```jsonc
[
  {
    "template_document_external_id": "tpl-weekly-report-001", // 模板文档 ID
    "source_id": "third-party-source-main",                   // 模板所属文档源 ID
    "revision_external_id": "rev-template-01",                // 模板版本 ID
    "title": "周报模板.docx",                                  // 模板名称
    "output_type": "static_page",                             // 产物类型：static_page/html/report/any
    "description": "采购审批周报模板",                          // 模板说明
    "tags": ["周报", "采购", "风险"]                            // 模板标签
  }
]
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `template_document_external_id` | 是 | 模板文档 ID；必须先按 1.1 解析 |
| `source_id` | 是 | 模板所属文档源 ID |
| `revision_external_id` | 否 | 模板版本 ID |
| `title` | 是 | 模板展示名 |
| `output_type` | 是 | `static_page`、`html`、`report` 或 `any` |
| `description` | 否 | 模板说明 |
| `tags` | 否 | 模板标签 |

### 3.2 模板解析

模板也是文档，用 1.1 的解析接口。建议在 `metadata` 标记模板用途。

```jsonc
{
  "source_id": "third-party-source-main",          // 模板所属文档源 ID
  "dataset_external_id": "workspace-template-main",// 模板数据集 ID
  "dataset_title": "报表模板库",                   // 模板库名称
  "document_external_id": "tpl-weekly-report-001", // 模板文档 ID
  "revision_external_id": "rev-template-01",       // 模板版本 ID
  "title": "周报模板.docx",                         // 模板标题
  "content_type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document", // 模板文件 MIME 类型
  "content_url": "https://third.example.com/templates/tpl-weekly-report-001.docx", // 模板下载地址
  "metadata": {                                    // 模板元数据
    "document_role": "template",                   // 固定传 template，表示这是模板文档
    "template_kind": "report",                     // 模板类型：report/static_page/html
    "output_type": "static_page"                   // 目标产物类型
  },
  "idempotency_key": "parse-template:tpl-weekly-report-001:rev-template-01" // 幂等键
}
```

### 3.3 按模板生成静态页/报表

```http
POST /v1/external/channels/{connection_id}/events
Authorization: Bearer <V3 inbound token>
Content-Type: application/json
```

```jsonc
{
  "platform": "generic_chat",                      // 平台类型
  "tenant_external_id": "tenant-ext-001",          // 第三方租户/客户 ID
  "bot_external_id": "bot-v3",                     // 第三方机器人/应用 ID
  "conversation_external_id": "conv-20260520-0001",// 会话 ID
  "sender_external_id": "user-10001",              // 用户 ID
  "message_external_id": "msg-report-20260520-0001", // 消息 ID
  "message_type": "text",                          // 消息类型
  "text": "根据本轮经营数据和模板生成经营分析静态页。", // 用户产物生成需求
  "default_prompt": "按模板结构输出，结论面向业务负责人。", // 本轮默认提示词
  "artifact_type": "static_page",                  // 推荐：直接声明产物类型；V3 自动进入静态页链路
  "template": {                                    // 推荐：模板作为一级业务字段传入
    "source_id": "third-party-source-main",        // 模板文档源 ID
    "document_external_id": "tpl-weekly-report-001", // 模板文档 ID
    "revision_external_id": "rev-template-01",     // 模板版本 ID
    "mode": "reference",                           // 表示模板作为结构/版式/字段参考
    "output_type": "static_page"                   // 模板目标产物；可省略，默认跟随 artifact_type
  },
  "available_document_source_id": "third-party-source-main", // 本轮文档源 ID
  "available_document_external_ids": [             // 本轮普通业务资料；模板文档会由 template 自动纳入可见范围
    "doc-20260520-0001"
  ],
  "dataset_external_ids": ["workspace-operating-data"], // 可选：本会话授权的业务资料分组
  "idempotency_key": "report:tenant-ext-001:msg-report-20260520-0001", // 幂等键
  "received_at": "2026-05-20T10:10:00Z"            // 消息时间
}
```

生成响应字段同 2.1。新接入推荐使用 `artifact_type + template`：第三方不需要理解内部 `render_mode`、`output_format` 和 `document_template_skill`，V3 会自动映射为现有静态页/Image2/Codex 发布链路。模板文档只作为页面结构、版式风格和字段组织参考，事实内容仍以本会话授权资料和检索证据为准。

`template` 字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `template.source_id` | 建议填 | 模板所属文档源 ID；未单独传 `available_document_source_id` 时，V3 会用它补齐 |
| `template.document_external_id` | 文档模板必填 | 模板文档 ID；兼容 `template_document_external_id`、`documentExternalId` |
| `template.revision_external_id` | 否 | 模板版本 ID |
| `template.mode` | 否 | 推荐传 `reference`，表示作为结构/版式/字段参考 |
| `template.output_type` | 否 | `static_page`、`html`、`report`、`document`、`table`、`image` 或 `any`；不传时跟随 `artifact_type` |
| `template.template_reference_id` | 否 | V3 内置静态页模板引用；使用第三方文档模板时通常不用传 |

兼容旧写法仍然有效：已接入第三方可以继续传 `render_mode: "artifact"`、`output_format: "image_text"`，并在 `requested_skills[].arguments.output_type` 中传 `static_page`。如果同时传了 `template` 和旧 `requested_skills`，V3 会去重，不重复加载同一模板文档。

V3 会先创建静态页草稿并提交 Image2 效果图任务；效果图只用于流式/状态卡片预览，不要求客户确认，也不要求第三方单独拉取图片。若服务端已完整启用 `static_page_image2_data_publish` 固定 Cloudflare Codex 能力，V3 会在效果图预览完成后自动续接生成并发布新的 generated-artifact 页面；若该能力未完整启用，V3 会同步生成一份内置 HTML 静态页并发布为 generated-artifact，本次回复优先返回 `artifact_links[0]`、`card.generated_artifact_url` / `card.public_url`，同时兼容保留 `render_output_id` 和下载/预览地址。

`static_page_image2_data_publish` 只有在 V3 平台任务开关、平台 allowlist、Codex Host agent allowlist、真实执行模式、真实 Codex 执行许可、可信宿主类型和任务工作区都就绪时才算已启用。第三方不需要关心这些内部配置，只需按响应字段判断是否已拿到最终 HTML 或仍在自动发布队列。

若进入静态页/Image2 流水线，响应通常为：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `artifact_link` 或 `task_status` |
| `reply.task_status` | `static_page_published`、`static_page_rendered`、`static_page_image2_auto_publish_pending` 或 `static_page_image_preview_queued` |
| `reply.text` | 给用户展示的排队/处理说明；若已直接生成 HTML，会说明可通过 `card.render_output_id` 或下载链接获取产物 |
| `reply.card.type` | `v3_static_page_image2_pipeline` |
| `reply.card.draft_id` | V3 静态页草稿 ID |
| `reply.card.image_job_id` | Image2 效果图任务 ID |
| `reply.card.render_output_id` | 已直接生成 HTML 时返回；第三方用它查询、预览或下载静态页 |
| `reply.card.html_preview_url` | 已直接生成 HTML 时返回；浏览器 inline 预览地址 |
| `reply.card.html_download_url` | 已直接生成 HTML 时返回；HTML 附件下载地址 |
| `reply.card.generated_artifact_url` / `reply.card.public_url` | 已发布 generated-artifact 时返回；第三方优先把这个链接展示或转存 |
| `reply.card.demo_generated_artifact_publish` | `true` 表示固定 Codex 发布能力未启用时，V3 已用内置 HTML 生成并发布演示可访问页面 |
| `reply.card.direct_html_fallback` | `true` 表示固定 Codex 发布能力未启用，本次已走 V3 内置 HTML 直出兜底 |
| `reply.card.auto_publish_after_preview` | `true` 表示效果图完成后会自动进入固定 Cloudflare Codex 发布链路 |
| `reply.card.codex_auto_publish_ready` | `true` 表示服务端当前已完整启用固定 Codex 自动发布；`false` 表示本次会走内置 HTML 直出兜底 |
| `reply.card.codex_auto_publish_disabled_reason` | `codex_auto_publish_ready=false` 时返回内部诊断原因；第三方通常只用于日志，不需要展示给最终用户 |
| `reply.card.effect_image_confirmation_required` | 固定为 `false`，效果图只作为客户可见预览，不作为阻塞确认点 |
| `reply.card.codex_host_workflow_execution_id` | 初始响应通常为空；效果图预览完成并成功续接后，内部运行事件会记录固定发布任务 ID |

若调用流式接口，V3 会在文本 delta 后额外输出 `external_channel.static_page_effect_image_queued` 事件，事件里的 `card` 与上表一致。若 `card.render_output_id` 已存在，可直接按 3.4 查询/预览/下载；若只有 `draft_id` 和 `image_job_id`，表示当前仍在效果图或自动发布阶段，第三方继续等待 `completed` 响应或后续状态事件即可。效果图是过程预览，不是最终交付物。

固定发布任务完成后，V3 会在现有运行事件/状态表面记录最终发布结果，不需要第三方补发确认请求。最终回复形态仍使用 2.1 的 `reply` 对象：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `artifact_link` |
| `reply.task_status` | `static_page_published` |
| `reply.artifact_links[0]` | 最终 V3 generated-artifact 页面链接，例如 `https://v3.elepcloud.com/generated-artifacts/.../index.html` |
| `reply.card.type` | `v3_static_page_image2_publish_completed` |
| `reply.card.codex_host_workflow_execution_id` | 固定发布任务 ID；初始响应为空时，会在最终事件中补齐 |
| `reply.card.validation_summary` | 口径摘要，如最新快照日期、源行数、明细行数、单位策略和告警；不会包含原始明细行或内部检索日志 |

如果第三方使用 `assistant_run_id` 做轮询/观测，应读取该运行下的最终状态事件 `assistant_run.external_channel_static_page_publish_completed`；事件中的 `public_url` 与 `artifact_links[0]` 是同一个最终页面链接。第三方不需要再针对 Image2 效果图做确认、下载或二次提交。

若已生成静态页/报表/HTML 产物，重点读取：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `artifact_link`、`text` 或 `task_status` |
| `reply.text` | 给用户展示的说明 |
| `reply.artifact_links` | 产物链接数组；静态页优先返回 generated-artifact 页面 URL，兼容返回 HTML 下载地址；模板 HTML 产物会返回 `/v1/external/channels/{connection_id}/html-artifacts/{artifact_id}/files/0` |
| `reply.card` | 可能包含 `render_output_id`、`html_preview_url`、`html_download_url`、`draft_id`、`image_job_id`、产物状态或结构化卡片 |
| `assistant_run_id` | 本次生成运行 ID |

### 3.4 查询、预览、下载产物

模板 HTML 产物直接下载：

```http
GET /v1/external/channels/{connection_id}/html-artifacts/{artifact_id}/files/{file_index}
Authorization: Bearer <V3 inbound token>
```

静态页/报表渲染产物：

```http
GET /v1/external/channels/{connection_id}/static-page-renders/{render_output_id}
Authorization: Bearer <V3 inbound token>
```

```http
GET /v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/preview
Authorization: Bearer <V3 inbound token>
```

```http
GET /v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/download
Authorization: Bearer <V3 inbound token>
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `connection_id` | 是 | V3 分配的第三方通道 ID |
| `artifact_id` | HTML 产物下载必填 | `reply.artifact_links` 中的 HTML artifact ID |
| `file_index` | HTML 产物下载必填 | 文件序号；当前模板 HTML 产物固定传 `0` |
| `render_output_id` | 静态页/报表必填 | V3 生成的静态页/报表渲染 ID |

## 4. 枚举与错误

### 4.1 `output_format`

| 值 | 注释 |
| --- | --- |
| `rich_text` | 富文本回答 |
| `image_text` | 图文排版回答 |
| `markdown_table` | MD 表格回答 |
| `json` | JSON 回答 |

中文值也可传：`富文本`、`图文排版`、`MD表格`、`JSON`。

### 4.2 `render_mode`

| 值 | 注释 |
| --- | --- |
| `normal` | 普通聊天回答 |
| `artifact` | 生成可预览/下载产物 |

### 4.3 错误格式

```jsonc
{
  "code": "external_channel_answer_policy_invalid", // 稳定错误码
  "message": "output_format must be one of rich_text, image_text, markdown_table, or json", // 错误说明
  "details": {                                      // 结构化错误详情
    "reason": "invalid_output_format"               // 具体原因
  }
}
```

常见错误：

| `code` | 注释 |
| --- | --- |
| `external_channel_connection_not_found` | 未找到 `connection_id` |
| `external_channel_disabled` | 通道已停用 |
| `external_channel_event_payload_invalid` | 聊天请求字段不合法 |
| `external_channel_answer_policy_invalid` | `default_prompt`、`output_format` 或 `render_mode` 不合法 |
| `external_document_content_url_invalid` | 文档下载地址不是合法 URL |
| `external_document_content_url_insecure` | 文档下载地址不是 HTTPS |
| `external_document_download_failed` | V3 下载文档失败 |
| `external_document_too_large` | 文档超过大小限制 |
| `html_artifact_not_found` | 找不到 HTML 产物 |
| `html_artifact_file_not_found` | 找不到 HTML 产物文件 |
| `html_artifact_file_unavailable` | HTML 产物文件暂不可下载 |
| `static_page_render_output_not_found` | 找不到产物渲染结果 |
