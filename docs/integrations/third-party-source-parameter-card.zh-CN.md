# V3 第三方资料源参数卡

**文档状态：** 对外草案 v0.1  
**最后更新：** 2026-05-18  
**适用模式：** 纯第三方模式 / 原第三方接入方案  
**配套样例：** `docs/integrations/third-party-source-parameter-card.sample.json`

这张参数卡只解决一件事：第三方资料库仍在第三方服务器时，V3 要如何安全、可验收地拉取用户、文档、正文和 ACL。

如果当前第一阶段只做“第三方上传后通知 V3 解析单份文档，再按文档 ID 问答”，优先使用 `third-party-document-parse-first-phase.zh-CN.md`。本参数卡用于后续批量资料源、增量同步或 V3 主动拉取第三方资料库的场景。

第三方不需要把数据库账号、对象存储密钥或内部后台地址交给 V3。第三方只需要提供一个受控的 HTTPS 资料源 API，V3 服务端使用专用凭证调用，第三方在接口内完成租户、用户和权限边界控制。

## 1. 最小交付内容

第三方给 V3 的资料源参数卡至少包含：

| 模块 | 必填内容 | 用途 |
| --- | --- | --- |
| 环境 | sandbox/production 名称、`base_url`、是否允许 loopback | V3 确定请求目标 |
| 资料源 | `source_id`、数据集名称、同步模式、权限模式 | V3 建立 source connection 和数据集 |
| 鉴权 | 支持 Bearer 或 HMAC、凭证交付方式 | V3 服务端调用第三方接口 |
| 用户 | 至少 3 个测试用户 | 验证有权、无权、deny 优先 |
| 文档 | 至少 5 份测试文档 | 验证解析、切片、引用和权限过滤 |
| ACL | 每份测试文档的权限快照或 ACL URL | V3 检索前过滤不可见证据 |
| 运维 | 重试联系人、升级联系人、维护窗口 | 联调失败时快速定位 |

参数卡不得包含明文 token、signing secret、password、private key、数据库连接串或对象存储密钥。真实凭证必须线下交付，或通过双方认可的密钥管理通道交付。

## 2. 推荐接口

第三方资料源 API 建议提供：

```http
GET /health
GET /users
GET /departments
GET /groups
GET /roles
GET /users/{user_external_id}/memberships
GET /documents?cursor={cursor}&updated_after={iso_time}&limit={limit}
GET /documents/{document_external_id}/content?revision={revision}
GET /documents/{document_external_id}/acl?revision={revision}
```

所有资料接口建议使用同一种服务端鉴权方式。V3 调用第三方资料源时使用“资料源专用凭证”，不要复用第三方调用 V3 聊天入口的 inbound token。

## 3. 文档列表返回要求

文档列表至少返回：

```json
{
  "items": [
    {
      "document_external_id": "doc-procurement-001",
      "title": "采购审批制度",
      "document_type": "markdown",
      "revision": "rev-20260518-001",
      "updated_at": "2026-05-18T09:20:00Z",
      "deleted": false,
      "content_hash": "sha256:example",
      "content_url": "/documents/doc-procurement-001/content",
      "acl_url": "/documents/doc-procurement-001/acl"
    }
  ],
  "next_cursor": null
}
```

关键要求：

- `document_external_id` 必须稳定；
- `revision` 必须能表达版本变化；
- 正文和 ACL 必须对应同一 `revision`；
- 删除、撤销、权限变化必须能通过列表或 ACL 体现；
- 不要在列表接口里返回完整正文。

## 4. 正文返回要求

正文接口可以返回 Markdown、纯文本、HTML、结构化段落或表格数据。建议优先返回已解析 Markdown 或结构化段落，减少 V3 处理原始文件的不确定性。

```json
{
  "document_external_id": "doc-procurement-001",
  "revision": "rev-20260518-001",
  "content_type": "text/markdown",
  "title": "采购审批制度",
  "body": "# 采购审批制度\n\n超过 10 万元的采购需要部门负责人审批。",
  "content_hash": "sha256:example"
}
```

如果第三方只能提供文件下载流，需同时提供文件名、类型、大小、哈希和有效期，并确认 V3 是否允许短期缓存解析结果。

## 5. ACL 返回要求

ACL 接口至少表达 allow、deny、主体类型和快照时间：

```json
{
  "document_external_id": "doc-procurement-001",
  "revision": "rev-20260518-001",
  "acl_hash": "sha256:acl-example",
  "captured_at": "2026-05-18T09:21:00Z",
  "allow": [
    { "subject_type": "group", "subject_external_id": "group-procurement", "level": "read" }
  ],
  "deny": [
    { "subject_type": "user", "subject_external_id": "user-denied", "reason": "restricted" }
  ]
}
```

V3 侧验收时按以下规则检查：

- deny 优先于 allow；
- 停用用户不可访问；
- 用户、部门、用户组、角色都应能参与权限判断；
- 权限撤销后，同一用户不应再拿到旧证据；
- 模型上下文里只能出现当前用户可见的资料片段。

## 6. 本地 mock 与校验命令

启动 V3 自带 mock 资料源：

```powershell
node tools/mock-third-party-source-gateway.mjs --port 43181 --token mock-source-token
```

校验参数卡：

```powershell
node tools/validate-third-party-source-parameter-card.mjs --manifest docs/integrations/third-party-source-parameter-card.sample.json
```

跑资料源 smoke：

```powershell
node tools/smoke-third-party-source-gateway.mjs --base-url http://127.0.0.1:43181 --token mock-source-token
```

第三方真实环境就把 `--base-url` 和 `--token` 换成对方提供的 sandbox 值。参数卡里仍然不要写明文 token。

## 7. V3 触发资料源同步

V3 后端已支持 `connector_context.http_source` 拉取第三方资料源。触发同步时，请求体可参考 `docs/integrations/third-party-source-sync-request.sample.json`：

```http
POST /v1/external/sources/{source_id}/sync
Host: v3.elepcloud.com
Content-Type: application/json
```

请求体重点字段：

```json
{
  "sync_kind": "full",
  "dataset_id": "<v3-dataset-uuid>",
  "connector_context": {
    "http_source": {
      "base_url": "https://customer-sandbox.example.com",
      "auth": {
        "request_auth_modes": ["bearer"],
        "token_env": "THIRD_PARTY_SOURCE_TOKEN"
      }
    }
  }
}
```

`token_env` 是 V3 服务器上的环境变量名，不是 token 明文。上线前需要把第三方资料源专用 token 配到 8 服务器服务环境中。不要把 token 写进 `connector_context`，因为 workflow context 会进入数据库和任务事件。

## 8. 联调验收口径

第一轮验收建议只看资料问答闭环：

1. V3 能请求第三方资料源；
2. 能拉到至少 3 个测试用户；
3. 能拉到至少 5 份测试文档；
4. 能拉到每份文档对应 ACL；
5. 能解析并进入 V3 数据集/检索证据；
6. 同一问题下，有权用户能回答，无权用户不可见，被 deny 用户不可见；
7. V3 观测页面不暴露明文 token、原始正文和第三方敏感响应体。

产物发布和业务动作可以后置，除非本轮第三方明确要验收页面发布或动作回传。
