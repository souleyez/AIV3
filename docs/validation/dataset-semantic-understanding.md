# DataMax 通用数据集语义理解验证记录

## 验证边界

- 合同版本：`1.0.0`；当前生成器版本：`semantic_profile_v3`。
- 当前阶段：本地门禁、8 服务器发布、Newbai 真实 canary、五类扩展 canary 和浏览器验收均已通过。
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

- `cargo fmt --all -- --check`：通过。
- `cargo test -p storage dataset_semantic --lib`：3/3 通过。
- `cargo test -p platform-api semantic_understanding --lib`：10/10 通过。
- `cargo test -p platform-api dataset_semantic --lib`：17/17 通过。
- `cargo test -p platform-api semantic_profile_adapters --lib`：9/9 通过。
- `cargo test -p retrieval-worker semantic_profile`：backfill 参数、默认 feature-off 和 job policy 聚焦测试通过。
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

## 8 服务器发布证据

- PostgreSQL 服务端版本：18.4；migration 0019 的两个表均存在。
- 发布备份：`/srv/aiv3/backups/dataset-semantic-canary-20260714T021235Z`，包含环境、schema、旧二进制、SHA256、API/canary 输出和 `auto_delete=false` 的 cleanup manifest。
- 相关服务：platform-api、retrieval worker、document-enrichment worker、Web 和 PostgreSQL 均为 active；health/ready 通过。
- rollout 只开放一个 tenant 和六个明确 dataset UUID，不做全局 backfill。
- Newbai dry-run：485 direct + 1 membership = 486 去重文档；7 个数据库对象 + 2 个辅助对象；写入 0。
- Newbai v3 ready：快照 `bdd36db6-d301-4f5e-99f8-5bad0d1554f8`，169 节点、160 条关系、247,194 bytes manifest；backfill 0.77 秒，最大 RSS 74,516 KB。
- Newbai 公开 API：9 个业务对象、155 个安全字段、155 条关系、229,362 bytes；50 请求 p95 0.494908 秒；ETag 304；敏感 locator 命中 0。
- 私有 canary 无凭证返回 404；公开页面图谱 Canvas、业务解释、结构识别和顶部选择器通过真实浏览器复验。
- 普通文档、表格、资产、音视频、网页/API 五类 v3 ready 快照全部成功，单次耗时为 0.29–0.49 秒。
- 表格 v2 的原始行标题问题在发布门禁中被发现；v3 降级为“表格数据”并复验原始行标记为 0。

完整发布、回滚、snapshot UUID 和清理边界见 `docs/operations/dataset-semantic-understanding-rollout.md`。
