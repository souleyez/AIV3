# DataMax V3 Main/CI/8-Server Release Line

**Status:** Task 2 operator runbook

**Baseline date:** 2026-07-16

**Scope:** Reconcile GitHub `main`, deterministic CI and `/srv/aiv3/repo`, deploy the Task 1 connection-redaction fix with the existing PostgreSQL credential, prove a fresh safe journal window, and only then rotate the PostgreSQL 18.4 application credential through a root-only channel.

## 1. Authority and enforced boundaries

The 2026-07-16 GitHub audit found that the current private-repository plan does not provide usable branch-protection rules or environment reviewers. Do not describe either control as enabled. Until the repository plan changes, the release owner must manually enforce all three merge gates against the exact candidate and merged `main` SHA:

| Job id | Display name | Normal runner / Task 2 contingency | Manual decision |
|---|---|---|---|
| `no-credential-smoke` | `No-Credential Smoke` | `ubuntu-24.04` / isolated workstation runner | must be `success` |
| `rust-minimal` | `Rust P0` | `ubuntu-24.04` / isolated workstation runner | must be `success` |
| `web` | `Web` | `ubuntu-24.04` / isolated workstation runner | must be `success` |

These are the only three Task 2 merge-gate jobs. Untrusted PR code must never run on 8 server.

The first PR run (`29462642453`) proved a second GitHub account constraint: all three GitHub-hosted jobs were rejected before runner assignment because recent account payments failed or the Actions spending limit must be increased. This was a control-plane failure, not a test result. Task 2 may set `DATAMAX_CI_USE_ISOLATED_WINDOWS=true` only for a temporary non-production workstation runner; the workflow hardcodes its labels as `self-hosted`, `Windows`, `X64`, `aiv3-ci-isolated`, so the variable cannot redirect jobs to 8 server. While that override exists, every PR job also requires a same-repository head whose exact SHA equals `DATAMAX_CI_TRUSTED_SHA`; an absent or mismatched value skips the job. Record the exact runner name and SHA. After the final receipt checks, delete both variables and unregister the temporary runner so the workflow returns to its `ubuntu-24.04` default. Restoring GitHub-hosted capacity remains an account-level follow-up.

`.github/workflows/datamax-release.yml` is a separately dispatched, read-only handoff. Its only job is `server8-release-identity` / `Server8 Release Identity Gate`. It accepts a lowercase 40-character `expected_sha`, requires the dispatch ref and checked-out commit to be that exact merged `main` SHA, reruns the connection-log scanner, and verifies that `/srv/aiv3/repo` is a clean ancestor of the requested release. The workflow may read the server checkout but must not fetch into it, build, change files, restart services, edit secrets, run migrations or deploy. The `datamax-server8` environment name is an audit grouping only while environment reviewers are unavailable.

Actual deployment and PostgreSQL credential rotation are root-SSH operations performed from the approved operator path. GitHub Actions does not perform them.

## 2. Immutable release invariants

- Frozen deployed ancestor: `1edff9cdac585671f2ed64ed25690f2be121c750`.
- Connection-redaction floor: `6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7`.
- The release SHA must be the exact current `origin/main` SHA and must contain both ancestors above.
- The server may move only by fast-forward ancestry. No force push, rebase, history rewrite, `git reset --hard`, arbitrary ref deployment or cherry-pick is allowed.
- Feature flags remain unchanged. In particular, graph answer supply and asset import/parser gates are not enabled by this release.
- The old PostgreSQL credential remains active while the redacted binaries are first deployed and verified. Rotation before that proof is forbidden because an old binary could log the replacement secret.
- Once PostgreSQL rotation begins, no binary that lacks commit `6bc3ac45` may run again. `1edff9cd` is never a post-rotation rollback target.
- No command, workflow log, journal receipt, validation ledger or chat message may print a connection URL, password, token, environment-file content or matching journal line.
- Historical journal entries are retained. This runbook does not delete, vacuum or rewrite journals.

## 3. Select and prove the exact merged main SHA

From a clean operator checkout, inspect the reconciliation PR and verify the three exact job names above. Because GitHub cannot enforce them automatically on the current plan, the release owner records the decision manually and does not merge when any job is absent, pending, cancelled, skipped or failed.

```bash
gh pr checks "<PR_NUMBER>"
```

After merge, wait for the `push` run on `main`. Set the immutable release identity from `origin/main`, then inspect the run and its jobs:

```bash
git fetch origin main
RELEASE_SHA="$(git rev-parse origin/main)"
test "${#RELEASE_SHA}" -eq 40
git merge-base --is-ancestor 1edff9cdac585671f2ed64ed25690f2be121c750 "$RELEASE_SHA"
git merge-base --is-ancestor 6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7 "$RELEASE_SHA"

gh run list --workflow datamax-ci.yml --branch main --commit "$RELEASE_SHA"
gh run view "<MAIN_CI_RUN_ID>" --json headSha,status,conclusion,jobs \
  --jq '{headSha,status,conclusion,jobs:[.jobs[]|{name,status,conclusion}]}'
```

Required result: `headSha` equals `$RELEASE_SHA`; `No-Credential Smoke`, `Rust P0` and `Web` are present exactly once and all conclude `success`. Do not substitute an older green run or a feature-branch run.

Dispatch the read-only release identity gate against `main`:

```bash
gh workflow run datamax-release.yml --ref main -f expected_sha="$RELEASE_SHA"
gh run list --workflow datamax-release.yml --branch main
gh run watch "<RELEASE_GATE_RUN_ID>" --exit-status
```

Required result: `Server8 Release Identity Gate` succeeds for the same SHA and its summary says `Mutation performed by this workflow: none`.

## 4. Root-SSH server preflight

Enter 8 server through the approved root SSH alias. Keep shell tracing disabled so expanded environment values cannot reach the terminal or journal.

```bash
ssh 8服务器
set +x
set -euo pipefail
umask 077

RELEASE_SHA="<EXACT_40_CHARACTER_MAIN_SHA>"
FROZEN_ANCESTOR="1edff9cdac585671f2ed64ed25690f2be121c750"
REDACTION_FLOOR="6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7"
REPO="/srv/aiv3/repo"
ENV_FILE="/etc/aiv3/aiv3.env"

cd "$REPO"
test -z "$(git status --porcelain)"
PRE_DEPLOY_SHA="$(git rev-parse HEAD)"
git merge-base --is-ancestor "$FROZEN_ANCESTOR" "$PRE_DEPLOY_SHA"

git fetch origin main
test "$(git rev-parse origin/main)" = "$RELEASE_SHA"
git merge-base --is-ancestor "$PRE_DEPLOY_SHA" "$RELEASE_SHA"
git merge-base --is-ancestor "$FROZEN_ANCESTOR" "$RELEASE_SHA"
git merge-base --is-ancestor "$REDACTION_FLOOR" "$RELEASE_SHA"

test "$(stat -c '%U:%G' "$ENV_FILE")" = "root:root"
test "$(stat -c '%a' "$ENV_FILE")" = "600"

FLAG_SNAPSHOT="/root/aiv3-feature-flags.pre.${RELEASE_SHA}"
awk -F= '$1 ~ /(_ENABLED|_ALLOWLIST|_MODE)$/ {print}' "$ENV_FILE" \
  | LC_ALL=C sort > "$FLAG_SNAPSHOT"
chmod 600 "$FLAG_SNAPSHOT"
```

Every test must pass. A dirty checkout, missing frozen ancestor, non-fast-forward relationship, mismatched `origin/main`, or non-root-only secret file stops the release.

Record only `$PRE_DEPLOY_SHA`, `$RELEASE_SHA`, the boolean ancestry results, and `root:root 600`; do not record environment values.

## 5. Fast-forward and build the redacted binaries

Move the deployment checkout to the exact main SHA without rewriting history:

```bash
cd "$REPO"
git switch main
git merge --ff-only "$RELEASE_SHA"
git branch --set-upstream-to=origin/main main
test "$(git rev-parse HEAD)" = "$RELEASE_SHA"
test -z "$(git status --porcelain)"
```

The redaction helpers are statically linked through the shared observability/event-bus crates, so rebuild every Rust runtime package rather than only the crates with direct source edits. `retrieval-worker` also builds the dataset-semantic-link and document-enrichment binaries.

```bash
CC=clang CXX=clang++ cargo build --locked --release \
  -p platform-api \
  -p assistant-run-worker \
  -p chat-session-worker \
  -p codex-host-agent \
  -p dataset-output-worker \
  -p report-planner-worker \
  -p report-render-worker \
  -p retrieval-worker \
  -p memory-worker \
  -p external-source-worker \
  -p external-action-worker \
  -p media-worker \
  -p ingest-worker \
  -p static-page-worker
```

Do not rotate PostgreSQL yet.

## 6. Restart rebuilt Rust services with the old credential

Capture the beginning of the fresh journal window before restarting all sixteen rebuilt Rust services:

```bash
FRESH_WINDOW_START="$(date --iso-8601=seconds)"

RUST_SERVICES=(
  aiv3-platform-api.service
  aiv3-assistant-run-worker.service
  aiv3-chat-session-worker.service
  aiv3-codex-host-agent.service
  aiv3-dataset-output-worker.service
  aiv3-dataset-semantic-link-worker.service
  aiv3-report-planner-worker.service
  aiv3-report-render-worker.service
  aiv3-retrieval-worker.service
  aiv3-document-enrichment-worker.service
  aiv3-memory-worker.service
  aiv3-external-source-worker.service
  aiv3-external-action-worker.service
  aiv3-media-worker.service
  aiv3-ingest-worker.service
  aiv3-static-page-worker.service
)

test "${#RUST_SERVICES[@]}" -eq 16
systemctl restart "${RUST_SERVICES[@]}"
sleep 30
systemctl is-active "${RUST_SERVICES[@]}"
```

The fresh-window audit must output only service names and counts. It must never print matching lines:

```bash
count_credential_urls() {
  local since="$1"
  shift
  local unit count total=0
  for unit in "$@"; do
    count="$(
      journalctl -u "$unit" --since "$since" --no-pager -o cat 2>/dev/null \
        | grep -Eic '(postgres(ql)?|nats)://[^[:space:]"]+:[^[:space:]"@]+@' \
        || true
    )"
    printf '%s\t%s\n' "$unit" "$count"
    total=$((total + count))
  done
  printf 'TOTAL\t%s\n' "$total"
  test "$total" -eq 0
}

mapfile -t ALL_AIV3_SERVICES < <(
  systemctl list-units 'aiv3-*.service' --all --no-legend --plain \
    | awk '{print $1}' \
    | sort
)
count_credential_urls "$FRESH_WINDOW_START" "${ALL_AIV3_SERVICES[@]}"
```

Required result: every service and `TOTAL` report `0`. This is the mandatory proof that a replacement credential will not immediately be exposed by the deployed startup/error paths. Any nonzero count stops rotation; retain the old credential, diagnose without printing the matching record, and follow the pre-rotation rollback rule if service health is impaired.

Before crossing the rotation boundary, verify the changed services and public behavior:

```bash
for unit in "${RUST_SERVICES[@]}"; do
  test "$(systemctl is-active "$unit")" = "active"
  test "$(systemctl show "$unit" -p NRestarts --value)" = "0"
done

test "$(curl -sS -o /dev/null -w '%{http_code}' http://127.0.0.1:3000/healthz)" = "200"
test "$(curl -sS -o /dev/null -w '%{http_code}' http://127.0.0.1:3000/readyz)" = "200"
test "$(curl -sS -o /dev/null -w '%{http_code}' https://doc.elepcloud.com/)" = "200"
test "$(curl -sS -o /dev/null -w '%{http_code}' https://v3.elepcloud.com/)" = "200"
```

## 7. Rotate the PostgreSQL 18.4 secret through the root-only channel

The retained-journal audit found 1,610 historical credential-bearing PostgreSQL URL matches across 13 services and zero NATS credential-bearing matches. Therefore `rotation_required=postgresql`; NATS rotation is not required by Task 2.

Seventeen services read `/etc/aiv3/aiv3.env` and must reload it after PostgreSQL rotation:

```bash
ENV_DEPENDENTS=(
  aiv3-assistant-run-worker.service
  aiv3-chat-session-worker.service
  aiv3-codex-host-agent.service
  aiv3-dataset-output-worker.service
  aiv3-dataset-semantic-link-worker.service
  aiv3-document-enrichment-worker.service
  aiv3-external-action-worker.service
  aiv3-external-source-worker.service
  aiv3-ingest-worker.service
  aiv3-media-worker.service
  aiv3-memory-worker.service
  aiv3-platform-api.service
  aiv3-report-planner-worker.service
  aiv3-report-render-worker.service
  aiv3-retrieval-worker.service
  aiv3-static-page-worker.service
  aiv3-web.service
)
test "${#ENV_DEPENDENTS[@]}" -eq 17
```

`aiv3-codex-responses-shim.service` was not an environment-file dependent in the audit and is intentionally not in this restart set.

Perform the secret change only in an interactive root session with `set +x` and `umask 077`. Obtain the replacement from the approved secret channel; never place it in a shell command, command-line argument, here-document, GitHub input, repository file or validation receipt. The replacement must be at least 48 characters and contain only URL-safe unreserved characters (`A-Z`, `a-z`, `0-9`, `_`, `-`), with no padding. Fail closed before editing if that contract is not met. This lets the identical raw value be used as both the PostgreSQL password and the URL password component without ambiguous percent-encoding.

1. Confirm PostgreSQL is exactly 18.4 without printing connection metadata:

   ```bash
   sudo -u postgres /usr/pgsql-18/bin/psql -X -Atqc 'SHOW server_version;'
   ```

2. Create a root-only backup of the environment file. It contains the revoked old secret and remains root-only:

   ```bash
   ROTATION_ID="$(date -u +%Y%m%dT%H%M%SZ)"
   ENV_BACKUP="/root/aiv3.env.pre-pg-rotation.${ROTATION_ID}"
   install -o root -g root -m 600 "$ENV_FILE" "$ENV_BACKUP"
   stat -c '%U:%G %a %n' "$ENV_BACKUP"
   ```

3. In a no-swap root editor, replace only the password component of `PLATFORM_DATABASE_URL`, preserving scheme, host, explicit port, database name and query options. Do not paste the URL into the terminal:

   ```bash
   vi -n "$ENV_FILE"
   chown root:root "$ENV_FILE"
   chmod 600 "$ENV_FILE"
   grep -q '^PLATFORM_DATABASE_URL=' "$ENV_FILE"
   test "$(stat -c '%U:%G %a' "$ENV_FILE")" = "root:root 600"
   ```

4. In a separate interactive local PostgreSQL session, use psql's password prompt for the application role recorded in the approved secret metadata. The password must be prompted, not embedded in SQL text or shell history:

   ```bash
   sudo -u postgres /usr/pgsql-18/bin/psql -X
   ```

   At the psql prompt run `\password <application_role>`, enter the same approved replacement twice, then `\q`. Do not use `ALTER ROLE ... PASSWORD 'literal'`.

After the database accepts the new password, the release has crossed the binary safety boundary: never start a binary that lacks `$REDACTION_FLOOR`. A credential rollback is allowed only as one coordinated operation that restores the old PostgreSQL role password and the root-only environment backup, then restarts the 17 dependents while keeping the newly deployed redacted binaries.

Immediately restart all 17 dependents and verify they loaded the new root-only environment:

```bash
ROTATED_WINDOW_START="$(date --iso-8601=seconds)"
systemctl restart "${ENV_DEPENDENTS[@]}"
sleep 30

for unit in "${ENV_DEPENDENTS[@]}"; do
  test "$(systemctl is-active "$unit")" = "active"
  test "$(systemctl show "$unit" -p NRestarts --value)" = "0"
done

count_credential_urls "$ROTATED_WINDOW_START" "${ALL_AIV3_SERVICES[@]}"
```

Required result: all 17 services are active with `NRestarts=0`, and the post-rotation credential-bearing URL count is zero for every AIV3 service. Record only counts and timestamps.

## 8. Runtime release identity, health and public-entry gate

Run final checks without printing application data or configuration values:

```bash
cd "$REPO"
test "$(git rev-parse HEAD)" = "$RELEASE_SHA"
test "$(git rev-parse origin/main)" = "$RELEASE_SHA"
test -z "$(git status --porcelain)"
git merge-base --is-ancestor "$FROZEN_ANCESTOR" HEAD
git merge-base --is-ancestor "$REDACTION_FLOOR" HEAD

test "$(curl -sS -o /dev/null -w '%{http_code}' http://127.0.0.1:3000/healthz)" = "200"
test "$(curl -sS -o /dev/null -w '%{http_code}' http://127.0.0.1:3000/readyz)" = "200"
test "$(curl -sS -o /dev/null -w '%{http_code}' https://doc.elepcloud.com/)" = "200"
test "$(curl -sS -o /dev/null -w '%{http_code}' https://v3.elepcloud.com/)" = "200"

for unit in "${ENV_DEPENDENTS[@]}"; do
  printf '%s\t%s\t%s\n' \
    "$unit" \
    "$(systemctl is-active "$unit")" \
    "$(systemctl show "$unit" -p NRestarts --value)"
done
```

Verify all eighteen AIV3 units, including the responses shim, and compare the root-only feature-flag snapshot without printing any flag values:

```bash
test "${#ALL_AIV3_SERVICES[@]}" -eq 18
for unit in "${ALL_AIV3_SERVICES[@]}"; do
  test "$(systemctl is-active "$unit")" = "active"
  test "$(systemctl show "$unit" -p NRestarts --value)" = "0"
done

awk -F= '$1 ~ /(_ENABLED|_ALLOWLIST|_MODE)$/ {print}' "$ENV_FILE" \
  | LC_ALL=C sort \
  | cmp -s "$FLAG_SNAPSHOT" -
printf 'feature_flags_unchanged\ttrue\n'
rm -f "$FLAG_SNAPSHOT"
```

The runtime release is valid only when:

```text
local main SHA == origin/main SHA == 8-server checkout SHA
three exact CI jobs == success on that SHA
read-only release identity job == success on that SHA
server worktree == clean and still contains 1edff9cd and 6bc3ac45
healthz == 200
readyz == 200
https://doc.elepcloud.com/ == 200
https://v3.elepcloud.com/ == 200
18 AIV3 services == active, NRestarts == 0
feature flags == unchanged (root-only value comparison)
pre-rotation fresh-window credential URL count == 0
post-rotation fresh-window credential URL count == 0
```

## 9. Rollback rules

### Before PostgreSQL rotation

If the redacted release fails before the database password changes, stop rotation. It is permissible to return temporarily to `$PRE_DEPLOY_SHA` because the old credential is still active, but the runtime remains security-incomplete and Task 2 is not closed:

```bash
cd "$REPO"
git switch --detach "$PRE_DEPLOY_SHA"
CC=clang CXX=clang++ cargo build --locked --release \
  -p platform-api -p assistant-run-worker -p chat-session-worker -p codex-host-agent \
  -p dataset-output-worker -p report-planner-worker -p report-render-worker \
  -p retrieval-worker -p memory-worker -p external-source-worker \
  -p external-action-worker -p media-worker -p ingest-worker -p static-page-worker
systemctl restart "${RUST_SERVICES[@]}"
```

Do not claim completion, and do not rotate until a descendant containing `$REDACTION_FLOOR` passes the fresh-window gate.

### After PostgreSQL rotation

The old deployed line is no longer a valid binary rollback target. Never run `1edff9cd` or restore an old binary because it could log whichever credential is active.

If the new credential itself causes a post-rotation outage, the only permitted credential rollback is atomic and coordinated: keep the current redacted binaries, restore the old PostgreSQL role password through the root-only channel, atomically restore the root-only environment backup, restart all 17 dependents, and repeat the health and count-only fresh-window gates. Never restore only one side of the database/environment pair.

Any post-rotation target must satisfy this guard before build or restart:

```bash
ROLLBACK_TARGET="<REVIEWED_SAFE_SHA>"
git merge-base --is-ancestor "$REDACTION_FLOOR" "$ROLLBACK_TARGET"
git merge-base --is-ancestor "$FROZEN_ANCESTOR" "$ROLLBACK_TARGET"
```

If no reviewed safe target exists, keep the current redacted release running, disable only the affected request path through an already-approved feature flag if necessary, and prepare a forward hotfix from `main`. The database credential stays rotated.

## 10. Release receipt

Append a bounded receipt to `docs/validation/2026-07-16-datamax-v3-quality-readiness.md` containing only:

- reconciliation PR number and merge timestamp;
- exact 40-character runtime `origin/main` SHA;
- main CI run id and the three exact job results;
- read-only release identity run id and `server8-release-identity=success`;
- server pre-deploy and post-deploy SHAs;
- `1edff9cd` and `6bc3ac45` ancestry booleans;
- build result and the sixteen initially restarted Rust service names;
- pre-rotation fresh-window start/end and per-service counts only;
- PostgreSQL version `18.4`, `rotation_required=postgresql`, rotation completion timestamp, and `root:root 600` permission result;
- the 17 environment-dependent service states and restart counts;
- post-rotation fresh-window start/end and per-service counts only;
- local health/ready and both public-entry HTTP status codes;
- rollback floor `6bc3ac45` and confirmation that old leaking binaries are forbidden;
- confirmation that feature flags were unchanged and no provider call, migration, business-data mutation, journal deletion or secret output occurred.

Never include the environment-file backup path if it contains operational naming that should remain host-local, and never include connection endpoints, usernames, passwords, URLs with userinfo, tokens, matching journal lines, source rows or environment-file content.

## 11. Merge the docs-only receipt and close final identity

The runtime evidence cannot be committed before it exists. Therefore Task 2 closes through a second, docs-only receipt PR rather than by pushing directly to `main`. Create a short-lived branch from the runtime release `main`, update only this runbook, the active plan/pointer and the validation ledger, and commit the bounded receipt from Section 10. Do not change workflows, source, migrations, lockfiles or runtime configuration in this PR.

Require `No-Credential Smoke`, `Rust P0` and `Web` on the exact receipt PR revision, merge with a merge commit (never squash/rebase), and wait for the same three jobs on the resulting `main` SHA. Define that commit as `RECEIPT_SHA`.

On 8 server, prove that the delta from the built runtime SHA is documentation-only before fast-forwarding the checkout. No build or restart is required for this docs-only movement:

```bash
RUNTIME_SHA="<EXACT_40_CHARACTER_RUNTIME_SHA_FROM_THE_RECORDED_RECEIPT>"
test "${#RUNTIME_SHA}" -eq 40
[[ "$RUNTIME_SHA" =~ ^[0-9a-f]{40}$ ]]
git -C "$REPO" fetch origin main
RECEIPT_SHA="$(git -C "$REPO" rev-parse origin/main)"
git -C "$REPO" merge-base --is-ancestor "$RUNTIME_SHA" "$RECEIPT_SHA"

while IFS= read -r path; do
  case "$path" in
    docs/operations/datamax-release-line.md|docs/plans/2026-07-16-datamax-v3-quality-and-production-readiness.md|docs/plans/datamax-active-execution-plan.md|docs/validation/2026-07-16-datamax-v3-quality-readiness.md) ;;
    *) printf 'non-documentation receipt delta rejected\n' >&2; exit 1 ;;
  esac
done < <(git -C "$REPO" diff --name-only "$RUNTIME_SHA..$RECEIPT_SHA")

git -C "$REPO" merge --ff-only "$RECEIPT_SHA"
test "$(git -C "$REPO" rev-parse HEAD)" = "$RECEIPT_SHA"
test -z "$(git -C "$REPO" status --porcelain)"
```

Fast-forward the local `main` worktree while preserving unrelated untracked user files. Dispatch `DataMax Release Identity Gate` once more with `expected_sha=$RECEIPT_SHA`; this final run occurs after the server docs-only fast-forward and must succeed. Recheck health, readiness, both public entries, all eighteen service states and the post-rotation count-only journal window without restarting services.

Task 2 is complete only when local `main`, GitHub `origin/main` and `/srv/aiv3/repo` all equal `RECEIPT_SHA`; both the runtime and receipt lineages passed the three exact jobs; the built binaries descend from `RUNTIME_SHA` containing `6bc3ac45`; the final read-only identity run succeeds on `RECEIPT_SHA`; and the temporary runner variables and registration are removed.
