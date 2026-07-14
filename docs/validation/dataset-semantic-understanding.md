# DataMax 通用数据集语义理解验证记录

## 验证边界

- 合同版本：`1.0.0`；生成器版本：`semantic_profile_v1`。
- 当前阶段：本地实现、隔离夹具和生产 Web 构建已通过；8 服务器 feature-off 发布与真实 canary 记录在 Task 13 完成后补入本文。
- 无凭证 smoke 不读取数据库密码、provider token、连接串、绝对源路径或业务样本值；成功输出只包含状态、版本、计数和安全标签。

## 隔离夹具覆盖

| 场景 | 关键断言 | 当前结果 |
| --- | --- | --- |
| 数据库 | 主节点为“租赁合同”等业务对象；多行聚合；外键为 confirmed | 通过 |
| 普通文档 | 章节和实体形成可追溯语义字段 | 通过 |
| 表格 | 表头、日期、金额和分类字段进入同一合同 | 通过 |
| 资产 | 安全画像标签进入同一合同 | 通过 |
| 音视频 | 页面或分段进入同一合同 | 通过 |
| 网页/API | 资源和字段进入同一合同 | 通过 |
| 空/未知 | 技术字段保持 unresolved，且不进入 headline | 通过 |
| 私有数据集 | 未授权请求沿用现有 visibility/owner/secret policy 被拒绝 | 通过 |

## 2026-07-14 本地证据

- `cargo test -p platform-api dataset_semantic_snapshot --lib`：9/9 通过。
- `cargo test -p platform-api dataset_semantic_understanding_support --lib`：由无凭证 smoke 聚焦执行。
- `cargo test -p platform-api semantic_relation_builder --lib`：由无凭证 smoke 聚焦执行；confirmed、observed、inferred 不互相升级。
- `pnpm --dir apps/web test`：424/424 通过。
- `pnpm --dir apps/web build`：生产构建通过；仅保留仓库既有的 middleware 命名和 NFT trace 警告。
- 浏览器隔离复验：数据库、文档、表格、资产四类页面均加载真实语义合同；字段检查器显示原始字段、角色、非空率、去重数、标签来源、可信度、证据和安全示例。
- 响应式复验：1180px 与 760px 均无页面或图谱横向溢出；760px 图谱高度 340px，检查器可读；控制台新增错误为 0。

执行无凭证门禁：

```bash
bash scripts/run-dataset-semantic-understanding-smoke.sh --no-credentials
```

Windows 本机需避开指向旧 WSL 的 `C:\Windows\System32\bash.exe`，使用 Git Bash：

```powershell
& 'C:\Program Files\Git\bin\bash.exe' scripts/run-dataset-semantic-understanding-smoke.sh --no-credentials
```

成功时只输出：

```json
{"status":"passed","schema_version":"1.0.0","source_kind_count":6,"smoke_case_count":8,"credentials_used":false,"safe_labels":["业务对象","关键字段","已确认事实","推断关系"]}
```

## 安全与诚实性断言

- `document_facts=0` 时 limitation 明确包含“暂无已确认事实”。
- 外键和明确引用才可成为 confirmed；结构归属为 observed；共享键、共现和相似关系保持 inferred。
- 公共投影过滤 email、数据库 URL、token/password-like 内容、电话号码/证件号和绝对路径证据。
- 失败重建不覆盖上一版 ready 快照；API 标记 stale 并保留安全失败码。
- ETag/304 不重复传输合同正文；浏览器切换数据集时中止并忽略过期请求。

## Task 13 待补证据

- GitHub main、8 服务器 clean HEAD、PostgreSQL 18、相关服务 active。
- migration 0019 实施结果、旧二进制和 schema/migration state 备份位置。
- feature-off health/ready、公开接口保护和主站基础回归。
- 新百 dry-run 的 486 条兼容归属、7 类源对象、无秘密摘要。
- 新百单数据集 ready canary、公开页面、API p95、快照字节数、worker 时长和错误日志。
- 文档、表格、资产、音视频、网页/API 各至少一个成功 canary。
