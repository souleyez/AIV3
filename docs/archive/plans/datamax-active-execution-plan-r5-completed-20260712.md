> **ARCHIVED 2026-07-16 — NON-EXECUTABLE:** 仅作历史设计与验收证据；禁止继续 Task、继承 approval/基线/开关或执行部署命令。唯一活动入口为 `docs/plans/datamax-active-execution-plan.md`。

# DataMax V3 Remaining Release Train Implementation Plan (R5 Completed)

> **For Codex:** REQUIRED SUB-SKILL: Use `@executing-plans` to implement this plan task-by-task. Do not combine live-write tasks, skip approval gates, or continue after a failed gate.

**Goal:** 从已同步 GitHub、已完成 PostgreSQL 18.4 升级的 guarded RC 出发，先完成 8 服务器 feature-off 暗发布，再按测试租户逐步打通资产导入、异步解析、统一检索、第三方私有入口和 V3/Codex 客户端联合验收。

**Architecture:** 保持现有“PostgreSQL 主状态 + platform-api + 独立 worker + Next.js Web”部署形态。所有新能力默认关闭；主站写入、parser、asset evidence 和第三方私有入口分别使用独立 flag、独立提交、独立部署、独立 live 回执和独立回退点。资产画像以独立 evidence 进入统一检索，不伪装成 document/chunk，也不新增 parser 微服务或新基础设施。

**Tech Stack:** Rust workspace、Axum、SQLx、PostgreSQL 18.4、Next.js 16、Node smoke scripts、NATS/现有 worker 唤醒链路、systemd、GitHub Actions、8 服务器 `8.155.8.7`。

**Plan Date:** 2026-07-10

**Revision:** R5

**Status:** COMPLETE / FROZEN — Task 7–13 已全部 `PASS`、P0 已关闭；2026-07-12 逐项完成审计通过，本文整体归档且不得继续追加或执行。

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
| commit / push | 是 | R5 当前 Task 的已验证精确文件可自主 fast-forward 提交并推送；禁止 force、夹带文件、额外分支或 tag |
| 8 服务器 fast-forward、构建、migration、重启 | 是 | 用户已将 8 服务器确认为非生产验证环境；R5 Task 范围内可自主执行并保留回滚证明 |
| 8 服务器 feature flag/env 修改、测试写入、provider pilot、live smoke | 是 | 仅限本计划测试身份与隔离 scope；保持最小并发、次数上限、结束回滚和脱敏回执 |
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

| 项目 | 2026-07-11 已验证状态 |
| --- | --- |
| 仓库 | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3` |
| 分支 | `main` |
| Task 9 部署代码 SHA | `8bc4fe059719bfce54e1e0593582fcae9b9ea3b5`，包含 workflow-version 修复提交 `8bc4fe05` |
| 应用 RC commit | `f9463861ced34022fb98ed959bccda015c8b7800` |
| 收尾文档 commit | `b02008ab6b26276c5d4ca89d64b5bbe0f28d89d8` |
| GitHub Actions | `29153150946`：Task 9 workflow-version 修复 SHA 的 Rust Minimal 与 No-Credential Smoke 全部成功 |
| tracked 工作区 | Task 9 runtime/fix 基线为 `8bc4fe059719bfce54e1e0593582fcae9b9ea3b5`；本次仅追加 Task 9 closeout 文档回执 |
| Task 9 worktree | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3-task9`，分支 `codex/task9-asset-parser`；候选已拆分为实现/回执两笔提交并快进到 `main` |
| Task 9 live 修复 worktree | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3-task9-live-fix`，分支 `codex/task9-parser-workflow-version-fix`；修复已 commit/push/deploy，部署前与 `origin/main` 一致 |
| Task 10 实现 commit | `08b37723ace5d14a8a5c1b8aeba7490f3db47f3c`，提交信息 `feat: retrieve accepted asset evidence` |
| Task 10 GitHub Actions | `29155355921`：Rust Minimal 与 No-Credential Smoke 全部成功 |
| Task 10 worktree | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3-task10`，分支 `codex/task10-asset-evidence`；实现已 fast-forward 到 `origin/main` |
| Task 11 实现 commit | `be2ef77f8ffc266edc863e5ad7980b4bea3b5ce0`，提交信息 `feat: add private external asset import pilot` |
| Task 11 GitHub Actions | `29157911807`：Rust Minimal 与 No-Credential Smoke 全部成功 |
| Task 11 worktree | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3-task11`，分支 `codex/task11-private-asset-import`；实现已 fast-forward 到 `origin/main` |
| Task 12 operator fix | `f71fe4cfe4a059222bc6d4e3363fc3bec376e11a`，修复 smoke 对当前 `provider_id/model_id` 与 status `providers[]` 契约的读取 |
| Task 12 GitHub Actions | `29159537855`：Rust Minimal 与 No-Credential Smoke 全部成功 |
| Task 13 实现 commit | `d593605545e392fa17f181d195b4537d9b75f978`，补齐文件哈希、附件、sandbox publish、任务卡和 `revision_of` 报告续版门禁 |
| Task 13 客户端契约镜像 | `codex-web` `a40d3a2`；canonical/mirror SHA-256 均为 `0e41b068...f40d26` |
| Task 13 GitHub Actions | `29160764770`：No-Credential Smoke `24s`、Rust Minimal `4m48s`，全部成功 |
| 明确排除的 untracked | `decks/`、`docs/plans/2026-07-07-server10-aiv3-deployment-plan.md` |

已经证明：Task 8 的 `cargo fmt --check`、`cargo check --workspace`、受影响 Rust tests、Web 402/402、Web build、资产 self-test/preflight、公开契约 guard、主站和静态页 self-test 均通过；Task 9 的本地/CI/服务器定向测试、一次性数据库集成门禁、完整 ingest-worker 回归、release build、资产 preflight、普通 ingest runtime gate 和 feature-off 复验也已通过。

### 2.2 8 服务器

| 项目 | 2026-07-11 只读核对状态 |
| --- | --- |
| 应用仓库 | `/srv/aiv3/repo` |
| 应用仓库 HEAD | `d593605545e392fa17f181d195b4537d9b75f978`，`main` 与 `origin/main` 一致，工作区干净 |
| 运行服务 | platform-api、ingest-worker、retrieval-worker、Web 均 active；最终 PID 分别为 `954732`、`872657`、`601686`、`712419` |
| PostgreSQL | 18.4 active/enabled，checksums on |
| PG17 | inactive/disabled，数据目录与备份保留 |
| `asset_parse_runs` | 15 行、14 行 pending；Task 11 未开启 parser 或调用 provider |
| 新能力 flags | `ASSET_RETRIEVAL_EVIDENCE_WRITE_ENABLED=false`、`EXTERNAL_ASSET_IMPORT_ENABLED=false`，第三方 allowlist 为空且其余 R5 新能力 flags 均未开启 |
| 核心服务 | platform-api、Web、assistant/chat/static/report、ingest、retrieval 均 active |
| API | `healthz=200`、`readyz=200` |

数据库升级回滚资产：`/srv/aiv3/backups/postgresql-17-to-18-20260710T071104Z`。Task 7 应用 pre-deploy 逻辑备份：`/srv/aiv3/backups/feature-off-bf602a6f-20260710T101458Z`。Task 9–11 回退与 live 资产继续按各 Task 回执保留。Task 12 单请求基线回执位于 `/srv/aiv3/backups/task12-baseline-f71fe4cf-20260711T162748Z`；成功分级并发与 heavy-static 回执位于 `/srv/aiv3/backups/task12-staged-f71fe4cf-20260711T163911Z`。Task 13 基线与联合回执分别位于 `/srv/aiv3/backups/task13-baseline-d5936055-20260711T170444Z` 和 `/srv/aiv3/backups/task13-live-d5936055-20260711T170522Z`；所有测试数据均保留 no-delete manifest。

### 2.3 已完成与未完成

- 已完成并冻结：原 Task 1–6、Task 14。
- 已完成并关闭 P0：guarded RC、`0017_asset_parse_runs.sql`、六服务 feature-off 暗发布、主站 streaming live 和 `generic-chat-main` 静态页 live。
- Task 8 已完成隔离测试 scope 的 PNG+ZIP 写入、scope readback、同批幂等复投、flag rollback、任务卡 Web 发布和认证浏览器复验；任务卡刷新稳定、字段账本可见、重复卡为 0，Task 8 `PASS`。
- Task 9 已完成 workflow-version 修复、一次性数据库 enqueue FK 门禁、feature-off 部署、单图真实 provider pilot、会话注销、flag rollback 和完整回归，状态为 `PASS`。
- Task 10 已完成独立 migration、permission-safe/幂等数据库门禁、统一 document/asset ranking、feature-off 部署、同租户文档控制、asset evidence live canary、关闭后只读复验和合并 no-delete manifest，状态为 `PASS`。
- Task 11 已完成私有 adapter、公开契约 guard、一次性数据库门禁、feature-off 部署、单 connection live、跨连接拒绝、flag/allowlist rollback 和 no-delete manifest，状态为 `PASS`。
- Task 12 已完成 operator smoke 契约修复、认证单请求基线、5→10→20 主站/第三方分级并发、5 路 heavy static-page、fallback/queue guard 和完整收口，状态为 `PASS`。
- Task 13 已完成 canonical/mirror 契约同步、精确 API/错误 Web 基址 preflight、config→upload→attach→publish→task-card→报告续版联合验收、会话撤销和 no-delete manifest，状态为 `PASS`。
- 尚未完成：无功能 Task；只剩 R5 完成审计与计划归档。
- Task 9 首次 live 使用批准 `TASK9-PARSER-PILOT-20260711-01` 在获批 `13941bc6` 基线上执行；单图导入在 enqueue 时因未注册的 `asset-profile-parse-v1` workflow version 外键失败，provider 调用和 parser task 均为 0。会话已撤销、三项开关已恢复、回归通过、对象保留且 cleanup manifest 不自动删除。
- 最小修复改为复用运行时已注册的 upload-ingest workflow version；修复已发布并在 8 服务器验证。renewed pilot 以一次 provider attempt 产生 2 条 profile，安全终态为 `partial`；Task 10 已解锁。

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
| M3 | 9–10 | PASS | accepted profile、asset evidence、统一检索 | 已完成 |
| M4 | 11 | PASS | 单 connection 私有入口、公开契约不变 | 已完成 |
| M5A | 12 | PASS | 受控并发结论与瓶颈归因 | 已完成 |
| M5B | 13 | PASS | V3/Codex 联合回执 | 已完成 |

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

**Status:** PASS；workflow-version 根因修复已 commit/push、CI 全绿并 feature-off 部署；renewed 单图 live 以一次真实 provider attempt 到达安全 `partial` 终态，随后会话注销、三项开关恢复、完整回归和最终现场审计均通过。Task 10–11 已于后续独立切片中 `PASS`，当前进入 Task 12。

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

**Current feature-off receipt (2026-07-11):**

- 实现提交 `948d04e1`、候选回执提交 `33ad2d31` 已快进到 `main` 并推送；部署代码 SHA `33ad2d3198b4da32df18bdc7ffccdddacf0bbdb8`，GitHub Actions `29139080938` 全绿。
- 8 服务器部署前逻辑备份位于 `/srv/aiv3/backups/task9-feature-off-33ad2d31-20260711T041335Z`；dump 57,299,772 bytes、restore list 515 行、SHA-256 `PASS`，同目录保存 0600 旧环境与可执行旧 API/worker 二进制。
- 一次性 `aiv3_task9_test_*` 数据库上的 enqueue dedupe、normalized profile 落库和 bounded retry 三项集成测试通过；测试库已 drop，独立复核剩余数量为 0。
- 服务器门禁通过：asset import 62/62、fashion adapter 10/10、parser worker 8/8、完整 ingest-worker 52+24、ingest check、资产 self-test、公开契约 guard 和两个 release binaries。
- `/etc/aiv3/aiv3.env` 已显式设置 `ASSET_PARSE_ENABLED=false`、`ASSET_PARSE_MAX_ATTEMPTS=2`、`ASSET_PARSE_PROVIDER_TIMEOUT_MS=120000`；仅重启 platform-api 与 ingest-worker，Web PID 保持不变。
- 部署后 health/ready、普通 ingest runtime gate、主站 streaming self-test、静态页 self-test、资产 preflight 和公开契约 guard 通过；worker task-key filter 为 unset/all，供应商运行时配置类别存在但未调用，六项暗发布开关均关闭。
- 连续观察 633 秒后，部署后新增 `parse_asset_profile` task 仍为 0；远端工作区干净，priority error 与 ERROR/FATAL/panic 均为 0。未启用 parser、未调用真实 provider、未上传或解析单图。

**First live attempt and local fix receipt (2026-07-11):**

- approval `TASK9-PARSER-PILOT-20260711-01` 在执行时重新核对通过的 `13941bc6` 上启动；一次性 local-key 身份、bootstrap/pilot dataset、私有 asset library 与 membership 均通过产品 API 创建。
- 单图导入写入 1 asset、1 dataset membership、1 pending parse run 和 1 seeded profile；enqueue 创建 workflow execution 时使用未注册的 `asset-profile-parse-v1`，触发 `23503 foreign_key_violation`。`parse_asset_profile` task 为 0，真实 provider attempts 为 0。
- `EXIT` rollback 恢复 `MAIN_SITE_ASSET_IMPORT_ENABLED=false`、`ASSET_PARSE_ENABLED=false`、allowlist 为空；仅 API/worker 被重启。session 已 logout/revoke，cookie jar 已销毁，health/ready 为 200，journal error markers 为 0。
- 0600 no-delete manifest 位于 `/srv/aiv3/backups/task9-live-13941bc6-20260711T120212Z/task9-live-cleanup-manifest.private.json`；保留上述对象，不允许自动清理。
- live 后服务器回归通过：asset runner self-test/preflight、公开契约 guard、主站/静态页 self-test、platform-api asset import 62/62、完整 ingest-worker 52+24；回归后 parser task 仍为 0。
- 本地 fix worktree `ai-data-platform-v3-task9-live-fix` 基于 `1d7bf519`；`asset_import_support` 改为复用 workflow catalog 已注册的 upload-ingest version，并加入版本一致性和 catalog 缺失 fail-closed 回归测试。`cargo fmt --check`、asset import 64/64、parser worker 8/8、platform-api check、workspace check、runner self-test 与 `git diff --check` 均通过。
- fix commit `8bc4fe059719bfce54e1e0593582fcae9b9ea3b5` 已推送，GitHub Actions `29153150946` 全绿；8 服务器以该 SHA 完成 fast-forward、release build 和仅 platform-api 重启，部署回退资产位于 `/srv/aiv3/backups/task9-workflow-version-fix-8bc4fe05-20260711T125907Z`。
- 一次性数据库门禁成功创建一条使用已注册 `UploadIngest 0.1.0` workflow version 的 queued parser task，并证明 asset、membership、pending parse run、profile 和 workflow definition 均存在；测试库已 drop，报告为上述 fix backup 下的 `disposable-db-gate.json`。

**Renewed live and final rollback receipt (2026-07-11):**

- renewed pilot `TASK9-PARSER-PILOT-RETRY-20260711-02` 使用新的一次性测试身份与隔离 scope，单并发导入 1 个 PNG；未复用旧 session、Cookie、scope 或 approval id。
- queue wait `2089 ms`，terminal latency `12171 ms`；workflow task 仅一次 provider attempt 后成功收敛，runner 安全终态为 `partial`，生成 2 条 profile。无 batch/ZIP、远程 URL 或额外 provider 尝试。
- 0600 脱敏报告位于 `/srv/aiv3/backups/task9-live-8bc4fe05-20260711T131030Z/smoke/20260711T131032467Z-872589-parser-pilot.json`；0600 私有 no-delete manifest 位于同级 `scope-cleanup-manifest.private.json`，`automaticCleanupAllowed=false`，保留测试对象供人工复核。
- live 结束后一次性 session 已撤销、Cookie 为 0；`MAIN_SITE_ASSET_IMPORT_ENABLED=false`、`ASSET_PARSE_ENABLED=false`、allowlist 为空。API/worker/Web active，health/ready 200，parser task `total|runnable|over-max=1|0|0`，一次性测试数据库为 0，API/worker error markers 为 0。
- post-live 回归通过：platform-api asset import 64/64、ingest-worker 52/52 + 24/24、asset runner self-test/preflight、公开契约 guard、主站 streaming self-test 和静态页 self-test。
- Windows PowerShell 向远端 `bash -s` 传输时在脚本最终成功输出后追加了一行 CR，令外层 wrapper 误报 `exit 127`；全部语义门禁、rollback 和独立复核均已通过，因此没有重复调用 provider。后续远端脚本改用 LF-normalized base64 transport。

**Stop:** provider call inside API request, unbounded retry, overwritten accepted profile, queue wait over five minutes or document-ingest regression.

**Done:** independent commit/deploy/live receipt exists; one image reaches completed/partial/failed with attributable safe state; flag is off again.

---

## 8. Task 10：写入 asset evidence 并进入统一检索

**Status:** PASS；独立 migration、commit/CI、feature-off release、同租户 document control、asset evidence live canary、write-off 只读复验和 rollback 回归均已完成。

**Goal:** Materialize accepted asset profiles as first-class retrieval evidence under existing tenant/dataset membership guards.

**Files:**

- Create: `crates/storage/migrations/0018_asset_retrieval_evidences.sql`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/platform-api/src/asset_profile_supply_support.rs`
- Modify: `crates/platform-api/src/assistant_run_model_supply_item_support.rs`
- Modify: `crates/platform-api/src/assistant_run_model_supply_budget_support.rs`
- Modify: `crates/platform-api/src/lib.rs`
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

**Completion receipt (2026-07-11):**

- 独立 worktree 先保留 migration red phase，再实现 `0018_asset_retrieval_evidences.sql`、append-only repository、accepted-profile materializer、safe evidence ref、统一 document/asset ranking 和 compact assistant supply；没有伪装 document/chunk，也没有复制 raw provider JSON 或 locator。
- 本地门禁通过：storage migrations 6/6、storage asset retrieval 2/2、platform asset-profile supply 14/14、assistant supply 20/20、contracts unified hit、retrieval-worker disposable-DB gate registration、workspace check、fmt、主站 self-test 和 diff check。
- 实现提交 `08b37723ace5d14a8a5c1b8aeba7490f3db47f3c` 已推送；GitHub Actions `29155355921` 的 Rust Minimal 与 No-Credential Smoke 均成功。
- 8 服务器一次性数据库门禁证明真实 `0018` migration、同 tenant/dataset membership、重复写入幂等、多 membership、跨 dataset/tenant 拒绝和 permission-safe search；测试库已 drop，报告为 `/srv/aiv3/backups/task10-disposable-db-08b37723-20260711T140524Z/report.json`。
- feature-off 部署前使用 PostgreSQL 18.4 客户端完成 57,306,878-byte custom dump 和 515-line restore list；回退目录 `/srv/aiv3/backups/task10-feature-off-08b37723-20260711T141257Z`。服务器 fast-forward 到精确 SHA，release platform-api build 通过，显式设置 evidence write false，仅重启 platform-api；ingest/retrieval/Web PID 未变，`0018` 已应用且初始 0 行。
- 生产运行路径实际位于 platform-api + storage；retrieval-worker 变更仅为 disposable-DB gate，因此没有为无运行时代码的 worker 制造重启。feature-off 定向测试、公开契约、主站 streaming 和静态页 self-test 均通过。
- live canary 在 Task 9 保留的私有 asset scope 上物化 1 条 eligible evidence；同用户私有 document-control dataset 经产品 API register/ingest 后返回 1 条普通文档证据且 0 条 asset hint。精确 selected scope 下，write-on 和 write-off 均读取到 1 个 `asset-evidence://` ref，行数保持 `1 -> 1 -> 1`，unsafe rows 为 0。
- 结束后 session 已撤销，Cookie 已移除，evidence-write 恢复 false；最终 API/ingest/retrieval/Web PID 为 `922486`/`872657`/`601686`/`712419`，health/ready 200、priority error 0、远端工作区干净。安全回执、最终回归和合并 no-delete manifest 位于 `/srv/aiv3/backups/task10-live-08b37723-20260711T144614Z`。
- 合并 cleanup inventory 覆盖 8 个尝试备份目录、7 个测试 dataset、4 个控制文档、8 个 assistant run、4 个已撤销 session 和 1 条 asset evidence；`automatic_cleanup_allowed=false`，未自动删除任何测试或 pilot 数据。

**Stop:** fabricated document/chunk, cross-scope match, locator leak or material ordinary-chat regression.

**Done:** migration, commit, deployment and canary are independently proven; asset evidence is attributable, permission-safe and idempotent; writes are off again.

---

## 9. Task 11：第三方私有 `asset-imports` pilot

**Status:** PASS；实现、CI、一次性数据库门禁、feature-off 部署、单 connection live、跨连接拒绝、公开契约 guard、rollback 和 retained cleanup manifest 均已完成。

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

**Current receipt (2026-07-11):**

- 实现提交 `be2ef77f8ffc266edc863e5ad7980b4bea3b5ce0` 已推送；GitHub Actions `29157911807` 的 Rust Minimal 与 No-Credential Smoke 全绿。
- PostgreSQL 18.4 一次性数据库门禁证明 request replay、replacement、historical replay 不回滚、request conflict、library/dataset/connection owner isolation；测试库已 drop，报告位于 `/srv/aiv3/backups/task11-disposable-db-be2ef77f-20260711T152830Z`。
- feature-off 部署备份为 `/srv/aiv3/backups/task11-feature-off-be2ef77f-20260711T153414Z`；57,385,780-byte custom dump 与 528-line restore list 已验证，只重启 platform-api，ingest/retrieval/Web PID 未变。
- live 仅 allowlist 一个临时连接，完成 single、exact replay、idempotency conflict、batch、ZIP、replacement、historical replay no rollback 和第二连接拒绝；安全聚合为 asset 5、dataset membership 5、parse run 5、request history entry 6，provider call 0。
- public contract guard 在开启窗口与关闭后均通过；响应只含 request id、opaque task ref、parse status 与安全 asset summary，没有 tenant/dataset/library/collection/object locator/internal metadata 泄漏。
- 结束后 `EXTERNAL_ASSET_IMPORT_ENABLED=false`、allowlist 为空、两个临时连接 disabled；health/ready 200、priority error 0，ingest/retrieval/Web PID 保持 `872657`/`601686`/`712419`。
- 成功 live、安全回归和 mode-0600 no-delete manifest 位于 `/srv/aiv3/backups/task11-live-be2ef77f-20260711T155457Z`；两次 operator PATH 失败尝试分别保留 mode-0600 cleanup manifest，未自动删除任何对象。

---

## 10. Task 12：生产级 operator 与受控并发门禁

**Status:** PASS；operator smoke 契约修复、CI、认证 preflight、单请求基线、5→10→20 分级 chat、5 路 heavy static-page、fallback/queue guard、会话撤销和 no-delete manifest 均已完成。

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

**Current receipt (2026-07-11):**

- `f71fe4cfe4a059222bc6d4e3363fc3bec376e11a` 修复 operator smoke 对当前 API 契约的读取与脱敏误报；CI `29159537855` 全绿，8 服务器仅 fast-forward 脚本，服务零重启。
- 单请求基线：主站 p95 `3115 ms`、第三方 p95 `1308 ms`、静态页 p95 `9280 ms` 且产物 `1/1`；认证 model-gateway、前后 fallback guard、health/ready 和 priority-error 门禁通过。
- 分级主站 p95 为 `3125/6212/6349 ms`，第三方 p95 为 `1196/1409/2339 ms`；5/10/20 各级失败数均为 0，assistant/profile queue residual 均为 0。
- heavy static-page `5/5` 完成，p95 `16918 ms`；Codex concurrency guard 为 2，watched fallback queue 未超过 2。
- platform-api、assistant-run-worker、static-page-worker、Web 均未重启，priority error 0；全部临时 session 已撤销，cleanup 未执行。
- 单请求与分级安全回执分别位于 `/srv/aiv3/backups/task12-baseline-f71fe4cf-20260711T162748Z` 和 `/srv/aiv3/backups/task12-staged-f71fe4cf-20260711T163911Z`。
- 结论：当前 20 路 chat 和 5 路 heavy static-page 无容量/队列/worker 瓶颈，不需要配置或容量变更；Task 13 可继续。

---

## 11. Task 13：V3 与 Codex 客户端联合验收

**Status:** PASS；隔离测试身份、受控 workspace、private/sandbox publish、报告续版、会话撤销和 no-delete 回执均已完成。

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

**Current receipt (2026-07-12):**

- canonical 契约与 `codex-web` 镜像完成精确同步，双方 SHA-256 均为 `0e41b06835321ea5364c37275047a50ce457d043d25ad19a4e858998b0f40d26`；客户端提交为 `a40d3a2`，未夹带其现存其他改动。
- `d593605545e392fa17f181d195b4537d9b75f978` 补齐首次与续版文件哈希、dataset/library 附件、任务卡、sandbox publish 和 `revision_of` 报告续版验证；CI `29160764770` 全绿。
- 精确 `http://127.0.0.1:3000` platform-api preflight 通过；Web `http://127.0.0.1:3002` 即使 HTTP 200 仍因返回 HTML 被正确拒绝。
- 8 服务器只 fast-forward 脚本，无构建、migration、flag 变更或服务重启；API/Web/ingest PID 保持 `954732`/`712419`/`872657`。
- 一次性 local-key 身份、1 个 bootstrap dataset、1 个 private pilot dataset、1 个 private asset library 与 membership 均通过产品 API 创建；联合链路生成 1 个 config package、2 个 artifact、4 个文件，全部哈希/附件/任务卡/发布/续版检查通过。
- 首版生成独立 public sandbox link 并验证无 script tag；续版只做独立 private publish，没有覆盖稳定 URL。客户端未修改 V3 source、migration、auth、public API、systemd 或部署状态。
- 会话已 logout 并复核失效，Cookie 已销毁；六项暗发布 flags 关闭、两项 allowlist 为空、health/ready 正常、priority error 为 0。
- 脱敏联合回执和不自动删除的 private cleanup manifest 位于 `/srv/aiv3/backups/task13-live-d5936055-20260711T170522Z`。

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
5. 当前 R5 Task 的精确 commit、push、8 服务器部署和受控 live 适用 standing authorization；仅在 force/删除、外部客户系统、10/120 服务器或明显扩展计划时重新请求授权。

### 13.2 新线程启动指令

```text
在 C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3 工作。

唯一活动计划：docs/plans/datamax-active-execution-plan.md
唯一验证账本：docs/validation/datamax-main-gap-closure.md

先读取计划的“当前可信基线、授权边界、通用门禁”和当前 READY Task。
先运行 git status --short --branch；不得 stage 或修改 decks/ 和
docs/plans/2026-07-07-server10-aiv3-deployment-plan.md。

Task 7–13 已全部 PASS；当前没有 READY 功能 Task。只执行 R5 逐项完成审计和整体归档，
不得重跑 live、不得删除 Task 9/10/11/12/13 的 no-delete pilot 数据，也不得从旧计划恢复任务。
```

### 13.3 本计划完成条件

Task 7–13 全部 `PASS`，所有临时 flags/allowlists 恢复关闭，P0 关闭，parser/evidence/private pilot 有独立提交和回退证明，operator 与客户端联合验收有脱敏回执。完成后将本文整体迁入 `docs/archive/plans/`，再决定是否建立新的工程治理计划。

**Closure (2026-07-12):** 上述条件全部满足。完成审计见 `docs/validation/datamax-r5-completion-audit-20260712.md`。本轮不自动建立新的工程治理计划；后续治理需以新的用户目标、独立活动计划和新基线启动。
