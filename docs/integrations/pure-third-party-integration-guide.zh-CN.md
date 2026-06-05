# DataMax 纯第三方简单版接口文档

**版本：** 2026-06-01
**Base URL：** `https://v3.elepcloud.com`  
**鉴权：** `Authorization: Bearer <DataMax inbound token>`
**请求格式：** `Content-Type: application/json`

最小顺序：

1. 文档解析：把普通文档和模板文档解析入 DataMax。
2. 聊天同步：传用户 ID、会话 ID、文档范围、默认提示词和输出格式；文档范围首次传入后同一会话持续有效。
3. 生成产物：从模板列表选择模板，解析模板，按模板生成报表/HTML 产物。

可选补充：第三方可通过数据库源接口登记业务库。第一版外部自助接口只支持 MySQL；已配置服务端 `connection_env` 的业务库可直接进入 DataMax 数据集同步链路，只传原始连接串/账号/密码时 DataMax 不明文落库，会先创建待密钥绑定的业务库记录。数据库对接细节已合并进完整 API 文档的 `11.6 第三方数据库对接`。

## 1. 文档解析

### 1.1 发起解析

```http
POST /v1/external/channels/{connection_id}/documents/parse
Authorization: Bearer <DataMax inbound token>
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
  "content_url": "https://third.example.com/files/doc-20260520-0001.docx", // DataMax 下载文件的 HTTPS 地址
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
| `content_type` | 否 | 文件 MIME 类型；不传时 DataMax 使用下载响应的 Content-Type |
| `content_url` | 是 | 文件下载地址；生产只接受 HTTPS |
| `metadata` | 否 | 非敏感业务元数据 |
| `idempotency_key` | 否 | 幂等键；建议包含文档 ID 和版本 ID |

`dataset_external_id` 是业务稳定分组，不是每次上传生成的任务 ID、文件 ID 或下载任务 ID。DataMax 会为每个第三方通道连接建立对应系统账户；该通道解析入库、文档分组移动和对话运行都归属同一个系统账户，普通资料库列表默认不展示系统解析源。多个第三方接入时，不同通道连接会落到不同系统账户，便于隔离和审计。

响应：

```jsonc
{
  "accepted": true,                                // 是否已接收解析任务
  "source_id": "third-party-source-main",          // 文档源 ID
  "document_external_id": "doc-20260520-0001",     // 第三方文档 ID
  "revision_external_id": "rev-20260520-01",       // 第三方文档版本 ID
  "document": {                                    // DataMax 文档摘要
    "id": "7f4b2c4f-3f64-4f8b-8b9d-111111111111", // DataMax 内部文档 ID
    "dataset_id": "5af2f8a6-0d3c-4a12-a77a-222222222222", // DataMax 内部数据集 ID
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
Authorization: Bearer <DataMax inbound token>
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

第三方需要把已解析文档移动到另一个资料库/分组时调用。DataMax 不重新下载、不重新解析，只修改该 `document_external_id` 对应文档的归属数据集。

```http
PATCH /v1/external/channels/{connection_id}/documents/{document_external_id}/dataset
Authorization: Bearer <DataMax inbound token>
Content-Type: application/json
```

```jsonc
{
  "source_id": "third-party-source-main",       // 文档源 ID；连接已配置默认源时可省略
  "dataset_external_id": "workspace-docs-new",  // 目标第三方稳定数据集/资料库 ID；不存在时 DataMax 自动创建
  "dataset_title": "新资料库",                   // 目标数据集展示名；自动创建时使用
  "revision_external_id": "rev-20260520-01"     // 可选；只移动指定版本。不传则移动同一文档 ID 下所有版本
}
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `source_id` | 条件必填 | 文档源 ID；DataMax 无法从连接或文档 ID 推断时必填 |
| `dataset_external_id` | 条件必填 | 目标第三方稳定数据集/资料库 ID；和 `dataset_id` 二选一。UUID 可以使用，只要它在第三方业务侧是长期复用的稳定分组 ID |
| `dataset_id` | 条件必填 | 目标 DataMax 数据集 UUID；和 `dataset_external_id` 二选一 |
| `dataset_title` | 否 | 目标数据集名称；自动创建数据集时使用 |
| `revision_external_id` | 否 | 第三方文档版本 ID；不传则移动同一外部文档 ID 的全部版本 |

移动目标也应是业务稳定分组。若第三方使用 UUID 作为稳定分组 ID，DataMax 会按该 UUID 建立或复用对应资料库；响应中的 `dataset_id` 会回显实际归属的数据集。

响应：

```jsonc
{
  "accepted": true,                              // 是否已完成移动
  "source_id": "third-party-source-main",        // 文档源 ID
  "document_external_id": "doc-20260520-0001",   // 第三方文档 ID
  "revision_external_id": "rev-20260520-01",     // 本次移动限定的版本 ID；未传则为空
  "dataset_id": "5af2f8a6-0d3c-4a12-a77a-333333333333", // 目标 DataMax 数据集 ID
  "dataset_external_id": "workspace-docs-new",   // 目标第三方稳定数据集/资料库 ID
  "moved_count": 1,                              // 移动的 DataMax 文档记录数量
  "previous_dataset_ids": [                      // 移动前的 DataMax 数据集 ID 列表
    "5af2f8a6-0d3c-4a12-a77a-222222222222"
  ],
  "documents": []                                // 移动后的文档摘要列表
}
```

### 1.4 查询数据库源状态

数据库账号、表映射和同步由 DataMax 侧配置。第三方只用通道 token 查询已授权数据库源的状态，不传数据库密码，不发 SQL。

```http
GET /v1/external/channels/{connection_id}/database-sources/{source_external_id}/status
Authorization: Bearer <DataMax inbound token>
```

响应：

```jsonc
{
  "source_id": "hy-sql-main",                     // DataMax 数据库源 ID；路径中的 source_external_id
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
    "datasets": [],                               // 该数据库源已同步过的目标数据集列表；每项可带 readiness/文档数/证据数
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
| `connection_id` | 是 | DataMax 分配的第三方通道 ID |
| `source_external_id` | 是 | DataMax 已授权给该通道的数据库源 ID |
| `status.sync_readiness.signal` | 否 | `ready` 表示数据库同步数据可用于问答/报表；`sync_running`、`sync_failed`、`no_documents` 表示仍需等待或排查 |
| `status.sync_readiness.row_failure_groups` | 否 | 数据库行转换失败分组；包含 `table`、`reason`、`reported_failed_row_count`、`sample_count`、`sample_source_primary_keys` |
| `status.recent_sync_runs[].row_failure_groups` | 否 | 最近同步任务中的失败行分组，便于判断本次同步哪个表/原因失败较多 |
| `status.dataset_readiness.signal` | 否 | `ready` 表示目标数据集已具备可检索证据 |
| `status.datasets[].readiness.signal` | 否 | 单个目标数据集的可问状态；用于区分默认数据集为空、但历史显式目标数据集已可问的情况 |
| `status.datasets[].indexed_document_count` | 否 | 该目标数据集中来自此数据库源的已索引文档数 |
| `status.datasets[].retrieval_evidence_count` | 否 | 该目标数据集中来自此数据库源的检索证据数 |
| `status.health_findings.items` | 否 | 配置、同步、索引、行转换失败等问题列表 |

### 1.5 创建/更新数据库源

第三方在本地创建或更新业务库时，同步到 DataMax。第一版仅支持 `mysql`。推荐优先传 `connection_env`：即 DataMax 服务器上已经配置好的数据库连接串环境变量名。若只传 `connection_url` / `username` / `password`，DataMax 不会把明文凭据写入 PostgreSQL、日志或模型上下文，只会创建 `pending_secret_binding` 状态的业务库记录，后续由 DataMax 侧绑定服务端密钥后再同步。

```http
POST /v1/external/channels/{connection_id}/database-sources
Authorization: Bearer <DataMax inbound token>
Content-Type: application/json
```

```jsonc
{
  "source_external_id": "db-20260601-0001",      // 第三方业务库 ID；用于幂等创建/更新
  "name": "生产经营库",                           // 业务库展示名
  "connector_kind": "mysql",                      // 第一版只支持 mysql
  "connection_env": "THIRD_PARTY_DB_MAIN_URL",    // 推荐：DataMax 服务器环境变量名；不会返回真实连接串
  "connection_url": null,                         // 可选：原始连接串；若未配置 connection_env，本版只登记为待密钥绑定
  "username": null,                               // 可选：数据库用户名；不会明文落库
  "password": null,                               // 可选：数据库密码；不会明文落库
  "database": "hy_sql",                           // 数据库名；connection_env 模式必填
  "tables": ["bi_traffic_area"],                  // 业务表名白名单；后续画像/同步使用
  "dataset_external_id": "xinbai-operating-analysis", // 可选：同步目标稳定数据集/资料库 ID
  "dataset_title": "新百经营分析数据集",            // 可选：自动创建数据集时使用
  "idempotency_key": "datasource:db-20260601-0001" // 幂等键；建议包含 source_external_id
}
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `source_external_id` | 是 | 第三方稳定业务库 ID；后续聊天可放入 `business_datasource_ids` |
| `name` | 否 | 展示名；不传时使用业务库 ID |
| `connector_kind` | 否 | 第一版只支持 `mysql`；其他类型会返回 `unsupported_connector_kind` |
| `connection_env` | 可用库建议必填 | DataMax 服务器环境变量名；有它时业务库可进入连接测试、画像和同步链路 |
| `connection_url` | 否 | 原始连接串；未同时传 `connection_env` 时只创建待密钥绑定记录，不明文落库 |
| `username` / `password` | 否 | 原始账号密码；不明文落库，不进入模型上下文 |
| `database` | `connection_env` 模式必填 | MySQL 数据库名 |
| `tables` | 否 | 表名白名单；为空表示后续由 DataMax 画像/配置决定 |
| `dataset_external_id` | 否 | 同步目标稳定数据集/资料库 ID |
| `dataset_title` | 否 | 自动创建数据集时使用 |
| `idempotency_key` | 否 | 幂等键 |

响应：

```jsonc
{
  "accepted": true,                              // 是否已接收创建/更新
  "source_external_id": "db-20260601-0001",      // 第三方业务库 ID
  "source_id": "db-20260601-0001",               // DataMax 数据库源 ID；当前与 source_external_id 保持一致
  "source": {
    "id": "db-20260601-0001",                   // DataMax 数据库源 ID
    "name": "生产经营库",                         // 展示名
    "connector_kind": "mysql",                    // 连接类型
    "status": "ready"                            // ready 或 pending_secret_binding
  },
  "redacted_summary": {                          // 脱敏摘要；不含密码或原始连接串
    "kind": "mysql",
    "database": "hy_sql",
    "connection_env": "THIRD_PARTY_DB_MAIN_URL",
    "table_count": 0,
    "tables": []
  },
  "credential_status": "ready",                  // ready 或 pending_secret_binding
  "warnings": []                                 // 非阻断提醒
}
```

## 2. 聊天同步

### 2.1 普通聊天

```http
POST /v1/external/channels/{connection_id}/events
Authorization: Bearer <DataMax inbound token>
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
  "business_datasource_ids": [],                   // 可选：本轮指定业务库 ID；对应 database-sources 的 source_external_id
  "documentExternalId": "doc-20260520-0001",        // 可选兼容写法：单文档 ID；有数组时不用传
  "requested_skills": [],                          // 本轮指定 skill；没有传空数组或省略
  "mention_external_user_ids": [],                 // 本条消息 @ 的用户 ID；没有传空数组或省略
  "attachment_refs": [],                           // 附件引用；可传对象数组，也兼容字符串 URL 数组
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
| `artifact_type` | 产物生成建议填 | 推荐产物语义字段；静态页传 `static_page` 后，DataMax 会自动进入报表页面生成流程 |
| `template` | 使用模板时填 | 产物模板引用对象；用于结构、版式、字段组织和风格参考，不扩大事实证据范围 |
| `available_document_source_id` | 文档问答建议填 | 本次授权所属文档源 ID；连接配置默认文档源时可省略，但单独传此字段不会授权整源文档回答 |
| `available_document_external_ids` | 文档问答建议填 | 允许 DataMax 使用的文档 ID；也可用 `documentExternalId` 传单个文档；首次传入后同一 `conversation_external_id` 后续有效 |
| `dataset_external_id` | 分组文档问答建议填 | 第三方稳定业务分组/资料库 ID；传入后表示本会话可使用该分组下的全部文档，同一 `conversation_external_id` 后续有效；UUID 也可以使用，只要它在第三方业务侧是稳定分组 ID |
| `dataset_external_ids` | 多分组文档问答建议填 | 第三方稳定业务分组/资料库 ID 数组；一个工作区选择多个分组时使用。兼容别名：`datasetExternalIds`、`availableDatasetExternalIds` |
| `business_datasource_ids` | 业务库问答/报表建议填 | 本轮指定业务库 ID 数组；值来自 1.5 的 `source_external_id`。兼容别名：`businessDatasourceIds`、`businessDataSourceIds`、`databaseSourceIds` |
| `requested_skills` | 否 | 本轮 skill 列表 |
| `mention_external_user_ids` | 否 | 被 @ 的第三方用户 ID |
| `attachment_refs` | 否 | 附件引用列表；推荐对象数组，也兼容 `["https://example.com/a.docx"]` 字符串 URL 数组；图片消息可传图片下载 URL |
| `idempotency_key` | 是 | 幂等键 |
| `received_at` | 是 | ISO 8601 时间 |

`dataset_external_id` / `dataset_external_ids` 可以和 `available_document_external_ids` 同时传，DataMax 会按并集合并授权：分组内文档整组生效，分组外的显式文档也生效，已经包含在分组内的显式文档自动去重。如果只想授权具体少数文档，应只传 `available_document_external_ids` 或 `documentExternalId`，不传分组字段。第三方内部读权限由第三方在传入这些范围前完成判断；DataMax 按本轮/本会话传入的文档或分组范围供料，不会因为只传 `available_document_source_id` 自动扩大到整源文档。

`attachment_refs` 推荐对象格式：

```jsonc
[
  {
    "attachment_external_id": "att-001",
    "filename": "资料.docx",
    "content_type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "download_url_redacted": "https://example.com/a.docx"
  }
]
```

若第三方暂时只能传字符串 URL 数组，DataMax 会兼容为附件引用对象；文档入库建议走 1.1 文档解析接口，聊天图片结构化抽取可直接在本接口传图片附件。

### 2.1.1 图片订单字段抽取

第三方聊天里用户直接发图片时，可用同一个 `POST /events` 接口让 DataMax 识别图片中的订单/充值表格，并返回可入库字段。推荐传 `message_type: "image"`、`output_format: "json"` 和图片 `attachment_refs`。如果要强制走订单截图抽取，可加 `requested_skills[].skill_id = "order_screenshot_extract"`。

请求示例：

```jsonc
{
  "platform": "generic_chat",                      // 平台类型
  "tenant_external_id": "tenant-ext-001",          // 第三方租户/客户 ID
  "bot_external_id": "bot-v3",                     // 第三方机器人/应用 ID
  "conversation_external_id": "conv-order-001",    // 会话 ID
  "sender_external_id": "user-10001",              // 用户 ID
  "message_external_id": "msg-order-image-001",    // 第三方消息 ID
  "message_type": "image",                         // 图片消息
  "text": "请识别这张充值记录截图，按订单字段返回 JSON。", // 用户要求
  "output_format": "json",                         // 推荐 JSON，reply.text 会返回 JSON 字符串
  "requested_skills": [                             // 可选；传了就强制走订单截图字段抽取
    {
      "skill_id": "order_screenshot_extract",      // 订单/充值截图字段抽取
      "mode": "required",                          // required/preferred/disabled
      "arguments": {
        "schema": {
          "record_type": "recharge_order"           // 可选：第三方自己的记录类型
        }
      }
    }
  ],
  "attachment_refs": [
    {
      "attachment_external_id": "img-order-001",    // 第三方图片 ID
      "filename": "recharge-orders.png",            // 文件名
      "content_type": "image/png",                  // 图片 MIME
      "size_bytes": 2048,                           // 文件大小；没有可省略
      "download_url_redacted": "https://example.com/recharge-orders.png" // DataMax 拉取图片用 URL；响应不会回显原始 URL
    }
  ],
  "idempotency_key": "chat:tenant-ext-001:msg-order-image-001", // 幂等键
  "received_at": "2026-06-04T10:00:00Z"             // 消息时间
}
```

响应示例：

```jsonc
{
  "accepted": true,                                 // 已接收
  "assistant_run_id": "assistant-run-id",           // DataMax 运行 ID
  "idempotency_key": "chat:tenant-ext-001:msg-order-image-001", // 幂等键
  "reply": {
    "target_conversation_external_id": "conv-order-001", // 回写会话 ID
    "reply_type": "card",                         // 结构化卡片
    "task_status": "answered",                    // answered 或 needs_review
    "text": "{...JSON...}",                       // output_format=json 时为完整 JSON 字符串
    "card": {
      "type": "v3_order_screenshot_extract",      // 固定类型
      "status": "answered",                       // answered 或 needs_review
      "extraction_id": "img-extract-xxx",         // 抽取 ID
      "record_count": 2,                          // 记录数
      "needs_review": false,                      // 是否建议人工复核
      "records": [
        {
          "recharge_amount": 100,                 // 充值额度数字
          "recharge_amount_raw": "$100",          // 充值额度原文
          "pay_amount": 700,                      // 支付金额数字
          "pay_amount_raw": "$700",               // 支付金额原文
          "payment_method": "alipay",             // 支付方式归一值
          "payment_method_label": "支付宝",        // 支付方式原文
          "order_no": "A1778730534",              // 订单号
          "status": "success",                    // 状态归一值；如 success/processing/failed
          "status_label": "成功",                 // 状态原文
          "created_at": "2026/5/14 11:48:54"      // 创建时间原文
        }
      ],
      "attachments": [
        {
          "attachment_external_id": "img-order-001", // 图片 ID
          "filename": "recharge-orders.png",         // 文件名
          "content_type": "image/png",               // MIME
          "size_bytes": 2048,                        // 大小
          "download_url_present": true               // 是否收到了下载 URL；不会回显原始 URL
        }
      ]
    },
    "requires_confirmation": false
  }
}
```

字段说明：

| 字段 | 注释 |
| --- | --- |
| `reply.card.type` | 固定 `v3_order_screenshot_extract` |
| `reply.card.status` | `answered` 表示已抽到记录；`needs_review` 表示图片、配置或识别结果需要复核 |
| `reply.card.records` | 可直接入库的记录数组 |
| `records[].*_raw` / `records[].*_label` | 图片原文，便于第三方保留展示或复核 |
| `records[].payment_method` | 支付方式归一值；例如 `alipay`、`wechat_pay`、`bank_card` |
| `records[].status` | 状态归一值；例如 `success`、`processing`、`failed`、`cancelled` |
| `reply.card.failure_reason` | `needs_review` 时可能存在，说明未完成自动抽取的原因 |
| `idempotency_key` | 同一图片消息重试时保持不变，DataMax 会返回同一份结构化结果 |

响应：

```jsonc
{
  "accepted": true,                                // DataMax 是否接收本条消息
  "assistant_run_id": "assistant-run-id",          // DataMax 本次运行 ID
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

### 2.1.2 可选主动回推

如果第三方页面等待时间较短，或 SSE 可能中断，可在 DataMax 通道配置里提供助手回复回推地址。DataMax 后台结果完成或失败后，会向该地址主动 POST 一次最终 `reply`，第三方收到后追加到对应 `conversation_external_id` 的会话即可。

配置项：

| 配置键 | 注释 |
| --- | --- |
| `reply_dispatch_url` | 第三方接收助手最终回复的 HTTPS 地址；别名：`external_reply_dispatch_url`、`outbound_reply_url`、`assistant_reply_dispatch_url` |
| `reply_dispatch_bearer_token` | 可选，DataMax 回推时使用的 Bearer Token；别名：`external_reply_bearer_token`、`outbound_reply_bearer_token` |
| `reply_dispatch_signing_secret` | 可选，DataMax 回推签名密钥；别名：`external_reply_signing_secret`、`outbound_reply_signing_secret` |

回推载荷：

```jsonc
{
  "schema": "v3.external_channel.outbound_reply.v1", // 固定结构版本
  "event_type": "assistant_reply",                   // 固定为助手回复
  "trigger": "async_result_completed",               // 后台异步结果完成或失败后触发
  "source_event_name": "assistant_run.external_channel_static_page_publish_completed", // DataMax 内部公开事件名
  "assistant_run_id": "assistant-run-id",             // DataMax 本次运行 ID
  "idempotency_key": "outbound:assistant-run-id:hash",// 回推幂等键
  "conversation_external_id": "conv-20260520-0001",  // 目标会话 ID
  "reply": {},                                       // 结构同 POST /events 响应中的 reply
  "artifact_links": [],                              // 便于直接取链接；没有为空数组
  "task_status": "static_page_published",            // 当前任务状态
  "requires_confirmation": false                     // 是否需要用户确认
}
```

DataMax 回推会带 `Authorization: Bearer <reply_dispatch_bearer_token>`，并在配置签名密钥时带 `x-v3-signature`、`x-v3-timestamp`、`x-v3-nonce`、`x-v3-content-sha256`。如果未配置回推地址，第三方仍可继续使用 `/events/stream` 断线续传或 `status_url` 轮询。

### 2.1.2 平台能力路由与状态总览

第三方页面不需要直接调用 DataMax 内部能力，也不需要解析内部工具名。第三方只需发送普通聊天消息、文档范围、数据集范围、模板、业务数据源 ID 或确认回调；DataMax 会在平台侧识别用户意图，并通过现有 `reply` 字段返回文本、状态、确认卡或产物链接。

| 用户意图/平台能力 | 第三方请求方式 | 前端重点读取 | 常见状态 |
| --- | --- | --- | --- |
| 普通问答 | 普通消息 + 已授权文档/数据集范围 | `reply.text`、`reply.task_status` | `answered`、`needs_input`、`failed` |
| 静态页/报表 | 普通消息提出生成/修改报表；推荐传 `artifact_type=static_page` | `reply.artifact_links[0]`、`reply.card.public_url`、`reply.card.generated_artifact_url`、`reply.card.status_url` | `processing`、`static_page_published`、`failed` |
| 数据接入/建表分析 | 普通消息提出入库、建表、字段映射、清洗、schema、ETL 或数据库分析 | `reply.card.type`、`reply.card.result_summary`、`reply.card.staging_plan`、`reply.card.dataset_id`、`reply.card.sync_run_id` | `data_ingestion_analysis_queued`、`data_ingestion_analysis_retrying`、`data_ingestion_analysis_completed`、`data_ingestion_analysis_needs_human`、`data_ingestion_analysis_failed`、`data_ingestion_analysis_cancelled` |
| 文档处理/深解析 | 普通消息提出解析状态、重解析、深解析、VLM 升级解析或事实抽取 | `reply.card.type`、`reply.card.documents`、`reply.card.status_counts` | `document_processing_status`、`document_processing_review_required`、`document_processing_reparse_queued` |
| 采集/资料库规划 | 普通消息提出资料采集、爬虫规划或来源接入 | `reply.card.type=v3_collection_setup_analysis`、`reply.requires_confirmation` | `needs_confirmation`、`capability_analysis_recorded` |
| 第三方系统对接规划 | 普通消息提出 OA、文档库、数据库、用户/权限、API 或连接器对接 | `reply.card.type=v3_integration_setup_analysis`、`reply.requires_confirmation` | `needs_confirmation`、`capability_analysis_recorded` |
| 主动消息/主动发起对话 | 普通消息要求完成后通知某人、发给负责人或跨会话确认 | `reply.card.type=v3_message_channel_outreach`、`reply.card.target_summary`、`reply.requires_confirmation` | `message_outreach_confirmation_required`；后续如启用安全自动派发，可出现 `message_outreach_queued`、`message_outreach_sent`、`message_outreach_failed` |

确认类状态只表示 DataMax 已识别到受控能力请求，并不表示动作已经执行。第三方应把 `requires_confirmation=true` 或 `reply.reply_type=requires_confirmation` 展示为“待 DataMax/人工确认”，不要自行扩大文档、数据集、用户、消息渠道或公开接口权限。

### 2.2 流式聊天

```http
POST /v1/external/channels/{connection_id}/events/stream
Authorization: Bearer <DataMax inbound token>
Content-Type: application/json
```

请求体与 `POST /events` 相同。

最小消费规则：

| event | 第三方怎么处理 |
| --- | --- |
| `external_channel.started` | 只表示 DataMax 开始处理，不展示为助手回复 |
| `external_channel.retrieval_started` | 展示为“正在检索资料/数据源”；不作为最终回复 |
| `external_channel.delta` | 追加 `data.delta` 到聊天气泡 |
| 其他 `external_channel.*` | 读取 `data.display_text` 展示进度；读取 `data.status` 判断状态；读取 `data.status_url` 和 `data.poll_after_seconds` 继续轮询 |
| `external_channel.needs_input` | 展示 `data.display_text` 或 `data.data.card.question`，让用户补充后继续同一会话 |
| `external_channel.completed` | 读取 `data.data.response.reply`；结构与 `POST /events` 的 `reply` 相同 |
| `done` | `data.ok=true` 表示本次 SSE 正常结束 |
| `error` | 展示 `data.error.message`，并保留 `idempotency_key` 便于排查 |

除 `external_channel.delta`、`done`、`error` 外，结构化事件的 `data` 都带以下通用字段：

```jsonc
{
  "schema": "v3.external_channel.sse.v1",          // SSE 结构版本
  "event_id": "assistant-run-id:000020",           // 可用于去重
  "sequence": 20,                                  // 本轮流内阶段序号
  "assistant_run_id": "assistant-run-id",          // DataMax 本次运行 ID
  "idempotency_key": "chat:tenant-ext-001:msg-1",  // 幂等键
  "conversation_external_id": "conv-20260520-0001",// 第三方会话 ID
  "phase": "static_page",                          // 阶段
  "status": "processing",                          // 公开状态
  "display_text": "DataMax 正在生成页面方案。",          // 可展示文案
  "status_url": "https://v3.elepcloud.com/...",    // 后续状态查询地址；没有为 null
  "poll_after_seconds": 15,                        // 建议轮询间隔；没有为 null
  "data": {}                                       // 该事件的具体业务数据
}
```

断线续传：记录最后一个结构化事件的 `sequence` 或 `event_id`。重连时使用同一个 `idempotency_key`，并传以下任一项：

```jsonc
{
  "stream_since_sequence": 20 // 只回放 20 之后的公开事件
}
```

也可以用 Query：`?since_sequence=20`，或 Header：`Last-Event-ID: assistant-run-id:000020`。DataMax 会回放未消费的公开事件；任务未完成时会继续输出后续状态。

最小代码示例：

```js
// 1. 直接 POST 读取 SSE。浏览器和 Node 服务端都可以用 fetch。
async function sendV3Stream({ url, token, body, onText, onProgress, onFinal }) {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${token}`,
      accept: 'text/event-stream',
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let lastSequence = 0;
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const frames = buffer.split('\n\n');
    buffer = frames.pop() || '';
    for (const frame of frames) {
      const event = frame.match(/^event: (.+)$/m)?.[1] || 'message';
      const data = JSON.parse(frame.match(/^data: (.+)$/m)?.[1] || '{}');
      if (data.sequence) lastSequence = data.sequence; // 断线续传用
      if (event === 'external_channel.delta') onText?.(data.delta || '');
      else if (event === 'external_channel.completed') onFinal?.(data.data?.response || data.response);
      else if (event !== 'done') onProgress?.(data.display_text || data.status || event, data);
    }
  }
  return { lastSequence };
}
```

```js
// 2. 断线后重连：同一个 idempotency_key + 上次 sequence。
body.stream_since_sequence = lastSequence;
await sendV3Stream({ url, token, body, onText, onProgress, onFinal });
```

```js
// 3. 如果拿到 status_url，用服务端按建议间隔轮询。
async function pollV3Status(statusUrl, token, seconds = 15) {
  while (true) {
    await new Promise((resolve) => setTimeout(resolve, seconds * 1000));
    const payload = await fetch(statusUrl, {
      headers: { authorization: `Bearer ${token}` },
    }).then((response) => response.json());
    const reply = payload.reply || {};
    if (reply.artifact_links?.[0] || reply.reply_type === 'artifact_link') return payload;
    if (reply.task_status === 'failed' || reply.task_status === 'cancelled') return payload;
    seconds = reply.card?.poll_after_seconds || seconds;
  }
}
```

如果前端只能使用原生 `EventSource`，请在第三方服务端做一个 GET 代理：服务端保存本轮请求体并调用 DataMax 的 POST `/events/stream`，浏览器只连接自己的 `GET /v3-stream-proxy?message_id=...`。浏览器重连时把最后的 `event_id` 传给服务端，服务端转成 `Last-Event-ID` 或 `stream_since_sequence`。

如果 `reply.task_status=needs_input` 或收到 `external_channel.needs_input`，表示当前可见资料不足但可以继续。第三方只需要把 `reply.text` / `reply.card.question` 展示给用户；用户补充制度名称、页码、关键词、文档范围或统计口径后，继续用同一个 `conversation_external_id` 发下一轮消息。

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

DataMax 只会把已由 DataMax 选中或已授权可见的文档、文件、数据集、数据库源预览交给受控分析任务；不会把原始数据库 URL、凭据、完整表 dump 或无限制本地路径放进任务包。

当服务端启用 `data_ingestion_analysis` 固定能力时，符合条件的请求会返回现有 `task_status` 形态：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `task_status` |
| `reply.task_status` | `data_ingestion_analysis_queued`、`data_ingestion_analysis_retrying`、`data_ingestion_analysis_completed`、`data_ingestion_analysis_needs_human`、`data_ingestion_analysis_failed`、`data_ingestion_analysis_cancelled`、人工确认后的 `data_ingestion_staging_dataset_ready`，以及同步阶段的 `data_ingestion_staging_sync_started`、`data_ingestion_staging_sync_running`、`data_ingestion_staging_sync_completed`、`data_ingestion_staging_sync_failed` |
| `reply.card.type` | 排队时为 `v3_data_ingestion_analysis`；完成后为 `v3_data_ingestion_analysis_result`；确认创建/复用 staging 数据集后为 `v3_data_ingestion_staging_plan_execution`；同步启动后为 `v3_data_ingestion_staging_sync` |
| `reply.card.result_summary` | 完成后返回安全摘要：来源摘要、行数/告警、字段映射摘要、staging 摘要、校验项和建议动作 |
| `reply.card.staging_plan` | 完成后可返回 `v3_data_ingestion_staging_plan`，用于人工确认后的数据集/数据源导入草稿；固定 `production_write_allowed=false` |
| `reply.card.dataset_id` | `data_ingestion_staging_dataset_ready` 时返回 DataMax 创建或复用的 staging 数据集 ID |
| `reply.card.dataset_key` | `data_ingestion_staging_dataset_ready` 时返回 staging 数据集 key |
| `reply.card.source_id` | `data_ingestion_staging_sync_started` 时返回本次使用的数据库源 ID |
| `reply.card.sync_run_id` | `data_ingestion_staging_sync_started` 时返回 DataMax 内部同步任务 ID |
| `reply.card.runtime_event.retryable` | 固定任务取消时为 `false`；第三方不需要自动重试取消态 |
| `reply.card.workflow_status` | 同步阶段返回 DataMax 内部工作流状态，例如 `running`、`succeeded`、`failed` |
| `reply.card.workflow_stage` | 同步阶段返回当前阶段，例如 `sync_users`、`fetch_content`、`ingest`、`index`、`completed` |
| `reply.card.imported_row_count` | 当前确认步骤不自动导入原始行，固定为 `0`；后续导入/数据库同步需走人工确认执行 |

当 `reply.task_status=data_ingestion_staging_sync_completed` 后，同一个 `conversation_external_id` 的后续问题会自动复用该 staging 数据集作为可见数据范围；第三方不必每轮重复传内部 `dataset_id`。如果第三方更换会话 ID，或希望切换数据范围，应重新传稳定 `dataset_external_id`、具体文档范围，或由 DataMax 侧重新确认新的 staging 数据集。

若没有选中或上传可分析的数据源/表格/文档，DataMax 会返回 `data_ingestion_analysis_source_required`，提示第三方先补充资料范围。凭据请求、生产表写入、覆盖导入、schema 迁移、公开 API/auth/请求响应字段变更都会转人工确认，不会自动执行。

### 2.4 采集/对接方案分析请求

第三方接口无需新增字段。客户可以在普通聊天消息里直接提出资料采集、爬虫规划、外部资料库接入、第三方系统对接、接口字段确认、权限配置或消息渠道配置等需求。DataMax 会先把这类请求识别为平台受控能力，不会直接执行外部动作。

常见返回形态：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | 涉及新增外部动作、权限、凭据、接口变更时为 `requires_confirmation`；只读状态检查或只读方案分析时可为 `task_status` |
| `reply.task_status` | 需要确认时为 `needs_confirmation`；只读分析记录时为 `capability_analysis_recorded` |
| `reply.card.type` | 采集/资料库规划为 `v3_collection_setup_analysis`；第三方系统/API/权限对接规划为 `v3_integration_setup_analysis` |
| `reply.card.requested_capability` | `collection_setup_analysis` 或 `integration_setup_analysis` |
| `reply.card.risk_level` | `low`、`medium`、`high` 或 `critical` |
| `reply.card.review_reason` | 需要确认或只读放行的原因 |
| `reply.card.next_actions` | DataMax 建议的下一步，不表示已经执行 |
| `reply.card.forbidden_actions` | 本轮禁止自动执行的动作摘要 |

第三方应把 `requires_confirmation` 展示为“待 DataMax/人工确认”状态。DataMax 不会在该流程中自动执行爬取、登录、外部写入、凭据收集、公开接口 URL/鉴权/请求字段/响应字段变更，也不会自动扩大文档、数据集或用户权限。

### 2.5 主动消息/主动发起对话请求

第三方接口无需新增字段。客户可以在普通聊天消息里要求 DataMax 在任务完成后通知某人、提醒负责人查看报表、向当前会话继续发起确认，或把结果通过已配置消息渠道发送给指定人员。DataMax 会先生成受控外发意图，不会让模型直接发送消息。

常见返回形态：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | 通常为 `requires_confirmation` |
| `reply.task_status` | `message_outreach_confirmation_required` |
| `reply.card.type` | `v3_message_channel_outreach` |
| `reply.card.status` | `message_outreach_confirmation_required` |
| `reply.card.requested_capability` | `message_channel_outreach` |
| `reply.card.risk_level` | 同会话运营通知通常为 `low`；新收件人、跨会话或跨渠道为 `medium`；含报表/文档/权限内容为 `high` |
| `reply.card.target_summary` | 只返回安全摘要，如当前通道、会话 ID、接收人数和是否同会话 |
| `reply.card.idempotency_key` | 本次外发意图幂等键；第三方可用于确认/审计对账 |

第三方应把该回复展示为“待 DataMax/人工确认后发送”。DataMax 不会自动发送原始模型文本，不会绕过收件人权限，不会跨渠道外发，也不会在消息里携带明文凭据或原始客户资料全文。若后续接入安全的同会话运营通知自动派发，仍会通过 `reply.card.status=message_outreach_queued|message_outreach_sent|message_outreach_failed` 或对应状态事件明确告知。

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
Authorization: Bearer <DataMax inbound token>
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
  "artifact_type": "static_page",                  // 推荐：直接声明产物类型；DataMax 自动进入报表页面生成流程
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

生成响应字段同 2.1。新接入推荐使用 `artifact_type + template`：第三方不需要理解内部 `render_mode`、`output_format` 和 `document_template_skill`，DataMax 会自动映射为现有报表页面发布流程。模板文档只作为页面结构、版式风格和字段组织参考，事实内容仍以本会话授权资料和检索证据为准。

`template` 字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `template.source_id` | 建议填 | 模板所属文档源 ID；未单独传 `available_document_source_id` 时，DataMax 会用它补齐 |
| `template.document_external_id` | 文档模板必填 | 模板文档 ID；兼容 `template_document_external_id`、`documentExternalId` |
| `template.revision_external_id` | 否 | 模板版本 ID |
| `template.mode` | 否 | 推荐传 `reference`，表示作为结构/版式/字段参考 |
| `template.output_type` | 否 | `static_page`、`html`、`report`、`document`、`table`、`image` 或 `any`；不传时跟随 `artifact_type` |
| `template.template_reference_id` | 否 | DataMax 内置静态页模板引用；使用第三方文档模板时通常不用传 |

兼容旧写法仍然有效：已接入第三方可以继续传 `render_mode: "artifact"`、`output_format: "image_text"`，并在 `requested_skills[].arguments.output_type` 中传 `static_page`。如果同时传了 `template` 和旧 `requested_skills`，DataMax 会去重，不重复加载同一模板文档。

DataMax 会创建报表页面草稿并自动完成页面生成与发布。生成过程不要求第三方额外确认，也不要求第三方调用内部生成能力。本次回复优先返回 `artifact_links[0]`、`card.generated_artifact_url` / `card.public_url`，同时兼容保留 `render_output_id` 和下载/预览地址。发布完成时，`reply.text` 也会带 Markdown 形式的可点击链接（如 `[点击查看报表](URL)`）和一行原始 `页面地址: URL`，第三方页面建议渲染 Markdown 链接或自动识别 URL；程序侧仍以结构化字段 `artifact_links[0]`、`card.public_url`、`card.generated_artifact_url` 为准。报表类静态页默认必须带时间范围选择；经营分析类报表默认按月展示，未指定时间时取最新可用月份，同时保留自定义时间范围能力。

DataMax 会把已经发布且被接受的静态页沉淀为模板库。后续静态页/报表需求如果命中相同数据集组合和相同 `default_prompt`，会优先复用已发布页面并刷新对应数据；如果未完全相同但本轮数据集与历史模板数据集存在交集，且 `default_prompt` 相同，DataMax 也可以套用该模板的视觉风格、页面结构和组件组织，事实数据仍以本轮已授权数据集和业务库为准。只有客户明确要求重新设计、换风格、第三方显式传入新的样式模板，或 `default_prompt` 表达了不同报表口径/主题时，才重新进入新的页面设计流程。

第三方操作人员可以先把 `card.public_url` 或 `artifact_links[0]` 作为基础页面链接单独发送。若页面需要按人员、角色、门店或区域拆成不同可发送版本，继续在对话里补充用户-角色-范围映射即可；DataMax 会在静态页卡片返回 `recipient_delivery`，说明当前是否已具备自动配置条件，或还缺哪些权限映射。

第三方不需要关心 DataMax 内部生成配置，只需按响应字段判断是否已拿到最终 HTML 页面，或仍在自动发布队列。

若进入静态页/报表生成流程，响应通常为：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `artifact_link` 或 `task_status` |
| `reply.task_status` | 顶层兼容状态；生成中、可重试、后台继续、待人工处理统一为 `processing`，需要用户补充为 `needs_input`，最终成功为 `static_page_published`，只有不可继续的失败/取消才返回 `failed` |
| `reply.card.status` | 静态页细分阶段；第三方按生成中、发布中、重试中、需人工处理、失败或取消等状态处理即可 |
| `reply.text` | 给用户展示的排队/处理说明；已发布页面会包含 Markdown 可点击链接和原始 `页面地址: URL`，第三方页面应按富文本/Markdown 或 URL 自动链接渲染 |
| `reply.card.type` | 系统卡片类型；第三方按不透明字符串记录即可 |
| `reply.card.draft_id` | DataMax 静态页草稿 ID |
| `reply.card.render_output_id` | 已直接生成 HTML 时返回；第三方用它查询、预览或下载静态页 |
| `reply.card.html_preview_url` | 已直接生成 HTML 时返回；浏览器 inline 预览地址 |
| `reply.card.html_download_url` | 已直接生成 HTML 时返回；HTML 附件下载地址 |
| `reply.card.generated_artifact_url` / `reply.card.public_url` | 已发布 generated-artifact 时返回；第三方优先把这个链接展示或转存 |
| `reply.card.data_url` | 已发布动态静态页且存在 `data.json` 时返回；用于第三方服务端转存页面数据快照 |
| `reply.card.data_snapshot_url` | 已发布动态静态页且存在 `data-snapshot.json` 时返回；与 `data_url` 同源，保留为渲染/审计快照 |
| `reply.card.dynamic_page_contract` | 动态静态页数据合同；说明 `data.json`、`data-snapshot.json`、刷新间隔、变更检测字段、默认时间范围控件和经营报表按月默认口径 |
| `reply.card.template_reference_id` | 本次使用的静态页模板引用；若为 `generated-static-page:{draft_id}`，表示来自 DataMax 已发布页面模板库 |
| `reply.card.template_reference` | 模板摘要；只用于视觉风格、结构和字段组织，不扩大事实证据范围 |
| `reply.card.template_match_policy` | 模板命中策略：`exact_dataset_artifact_key` 表示相同数据集组合直接复用，`dataset_overlap` 表示按数据集交集套用模板，`explicit_or_inferred_template` 表示显式或意图推断模板 |
| `reply.card.relaxed_template_match` | 当 `template_match_policy=dataset_overlap` 时返回匹配到的历史模板摘要，便于第三方记录为什么可以快速套用 |
| `reply.card.style_reuse_policy` | 默认 `reuse_style_unless_explicit_redesign`，表示除非明确要求换风格，否则复用模板样式 |
| `reply.card.data_refresh_policy` | 默认 `refresh_data_files_from_dataset_sources`，表示页面数据按本轮数据集/业务库刷新 |
| `reply.card.recipient_delivery` | 静态页分发辅助信息；包含 `can_create_recipient_specific_links`、`operator_external_user_id`、`target_external_user_ids`、识别到的角色范围和补充映射提示 |
| `reply.card.permission_review_status` | 权限/分发映射状态：`provided_for_auto_configuration` 表示已传映射可自动配置；`needs_user_role_scope_mapping` 表示需要补充用户-角色-门店/区域范围；`role_requirements_detected` 表示只识别到角色要求 |
| `reply.card.editable_after_publish` | `true` 表示最终页面生成后仍可继续让 DataMax 按人员、角色或门店范围调整并产出新的单独链接 |
| `reply.card.status_url` | 生成中返回；第三方服务端用 `GET` 轮询该 URL，直到 `reply.reply_type=artifact_link` 或顶层 `reply.task_status=failed` |
| `reply.card.poll_after_seconds` | 建议轮询间隔；生成中通常为 `15`，重试中通常为 `30` |
| `reply.card.preview_url` | 若返回过程预览链接，第三方可展示为生成进度预览；最终交付仍以 `public_url` 或 `artifact_links[0]` 为准 |

若调用流式接口，DataMax 会持续输出静态页中间过程：包括页面规划、生成中、发布中、已发布或问题原因。若本次 SSE 等待到达上限但后台仍在继续，第三方按 `status_url` 继续轮询。

静态页生成不应因为样本行、可选维度或局部模块数据不足就直接失败。DataMax 会先扩大供料并尽量补足；仍不足时，也会按已有数据先生成一版可用页面，并在页面或 `validation_summary.warnings` 中标出缺口。若 `card.public_url` 或 `artifact_links[0]` 已存在，可先把该页面作为可发送链接。若只存在 `card.render_output_id`，可直接按 3.4 查询/预览/下载；若 `card.public_url` 为空但 `card.status_url` 存在，表示当前仍在自动生成、重试或后台继续阶段，第三方继续轮询，或用原 `/events` 请求体和同一 `idempotency_key` 重试。若 `reply.card.status` 进入重试、失败或需人工处理但顶层仍为 `processing`，第三方继续轮询或提示 DataMax 正在补充处理；只有顶层 `reply.task_status=failed` 时，才按不可继续失败/取消提示稍后重试或由 DataMax 侧人工处理。过程预览不是最终交付物。

固定发布任务完成后，DataMax 会在现有运行事件/状态表面记录最终发布结果，不需要第三方补发确认请求。最终回复形态仍使用 2.1 的 `reply` 对象：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `artifact_link` |
| `reply.task_status` | `static_page_published` |
| `reply.artifact_links[0]` | 最终 DataMax generated-artifact 页面链接，例如 `https://v3.elepcloud.com/generated-artifacts/.../index.html` |
| `reply.card.type` | 系统卡片类型；第三方按不透明字符串记录即可 |
| `reply.card.data_url` | 若最终页面带动态数据文件，则为同目录 `data.json` 链接 |
| `reply.card.data_snapshot_url` | 若最终页面带动态数据文件，则为同目录 `data-snapshot.json` 链接 |
| `reply.card.dynamic_page_contract` | 动态页合同；第三方通常只需转存，页面会优先按本地 `data.json` 渲染 |
| `reply.card.template_reference_id` | 最终采用的模板引用；第三方可以记录下来作为后续同类需求的展示或审计信息 |
| `reply.card.template_match_policy` | 最终模板命中策略；用于区分新设计、相同数据集复用和数据集交集套模板 |
| `reply.card.style_reuse_policy` / `reply.card.data_refresh_policy` | 表示页面样式复用和数据刷新策略；通常保持默认即可 |
| `reply.card.recipient_delivery` | 与生成中卡片一致；第三方可据此判断是否直接发送基础链接，或提示操作人员补充用户-角色-范围映射后再生成分权限链接 |
| `reply.card.permission_review_status` | 与生成中卡片一致 |
| `reply.card.editable_after_publish` | 与生成中卡片一致 |
| `reply.card.validation_summary` | 口径摘要，如最新快照日期、源行数、明细行数、单位策略和告警；不会包含原始明细行或内部检索日志 |

如果第三方使用 `assistant_run_id` 做轮询，推荐直接调用 3.4 的运行结果查询接口；返回仍是 2.1 的 `reply` 对象。发布完成后，`reply.task_status=static_page_published`，`reply.artifact_links[0]` 就是最终页面链接。第三方不需要再做额外确认、下载或二次提交。

若已生成静态页/报表/HTML 产物，重点读取：

| 字段 | 注释 |
| --- | --- |
| `reply.reply_type` | `artifact_link`、`text` 或 `task_status` |
| `reply.text` | 给用户展示的说明；静态页发布完成时会包含 Markdown 链接和原始 `页面地址: URL` |
| `reply.artifact_links` | 产物链接数组；静态页优先返回 generated-artifact 页面 URL，兼容返回 HTML 下载地址；模板 HTML 产物会返回 `/v1/external/channels/{connection_id}/html-artifacts/{artifact_id}/files/0` |
| `reply.card` | 可能包含 `render_output_id`、`html_preview_url`、`html_download_url`、`generated_artifact_url`、`data_url`、`dynamic_page_contract`、`draft_id`、产物状态或结构化卡片 |
| `assistant_run_id` | 本次生成运行 ID |

### 3.4 查询、预览、下载产物

查询本次运行的最新第三方回复：

```http
GET /v1/external/channels/{connection_id}/assistant-runs/{assistant_run_id}/reply
Authorization: Bearer <DataMax inbound token>
```

返回字段同 2.1。若页面仍在生成，`reply.reply_type=task_status` 且 `reply.task_status=processing`，细分阶段读取 `reply.card.status`；若已经发布，`reply.reply_type=artifact_link` 且 `reply.artifact_links[0]` 为最终页面链接。也可以用原 `POST /events` 的同一 `idempotency_key` 重试查询，DataMax 会在最终产物发布后返回相同的 artifact link。

模板 HTML 产物直接下载：

```http
GET /v1/external/channels/{connection_id}/html-artifacts/{artifact_id}/files/{file_index}
Authorization: Bearer <DataMax inbound token>
```

静态页/报表渲染产物：

```http
GET /v1/external/channels/{connection_id}/static-page-renders/{render_output_id}
Authorization: Bearer <DataMax inbound token>
```

```http
GET /v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/preview
Authorization: Bearer <DataMax inbound token>
```

```http
GET /v1/external/channels/{connection_id}/static-page-renders/{render_output_id}/download
Authorization: Bearer <DataMax inbound token>
```

字段说明：

| 字段 | 必填 | 注释 |
| --- | --- | --- |
| `connection_id` | 是 | DataMax 分配的第三方通道 ID |
| `assistant_run_id` | 运行结果查询必填 | 2.1 响应返回的 DataMax 本次运行 ID |
| `artifact_id` | HTML 产物下载必填 | `reply.artifact_links` 中的 HTML artifact ID |
| `file_index` | HTML 产物下载必填 | 文件序号；当前模板 HTML 产物固定传 `0` |
| `render_output_id` | 静态页/报表必填 | DataMax 生成的静态页/报表渲染 ID |

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
| `external_document_download_failed` | DataMax 下载文档失败 |
| `external_document_too_large` | 文档超过大小限制 |
| `html_artifact_not_found` | 找不到 HTML 产物 |
| `html_artifact_file_not_found` | 找不到 HTML 产物文件 |
| `html_artifact_file_unavailable` | HTML 产物文件暂不可下载 |
| `static_page_render_output_not_found` | 找不到产物渲染结果 |
