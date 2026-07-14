# DataMax 跨数据集语义图受控发布与回滚

## 目的和边界

本文是 8 服务器跨数据集语义图的发布手册，执行范围仅包括：

- `platform-api` 的跨图查询接口；
- `dataset-semantic-link-worker` 和 `dataset-semantic-link-backfill`；
- Web 的“当前数据集 / 跨数据集”切换；
- PostgreSQL migration `0020_dataset_semantic_cross_graph.sql`；
- 两个明确数据集的首对 canary，以及通过后的逐项扩容。

本文不授权修改数据导入、解析、检索问答、资产导入或 Oracle 接入开关。跨图失败必须保持单数据集图、导入、解析和检索可用。所有 allowlist 都是精确 UUID 集合，`*` 不表示通配。

首对 canary 固定为：

```text
left  = d4923d83-6053-4feb-8005-b22ee51e0227  新百项目资料
right = 31588c60-0885-47c4-81fe-4ff5c27de8e7  新百经营分析
```

只允许精确身份关系折叠为共享节点；相似度始终是推断关系、虚线展示且不折叠。不得为了达到边数而制造共享资料、共享字段或引用关系。

## 2026-07-14 至 2026-07-15 执行记录

以下结果均已在最终代码基线和 8 服务器受控发布窗口复验。最终文档提交、GitHub 与 8 服务器的精确 SHA 以备份目录中的 `git-head-after.txt` 为准；应用代码基线为 `d4dc0f3a2a4bfa5a165f32bb76155ce1909842b7`。

| 项目 | 最终现场事实 | 证据 |
| --- | --- | --- |
| 代码同步 | 最终文档提交后 GitHub 与 8 服务器 HEAD 一致，服务器仓库 clean | `git-head-after.txt` |
| PostgreSQL 服务端 | 18.4 | `postgres-version.txt` |
| migration 0020 | 两张目标表、约束、索引及标准 migration 入口核对通过 | `schema-final.txt` |
| 新百项目资料单集快照 | `ready`，9 个对象、160 个字段、160 条关系 | `final-rollback-single-api.json` |
| 新百项目资料公开语义 | 质量投影 170/170 个主画布节点标签为中文；六类污染项 0 | `single-quality.feature-off.json` |
| 单集 API 缓存 | 200、ETag、304；正文 293,643 bytes | `final-rollback-single-api.json` |
| 首对 pair | `ready`；pair 投影 160 节点、0 条跨数据集关系边；API 精确折叠 1 个共享资料节点 | `dataset-cross-api-response-final-pair.json` |
| Phase D | 7 个数据集、21 个当前 pair 全部与两端 latest-ready 快照一致，待处理任务 0 | `final-restore-runtime.json` |
| 跨图 API | 7 集最终请求 200、10/10 次 ETag 304、cached p95 84.85ms、38,738 bytes、mixed 404 | `dataset-cross-api-final-report.json` |
| Web、服务、回滚恢复 | feature-off 与恢复后浏览器均通过；恢复后首次可交互 p95 654.4ms、筛选 68.4ms、60.62fps；目标服务全部 active | `dataset-graph-final-rollback-feature-off.json`、`dataset-graph-final-feature-on-browser-10.json`、`final-restore-runtime.json` |

当前单集 canary 证据见 `dataset-semantic-understanding-rollout.md`。首对现场数据只证明 1 个 exact shared document；共享字段、共享概念和明确 reference 的能力由脱敏 fixture 契约测试证明，不能写成现场已发现关系，也不得为增加边数制造关系。

## 开关和安全约束

跨图使用独立 fail-closed 开关：

```text
DATASET_CROSS_SEMANTIC_GRAPH_ENABLED=false
DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST=
DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST=
```

发布时同时满足以下条件才允许处理 pair：

1. 总开关为 `true`；
2. 当前 tenant UUID 在 tenant allowlist；
3. pair 两端 UUID 都在 dataset allowlist；
4. 两端均有当前 `ready` 单集语义快照；
5. 请求用户对每个显式数据集都通过现有可见性鉴权。

任何一项不满足都必须拒绝或返回不含隐藏信息的空/404 结果，不能静默忽略不可见数据集后继续返回部分跨图。配置值从 `/etc/aiv3/aiv3.env` 或批准的密钥渠道读取，不在终端、回执、GitHub、本文或共享记忆中打印数据库连接串、密码、session、HMAC、content hash、内部路径和原始敏感值。

## 证据与备份目录

每次执行先创建独立目录，建议命名：

```text
/srv/aiv3/backups/dataset-cross-semantic-graph-YYYYmmdd-HHMMSS
```

本轮实际备份目录：

```text
/srv/aiv3/backups/dataset-cross-semantic-graph-20260714-151153
```

目录中已保留环境、unit、二进制、Web、PostgreSQL schema/data、阶段回执和 `cleanup-manifest.json`；基础备份使用 `SHA256SUMS`，最终新增回执另使用 `FINAL-SHA256SUMS`。两份清单均执行 `sha256sum -c`，不得拿较早校验覆盖最终文件未入清单的事实。

目录权限为 `0700`；环境文件副本为 `0600`。至少保存：

```text
preflight.txt
git-head-before.txt
git-head-after.txt
services-before.txt
services-after.txt
postgres-version.txt
schema-before.sql
aiv3.env.before
platform-api.before
dataset-semantic-link-worker.before       # 仅原文件存在时
dataset-semantic-link-backfill.before     # 仅原文件存在时
web-next.before.tar.gz                    # 仅原目录存在时
SHA256SUMS
cleanup-manifest.json
phase-a-feature-off.json
phase-b-single-canary.json
phase-c-pair-dry-run.json
phase-c-pair-real-run.json
phase-c-api.json
phase-c-browser.json
rollback.json
restore.json
```

`cleanup-manifest.json` 必须明确：

```json
{
  "auto_delete": false,
  "scope": "dataset-cross-semantic-graph",
  "retained_data": [
    "dataset_semantic_link_snapshots",
    "dataset_semantic_link_runs",
    "dataset_semantic_snapshots",
    "business/pilot/canary datasets"
  ],
  "note": "No data is automatically deleted during rollout or rollback."
}
```

备份完成后，从备份目录生成 `SHA256SUMS`，再对文件逐项执行 `sha256sum -c SHA256SUMS`。schema dump 必须通过服务器现有批准的数据库身份完成，不把连接串放进命令历史或证据文件。

## Phase A：feature-off 发布

### A1. 预检

通过既有跳板链路进入 8 服务器：

```bash
ssh -J windows-jump 8服务器
cd /srv/aiv3/repo
git fetch origin
git status --porcelain
git rev-parse HEAD
git rev-parse origin/codex/dataset-understanding-mvp
```

停止条件：工作区不干净、服务器 HEAD 不是预期 fast-forward 基线、GitHub HEAD 未通过本地完整门禁，任一情况都不得继续。

记录 PostgreSQL 服务端和服务状态：

```sql
select version();

select to_regclass('public.dataset_semantic_link_snapshots') as snapshots,
       to_regclass('public.dataset_semantic_link_runs') as runs;
```

```bash
systemctl is-active \
  aiv3-platform-api.service \
  aiv3-web.service \
  aiv3-retrieval-worker.service \
  aiv3-document-enrichment-worker.service

systemctl show -p ActiveState -p SubState -p NRestarts \
  aiv3-platform-api.service \
  aiv3-web.service \
  aiv3-retrieval-worker.service \
  aiv3-document-enrichment-worker.service
```

若现场 unit 名与上述不同，以 `systemctl list-unit-files 'aiv3*'` 核对后的真实名称为准，并把映射写入 `preflight.txt`，不要猜测服务状态。

### A2. 备份和 fast-forward

1. 创建 `0700` 的本次备份目录。
2. 备份 `/etc/aiv3/aiv3.env`、PostgreSQL schema、当前 API/link 二进制和 Web `.next`。
3. 生成并校验 `SHA256SUMS`。
4. 写入 `cleanup-manifest.json`，保持 `auto_delete=false`。
5. 仅允许 fast-forward：

```bash
git merge --ff-only origin/codex/dataset-understanding-mvp
git status --porcelain
git rev-parse HEAD
```

fast-forward 后工作区仍必须 clean，HEAD 必须等于本轮批准的 GitHub commit。

### A3. migration 核对

当前目标表已存在，但仍要由应用的标准 migration 入口核对 `0020_dataset_semantic_cross_graph.sql`，不得手工改表绕过 migration。至少验证：

- 两张表及其索引存在；
- pair 端点按 UUID 排序，左右数据集不同；
- link snapshot 状态仅允许 `building|ready|failed|superseded`；
- link run 状态仅允许 `pending|running|succeeded|retry_wait|dead_letter`；
- snapshot 外键指向同 tenant、同 dataset 的单集快照；
- schema 核对不读取或导出 manifest 原始内容。

### A4. 构建

8 服务器 release 构建统一使用 Clang：

```bash
CC=clang CXX=clang++ cargo build --release -p platform-api
CC=clang CXX=clang++ cargo build --release -p retrieval-worker \
  --bin dataset-semantic-link-worker \
  --bin dataset-semantic-link-backfill
corepack pnpm --filter @ai-data-platform-v3/web build
```

构建产物生成后记录 SHA256；不要在服务切换前覆盖唯一可回滚副本。默认 GCC 在 `aws-lc-sys` 的已知保护失败不代表业务代码失败，但也不能绕过构建门禁。

### A5. link worker unit 建议

若服务器尚无独立 unit，建议新建 `aiv3-dataset-semantic-link-worker.service`。先与现有 aiv3 worker 的 `User`、`Group`、资源限制和 hardening 对齐，再安装；不要在 unit 中写明文秘密：

```ini
[Unit]
Description=DataMax dataset semantic link worker
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/srv/aiv3/repo
EnvironmentFile=/etc/aiv3/aiv3.env
ExecStart=/srv/aiv3/repo/target/release/dataset-semantic-link-worker
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Phase A 只执行 `daemon-reload` 并确认 unit 可解析；跨图开关关闭时保持该 unit stopped，不在 feature-off 阶段制造 pair 运行记录。

### A6. feature-off 切换和回归

在 `/etc/aiv3/aiv3.env` 中显式保持：

```text
DATASET_CROSS_SEMANTIC_GRAPH_ENABLED=false
DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST=
DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST=
```

顺序重启 API 和 Web；只有实际变更了原有 worker 二进制时才重启对应 worker。link worker 保持 stopped。每次重启后必须等待端口与 ready probe，而不是只看 systemd `active`。

Phase A 通过条件：

- 单集“新百项目资料”仍为 `ready` 169/160；
- 单集 API 200、ETag 和 304 均通过；
- Web 不显示跨数据集切换，单集图可交互；
- 跨图 endpoint fail-closed，不返回隐藏候选、标题或计数；
- 导入、解析、检索现有健康检查不受影响；
- 所有已重启服务 `active` 且 `NRestarts=0`；
- pair ready 计数仍为 0。

任一项失败立即停在 feature-off，不进入 Phase B/C。

## Phase B：单集图提质 canary

仅复验：

```text
d4923d83-6053-4feb-8005-b22ee51e0227  新百项目资料
```

最终单集 canary 为 ready 9 个对象/160 个字段/160 条关系；质量投影为 170 个节点/329 条有 evidence 的可见边，170/170 个节点标签为中文，公开污染命中 0，ETag/304 通过。新 API/Web 二进制部署后仍必须重新运行 Task 12 单集质量与浏览器 smoke；历史证据不能代替本轮回归。

通过条件包括：中文主标签 `>=95%`，数字/英文/标点开头 `<=5%`，raw row/SQL/MIME/策略串/扩展名/重复节点均为 0；标准视图、42px 中心节点、桌面画布和交互性能按质量文档实际测量。未测指标写 `pending` 或 `skipped`，不得写通过。

## Phase C：两数据集跨图 canary

### C1. 开放精确 pair，暂不启动 worker

先验证两端单集快照均为 `ready`，再把跨图配置改为：

```text
DATASET_CROSS_SEMANTIC_GRAPH_ENABLED=true
DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST=<exact-tenant-uuid>
DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST=d4923d83-6053-4feb-8005-b22ee51e0227,31588c60-0885-47c4-81fe-4ff5c27de8e7
```

只重启读取这些配置的 API；link worker 此时仍 stopped。核对 Web 探测到功能可用，但在 pair 尚未 ready 时必须诚实显示 `building|stale|empty`，不能返回 500，也不能伪造共享关系。

### C2. dry-run-first

在不打印环境值的前提下加载批准配置，运行：

```bash
target/release/dataset-semantic-link-backfill \
  --left-dataset-id d4923d83-6053-4feb-8005-b22ee51e0227 \
  --right-dataset-id 31588c60-0885-47c4-81fe-4ff5c27de8e7 \
  --dry-run \
  --summary-only \
  --pretty
```

dry-run 必须零写入。回执只允许 pair 公共标识、版本、节点/边计数、证据等级计数和安全标签质量计数；不得含 raw hash、HMAC、原值、连接串、内部路径或隐藏数据集信息。

停止条件：manifest 安全门禁失败、两端快照漂移、出现未知 evidence class、同名不同来源被折叠、相似度被升级为 confirmed，或回执包含禁止字段。真实候选为 0 是允许结果，不能为通过数量门禁而制造关系。

### C3. real run 和单并发 worker

dry-run 通过后才执行：

```bash
target/release/dataset-semantic-link-backfill \
  --left-dataset-id d4923d83-6053-4feb-8005-b22ee51e0227 \
  --right-dataset-id 31588c60-0885-47c4-81fe-4ff5c27de8e7 \
  --confirm-real-run \
  --summary-only \
  --pretty
```

real backfill 只入队，不等同于 pair 已 ready。随后启动独立 link worker；默认单并发，不提高并发。轮询安全状态字段，直到 run 为 `succeeded` 且对应 link snapshot 为 `ready`，或进入明确失败/超时：

```sql
select status, count(*)
from dataset_semantic_link_runs
where tenant_id = '<tenant-uuid>'::uuid
  and left_dataset_id = least(
        'd4923d83-6053-4feb-8005-b22ee51e0227'::uuid,
        '31588c60-0885-47c4-81fe-4ff5c27de8e7'::uuid)
  and right_dataset_id = greatest(
        'd4923d83-6053-4feb-8005-b22ee51e0227'::uuid,
        '31588c60-0885-47c4-81fe-4ff5c27de8e7'::uuid)
group by status;

select id, status, node_count, edge_count, failure_code, generated_at
from dataset_semantic_link_snapshots
where tenant_id = '<tenant-uuid>'::uuid
  and left_dataset_id = least(
        'd4923d83-6053-4feb-8005-b22ee51e0227'::uuid,
        '31588c60-0885-47c4-81fe-4ff5c27de8e7'::uuid)
  and right_dataset_id = greatest(
        'd4923d83-6053-4feb-8005-b22ee51e0227'::uuid,
        '31588c60-0885-47c4-81fe-4ff5c27de8e7'::uuid)
order by generated_at desc nulls last, created_at desc
limit 3;
```

不要把 `source_fingerprint` 或 manifest 原文复制进发布记录。

### C4. API、权限和浏览器验收

使用批准的本地测试身份或已有授权 session，请求 platform API：

```http
POST /v1/dataset-semantic-graphs/query
Content-Type: application/json

{
  "root_dataset_id": "d4923d83-6053-4feb-8005-b22ee51e0227",
  "dataset_ids": [
    "d4923d83-6053-4feb-8005-b22ee51e0227",
    "31588c60-0885-47c4-81fe-4ff5c27de8e7"
  ],
  "auto_neighbors": 0,
  "max_nodes": 160,
  "max_edges": 240,
  "depth": 1
}
```

同时复验 `auto_neighbors`，只允许从当前用户可见且有 confirmed/observed 关系的数据集中推荐；inferred-only 数据集不能被自动塞入。Web 通过 `/api/v3/dataset-semantic-graphs/query` 使用同一服务端契约。

API 通过条件：

- 每个显式 dataset 都逐项鉴权，mixed visible/invisible 请求整体 masked 404；
- public/private/secret/local-thread/cross-tenant 权限矩阵通过；
- exact shared document/field/concept 才折叠；reference 保留端点；similarity 不折叠；
- `confirmed`、`observed`、`inferred` 计数和文案分开；
- ETag 首次 200、相同作用域二次 304；
- cached p95 `<500ms`，正文 `<2MB`；
- 响应和日志对隐藏标题、隐藏贡献数、hash、HMAC、原值、内部路径的命中均为 0；
- pair 不 ready 时返回单集图和诚实状态，不返回 500。

浏览器通过条件：

- 默认仍为“当前数据集”，后端探测通过后才显示“跨数据集”；
- 两个数据集有清晰 cluster/ring，可靠共享节点为菱形并带“共享”标识；
- identity/reference 为实线，similarity 为虚线；
- 右侧检查器显示来源数据集、证据等级、匹配依据和当前可见贡献数；
- 无可靠邻居时显示“尚未发现有证据的跨数据集共享”；
- 单实例 ECharts、交互 p95、拖拽缩放 FPS 和控制台错误按 Task 12 工具实际记录。

## Phase D：逐项扩到现有语义 canary

仅当 Phase C 的权限、质量、API/Web、性能和回滚恢复全部通过后才允许开始。每轮只增加一个 dataset UUID：

1. 先确认新数据集单集快照 ready 且贡献范围不窄于目标可见范围；
2. 先 dry-run，确认安全和关系真实性；
3. 扩一项精确 dataset allowlist；
4. 单并发 real run；
5. 重新执行 mixed visibility、泄漏扫描、ETag/304、响应大小和 p95；
6. 失败时移除本轮新增 UUID，不影响已通过 pair。

本轮已经按单项扩容完成 Phase D，没有一次性开放全库，也没有做全库 pair 回填：

| 阶段 | 本轮新增 canary | 累计数据集 / 当前 pair | pair 状态 | cached API p95 | 缓存 / 大小 / 权限 |
| --- | --- | ---: | --- | ---: | --- |
| D1 | 普通文档 `f3e98587-4df0-490c-a0d6-dfb002d4058a` | 3 / 3 | 全部 `ready` | 78.50ms | 200；10/10 次 304；35,713 bytes；mixed 404 |
| D2 | 表格 `e2c74a53-5d6e-44e6-a83e-385eed32c675` | 4 / 6 | 全部 `ready` | 61.74ms | 200；10/10 次 304；35,796 bytes；mixed 404 |
| D3 | 资产 `029da916-b4f3-41a9-8f8e-842d9460917a` | 5 / 10 | 全部 `ready` | 68.79ms | 200；10/10 次 304；35,918 bytes；mixed 404 |
| D4 | 音视频 `57d1a87f-ff59-4a12-97db-8c93f90302c2` | 6 / 15 | 全部 `ready` | 99.58ms | 200；10/10 次 304；36,046 bytes；mixed 404；真实 local-thread scope |
| D5 | Web/API `1af82b02-7f05-4dd2-b2a5-b7be3c7a929c` | 7 / 21 | 全部 `ready`，待处理 0 | 74.90ms | 200；10/10 次 304；38,738 bytes；mixed 404 |

Phase D 只对合成 canary fixture 做了最小可见范围对齐，没有修改生产业务数据：

- D1：普通文档 canary 的 dataset owner 和唯一 source document owner 对齐到一次性 smoke 身份；原值分别保存在 `phase-d1-doc-scope-before.txt` 和 `phase-d1-doc-source-scope-before.txt`。
- D3：资产 canary 的 dataset owner 对齐到同一 smoke 身份；原值保存在 `phase-d3-asset-scope-before.txt`。资产记录本身没有 owner 列，未伪造该字段。
- D5：Web/API canary 的 dataset owner 与直接 source document owner 对齐到 smoke 身份；原值保存在 `phase-d5-web-scope-before.txt`。
- D4：保持原有 public + `local_only=true` 约束，验证时使用其真实 local-thread header；没有扩大或绕过 thread scope。

D1、D3 和 D5 的 metadata visibility 均保持 private。上述变更均为私有或原范围内的合成 fixture 范围调整，可依据 before receipt 手工逆向恢复。`cleanup-manifest.json` 已复核保持 `auto_delete=false`；发布、回滚和恢复都没有自动删除 canary、业务数据、单集快照、pair snapshot 或 run。

## 回滚演练

### R1. 开关回滚

优先使用开关回滚，不删除任何数据：

```text
DATASET_CROSS_SEMANTIC_GRAPH_ENABLED=false
DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST=
DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST=
```

1. 停止 `aiv3-dataset-semantic-link-worker.service`。
2. 重启 `aiv3-platform-api.service`，等待 ready；Web 重新探测后应隐藏跨数据集切换。若 Web 服务持有相关运行态或已更换构建，再重启 Web。
3. 验证跨图请求 fail-closed，且不泄漏候选、隐藏标题或贡献数。
4. 验证“新百项目资料”单集 API 仍 ready 169/160、ETag/304 通过，Web 单集图可交互。
5. 复验导入、解析、检索健康状态。
6. 保留 `dataset_semantic_link_runs`、`dataset_semantic_link_snapshots`、所有单集快照和业务/pilot/canary 数据；禁止 truncate、delete 或自动清理。

### R2. 二进制回滚

只有开关回滚仍不能恢复稳定运行时才恢复二进制/Web：

1. 在备份目录执行 `sha256sum -c SHA256SUMS`；
2. 恢复对应 `.before` 二进制、Web `.next` 和必要的 `aiv3.env.before`；
3. migration 0020 和历史 pair 数据继续保留；
4. 按 API、原有 worker、Web 的实际变更范围顺序重启；link worker 保持 stopped；
5. 重跑 Phase A 的 feature-off 单集回归。

### R3. 回滚后的恢复

回滚演练通过且 Phase C、Phase D 原验收全部通过时，才恢复精确 tenant 与 7 个 dataset allowlist：

1. 写回三个跨图配置，只开放已经逐项通过 Phase D 的 7 个 UUID；
2. 重启 API 并等待 ready；
3. 启动 link worker；
4. 不重跑或删除已有 ready pair，先验证其仍与当前两端 latest-ready 快照一致；
5. 复验跨图 200、ETag/304、权限矩阵和 Web 切换；
6. 把最终服务、配置边界和 pair 状态写入 `restore.json`。

如果 Phase C 未通过，回滚后保持 feature-off，不做恢复。任何恢复结果都必须写明实际 HEAD、时间、服务状态和测得指标。

### R4. 本轮演练事实

最终代码基线完成了第二次完整开关回滚与恢复。feature-off 浏览器 5 个样本通过，首次可交互 p95 1135.8ms、筛选 65.9ms、60.21fps，登录/注销 201/200；单集 API 200/304、293,643 bytes，仍返回 9 个对象、160 个字段和 160 条关系。资产导入 preflight 未调用 provider，检索返回 5 个命中；`asset_items`、`asset_profiles`、`asset_parse_runs` 和 `retrieval_evidences` 前后计数完全不变。本地最终回归另通过 asset import 70 项和 ingest parser 3 项。

随后恢复 1 个 tenant 与 7 个精确 dataset UUID，API/Web/link worker 均 active，21/21 个当前 pair 与两端 latest-ready 快照一致，待处理任务 0。恢复过程没有重跑、删除或伪造 pair 数据；一次因错误 API 路径遗留的 smoke session 已按专用 device fingerprint 精确注销，最终活动遗留 0。

## 发布验收记录

下表只记录最终现场实测；精确发布 SHA 由 `git-head-after.txt` 固化：

| Gate | 结果 | 证据 |
| --- | --- | --- |
| GitHub HEAD = 8 服务器 HEAD，repo clean | 通过 | `git-head-after.txt` |
| PostgreSQL 服务端 18.4，migration 0020 约束/索引正常 | 通过 | `postgres-version.txt`、`schema-final.txt` |
| 备份 SHA 全部通过，cleanup `auto_delete=false` | 通过 | `SHA256SUMS`、`FINAL-SHA256SUMS`、`cleanup-manifest.json` |
| Phase A feature-off 单集 API/Web 回归 | 通过：p95 1135.8ms、筛选 65.9ms、60.21fps，单集 200/304 | `dataset-graph-final-rollback-feature-off.json`、`final-rollback-single-api.json` |
| 新百项目资料 ready 9/160/160，中文与污染门禁 | 通过 | `single-quality.feature-off.json`、`final-rollback-single-api.json` |
| pair dry-run 零写入且安全 | 已通过 | `phase-c-pair-dry-run.json` |
| pair real run ready，关系分类真实 | 已通过：160 节点、0 跨边；仅 1 个 exact shared document | `phase-c-pair-real-run.json` |
| 跨图权限矩阵和泄漏扫描 | 通过：8/5/15/7 项契约与权限测试；3 份响应和 1 份日志敏感命中均为 0 | `dataset-cross-security-final-report.json` |
| API ETag/304、`<2MB`、cached p95 `<500ms` | 通过：最终 7 集 10/10 次 304、p95 84.85ms、38,738 bytes | `dataset-cross-api-final-report.json` |
| 浏览器模式、线型、检查器和性能 | 通过：10 样本首次可交互 p95 654.4ms、筛选 68.4ms、cached API 64.9ms、60.62fps、最大 long task 0ms | `dataset-graph-final-feature-on-browser-10.json` |
| feature-off 回滚不影响单集/导入/解析/检索 | 通过：导入 preflight 与检索成功且四张保护表计数不变；asset import 70 项、parser 3 项通过 | `final-rollback-runtime-regression.json`、`dataset-cross-final-local-regression.json` |
| 精确 allowlist 恢复后复验 | 通过：1 tenant / 7 datasets / 21 current ready pairs / 0 pending | `final-restore-runtime.json` |
| Phase D 单项扩容 | 已完成：7 datasets / 21 current ready pairs / 0 pending pair；每阶段最终 API gate 均留存 | `phase-d1-*` 至 `phase-d5-*`、`phase-d1-api-gate-final.json` 至 `phase-d5-api-gate-final.json` |

一期必需 gate 已全部由最终现场证据覆盖。systemd `active`、数据库中有行或页面能打开均未被单独当成端到端验收结论。
