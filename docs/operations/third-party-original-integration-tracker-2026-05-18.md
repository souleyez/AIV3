# 第三方原方案联调跟踪

**日期：** 2026-05-18  
**当前模式：** 原第三方接入方案  
**状态：** 联调中  

## 1. 本次口径

本次第三方对接继续使用原方案：

- 第三方调用 V3 现有外部聊天通道；
- 通道入口使用 `generic-chat-main`；
- 第三方资料按现有第三方资料源/同步/解析链路进入 V3 后再问答；
- V3 Edge / Local Data Plane 仅作为新增模式沉淀，不作为本次联调主线；
- 本次不切换到“第三方本地解析、本地索引、本地页面托管”的 Edge 模式。

## 2. 当前参数

公网入口：

```http
POST https://v3.elepcloud.com/v1/external/channels/generic-chat-main/events
Content-Type: application/json
Authorization: Bearer <V3 inbound token>
```

固定字段：

```text
connection_id: generic-chat-main
platform: generic_chat
tenant_external_id: tenant-ext-001
bot_external_id: bot-v3
```

字段注意：

- 用户字段使用 `sender_external_id`；
- 附件引用使用 `attachment_refs`；
- 每条新消息需要唯一 `message_external_id` 和 `idempotency_key`；
- 重试同一条消息时保持同一 `idempotency_key`；
- 必须直接使用 `https://`，不要先打 `http://` 再依赖重定向。

凭证状态：

- V3 inbound token 已生成，并已线下提供给第三方；
- 8 服务器 `generic-chat-main` 已配置入站 Bearer 强校验；
- 跟踪文档不记录明文 token。

## 3. 已完成验证

代码和部署：

- 最新提交：`f90182b Enforce external channel inbound bearer auth`；
- 8 服务器仓库已快进到 `f90182b`；
- `aiv3-platform-api` 已重启；
- `aiv3-platform-api`、`aiv3-ingest-worker`、`aiv3-retrieval-worker`、`aiv3-assistant-run-worker` 均为 active。

公网接口：

- 不带 `Authorization`：返回 `401 external_channel_auth_failed`；
- 带错误 token：返回 `401 external_channel_auth_failed`；
- 带正确 token：返回 `202 accepted`；
- 普通文本消息可返回文本回复。

观测摘要：

- `GET /v1/external/integrations` 中 `generic-chat-main` 显示 `inbound_auth_mode=bearer`；
- 配置摘要不暴露明文 token；
- API 日志检查未发现 inbound token 明文泄露。

## 4. 仍需第三方提供

资料接入：

- 第三方真实测试文档列表；
- 每份文档的稳定 `document_external_id`；
- 文档标题、更新时间、版本号或 revision；
- 正文、解析文本、下载地址或 V3 可读取的正文接口；
- 删除、更新、权限撤销信号。

权限接入：

- 测试用户列表；
- `sender_external_id` 与第三方用户身份的映射规则；
- 部门、用户组、角色样例；
- 文档 ACL 样例；
- 至少 3 个不同权限等级的测试用户；
- 至少 5 份覆盖有权、无权、部门可见、角色可见、权限撤销的测试文档。

动作和产物：

- 如本轮需要页面或产物发布，第三方需提供接收 endpoint；
- 如本轮需要业务动作，第三方需提供动作 dispatch endpoint；
- V3 出站调用第三方时，第三方需要另行提供 dispatch Bearer Token 或 HMAC signing secret；
- inbound token 不能复用为 dispatch token。

## 5. 当前缺口

P0：

- 第三方真实资料尚未接入 V3 解析链路；
- 第三方用户和文档 ACL 样例尚未落库；
- 外部消息进入后，还没有基于第三方真实资料做权限过滤问答验收。

P1：

- 第三方产物/页面发布 endpoint 未配置；
- 第三方业务动作 endpoint 未配置；
- 出站 dispatch 鉴权未配置。

P2：

- IP 白名单、证书策略、重放窗口和密钥轮换策略待生产联调确认；
- 第三方交接包可在资料/权限样例确定后再更新。

## 6. 下一步执行顺序

1. 等第三方给真实测试文档、测试用户和 ACL 样例；
2. 建立第三方资料源连接或临时导入路径；
3. 跑文档解析、切片、索引工作流；
4. 写入或同步外部用户和权限快照；
5. 用 `user-10001`、`user-20001`、`user-30001` 分别问同一批问题；
6. 验证有权可答、无权提示当前不可见/未供料、权限撤销后不可答；
7. 需要页面时，再接第三方产物发布 endpoint；
8. 需要业务动作时，再接第三方 dispatch endpoint。

## 7. 暂停项

- V3 Edge / Local Data Plane 不进入本次联调主线；
- 不要求第三方当前部署 Edge Agent；
- 不要求第三方当前本地托管 V3 生成页面；
- 不用本地迁移数据集替代第三方真实资料验收。
