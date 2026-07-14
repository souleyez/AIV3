# DataMax 通用数据集语义理解发布与回滚

## 发布结论

2026-07-14 已在 8 服务器完成受控发布。数据库、普通文档、表格、资产、音视频、网页/API 六类来源均生成 `schema_version=1.0.0`、`generation_version=semantic_profile_v3` 的 ready 快照。当前仅对一个 tenant 下的六个明确 dataset UUID 开放，没有执行全库回填。

服务器通过 `ssh -J windows-jump 8服务器` 访问，仓库位于 `/srv/aiv3/repo`。运行证据与不自动删除的清理清单位于：

```text
/srv/aiv3/backups/dataset-semantic-canary-20260714T021235Z
/srv/aiv3/backups/dataset-semantic-canary-20260714T021235Z/cleanup-manifest.json
```

`cleanup-manifest.json` 的 `auto_delete=false`。历史 v1/v2 ready 快照、canary 数据集、字典项和 Web/API fixture 均保留，不自动删除。

## 开关边界

语义生成由以下三个配置共同控制：

```text
DATASET_SEMANTIC_UNDERSTANDING_ENABLED
DATASET_SEMANTIC_UNDERSTANDING_TENANT_ALLOWLIST
DATASET_SEMANTIC_UNDERSTANDING_DATASET_ALLOWLIST
```

tenant 和 dataset allowlist 都是 fail-closed 的精确 UUID 集合；`*` 不代表通配。enqueue、document-enrichment worker、document-enrichment backfill 和 semantic backfill 共用同一门禁。当前 allowlist 只包含 Newbai 与五个分类型 canary，不允许自动扩大或一次全库回填。

## Phase A：feature-off 发布

1. 核对 GitHub 主线、8 服务器仓库、PostgreSQL 和相关服务。
2. 在 `/srv/aiv3/backups/dataset-semantic-canary-20260714T021235Z` 备份发布前环境文件、schema 和旧二进制，并生成 `SHA256SUMS`。
3. 应用 migration `0019_dataset_semantic_understanding.sql`；`dataset_semantic_snapshots` 与 `semantic_dictionary_entries` 均存在。
4. 构建 platform-api、retrieval-worker、document-enrichment worker/backfill 和 Web。服务器默认 GCC 10.2.1 在 `aws-lc-sys` 触发 memcmp 编译问题，改用服务器已有 Clang 15（`CC=clang`、`CXX=clang++`）后构建成功，未修改业务逻辑。
5. 保持功能关闭，重启 platform-api、retrieval worker、document-enrichment worker 和 Web；health/ready、公开接口保护和主站基础回归通过。

数据库服务端由 SQL `select version()` 确认为 PostgreSQL 18.4。服务器默认 `psql` 客户端仍显示 13.23，不能据此判断数据库服务端版本。

## Phase B：Newbai dry-run

目标数据集：`31588c60-0885-47c4-81fe-4ff5c27de8e7`（新百经营分析）。最终 v3 dry-run 结果：

| 项目 | 结果 |
| --- | ---: |
| direct 文档 | 485 |
| membership 文档 | 1 |
| 合并去重后文档 | 486 |
| 数据库源对象 | 7 |
| 辅助对象 | 2 |
| 语义字段 | 160 |
| 写入数 | 0 |

headline 为“系统识别到 9 类业务数据，覆盖区域客流、合同预警、固定租金、提成租金。”；技术字段、秘密、内部路径和原始大字段没有进入 dry-run 摘要。证据文件为 `newbai-v3-dry-run.json`。

## Phase C：Newbai real canary

仅为 Newbai 打开精确 tenant/dataset allowlist 后顺序执行 real backfill。最终 ready 快照：

```text
snapshot_id=bdd36db6-d301-4f5e-99f8-5bad0d1554f8
schema_version=1.0.0
generation_version=semantic_profile_v3
node_count=169
edge_count=160
manifest_bytes=247194
```

关键验收结果：

- 公开 API 返回 9 个业务对象、155 个安全字段、155 条公开关系；响应正文 229,362 bytes。
- 对象以区域客流、合同预警、固定租金、提成租金、租赁合同、门店和租金销售明细为主，没有 `cardparentname`。
- API 50 次请求 p95 为 0.494908 秒；ETag 复验返回 304。
- v3 backfill 用时 0.77 秒，最大 RSS 74,516 KB。
- 公共响应中的绝对路径、连接串及 file locator 标记计数为 0；私有 canary 无凭证访问返回 404。
- 浏览器验证图谱 Canvas、顶部数据集选择器、业务解释和结构识别面板均可交互，非预期控制台错误为 0。
- v1/v2 ready 快照继续保留，生成失败时可由 API 返回上一版 ready，并标记 stale，不影响现有导入与检索。

## Phase D：五类扩展 canary

所有 real backfill 都是单并发、明确 dataset UUID 和 limit 的顺序执行：

| 来源 | dataset UUID | v3 snapshot UUID | 结果 | 用时 |
| --- | --- | --- | --- | ---: |
| 普通文档 | `f3e98587-4df0-490c-a0d6-dfb002d4058a` | `284db4b2-0b62-4534-a7a4-bc9be353007b` | ready | 0.46s |
| 表格 | `e2c74a53-5d6e-44e6-a83e-385eed32c675` | `2410e18d-5ddf-444f-becf-3f3fb49e2796` | ready | 0.33s |
| 资产 | `029da916-b4f3-41a9-8f8e-842d9460917a` | `5f50c4ae-7263-4713-baa7-d8573913ef89` | ready | 0.29s |
| 音视频 | `57d1a87f-ff59-4a12-97db-8c93f90302c2` | `d2cab2cf-4d9c-44ca-bf86-6fc18503c6d3` | ready | 0.49s |
| 网页/API | `1af82b02-7f05-4dd2-b2a5-b7be3c7a929c` | `2c4757db-2b23-486c-b437-9112742c96c0` | ready | 0.48s |

网页/API fixture 首次使用了无效 chunk state `ready`，在生成快照前即失败；修正为系统支持的 `indexed` 后成功，未留下失败 ready 快照。表格 v2 dry-run 暴露原始 CSV 行作为对象标题，发布门禁因此停止；v3 增加行标题识别与“表格数据”安全降级后重新构建，公开 API 中 `2026-03-31`、`Douyin` 和内部路径匹配均为 0。

## 2026-07-14“新百项目资料”提质 canary

目标数据集为 `d4923d83-6053-4feb-8005-b22ee51e0227`（新百项目资料）。执行时 8 服务器仓库干净，单集运行基线为 `7cdb797e`，PostgreSQL 服务端为 18.4，platform-api、retrieval worker、document-enrichment worker 和 Web 均为 active、`NRestarts=0`。完整发布前备份和本次 allowlist 配置备份分别位于：

```text
/srv/aiv3/backups/dataset-semantic-newbai-materials-20260714-134501
/srv/aiv3/backups/dataset-semantic-newbai-allowlist-20260714-143208
```

完整备份包含环境文件、PostgreSQL 18 schema、旧二进制、Web `.next` 归档、`SHA256SUMS` 和 `auto_delete=false` 的清理清单。服务器 release 构建继续使用 `CC=clang CXX=clang++`；一次误用默认 GCC 的构建在 `aws-lc-sys` 保护处中止，发生在服务重启前，未改变运行态。

最终 dry-run 与 real run 结果：

| 项目 | 结果 |
| --- | ---: |
| direct 文档 | 8 |
| membership 文档 | 1 |
| 合并去重后文档 | 9 |
| 业务对象 | 9 |
| 语义字段 | 160 |
| 语义关系 | 160 |
| 业务标签 | 167 |
| 中文业务标签 | 167（100%） |
| manifest | 293,643 bytes |
| dry-run 写入 | 0 |

原始行、SQL/MIME、策略串、绝对路径/连接串、技术文件名和数字标识六类质量命中均为 0，dry-run `quality_gate_passed=true` 后才执行 `--limit 1 --confirm-real-run --summary-only`。真实结果为：

```text
snapshot_id=4e4505b1-12b7-4f22-877e-5fbff6cf50da
status=ready
node_count=169
edge_count=160
failure_code=none
updated_at=2026-07-14 14:32:15 +08:00
```

API 返回 `ready`、9 个对象、160 个字段和 160 条关系；首次请求 HTTP 200 并返回 ETag，`If-None-Match` 二次请求为 304。公开正文中绝对路径、连接串、SQL/MIME 和 64 位十六进制 hash 命中均为 0。

已执行不删数据的 allowlist 回滚演练：移除目标 UUID 后 API 诚实返回 `empty`、0/0/0 且无 ETag；恢复原 allowlist 并顺序重启三项后端服务后，无需重跑 backfill 即恢复 `ready` 9/160/160 和 304，ETag 与 canary source fingerprint 保持一致。恢复脚本第一次即时探测命中 systemd active 到端口监听之间的短暂启动窗口；带就绪重试的复验随后通过，四项服务仍为 active、`NRestarts=0`。

## 日常回填命令

先 dry-run，再明确确认 real run；每次只处理一个已经进入 tenant/dataset allowlist 的数据集：

```bash
target/release/dataset-semantic-backfill \
  --dataset-id <dataset-uuid> \
  --limit 1 \
  --dry-run \
  --summary-only

target/release/dataset-semantic-backfill \
  --dataset-id <dataset-uuid> \
  --limit 1 \
  --confirm-real-run \
  --summary-only
```

输出只记录状态、版本、计数、指纹和安全 headline；不得把数据库凭据、连接串、内部路径或原始大字段写入回执。

## 回滚

优先使用开关回滚，不删除业务数据或快照：

1. 将 `DATASET_SEMANTIC_UNDERSTANDING_ENABLED=false`，或从 dataset allowlist 移除目标 UUID。
2. 只重启 `aiv3-retrieval-worker` 和 `aiv3-document-enrichment-worker`；若需要立即隐藏 API/Web 展示，再重启 `aiv3-platform-api` 和 `aiv3-web`。
3. 验证 `/healthz`、`/readyz`、原有导入和检索；禁止把语义生成失败解释为解析或检索失败。
4. 若必须回滚二进制，从备份目录恢复对应 `.before` 文件和 `aiv3.env.before`，核对 `SHA256SUMS` 后按上述范围重启。
5. migration 表和历史 ready 快照默认保留；不要为回滚删除业务、pilot 或 canary 数据。

上一版 ready 快照是故障 fallback。只有在清理清单经过单独人工确认后，才可处理 fixture、字典项或历史快照。
