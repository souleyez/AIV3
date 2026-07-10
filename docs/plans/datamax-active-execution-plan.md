# DataMax V3 Remaining Release Train Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `@executing-plans` to implement this plan task-by-task. Do not combine live-write tasks, skip approval gates, or continue after a failed gate.

**Goal:** 从已同步 GitHub、已完成 PostgreSQL 18.4 升级的 guarded RC 出发，先完成 8 服务器 feature-off 暗发布，再按测试租户逐步打通资产导入、异步解析、统一检索、第三方私有入口和 V3/Codex 客户端联合验收。

**Architecture:** 保持现有“PostgreSQL 主状态 + platform-api + 独立 worker + Next.js Web”部署形态。所有新能力默认关闭；主站写入、parser、asset evidence 和第三方私有入口分别使用独立 flag、独立提交、独立部署、独立 live 回执和独立回退点。资产画像以独立 evidence 进入统一检索，不伪装成 document/chunk，也不新增 parser 微服务或新基础设施。

**Tech Stack:** Rust workspace、Axum、SQLx、PostgreSQL 18.4、Next.js 16、Node smoke scripts、NATS/现有 worker 唤醒链路、systemd、GitHub Actions、8 服务器 `8.155.8.7`。

**Plan Date:** 2026-07-10

**Revision:** R5

**Status:** ACTIVE — Task 7–8 已 `PASS`、P0 已关闭；Task 9 真实 parser 为下一 READY 任务，尚未开始实现、提交或部署。

---

## 1. 唯一入口与归档边界

1. 本文件是 DataMax V3 唯一活动开发计划。
2. 执行状态只在里程碑完成时更新本文；详细命令输出、脱敏回执和失败归因写入 `docs/validation/datamax-main-gap-closure.md`。
3. 以下旧计划已归档，禁止继续执行或用其旧基线覆盖本文：
   - `docs/archive/plans/datamax-active-execution-plan-20260710-pre-r5.md`
   - `docs/archive/plans/2026-07-10-datamax-v3-project-handoff-and-next-development-plan-r4.md`
   - `docs/archive/plans/2026-06-17-fashion-postchain-absorption-plan.md`
   - `docs/archive/plans/2026-06-17-multimodal-asset-library-fashion-gallery.md`
4. `docs/plans/2026-07-07-server10-aiv3-deployment-plan.md` 是未跟踪且明确排除的 10 服务器计划；不读取、不移动、不提交、不执行。
5. `decks/` 不属于本 release train；不得 stage、提交或部署。
6. PostgreSQL 18.4 升级已经完成，完成凭据保存在 R4 归档和 `docs/adr/0003-upgrade-postgresql-18.md`；本计划不得重新执行主版本升级。

### 1.1 授权边界

| 动作 | 是否由“执行对应 Task”自动授权 | 规则 |
| --- | --- | --- |
| 读取仓库、远端只读 preflight、本地无副作用测试 | 是 | 不打印凭证、数据库 URL、客户内容或内部 locator |
| 修改 Task 明确列出的本地代码和测试 | 是 | 使用独立 worktree；不得夹带其他任务 |
| commit / push | 否 | 每个任务分别取得用户明确授权 |
| 8 服务器 fast-forward、构建、migration、重启 | 否 | Task 7、9、10、11 分别审批 |
| feature flag/env 修改、真实写入、真实 provider、live smoke | 否 | 必须有测试身份、测试 scope、窗口和 approval id |
| 删除测试数据、PG17 目录或旧备份 | 否 | cleanup manifest 只记录，不自动执行 |
| 10/120 服务器、生产 source sync、fingerprint backfill | 否 | 不在本计划 |

### 1.2 状态口径

- `PASS`：该 Task 的代码、测试、部署、live 和 rollback 证明全部满足 Done。
- `AUTH_REQUIRED`：无技术失败，只缺明确授权、凭证或窗口；安全暂停。
- `PENDING`：前置 Task 尚未完成。
- `FAIL`：验证不通过或现场漂移；立即停止，不继续后序 Task。

---

## 2. 当前可信基线

### 2.1 本地与 GitHub

| 项目 | 2026-07-10 已验证状态 |
| --- | --- |
| 仓库 | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3` |
| 分支 | `main` |
| 本地 / `origin/main` | `a59faaca8509d56b1d84bb0a4ad2bf83cd2cfb23` |
| 应用 RC commit | `f9463861ced34022fb98ed959bccda015c8b7800` |
| 收尾文档 commit | `b02008ab6b26276c5d4ca89d64b5bbe0f28d89d8` |
| GitHub Actions | `29130645814`：Task 8 最终 SHA 的 Rust Minimal 与 No-Credential Smoke 全部成功 |
| tracked 工作区 | `main` 与 `origin/main` 已同步；除两项明确排除的 untracked 外无 Task 8 tracked 差异 |
| 明确排除的 untracked | `decks/`、`docs/plans/2026-07-07-server10-aiv3-deployment-plan.md` |

已经证明：`cargo fmt --check`、`cargo check --workspace`、受影响 Rust tests、Web 400/400、Web build、资产 self-test/preflight、公开契约 guard、主站和静态页 self-test 均通过。

### 2.2 8 服务器

| 项目 | 2026-07-10 只读核对状态 |
| --- | --- |
| 应用仓库 | `/srv/aiv3/repo` |
| 应用 HEAD / `origin/main` | `a59faaca8509d56b1d84bb0a4ad2bf83cd2cfb23`，工作区干净 |
| 与 GitHub 差距 | 无；guarded RC 与 R5 已 feature-off 部署 |
| PostgreSQL | 18.4 active/enabled，checksums on |
| PG17 | inactive/disabled，数据目录与备份保留 |
| `asset_parse_runs` | 已由 `0017` 创建，feature-off 部署后为 0 行 |
| 新能力 flags | 六项均未开启 |
| 核心服务 | platform-api、Web、assistant/chat/static/report、ingest、retrieval 均 active |
| API | `healthz=200`、`readyz=200` |

数据库升级回滚资产：`/srv/aiv3/backups/postgresql-17-to-18-20260710T071104Z`。Task 7 应用 pre-deploy 逻辑备份：`/srv/aiv3/backups/feature-off-bf602a6f-20260710T101458Z`。

### 2.3 已完成与未完成

- 已完成并冻结：原 Task 1–6、Task 14。
- 已完成并关闭 P0：guarded RC、`0017_asset_parse_runs.sql`、六服务 feature-off 暗发布、主站 streaming live 和 `generic-chat-main` 静态页 live。
- Task 8 已完成隔离测试 scope 的 PNG+ZIP 写入、scope readback、同批幂等复投、flag rollback、任务卡 Web 发布和认证浏览器复验；任务卡刷新稳定、字段账本可见、重复卡为 0，Task 8 `PASS`。
- 尚未执行：真实 parser、asset evidence、第三方私有入口、operator 并发、客户端联合验收。
- Task 9 已解除前置阻塞并进入 `READY`；本地实现可在独立 worktree 开始，commit/push、8 服务器部署、flag/env 和单图 live 仍需 Task 9 独立授权。

---

## 3. 剩余 Release Train

```text
Task 7  feature-off 暗发布并关闭 P0
  -> Task 8  主站测试租户资产导入 pilot
  -> Task 9  ingest-worker 真实 parser
  -> Task 10 asset evidence + unified retrieval
  -> Task 11 第三方私有 asset-imports pilot
  -> Task 12 operator / 受控并发门禁
  -> Task 13 V3 与 Codex 客户端联合验收
```

| 里程碑 | Task | 当前状态 | 交付 | 工程估算 |
| --- | --- | --- | --- | --- |
| M1C | 7 | PASS | 8 服务器 feature-off RC、P0 关闭 | 已完成 |
| M2 | 8 | PASS | 写入/幂等/rollback、任务卡 Web 发布和认证复验通过 | 已完成 |
| M3 | 9–10 | Task 9 READY / Task 10 PENDING | accepted profile、asset evidence、统一检索 | 3–5 天 |
| M4 | 11 | PENDING | 单 connection 私有入口、公开契约不变 | 1–2 天 |
| M5 | 12–13 | PENDING | 受控并发结论、V3/Codex 联合回执 | 1–2 天 |

禁止并行跨越 Task 7–11。Task 12 和 Task 13 只能在 Task 11 `PASS` 后进入，可在同一验收窗口串行执行但不得共用未脱敏回执。

---

## 4. 每个任务通用门禁

1. 开始前运行 `git status --short --branch`；发现未知 tracked 差异即停止。
2. 代码任务使用独立 worktree；部署任务只能部署经批准且已在 `origin/main` 的 commit。
3. 新行为先写失败测试，再写最小实现，再运行定向测试和 workspace check。
4. 新 schema 必须是 append-only migration；禁止改写已发布 migration。
5. 所有 feature flag 缺失、解析失败或 allowlist 读取失败时必须默认拒绝。
6. 不把 bearer、cookie、数据库 URL、客户行、文档正文、provider 原始 payload、locator、对象 key 写入命令参数、日志或回执。
7. 每次部署前保存远端 HEAD、工作区、服务、health/ready、当前 flag 关闭证明和数据库备份路径。
8. 每次部署后验证目标服务、普通问答、静态页、公开第三方契约和新增能力的关闭状态。
9. live task 结束后关闭 flag 并重启相应服务；测试数据只生成 cleanup manifest。
10. 回退使用 forward revert commit；禁止 `git reset --hard`、覆盖 PG17 数据目录或删除 additive migration 表。

最低本地门禁：

```powershell
cargo fmt --check
cargo check --workspace
npm run test:web
npm --prefix apps/web run build
npm run test:third-party-public-contract-guard
npm run smoke:main-assistant-streaming -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:fashion-design-asset-import -- --self-test --pretty
git diff --check
```

---

## 5. Task 7：8 服务器 feature-off 暗发布并关闭 P0

**Status:** PASS；fast-forward、PG18 备份、`0017` migration、构建、六服务重启、feature-off 回归、主站 streaming live、静态页 live 和观察窗口全部通过，P0 已关闭。

**Goal:** 部署执行前明确批准的 `APPROVED_RELEASE_SHA`。该提交必须包含应用 RC `f9463861...` 和本 R5 计划，但不得包含排除项；部署后创建 `asset_parse_runs` 底座，同时保持资产写、资产画像供料、第三方结构化图片、parser、asset evidence 和第三方私有导入全部关闭。

**Files / systems:**

- Verify: `.github/workflows/datamax-ci.yml`
- Verify: `crates/storage/migrations/0017_asset_parse_runs.sql`
- Verify: `crates/platform-api/src/asset_import_access_support.rs`
- Verify: `crates/platform-api/src/asset_import_support.rs`
- Verify: `crates/platform-api/src/asset_profile_supply_support.rs`
- Verify: `crates/platform-api/src/external_image_structured_extract_runtime_support.rs`
- Update receipt only: `docs/validation/datamax-main-gap-closure.md`
- Deploy: `/srv/aiv3/repo` and approved AIV3 systemd services

**Step 1: Prove the local release target**

```powershell
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
$approvedReleaseSha=(git rev-parse origin/main).Trim()
git merge-base --is-ancestor f9463861ced34022fb98ed959bccda015c8b7800 $approvedReleaseSha
gh run list --commit $approvedReleaseSha --limit 5 --json status,conclusion,headSha,url,workflowName
```

Expected: local HEAD equals `origin/main`; the target contains `f9463861...`; its DataMax CI is completed/success; only the two approved untracked exclusions exist. Record the full SHA as `APPROVED_RELEASE_SHA` in the approval receipt.

**Step 2: Run remote read-only preflight before fetch**

```bash
cd /srv/aiv3/repo
git status --short --branch
git rev-parse HEAD
systemctl is-active postgresql-18.service aiv3-platform-api.service aiv3-web.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz
```

Expected: repo clean at `e71e1ef7...`; PG18 and services active; health/ready succeed. Any unknown file or service failure is `FAIL`.

**Step 3: Fetch without changing the worktree**

```bash
APPROVED_RELEASE_SHA='<full sha from the approved release receipt>'
git fetch origin main
test "$(git rev-parse origin/main)" = "$APPROVED_RELEASE_SHA"
git merge-base --is-ancestor f9463861ced34022fb98ed959bccda015c8b7800 "$APPROVED_RELEASE_SHA"
git diff --stat HEAD..origin/main
```

Expected: exact approved target; no merge yet.

**Step 4: Prove all dark-release flags are off without printing env values**

```bash
if grep -Eqi '^(MAIN_SITE_ASSET_IMPORT_ENABLED|ASSET_PROFILE_SUPPLY_ENABLED|EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED|ASSET_PARSE_ENABLED|ASSET_RETRIEVAL_EVIDENCE_WRITE_ENABLED|EXTERNAL_ASSET_IMPORT_ENABLED)=(1|true|yes|on)$' /etc/aiv3/*.env 2>/dev/null; then
  echo 'a dark-release feature is unexpectedly enabled' >&2
  exit 1
fi
```

Expected: exit 0 and no env values printed.

**Step 5: Fast-forward and run server build gates**

```bash
APPROVED_RELEASE_SHA='<full sha from the approved release receipt>'
git merge --ff-only origin/main
test "$(git rev-parse HEAD)" = "$APPROVED_RELEASE_SHA"
cargo fmt --check
CC=clang CXX=clang++ cargo test -q -p storage migrations --lib
CC=clang CXX=clang++ cargo test -q -p platform-api asset_import --lib
CC=clang CXX=clang++ cargo check -q -p platform-api
npm --prefix apps/web run build
CC=clang CXX=clang++ cargo build --release \
  -p platform-api \
  -p assistant-run-worker \
  -p chat-session-worker \
  -p static-page-worker \
  -p report-planner-worker
```

Expected: all commands pass. Do not restart on build/test failure.

**Step 6: Create a PG18 pre-deploy logical backup**

```bash
APPROVED_RELEASE_SHA='<full sha from the approved release receipt>'
BACKUP_DIR="/srv/aiv3/backups/feature-off-${APPROVED_RELEASE_SHA:0:8}-$(date -u +%Y%m%dT%H%M%SZ)"
install -d -m 700 -o postgres -g postgres "$BACKUP_DIR"
runuser -u postgres -- env LD_LIBRARY_PATH=/usr/pgsql-18/lib \
  /usr/pgsql-18/bin/pg_dump -Fc -Z 6 \
  -f "$BACKUP_DIR/ai_data_platform_v3.dump" ai_data_platform_v3
env LD_LIBRARY_PATH=/usr/pgsql-18/lib \
  /usr/pgsql-18/bin/pg_restore --list \
  "$BACKUP_DIR/ai_data_platform_v3.dump" > "$BACKUP_DIR/restore-list.txt"
sha256sum "$BACKUP_DIR/ai_data_platform_v3.dump" > "$BACKUP_DIR/SHA256SUMS"
sha256sum -c "$BACKUP_DIR/SHA256SUMS"
```

Expected: dump and restore list are non-empty; checksum passes. Record only path, size and checksum status.

**Step 7: Restart the reviewed service set**

```bash
systemctl restart \
  aiv3-platform-api.service \
  aiv3-web.service \
  aiv3-assistant-run-worker.service \
  aiv3-chat-session-worker.service \
  aiv3-static-page-worker.service \
  aiv3-report-planner-worker.service
```

`platform-api` startup replays idempotent migrations and must create `0017` without modifying old migrations.

**Step 8: Verify schema, services and feature-off behavior**

```bash
runuser -u postgres -- env LD_LIBRARY_PATH=/usr/pgsql-18/lib \
  /usr/pgsql-18/bin/psql -AtX -d ai_data_platform_v3 \
  -c "SELECT to_regclass('public.asset_parse_runs'), count(*) FROM asset_parse_runs"
systemctl is-active \
  aiv3-platform-api.service \
  aiv3-web.service \
  aiv3-assistant-run-worker.service \
  aiv3-chat-session-worker.service \
  aiv3-static-page-worker.service \
  aiv3-report-planner-worker.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz
```

Expected: table exists with zero rows; all services active; API healthy. Authenticated readiness must report disabled; non-allowlisted POST must not write.

**Step 9: Run regression gates**

```bash
npm run smoke:main-assistant-streaming -- --self-test
npm run smoke:static-page-5way -- --self-test
npm run smoke:fashion-design-asset-import -- --preflight --pretty
npm run test:third-party-public-contract-guard
```

With reviewed server-side credentials, run targeted main-site streaming and static-page live smoke. If credentials are unavailable, record `AUTH_REQUIRED`; do not claim Task 7 `PASS`.

**Step 10: Observe and record**

Observe the six restarted units for 10 minutes. Record sanitized error types/counts, remote HEAD, service states, health/ready, table existence, flag-off proof and backup path in the validation ledger.

**Rollback:** If existing behavior regresses, keep all feature flags false, create an approved forward-revert commit, push it, then repeat ff-only/build/restart. Leave `asset_parse_runs` in place; do not restore the database merely to remove an additive empty table.

**Done:** remote HEAD equals the recorded `APPROVED_RELEASE_SHA`; server build and live regressions pass; `asset_parse_runs` exists; all six features remain off; targeted existing capabilities pass; 10-minute errors are acceptable. P0 closes only here.

**Current receipt (2026-07-10):**

- `APPROVED_RELEASE_SHA=bf602a6fff2fdd8540e66fd0d7087aa302b40244`；GitHub Actions `29084788399` 全绿。
- 8 服务器 fast-forward 后 `cargo fmt --check`、storage migration 5/5、asset-import 58/58、platform check、Web build 和五个 release binaries 全部通过。
- PG18 pre-deploy 备份：`/srv/aiv3/backups/feature-off-bf602a6f-20260710T101458Z`；dump 57,271,874 bytes、restore list 506 行、SHA-256 `PASS`。
- 六服务重启成功；`asset_parse_runs` 已创建且 0 行；六项新能力 flag 全部关闭；health/ready、Web 本机/公网均 HTTP 200。
- 主站流式 self-test、静态页 self-test、资产 preflight 和公开契约 guard 全部通过；远端工作区干净。
- 连续观察 627 秒：journal priority error 0、JSON ERROR/FATAL 0、`asset_parse_runs` 仍为 0 行。
- 主站 streaming live 无需复制用户 Cookie 即通过：create/continue 均成功，delta 87/77，首 delta 2,533/5,195 ms，无重复终稿；回执 `/srv/aiv3/repo/target/main-assistant-streaming-smoke-task7-bf602a6f/20260710104236.json`。
- 静态页 live 使用 `local-dev` 下 enabled `generic-chat-main` 的现有 bearer，仅在服务器子进程环境临时注入且未打印/落盘；单请求 accepted、artifact 1/1、无 terminal failure，12,521 ms；回执 `/srv/aiv3/repo/target/static-page-5way-smoke-task7-bf602a6f/20260710104421.json`。
- live 后六服务 active、health/ready 正常、六项新能力 flag 仍全部关闭、`asset_parse_runs=0`，journal priority error 0；Task 7 `PASS`，P0 关闭。

---

## 6. Task 8：主站测试租户资产导入 pilot

**Status:** PASS；隔离 scope 的 live write、幂等和 flag rollback、任务卡接线、两轮 CI/Web 发布及最终认证主站可见性复验全部通过。

**Goal:** Validate only the main-site write path, idempotency, task card and safe readback. Parser may remain pending and unified retrieval is not part of this task.

**Files:**

- Modify only if a proven defect exists: `crates/platform-api/src/asset_import_support.rs`
- Modify only if a proven defect exists: `apps/web/app/HomePageClient.js`
- Modify only if a proven defect exists: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Modify for the proven task-card wiring defect: `apps/web/app/lib/asset-library-view-model.js`
- Test the proven defect: `apps/web/app/lib/asset-library-view-model.test.mjs`
- Test/receipt: `scripts/smoke/fashion-design-asset-import.mjs`
- Receipt: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Record approved scope**

Record approval id plus test tenant, user, dataset and asset-library identifiers in a secure operator note. The validation ledger stores placeholders and aggregate counts only.

**Step 2: Prove disabled and isolation behavior first**

Run preflight with the flag off; verify authenticated readiness is disabled and a write attempt creates no asset, membership or parse-run rows. Verify a different tenant remains denied.

**Step 3: Enable only the approved tenant**

```text
MAIN_SITE_ASSET_IMPORT_ENABLED=true
MAIN_SITE_ASSET_IMPORT_TENANT_ALLOWLIST=<approved-test-tenant-uuid>
```

Restart only `aiv3-platform-api.service`; recheck health/ready and non-allowlisted rejection.

**Step 4: Execute the smallest live fixture**

```powershell
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_BASE_URL='https://v3.elepcloud.com'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_COOKIE='<read locally; do not print>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_DATASET_ID='<approved dataset uuid>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_ASSET_LIBRARY_ID='<approved asset library uuid>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_APPROVAL_ID='<approved id>'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_EXECUTE='true'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_ACK_LIVE_WRITE='true'
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_PRETTY='true'
npm run smoke:fashion-design-asset-import
```

Import one PNG, then one ZIP containing at most 10 images and 20 MiB expanded content.

**Step 5: Validate task and database aggregates**

Verify stable task id, no duplicate card, safe details, field ledger, retry surface and no unrelated artifact. Query aggregate counts for `asset_items`, `dataset_asset_memberships`, `asset_parse_runs` and `asset_profiles` without selecting locator or metadata.

**Step 6: Repeat for idempotency**

Repeat the same fixture/idempotency key. Expected deltas for asset, membership and parse-run counts are zero.

**Step 7: Disable and clear secrets**

Set `MAIN_SITE_ASSET_IMPORT_ENABLED=false`, restart platform-api and prove new writes stop. Create a cleanup manifest but do not delete rows or objects.

```powershell
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_COOKIE=$null
$env:FASHION_DESIGN_ASSET_IMPORT_SMOKE_BEARER=$null
```

**Stop:** cross-tenant visibility, duplicate writes, locator leakage, unrelated artifact creation or ordinary chat regression.

**Done:** smallest live fixtures and idempotency pass, flag is off again, test rows and cleanup manifest are recorded, parser result is not overstated.

**Current receipt (2026-07-10):**

- 用户批准 Task 8；真实 tenant/user/dataset/asset-library UUID 仅保存在服务器 0600 operator note，本文和共享账本只记录占位 scope。
- authenticated flag-off 和 wrong-tenant allowlist 写入探针分别返回 `feature_disabled` / `tenant_not_allowlisted`，四类业务计数均保持 0。
- live receipt：`/srv/aiv3/repo/target/fashion-design-asset-import-smoke-task8-live-bf602a6f/20260710T105852514Z-648174-execute.json`；上传 PNG+ZIP 各 1，导入资产 3、dataset membership 3、pending parse run 3、fashion profile 3。
- 使用完全相同的 PNG/ZIP 引用和 package external id 复投；asset/membership/parse/profile 增量均为 0，资产、parse run、profile 身份摘要均稳定。
- 测试用户/scope 下新增 HTML、report、static-page、client-artifact 数量均为 0；cleanup manifest 已生成但未执行。
- rollback 已完成：`MAIN_SITE_ASSET_IMPORT_ENABLED=false`、allowlist 清空，只重启 platform-api，health/ready 通过；回退后普通主站 streaming live、静态页 self-test 和公开契约 guard 通过。
- live 后发现主站任务卡 helper 未接入实际任务架；隔离分支 `codex/task8-asset-task-card` 以 asset-library scope 重建稳定、去重、脱敏卡片并接入现有右侧任务架，不新增 API、表或轮询。
- 本地候选门禁：任务卡/产物卡定向测试 27/27（含实际任务架去重、字段账本可见和原始定位信息不泄漏）、Web 402/402、Next webpack production build、资产 self-test/preflight、公开契约、`cargo fmt --check`、`cargo check --workspace` 和 `git diff --check` 全部通过；初始接线提交为 `59b5e2ff`。
- 2026-07-11 发布前只读复核：资产导入 flag 为 false、allowlist 为空、platform-api/Web 均 active、health/ready 均 HTTP 200；保留测试会话文件仍为 0600，仅用于获批后的最终验收。
- 2026-07-11 Git/GitHub 发布就绪复核：本地 `main`、`origin/main`、Task 8 worktree 和 8 服务器 HEAD 均为 `bf602a6f...`，远端尚无 Task 8 分支；GitHub CLI 已认证，当前基线 CI `29084788399` 为 success。获批后仅发布 `main`、等待该 SHA 的 DataMax CI 通过，再在 8 服务器构建并重启 Web，不重启 API、不打开写入 flag。
- 初次发布 `fd7c1a9223a6f413ce24ef0f22212090f81f9cdb` 的 CI `29129558997` 通过，8 服务器仅构建/重启 Web；认证浏览器随即证明任务卡可见、刷新稳定、重复卡 0、locator 泄漏 0，同时发现字段账本仅在数据对象中、DOM 不可见。
- 字段账本可见性先补失败测试，再以 `a59faaca8509d56b1d84bb0a4ad2bf83cd2cfb23` 修复；最终 CI `29130645814` 全绿，8 服务器第二次仅构建/重启 Web，platform-api PID 全程保持 `649091` 未重启。
- 最终认证浏览器通过 SSH loopback 直连已部署 Web：session 和批准资产库可见，刷新前后图库任务卡均为 1，标题稳定，字段账本可见，重复卡 0，raw locator 泄漏 0，Runtime exception 0；唯一 network error 为缺失 favicon。
- 部署后普通问答/静态页 self-test、公开契约 guard、health/ready、doc/v3 Web 均通过；写入/parser/evidence/private flags 仍关闭，远端工作区干净。测试会话已正常 logout/revoke，临时 Cookie 文件已删除，0600 operator note 保留脱敏撤销记录。Task 8 `PASS`。

---

## 7. Task 9：接通 `ingest-worker` 真实 parser

**Status:** READY；Task 8 已 PASS。下一步在新的独立 worktree 实现，不得复用 Task 8 测试会话或发布授权。

**Goal:** Move pending asset parse runs through an asynchronous worker to an accepted profile with bounded retry and safe provider handling.

**Files:**

- Modify: `crates/platform-api/src/asset_import_support.rs`
- Modify: `crates/platform-api/src/fashion_postchain_adapter_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/ingest-worker/src/main.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `scripts/smoke/fashion-design-asset-import.mjs`
- Modify: `docs/operations/v3-system-manual.zh-CN.md`
- Receipt: `docs/validation/datamax-main-gap-closure.md`

**Configuration defaults:**

```text
ASSET_PARSE_ENABLED=false
ASSET_PARSE_MAX_ATTEMPTS=2
ASSET_PARSE_PROVIDER_TIMEOUT_MS=120000
```

**Step 1: Write failing enqueue tests**

Require import success to enqueue `queue=ingest`, `task_key=parse_asset_profile`; flag false must not enqueue; tenant + asset + parser version must have at most one runnable task.

**Step 2: Run the focused tests and prove failure**

```powershell
cargo test -q -p platform-api asset_parse_enqueue --lib
cargo test -q -p ingest-worker parse_asset_profile
```

Expected: fail because the task branch and state transitions are absent.

**Step 3: Implement the minimal worker branch**

Add an explicit `parse_asset_profile` branch in `ingest-worker::process_task`; unknown task keys must not enter document ingest. Load only the approved asset/parse-run/locator, set `processing`, call the existing fashion adapter, then store normalized output.

**Step 4: Implement bounded terminal states**

- success -> accepted profile + `completed`
- incomplete fields -> accepted partial profile + `partial`
- retryable provider failure -> `retrying`, maximum two attempts
- permanent/security failure -> `failed` with safe error code

Never store raw provider response or log input bytes/locator. A parser-version change creates a new parse run and never overwrites the old accepted profile.

**Step 5: Keep remote URL fetch disabled until all guards exist**

Without SSRF, redirect, DNS, MIME, byte-limit and timeout guards, URL input returns `remote_source_disabled`; it must not make a network request.

**Step 6: Run focused and workspace validation**

```powershell
cargo test -q -p platform-api asset_import --lib
cargo test -q -p platform-api fashion_postchain_adapter --lib
cargo test -q -p ingest-worker parse_asset_profile
cargo check -q -p ingest-worker
cargo check --workspace
npm run smoke:fashion-design-asset-import -- --self-test --pretty
git diff --check
```

**Step 7: Review and commit one parser slice**

```powershell
git add crates/platform-api/src/asset_import_support.rs `
  crates/platform-api/src/fashion_postchain_adapter_support.rs `
  crates/platform-api/src/lib.rs `
  crates/ingest-worker/src/main.rs `
  crates/storage/src/lib.rs `
  crates/contracts/src/lib.rs `
  scripts/smoke/fashion-design-asset-import.mjs `
  docs/operations/v3-system-manual.zh-CN.md
git commit -m "feat: process asset profiles asynchronously"
```

Commit/push require separate approval.

**Step 8: Deploy off, then run a single-asset pilot**

Deploy platform-api and ingest-worker with `ASSET_PARSE_ENABLED=false`; first prove ordinary document ingest. In an approved window enable one test tenant, single concurrency, and one image. Expected terminal state within 180 seconds; queue wait must remain under five minutes.

**Step 9: Disable and record rollback proof**

Set the flag false, restart affected services and prove no new parse task is enqueued. Preserve the result for Task 10.

**Stop:** provider call inside API request, unbounded retry, overwritten accepted profile, queue wait over five minutes or document-ingest regression.

**Done:** independent commit/deploy/live receipt exists; one image reaches completed/partial/failed with attributable safe state; flag is off again.

---

## 8. Task 10：写入 asset evidence 并进入统一检索

**Status:** PENDING on Task 9; independent migration and release.

**Goal:** Materialize accepted asset profiles as first-class retrieval evidence under existing tenant/dataset membership guards.

**Files:**

- Create: `crates/storage/migrations/0018_asset_retrieval_evidences.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/asset_profile_supply_support.rs`
- Modify: `crates/platform-api/src/assistant_run_model_supply_item_support.rs`
- Modify: `crates/platform-api/src/assistant_run_model_supply_budget_support.rs`
- Modify: `crates/retrieval-worker/src/main.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: affected storage/retrieval/platform modules
- Receipt: `docs/validation/datamax-main-gap-closure.md`

**Default:** `ASSET_RETRIEVAL_EVIDENCE_WRITE_ENABLED=false`.

**Step 1: Write failing migration and storage tests**

Require `0018` to be the final migration; evidence must reference asset/profile directly and must not require document or chunk foreign keys. Require idempotency on tenant + dataset + asset + profile kind/version + content hash.

**Step 2: Run tests and prove failure**

```powershell
cargo test -q -p storage migrations --lib
cargo test -q -p storage asset_retrieval --lib
```

**Step 3: Add the append-only evidence schema**

Store tenant, dataset, asset, profile kind/version, materialized safe text, safe metadata, content hash and timestamps. Do not store raw provider JSON, locator or copied document/chunk identities.

**Step 4: Implement the materializer and guarded query**

Materialize only accepted profile, OCR, safe caption and labels. Retrieval-worker must join tenant and dataset membership before union ranking and return `source_kind=asset_profile` plus a safe evidence reference.

**Step 5: Keep model supply compact**

Assistant/report supply receives a budgeted summary and evidence ref, never the full attributes JSON. With no eligible asset evidence, ordinary question behavior must remain unchanged.

**Step 6: Validate isolation, idempotency and mixed ranking**

```powershell
cargo test -q -p storage migrations --lib
cargo test -q -p storage asset_retrieval --lib
cargo test -q -p retrieval-worker asset
cargo test -q -p platform-api asset_profile_supply --lib
cargo test -q -p platform-api assistant_run_model_supply --lib
cargo check --workspace
npm run smoke:main-assistant-streaming -- --self-test
git diff --check
```

Tests must cover cross-tenant denial, cross-dataset denial, multiple memberships, repeated materialization, document/asset mixed ranking and no-asset regression.

**Step 7: Commit the migration and feature-off implementation**

```powershell
git add crates/storage/migrations/0018_asset_retrieval_evidences.sql `
  crates/storage/src/lib.rs `
  crates/platform-api/src/asset_profile_supply_support.rs `
  crates/platform-api/src/assistant_run_model_supply_item_support.rs `
  crates/platform-api/src/assistant_run_model_supply_budget_support.rs `
  crates/retrieval-worker/src/main.rs `
  crates/contracts/src/lib.rs
git commit -m "feat: retrieve accepted asset evidence"
```

**Step 8: Deploy off, then run one-dataset canary**

Deploy platform-api, retrieval-worker and affected assistant/report worker with evidence writes off. After ordinary chat regression passes, enable the flag only for the test window, materialize one accepted profile and ask one asset question plus one document control question.

**Step 9: Disable writes**

Disable evidence writes and prove new rows stop while previously materialized evidence remains readable under the same membership guard.

**Stop:** fabricated document/chunk, cross-scope match, locator leak or material ordinary-chat regression.

**Done:** migration, commit, deployment and canary are independently proven; asset evidence is attributable, permission-safe and idempotent; writes are off again.

---

## 9. Task 11：第三方私有 `asset-imports` pilot

**Status:** PENDING on Task 10; public third-party contract must remain unchanged.

**Goal:** Open a private asset-import route to one reviewed connection without adding it to the public integration contract.

**Files:**

- Create: `crates/platform-api/src/external_asset_import_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/platform-api/src/asset_import_support.rs`
- Modify: `crates/platform-api/src/external_channel_image_idempotency_support.rs`
- Modify: `tools/third-party-public-contract-guard.test.mjs`
- Create: `docs/integrations/v3-third-party-asset-imports-private.md`
- Modify: `scripts/smoke/fashion-design-asset-import.mjs` or create one focused private smoke
- Receipt: `docs/validation/datamax-main-gap-closure.md`

**Defaults:**

```text
EXTERNAL_ASSET_IMPORT_ENABLED=false
EXTERNAL_ASSET_IMPORT_CONNECTION_ALLOWLIST=
```

**Step 1: Write failing access and scope tests**

Require rejection when flag is false, connection is absent from allowlist, or tenant/source/dataset/asset-library scope does not match. A client may never choose another tenant.

**Step 2: Implement the private adapter**

Reuse existing third-party authentication, connection, source, dataset, single/batch/ZIP and idempotency paths. Return only request id, task ref, parse status and safe asset summary.

**Step 3: Lock the public contract**

Extend the guard so public schema, URLs and required fields contain neither `asset-imports` nor `asset_library_external_ids`. Document the new route only in the private pilot document.

**Step 4: Validate locally**

```powershell
npm run test:third-party-public-contract-guard
npm run smoke:fashion-design-asset-import -- --self-test --pretty
npm run smoke:fashion-design-asset-import -- --preflight --pretty
cargo test -q -p platform-api external_asset_import --lib
cargo test -q -p platform-api external_channel_image_idempotency --lib
cargo check -q -p platform-api
git diff --check
```

**Step 5: Commit one private-contract slice**

```powershell
git add crates/platform-api/src/external_asset_import_support.rs `
  crates/platform-api/src/lib.rs `
  crates/platform-api/src/asset_import_support.rs `
  crates/platform-api/src/external_channel_image_idempotency_support.rs `
  tools/third-party-public-contract-guard.test.mjs `
  docs/integrations/v3-third-party-asset-imports-private.md `
  scripts/smoke/fashion-design-asset-import.mjs
git commit -m "feat: add private external asset import pilot"
```

**Step 6: Deploy off, then allow one connection**

First regress public third-party chat/static pages with the feature off. In an approved window enable exactly one connection and run single, batch, ZIP, replacement, duplicate-request and cross-connection-isolation cases.

**Step 7: Close the pilot**

Disable the flag, clear the connection allowlist, restart platform-api, record safe aggregates and create—but do not execute—a cleanup manifest.

**Stop:** public contract drift, visibility from another connection, non-private default, idempotency failure or internal field leakage.

**Done:** single-connection pilot passes, public contract remains byte/semantic compatible, feature and allowlist are closed again.

---

## 10. Task 12：生产级 operator 与受控并发门禁

**Status:** PENDING on Task 11; requires operator credentials and an approved production window.

**Goal:** Attribute queue/provider/worker bottlenecks without mixing load testing with feature development.

**Files:**

- Verify: `scripts/smoke/model-gateway-operator.mjs`
- Verify: `scripts/smoke/main-chat-20way.mjs`
- Verify: `scripts/smoke/external-channel-20way.mjs`
- Verify: `scripts/smoke/heavy-static-page-5way.mjs`
- Verify: `scripts/smoke/cloudflare-fallback-2way.mjs`
- Receipt: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Run no-live self-tests and operator preflight**

Use environment-held credentials. If any required identity or window is absent, mark `AUTH_REQUIRED` before starting load.

**Step 2: Establish the one-request baseline**

Run one main chat, one third-party chat and one static-page task. Capture sanitized success, queue wait and provider route.

**Step 3: Increase chat concurrency gradually**

Run five, then ten, then twenty main/third-party requests. Stop increasing on any error-rate rise, p95 regression or queue buildup.

```powershell
npm run smoke:main-chat-20way
npm run smoke:external-channel-20way
```

Scripts must be configured to stage concurrency rather than immediately saturate all requests.

**Step 4: Run heavy and fallback controls**

Run at most five local heavy static-page tasks and two fallback tasks. Asset parser remains single-concurrency.

```powershell
npm run smoke:heavy-static-page-5way
npm run smoke:cloudflare-fallback-2way
```

**Step 5: Attribute and decide**

Record p50/p95, success/partial/failed, queue wait, provider timeout, fallback count and user-visible error class. Separate provider failure, queue backlog, worker crash and business validation. “No code change” is a valid result when gates pass.

**Stop:** missing authorization, rising service errors, material ordinary-chat latency regression, queue wait over five minutes or possible customer-data exposure.

**Done:** a sanitized operator receipt identifies the bottleneck—or proves none—and states whether configuration, capacity or code should change.

---

## 11. Task 13：V3 与 Codex 客户端联合验收

**Status:** PENDING on Task 12; requires a test identity, controlled workspace and private/sandbox publish approval.

**Goal:** Prove config package -> local execution -> upload -> attach -> task card -> publish -> continue edit without allowing the client to mutate V3 product code or deployment state.

**Files:**

- Verify: `docs/integrations/v3-codex-client-boundary-contract.md`
- Verify: `scripts/smoke/v3-client-artifact-joint-smoke.mjs`
- Verify: `scripts/smoke/v3-codex-client-boundary-sync.mjs`
- Receipt: `docs/validation/datamax-main-gap-closure.md`

**Step 1: Run deterministic self-tests**

```powershell
npm run smoke:v3-client-artifact-joint -- --self-test
npm run smoke:v3-codex-client-boundary-sync -- --self-test
```

**Step 2: Preflight the exact platform-api base**

```powershell
npm run smoke:v3-client-artifact-joint -- --preflight --base-url <approved-platform-api-base>
```

If the URL returns frontend HTML instead of API health JSON, fail even when HTTP status is 200.

**Step 3: Inspect the config package**

Prove it contains no activation token, cookie, database URL or unauthorized dataset/asset-library identifier.

**Step 4: Execute in a controlled client workspace**

Use test identity and explicit approval. Generate a small artifact, upload manifest + files, attach to the approved dataset/asset library, and publish only private or sandbox output.

**Step 5: Verify the complete user-visible chain**

Verify hashes, attachment, task card, publish link and one “modify report” continuation. The client must not edit V3 source, migration, auth, public API, systemd or deploy state.

**Step 6: Record and close**

Record client version, V3 commit, placeholder test scope and artifact counts without credentials or file contents. Create a doc-only receipt commit only after approval.

**Stop:** wrong base URL, authorization drift, secret-bearing manifest, stable URL overwrite or attempted product/deploy mutation.

**Done:** the full config -> execute -> upload -> attach -> publish -> edit chain has a reviewed test-identity receipt.

---

## 12. 明确不做与后续治理队列

在 Task 7 关闭 P0 前，不执行 P5 拆分。整个 release train 不做：

- 10/120 服务器部署、高尔夫 skill 或演示 deck；
- fingerprint backfill、对象删除、source sync 或生产数据同步；
- 公开第三方 `asset-imports`；
- 为假设并发新增 Kubernetes、parser 微服务或消息系统；
- 恢复阻断普通回答的硬质量门禁；
- 删除 PG17 数据目录、PG17 RPM 或升级备份。

Task 7 通过后，工程治理另开计划：migration ledger/checksum、全量 Rust CI 矩阵、OpenAPI 收敛、`platform-api/src/lib.rs` 行为保持切片、Web cwd/module/middleware 治理、脚手架保留/实现/删除 ADR。

---

## 13. 计划维护与交接

### 13.1 每个 Task 完成时

1. 在 `docs/validation/datamax-main-gap-closure.md` 写脱敏回执。
2. 将本文对应状态更新为 `PASS`，下一 Task 更新为 `READY`。
3. 记录 commit、GitHub Actions、远端 HEAD、服务、health/ready、flag rollback 和备份路径。
4. 只 stage 当前 Task 明确文件；运行 `git diff --cached --check`。
5. commit、push 和下一次部署分别重新取得授权。

### 13.2 新线程启动指令

```text
在 C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3 工作。

唯一活动计划：docs/plans/datamax-active-execution-plan.md
唯一验证账本：docs/validation/datamax-main-gap-closure.md

先读取计划的“当前可信基线、授权边界、通用门禁”和当前 READY Task。
先运行 git status --short --branch；不得 stage 或修改 decks/ 和
docs/plans/2026-07-07-server10-aiv3-deployment-plan.md。

当前下一任务是 Task 9。先从 `origin/main` 新建独立 worktree，按失败测试开始
`parse_asset_profile` enqueue/worker 状态机；不得复用 Task 8 测试身份、Cookie、
live-write approval 或 deployment window。Task 9 commit/push、8 服务器部署、
flag/env 修改和单图 live 分别取得独立授权；不得提前进入 Task 10。
```

### 13.3 本计划完成条件

Task 7–13 全部 `PASS`，所有临时 flags/allowlists 恢复关闭，P0 关闭，parser/evidence/private pilot 有独立提交和回退证明，operator 与客户端联合验收有脱敏回执。完成后将本文整体迁入 `docs/archive/plans/`，再决定是否建立新的工程治理计划。
