# DataMax V3 管理台上线说明

## 路由划分

- `https://doc.elepcloud.com/`：DataMax 主站入口，未登录也可直接发起普通聊天、上传资料和使用主站功能；不挂到管理台。`/admin*` 会跳转到 `https://v3.elepcloud.com/admin*`。
- `https://v3.elepcloud.com/`：管理台域名，根路径跳转到 `/admin`。
- `/`：在非管理台专用域名上展示主站聊天页。
- `/admin/login`：管理台登录页。
- `/admin`：登录后的主工作台。
- `/admin/external-integrations`：登录后的第三方通道和外部集成管理。
- `/external-integrations/third-party-integration-api.zh-CN.html`：公开第三方接口文档。
- `/external-integrations/pure-third-party-integration-guide.zh-CN.html`：公开第三方接入指南。
- `/v1/*`：公开第三方 API 代理路径。
- `/api/v3/*`：公开 Next API 代理路径，具体鉴权仍由后端接口负责。

## 管理登录密钥

生产环境必须配置至少一个管理密钥变量：

```bash
ADMIN_CONSOLE_ACCESS_KEY="replace-with-strong-random-secret"
```

兼容读取顺序：

1. `ADMIN_CONSOLE_ACCESS_KEY`
2. `V3_ADMIN_CONSOLE_ACCESS_KEY`
3. `EXTERNAL_OBSERVABILITY_ACCESS_KEY`
4. `EXTERNAL_INTEGRATIONS_OBSERVATION_KEY`

如果没有配置任何密钥，管理台会按本地开发模式放行。生产部署不要依赖这个默认行为。

## Microsoft 管理登录

管理台支持在密钥登录之外增加 Microsoft Entra ID OIDC 登录。该方式不会向浏览器暴露管理密钥；Microsoft 校验通过后，DataMax 签发同一个管理台 HttpOnly Cookie。

生产启用需要配置：

```bash
ADMIN_MICROSOFT_AUTH_ENABLED="true"
ADMIN_MICROSOFT_TENANT_ID="<tenant-id>"
ADMIN_MICROSOFT_CLIENT_ID="<application-client-id>"
ADMIN_MICROSOFT_CLIENT_SECRET="<application-client-secret>"
ADMIN_MICROSOFT_REDIRECT_URI="https://v3.elepcloud.com/admin/microsoft/callback"
ADMIN_MICROSOFT_ALLOWED_EMAILS="admin1@example.com,admin2@example.com"
ADMIN_CONSOLE_SESSION_SECRET="<strong-random-state-signing-secret>"
```

可选配置：

```bash
ADMIN_MICROSOFT_ALLOWED_DOMAINS="example.com"
ADMIN_MICROSOFT_ISSUER="https://login.microsoftonline.com/<tenant-id>/v2.0"
```

安全约束：

- 必须显式启用 `ADMIN_MICROSOFT_AUTH_ENABLED=true`。
- 必须配置 `ADMIN_MICROSOFT_ALLOWED_EMAILS` 或 `ADMIN_MICROSOFT_ALLOWED_DOMAINS`，否则 Microsoft 登录不会放行任何账号。
- 回调会校验 `state`、`nonce`、ID token 签名、issuer、audience、有效期和邮箱白名单。
- 密钥登录仍保留为兜底；建议 `ADMIN_CONSOLE_ACCESS_KEY` 与 `ADMIN_CONSOLE_SESSION_SECRET` 使用不同强随机值。

## 观测页二级解锁

`/admin/external-integrations` 进入页面需要管理台登录。页面内的敏感观测数据和通道操作仍沿用原来的外部观测访问密钥：

```bash
EXTERNAL_OBSERVABILITY_ACCESS_KEY="replace-with-observation-secret"
```

可以和 `ADMIN_CONSOLE_ACCESS_KEY` 使用同一个值，也可以拆成两个值。建议生产拆开，方便后续按角色分权。

## 上线检查

1. 配置 `ADMIN_CONSOLE_ACCESS_KEY`。
2. 确认 `https://doc.elepcloud.com/` 展示主站聊天页，可未登录发消息。
3. 确认 `/admin` 未登录会跳到 `/admin/login`。
4. 如启用 Microsoft 登录，确认 Entra 应用回调地址包含 `/admin/microsoft/callback`。
5. 确认登录后 `/admin` 和 `/admin/external-integrations` 可访问。
6. 确认公开接口文档 HTML 仍可直接访问。
