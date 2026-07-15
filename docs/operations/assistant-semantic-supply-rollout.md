# DataMax 知识图谱辅助供料 feature-off / 未来 shadow 发布手册

## 1. 目的、现场和最终状态

本文只用于 8 服务器上知识图谱辅助问答供料的 Task 10 受控发布。已核实的现场基线为：

- 仓库：`/srv/aiv3/repo`；
- API：`aiv3-platform-api.service`，本机探测地址为 `http://127.0.0.1:3000`；
- 配置文件：`/etc/aiv3/aiv3.env`；
- PostgreSQL 服务端：18.4；
- 构建与切换范围：只构建、重启 `platform-api`；
- 首个 shadow 数据集：“新百项目资料”，dataset UUID 为 `d4923d83-6053-4feb-8005-b22ee51e0227`。

完整的未来发布顺序为：

1. 预检、备份并仅 fast-forward 到批准的 GitHub commit；
2. 离线执行不访问 provider 的聚焦门禁；
3. API-only 构建，以 `off` 首次发布并验证；
4. 仅对一个批准 tenant、一个批准 user 和“新百项目资料”开启短时 `shadow`；
5. 不发起问答请求，只做健康、日志安全和服务边界检查；
6. 正常结束也恢复到显式 `off`。

当前 Task 8 已返回 `retrieval_candidate=null`，因此本次 Task 10 只执行步骤 1–3 的 Phase A feature-off 发布；步骤 4–6 的 shadow 窗口不执行。本文不执行 Task 11 的 live A/B/C；不允许 `rerank` 或 `supplement`。Task 10 的预期最终状态是新二进制保留、四项配置显式 feature-off、API 健康、其他服务未重启。

当前 v2 只证明同次 off/A 实现路径自一致，没有独立采集的发布前冻结 feature-off 回执。因此 Phase A 只能得出 `feature_off_operational_install_only`，不能得出检索/答案提升、promotion 或 `feature_off_shadow_safe`。后者只属于未来具备非空离线候选、独立冻结基线并另行批准的 shadow 窗口。

## 2. 不可越过的边界

- 只允许编排已授权、可追溯的供料，不编排对话、意图、答案结构、结论、措辞、路由或动作；图谱提示不是事实或引用，模型始终自行理解和作答。
- 三个 allowlist 都是英文逗号分隔的**精确 UUID 等值列表**，三者必须同时命中才可生效。
- `*` 只是普通字符，不是通配符；本文禁止写入 `*`，也禁止用空 allowlist 表示“全部”。
- shadow 仅计算内部候选差异，必须保持候选输出、顺序、模型可见供料、公开响应、路由和副作用与 baseline 一致。
- 外部通道第一版只允许有效模式为 `shadow`；`rerank`、`supplement` 在外部通道必须 fail closed 到 `off`。
- shadow 验证禁止调用任何真实或占位 provider，禁止发送主站或外部通道问答消息，禁止运行 live answer smoke。
- 不构建、不重启 Web、retrieval worker、document-enrichment worker、link worker、ingest worker 或其他 worker。
- 不运行 migration，不修改图谱生成配置，不触发 backfill，不写入或重建 semantic snapshot / pair snapshot。
- 不删除、清空、回收或自动清理任何业务数据、pilot/canary 数据、assistant run、检索证据或图谱快照。
- 不把 tenant/user UUID、凭据、连接串、session、原始问题、业务值、内部路径或 snapshot 内容打印到终端、普通回执、GitHub 或共享记忆。

以下任一条件成立必须停止进入下一阶段：

- 服务器 worktree 不 clean，HEAD 不能 fast-forward 到批准 commit；
- PostgreSQL 服务端不能确认是 18.4；
- API 在切换前不 active，或 `/healthz`、`/readyz` 任一失败；
- 聚焦测试出现失败或零测试假绿；
- 三个精确 allowlist 不能从批准渠道取得或不是 canonical lowercase UUID；
- 发现权限泄漏、隐藏标签参与匹配、不可溯源断言、公开字段变化或 model-facing semantic trace；
- route/action/workflow/artifact/template/report/static-page 发生非预期变化；
- 同次 off/A 供料顺序或模型可见证据不自一致；或未来 shadow 缺少独立冻结的发布前 feature-off 基线；
- provider 调用或 retry 增加，检索 p95 增量超过 100ms 或 20%；
- snapshot 错误阻断正常回答，或 NewBai 目标证据门禁低于 8/8；
- Web/任一 worker 的启动时间、重启计数或状态在窗口内变化。

## 3. 单一发布会话和批准值

通过既有跳板链路进入 8 服务器，在同一个 root Bash 会话内执行全文命令；中途重连后不得沿用旧变量或旧判断。

```bash
ssh -J windows-jump 8服务器
sudo -i
bash

set -euo pipefail
umask 077

REPO=/srv/aiv3/repo
ENV_FILE=/etc/aiv3/aiv3.env
API_UNIT=aiv3-platform-api.service
API_BASE=http://127.0.0.1:3000
RELEASE_REF=origin/codex/dataset-understanding-mvp

read -r -p 'approved GitHub commit (40 lowercase hex): ' APPROVED_HEAD
[[ "$APPROVED_HEAD" =~ ^[0-9a-f]{40}$ ]]
```

当前 `retrieval_candidate=null` 的 Phase A feature-off 发布不读取或设置 tenant/user/dataset UUID，三个 allowlist 必须保持空。只有未来出现离线候选且 shadow 另行获准时，才在同一发布会话中从批准的密钥或配置渠道静默取得三个 canonical lowercase UUID，导出为 `APPROVED_TENANT_ID`、`APPROVED_DATASET_ID`、`APPROVED_USER_ID`，再执行下列门禁；不得写入本文、命令历史或回执：

```bash
export APPROVED_TENANT_ID APPROVED_DATASET_ID APPROVED_USER_ID
python3 - <<'PY'
import os
from uuid import UUID

for name in ("APPROVED_TENANT_ID", "APPROVED_USER_ID", "APPROVED_DATASET_ID"):
    value = os.environ[name]
    if value != str(UUID(value)) or "*" in value or "," in value:
        raise SystemExit(f"{name} must be one canonical lowercase UUID")
if os.environ["APPROVED_DATASET_ID"] != "d4923d83-6053-4feb-8005-b22ee51e0227":
    raise SystemExit("dataset is not the approved NewBai dataset")
print("approved exact UUID inputs: valid (values suppressed)")
PY
```

## 4. 预检

### 4.1 Git、API 和 PostgreSQL

```bash
test "$(id -u)" -eq 0
test "$(git -C "$REPO" rev-parse --is-inside-work-tree)" = "true"
test "$(git -C "$REPO" rev-parse --show-toplevel)" = "$REPO"
test -f "$ENV_FILE"
test -x "$REPO/target/release/platform-api"

cd "$REPO"
git fetch --prune origin
test -z "$(git status --porcelain)"

SERVER_HEAD_BEFORE="$(git rev-parse HEAD)"
TARGET_HEAD="$(git rev-parse "${RELEASE_REF}^{commit}")"
test "$TARGET_HEAD" = "$APPROVED_HEAD"
git merge-base --is-ancestor "$SERVER_HEAD_BEFORE" "$TARGET_HEAD"

git status --porcelain
git rev-parse HEAD
systemctl is-active "$API_UNIT"
curl -fsS "$API_BASE/healthz"
curl -fsS "$API_BASE/readyz"

PG_SERVER_VERSION="$(sudo -u postgres psql -X -A -t -c 'show server_version')"
test "$PG_SERVER_VERSION" = "18.4"
printf 'postgres_server_version=%s\n' "$PG_SERVER_VERSION"
```

`psql --version` 只反映客户端版本，不可代替 `show server_version`。若现场数据库不是本机 peer-auth 连接，改用批准的只读连接渠道执行同一 SQL；不得在参数、日志或回执中暴露 DSN。

### 4.2 记录所有非 API 服务状态

以下快照用于证明 Web 和所有 worker 没有被本轮重启。动态收集全部 `aiv3*.service`，只排除 API 自身。

```bash
capture_non_api_service_state() {
  local output="$1"
  mapfile -t units < <(
    systemctl list-unit-files --type=service --no-legend 'aiv3*.service' \
      | awk '{print $1}' \
      | grep -v '^aiv3-platform-api\.service$' \
      | sort -u
  )
  test "${#units[@]}" -gt 0
  : > "$output"
  for unit in "${units[@]}"; do
    printf '[%s]\n' "$unit" >> "$output"
    systemctl show "$unit" \
      -p ActiveState \
      -p SubState \
      -p NRestarts \
      -p ExecMainStartTimestampMonotonic \
      --no-pager >> "$output"
  done
}
```

## 5. 备份和不删除清单

先备份配置、运行中 API 二进制、HEAD 和非 API 服务快照。原始环境文件只保存在 `0700` 目录，不复制到 Git 仓库或普通回执。

```bash
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BACKUP_DIR="/srv/aiv3/backups/assistant-semantic-supply-${STAMP}"
install -d -m 0700 "$BACKUP_DIR"

cp --preserve=all "$ENV_FILE" "$BACKUP_DIR/aiv3.env.before"
cp --preserve=all "$REPO/target/release/platform-api" \
  "$BACKUP_DIR/platform-api.before"
printf '%s\n' "$SERVER_HEAD_BEFORE" > "$BACKUP_DIR/head.before"
capture_non_api_service_state "$BACKUP_DIR/non-api-services.before"
systemctl show "$API_UNIT" \
  -p ActiveState -p SubState -p NRestarts -p ExecMainStartTimestampMonotonic \
  --no-pager > "$BACKUP_DIR/platform-api-service.before"

python3 - <<'PY' > "$BACKUP_DIR/cleanup-manifest.json"
import json
print(json.dumps({
    "auto_delete": False,
    "protected": [
        "business_data",
        "pilot_and_canary_data",
        "assistant_runs_and_evidence",
        "dataset_semantic_snapshots",
        "dataset_semantic_link_snapshots"
    ],
    "note": "Rollout and rollback never delete protected data."
}, ensure_ascii=False, indent=2))
PY

(
  cd "$BACKUP_DIR"
  sha256sum \
    aiv3.env.before \
    platform-api.before \
    head.before \
    non-api-services.before \
    platform-api-service.before \
    cleanup-manifest.json > SHA256SUMS
  sha256sum -c SHA256SUMS
)
```

备份目录和 cleanup manifest 默认保留；本文没有任何自动清理步骤。

## 6. 安全写入和核对四项配置

下面两个函数只修改四个目标 key，保留环境文件其他内容、所有者和权限。重复 key、非 canonical UUID、`*`、多个 UUID 或非 `off|shadow` 模式都会 fail closed。

```bash
set_semantic_supply_env() {
  local requested_mode="$1"
  REQUESTED_SEMANTIC_SUPPLY_MODE="$requested_mode" python3 - <<'PY'
import os
import stat
import tempfile
from pathlib import Path
from uuid import UUID

path = Path("/etc/aiv3/aiv3.env")
mode = os.environ["REQUESTED_SEMANTIC_SUPPLY_MODE"]
keys = (
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_MODE",
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_TENANT_ALLOWLIST",
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_DATASET_ALLOWLIST",
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_USER_ALLOWLIST",
)

def exact_uuid(name):
    value = os.environ[name]
    if value != str(UUID(value)) or "*" in value or "," in value:
        raise SystemExit(f"{name} must be one canonical lowercase UUID")
    return value

if mode == "off":
    values = ("off", "", "", "")
elif mode == "shadow":
    tenant = exact_uuid("APPROVED_TENANT_ID")
    dataset = exact_uuid("APPROVED_DATASET_ID")
    user = exact_uuid("APPROVED_USER_ID")
    if dataset != "d4923d83-6053-4feb-8005-b22ee51e0227":
        raise SystemExit("shadow dataset is not NewBai")
    values = ("shadow", tenant, dataset, user)
else:
    raise SystemExit("Task 10 permits only off or shadow")

updates = dict(zip(keys, values, strict=True))
source = path.read_text(encoding="utf-8").splitlines()
counts = {key: 0 for key in keys}
for line in source:
    stripped = line.lstrip()
    if not stripped or stripped.startswith("#") or "=" not in stripped:
        continue
    key = stripped.split("=", 1)[0].strip()
    if key in counts:
        counts[key] += 1
duplicates = [key for key, count in counts.items() if count > 1]
if duplicates:
    raise SystemExit("duplicate semantic supply environment keys")

output = []
seen = set()
for line in source:
    stripped = line.lstrip()
    key = stripped.split("=", 1)[0].strip() if "=" in stripped else ""
    if not stripped.startswith("#") and key in updates:
        output.append(f"{key}={updates[key]}")
        seen.add(key)
    else:
        output.append(line)
for key in keys:
    if key not in seen:
        output.append(f"{key}={updates[key]}")

metadata = path.stat()
fd, temporary = tempfile.mkstemp(prefix=".aiv3.env.", dir=path.parent)
try:
    with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as stream:
        stream.write("\n".join(output) + "\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.chmod(temporary, stat.S_IMODE(metadata.st_mode))
    os.chown(temporary, metadata.st_uid, metadata.st_gid)
    os.replace(temporary, path)
except BaseException:
    try:
        os.unlink(temporary)
    except FileNotFoundError:
        pass
    raise

entry_count = 0 if mode == "off" else 1
print(f"semantic_supply_mode={mode} exact_allowlist_counts={entry_count}/{entry_count}/{entry_count}")
PY
}

verify_semantic_supply_env() {
  local expected_mode="$1"
  EXPECTED_SEMANTIC_SUPPLY_MODE="$expected_mode" python3 - <<'PY'
import os
from pathlib import Path

keys = (
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_MODE",
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_TENANT_ALLOWLIST",
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_DATASET_ALLOWLIST",
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_USER_ALLOWLIST",
)
values = {}
for line in Path("/etc/aiv3/aiv3.env").read_text(encoding="utf-8").splitlines():
    stripped = line.strip()
    if not stripped or stripped.startswith("#") or "=" not in stripped:
        continue
    key, value = stripped.split("=", 1)
    key = key.strip()
    if key in keys:
        if key in values:
            raise SystemExit("duplicate semantic supply environment keys")
        values[key] = value.strip()
if set(values) != set(keys):
    raise SystemExit("semantic supply environment key missing")

mode = os.environ["EXPECTED_SEMANTIC_SUPPLY_MODE"]
if values[keys[0]] != mode:
    raise SystemExit("semantic supply mode mismatch")
if mode == "off":
    expected = ("", "", "")
elif mode == "shadow":
    expected = (
        os.environ["APPROVED_TENANT_ID"],
        os.environ["APPROVED_DATASET_ID"],
        os.environ["APPROVED_USER_ID"],
    )
else:
    raise SystemExit("Task 10 permits only off or shadow")
actual = tuple(values[key] for key in keys[1:])
if actual != expected or any("*" in item for item in actual):
    raise SystemExit("semantic supply exact allowlist mismatch")
entry_count = 0 if mode == "off" else 1
print(f"verified_mode={mode} exact_allowlist_counts={entry_count}/{entry_count}/{entry_count} values=suppressed")
PY
}

wait_for_api() {
  local attempt
  for attempt in $(seq 1 30); do
    if systemctl is-active --quiet "$API_UNIT" \
      && curl -fsS "$API_BASE/healthz" >/dev/null \
      && curl -fsS "$API_BASE/readyz" >/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}
```

## 7. Fast-forward、无 provider 门禁和 API-only 构建

### 7.1 只允许 fast-forward

```bash
cd "$REPO"
test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD)" = "$SERVER_HEAD_BEFORE"
git merge --ff-only "$TARGET_HEAD"
test "$(git rev-parse HEAD)" = "$APPROVED_HEAD"
test -z "$(git status --porcelain)"
```

禁止用 reset、checkout 或强制更新掩盖分叉/脏工作区。

### 7.2 离线聚焦门禁

这些 filter 都是进程内单元/契约测试，不创建 live AssistantRun，也不访问外部 provider。每个 filter 必须至少执行一项测试，不能只看 Cargo 退出码。

```bash
run_filtered_test() {
  local filter="$1"
  local log="$BACKUP_DIR/test-${filter}.log"
  cargo test -p platform-api "$filter" --lib -- --nocapture 2>&1 | tee "$log"
  grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$log"
}

run_filtered_test semantic_supply_shadow
run_filtered_test semantic_supply_model_autonomy
run_filtered_test external_channel_public_citation
run_filtered_test assistant_run_provider_input_adds_model_supply_brief
run_filtered_test assistant_run_provider_input_keeps_plain_ordinary_chat_unrestricted

bash scripts/run-semantic-supply-ab-smoke.sh --self-test --pretty \
  2>&1 | tee "$BACKUP_DIR/semantic-supply-self-test.log"
```

`semantic_supply_shadow` 必须覆盖三 allowlist 精确命中、`*` 非通配、外部通道只允许 shadow，以及 off/shadow 排名签名字节等价。model autonomy / public citation filter 必须证明内部 semantic 字段不进入 model/public supply；两个精确 provider-input filter 分别保护现有 model supply brief 和无数据集普通问答契约。禁止改回宽泛 `assistant_run_provider_input` filter，因为它会混入冻结基线中的已知无关失败；此阶段也禁止换成任何 live capture、主站问答或外部通道 smoke。

### 7.3 只构建 API

8 服务器统一使用已验证的 Clang 工具链：

```bash
cd "$REPO"
CC=clang CXX=clang++ cargo build --locked --release -p platform-api
test -x target/release/platform-api
sha256sum target/release/platform-api > "$BACKUP_DIR/platform-api.after.sha256"
test -z "$(git status --porcelain)"
```

不要构建 Web 或任何 worker；不要执行 migration、backfill 或图谱生成命令。

## 8. Phase A：显式 feature-off 首次发布

先写入显式 off 和三个空 allowlist，再且只重启 API：

```bash
FEATURE_OFF_START="$(date --iso-8601=seconds)"
set_semantic_supply_env off
verify_semantic_supply_env off

systemctl restart "$API_UNIT"
wait_for_api

cd "$REPO"
git status --porcelain
git rev-parse HEAD
systemctl is-active aiv3-platform-api.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz

test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD)" = "$APPROVED_HEAD"
```

Feature-off 的同次 off/A 路径自一致门禁以第 7.2 节的离线测试为准；它不是独立冻结基线，也不支持晋级。Task 10 不通过真实问答“再证明一次”，因为那会产生 provider 调用和回答副作用；只有未来先产生离线候选、实现独立基线、另行改代码解除 live harness 硬锁并重新审查后，真实答案比较才可能进入新的 Task 11。

记录非 API 服务仍原样：

```bash
capture_non_api_service_state "$BACKUP_DIR/non-api-services.feature-off"
cmp -s \
  "$BACKUP_DIR/non-api-services.before" \
  "$BACKUP_DIR/non-api-services.feature-off"
```

若 API 不健康、HEAD/status 不符、feature-off 测试不等价或非 API 服务发生变化，立即按第 11 节回滚，不进入 shadow。

## 9. Phase B：三重精确 allowlist shadow

### 9.1 开启 shadow

shadow 只能使用已静默读入的一个 tenant、一个受控 user 和固定 NewBai dataset。配置函数会拒绝多个条目和 `*`。

```bash
SHADOW_START="$(date --iso-8601=seconds)"
set_semantic_supply_env shadow
verify_semantic_supply_env shadow

systemctl restart "$API_UNIT"
wait_for_api

cd "$REPO"
git status --porcelain
git rev-parse HEAD
systemctl is-active aiv3-platform-api.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz

test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD)" = "$APPROVED_HEAD"
```

### 9.2 shadow 窗口允许和禁止的验证

允许：

- 重跑第 7.2 节的纯单元/契约测试；
- 调用 `/healthz` 和 `/readyz`；
- 只读检查 API systemd 状态和本窗口日志；
- 比较非 API 服务快照。

禁止：

- 调用 AssistantRun、chat、external-channel message 或 continue endpoint；
- 运行 NewBai live capture、主站问答、外部直答或文档理解 live smoke；
- 使用 placeholder、mock gateway 或真实 provider 生成回答；
- 为获得 shadow receipt 而人工创建 run；
- 把模式改成 `rerank` / `supplement`；
- 修改图谱、快照、字典、pair、dataset membership 或任何业务/pilot 数据。

因此未来获准的 Task 10 shadow 窗口可以没有 `assistant semantic supply receipt`，这是“未发问、未调用 provider”的预期结果，不得把 receipt=0 当作失败后补发问题。算法和字节等价由离线门禁验证；只有未来先产生离线候选、另行解除当前机械硬锁并重新审查后，live 证据与答案验证才可能进入新的 Task 11。当前 `retrieval_candidate=null` 的执行不进入本节 shadow 窗口。

## 10. 日志安全和服务边界检查

结束 shadow 观察时先记录时间，只输出计数，不输出原始日志行：

```bash
SHADOW_END="$(date --iso-8601=seconds)"
journalctl -u "$API_UNIT" \
  --since "$SHADOW_START" \
  --until "$SHADOW_END" \
  --no-pager -o cat \
  | python3 -c '
import json, re, sys
lines = list(sys.stdin)
receipt = [line for line in lines if "assistant semantic supply receipt" in line]
sensitive = re.compile(
    r"(?i)(authorization\s*[:=]|bearer\s+[A-Za-z0-9]|"
    r"(?:password|api[_-]?key|access[_-]?token)\s*[:=]|"
    r"postgres(?:ql)?://|[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-"
    r"[89ab][0-9a-f]{3}-[0-9a-f]{12}|[0-9a-f]{64})"
)
unsafe_receipts = sum(bool(sensitive.search(line)) for line in receipt)
errors = sum(
    bool(re.search(r"(?i)(panic|segmentation fault|readiness check failed)", line))
    for line in lines
)
report = {
    "line_count": len(lines),
    "semantic_receipt_count": len(receipt),
    "unsafe_semantic_receipt_count": unsafe_receipts,
    "fatal_or_readiness_error_count": errors,
}
print(json.dumps(report, sort_keys=True))
if unsafe_receipts or errors:
    raise SystemExit(1)
' | tee "$BACKUP_DIR/shadow-journal-safety.json"

capture_non_api_service_state "$BACKUP_DIR/non-api-services.shadow"
cmp -s \
  "$BACKUP_DIR/non-api-services.before" \
  "$BACKUP_DIR/non-api-services.shadow"
```

当前实现的 semantic receipt 只允许出现模式、reason、候选/命中/alias/可见来源/补证候选/排名变化计数和耗时；不得出现 UUID、label、业务值、hash、路径或凭据。若安全扫描失败，不打印命中行，直接回滚并在受限备份目录内调查。

本文允许的 HTTP 调用只有本机 `/healthz` 和 `/readyz`，因此 `provider_calls_initiated_by_runbook` 必须为 0。若现场监控显示 shadow 窗口 provider 请求或 retry 增加，即使来源不明，也先按失败处理并恢复 off。

## 11. 正常收尾与回滚

### 11.1 正常收尾：恢复 off

shadow 验证完成后不长期保留开关，恢复显式 off，并且仍只重启 API：

```bash
set_semantic_supply_env off
verify_semantic_supply_env off

systemctl restart "$API_UNIT"
wait_for_api

cd "$REPO"
git status --porcelain
git rev-parse HEAD
systemctl is-active aiv3-platform-api.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz

test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD)" = "$APPROVED_HEAD"
capture_non_api_service_state "$BACKUP_DIR/non-api-services.final"
cmp -s \
  "$BACKUP_DIR/non-api-services.before" \
  "$BACKUP_DIR/non-api-services.final"
```

### 11.2 一级回滚：仅关闭 semantic supply

任何 shadow 专项门禁失败，先执行与正常收尾相同的 `set_semantic_supply_env off`、API-only restart 和 health/ready 检查。不得停止或重启 Web/worker，不得删除快照、证据、run、业务或 pilot 数据。

关闭后若 API 健康、feature-off 离线等价门禁仍通过，则保留批准 HEAD 和新二进制，记录 `rollback=feature_off_only`。

### 11.3 二级回滚：恢复发布前配置和 API 二进制

只有 feature-off 本身不等价、API-only 构建存在缺陷，或一级回滚不能恢复健康时才执行。执行前校验备份；整个过程仍只重启 API。

```bash
(
  cd "$BACKUP_DIR"
  sha256sum -c SHA256SUMS
)

cp --preserve=all "$BACKUP_DIR/aiv3.env.before" \
  /etc/aiv3/.aiv3.env.semantic-supply-rollback
mv -f /etc/aiv3/.aiv3.env.semantic-supply-rollback "$ENV_FILE"

cp --preserve=all "$BACKUP_DIR/platform-api.before" \
  "$REPO/target/release/.platform-api.semantic-supply-rollback"
mv -f "$REPO/target/release/.platform-api.semantic-supply-rollback" \
  "$REPO/target/release/platform-api"

systemctl restart "$API_UNIT"
wait_for_api

cd "$REPO"
git status --porcelain
git rev-parse HEAD
systemctl is-active aiv3-platform-api.service
curl -fsS http://127.0.0.1:3000/healthz
curl -fsS http://127.0.0.1:3000/readyz

test -z "$(git status --porcelain)"
capture_non_api_service_state "$BACKUP_DIR/non-api-services.rollback"
cmp -s \
  "$BACKUP_DIR/non-api-services.before" \
  "$BACKUP_DIR/non-api-services.rollback"
```

二级回滚不 reset Git HEAD；回执必须同时记录 approved source HEAD 和实际运行的发布前 binary SHA-256。若必须回退源码、迁移或其他服务，已超出本文权限，停止并另行制定计划。

## 12. 最终回执和完成标准

回执只允许记录以下非敏感内容：

- UTC 开始/结束时间、批准 commit、服务器最终 HEAD 和 clean 状态；
- PostgreSQL 服务端版本 18.4；
- 发布前/后/实际运行 API binary SHA-256；
- 离线 filter 的非零 passed 数和 self-test 结论；
- feature-off / shadow / final-off 的 API active、health、ready；当前 Phase A 回执必须将 shadow 和 final-off 写为 `not_executed`/`N/A`，不得用缺失值或默认值伪装已执行 shadow；
- 配置模式与三个 allowlist 的条目数量，只写 `0/0/0` 或 `1/1/1`，不写 UUID；
- semantic receipt 数、unsafe receipt 数、fatal/readiness error 数；
- `provider_calls_initiated_by_runbook=0`；
- 非 API 服务快照是否 byte-equal；
- `cleanup_manifest_auto_delete=false`；
- 最终结论：当前 Phase A 只能写 `feature_off_operational_install_only`、`rollback_feature_off_only` 或 `rollback_binary`；`feature_off_shadow_safe` 只允许未来完整 shadow 窗口使用。

只有未来具备非空离线候选，并且以下项目全部满足才可写 `feature_off_shadow_safe`：

1. GitHub approved HEAD = 8 服务器 HEAD，repo clean；
2. PostgreSQL 服务端为 18.4；
3. 所有聚焦 filter 非零通过，存在独立冻结且哈希绑定的发布前 feature-off 基线，off/shadow 排名签名等价，model/public/provider contract 门禁通过；
4. 首次 feature-off、allowlisted shadow、最终 feature-off 三阶段 API 均 active、health、ready；
5. shadow 三 allowlist 恰好为一个 tenant、一个 NewBai dataset、一个 user，且不含 `*`；
6. shadow 阶段没有问答请求、provider 调用、retry 或客户可见副作用；
7. 日志没有敏感 receipt、panic 或 readiness error；
8. Web 和所有 worker 状态、启动时间和 `NRestarts` 与发布前一致；
9. 没有 migration、backfill、图谱/快照修改或数据删除；
10. 最终模式已经恢复为显式 `off`。

## 13. 手册命令自审

发布前和每次修改本文后，在本地仓库执行：

```powershell
$doc = 'docs/operations/assistant-semantic-supply-rollout.md'

# 四项配置必须完整，三 allowlist 必须明确 exact，星号必须明确非通配。
rg -n 'ASSISTANT_RUN_SEMANTIC_SUPPLY_(MODE|TENANT_ALLOWLIST|DATASET_ALLOWLIST|USER_ALLOWLIST)' $doc
rg -n '精确|exact|不是通配符|不允许.*\*|禁止.*\*' $doc

# 可执行 restart 只能指向 platform-api；Web/worker 只能做状态快照和禁止项说明。
rg -n 'systemctl restart' $doc
rg -n 'aiv3-(web|.*worker)' $doc

# HTTP 探测只能是本机 healthz / readyz，不得出现问答或外部消息 endpoint。
rg -n 'curl ' $doc
if (rg -n 'curl .*[a]ssistant|curl .*[c]hat|curl .*[e]xternal' $doc) { throw 'provider-capable endpoint found' }

# 不得出现数据库破坏命令，也不得出现 Web/worker 构建或重启命令。
if (rg -n -i '^\s*([d]rop|[t]runcate|[d]elete)\b|cargo build .*[w]orker|[p]npm .*build|systemctl restart .*[w]eb|systemctl restart .*[w]orker' $doc) { throw 'out-of-scope command found' }

git diff --check -- $doc
if ((Get-Content -Raw $doc) -match '(?m)[ \t]+$') { throw 'trailing whitespace found' }
git diff -- $doc
git status --short -- $doc
```

预期自审结论：所有可执行重启均且仅为 `aiv3-platform-api.service`；所有 HTTP 调用均且仅为 `127.0.0.1:3000/healthz|readyz`；不存在 Web/worker build/restart、provider-capable endpoint、migration/backfill 或数据删除命令。
