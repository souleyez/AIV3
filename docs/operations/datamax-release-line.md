# DataMax V3 Main/CI/8-Server Release Line

**Status:** Task 2 runtime receipt recorded; final docs-only identity gate is external to this commit

**Baseline date:** 2026-07-16

**Scope:** Keep GitHub `main`, deterministic CI, local `main` and `/srv/aiv3/repo` on one fast-forward lineage; keep deployment mutation in the root-SSH boundary; classify database-secret work from current evidence instead of assuming that a password exists.

## 1. Authority and release boundaries

The current private-repository plan does not provide usable branch-protection rules, rulesets or protected-environment reviewers. Do not describe those controls as enabled. Until the repository plan changes, the release owner must manually verify these exact jobs on each PR head and merged `main` SHA:

| Job id | Display name | Normal runner | Required result |
|---|---|---|---|
| `no-credential-smoke` | `No-Credential Smoke` | `ubuntu-24.04` | `success` |
| `rust-minimal` | `Rust P0` | `ubuntu-24.04` | `success` |
| `web` | `Web` | `ubuntu-24.04` | `success` |

Untrusted PR code must never run on 8 server. The temporary exact-SHA-gated Windows runner used to work around the 2026-07-16 GitHub-hosted billing rejection is a Task 2-only contingency. Its two repository variables and runner registration must be removed after the final receipt gate, returning the workflow expression to its `ubuntu-24.04` default.

`.github/workflows/datamax-release.yml` is a separately dispatched, read-only handoff. It verifies the exact merged `main` identity, Node.js `22.22.1`, the sensitive-log scanner and deployment ancestry. It uses an ephemeral per-command `safe.directory=/srv/aiv3/repo`; it does not change global Git configuration. It intentionally does not read the two root-only tracked semantic-supply fixture inputs. Full deployment-worktree cleanliness is therefore an authoritative root-SSH gate, not a runner claim.

GitHub Actions does not fetch into the deployment checkout, build, edit configuration, restart services, run migrations, rotate secrets or deploy. The `datamax-server8` environment is audit grouping only.

## 2. Immutable invariants

- Frozen deployed ancestor: `1edff9cdac585671f2ed64ed25690f2be121c750`.
- Connection-redaction floor: `6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7`.
- Runtime release: `ff60d0deee81a2cbb98a70a4cbda9693962349d0`.
- The server may move only by fast-forward ancestry. No force push, rebase, history rewrite, `git reset --hard`, arbitrary-ref deployment or cherry-pick is allowed.
- Root-owned tracked fixture inputs keep their restrictive ownership and permissions. Do not loosen them to satisfy the non-root runner.
- Feature flags remain unchanged. Graph answer supply and asset import/parser gates are not enabled by this release.
- No command, workflow log, receipt or chat message may print a connection URL, password, token, environment-file content or matching journal line.
- Historical journal entries are retained. This runbook does not delete, vacuum or rewrite journals.
- A database credential is rotated only after current configuration proves that an active credential exists, identifies its owner and enforcement path, and provides a tested fail-closed rollback. Historical string matches alone do not authorize mutation.

## 3. Exact CI and release-identity procedure

For each candidate, verify that all three jobs ran on the exact head; a missing, skipped, cancelled, pending or failed job closes the gate:

```bash
gh pr checks "<PR_NUMBER>"
gh run view "<RUN_ID>" --json headSha,status,conclusion,jobs \
  --jq '{headSha,status,conclusion,jobs:[.jobs[]|{name,status,conclusion}]}'
```

After merge, verify the `push` run on the exact `origin/main` SHA and both required ancestors:

```bash
git fetch origin main
RELEASE_SHA="$(git rev-parse origin/main)"
test "${#RELEASE_SHA}" -eq 40
git merge-base --is-ancestor 1edff9cdac585671f2ed64ed25690f2be121c750 "$RELEASE_SHA"
git merge-base --is-ancestor 6bc3ac45c42d4242a5d3dc1209e7e2ba17300ba7 "$RELEASE_SHA"
gh run list --workflow datamax-ci.yml --branch main --commit "$RELEASE_SHA"
```

Dispatch the read-only identity gate only after the server is at the requested SHA or an allowed ancestor:

```bash
gh workflow run datamax-release.yml --ref main -f expected_sha="$RELEASE_SHA"
gh run watch "<RELEASE_GATE_RUN_ID>" --exit-status
```

Required summary: exact main identity, sensitive-log scanner passed, deployment ancestry compatible, full worktree cleanliness delegated to root SSH, and mutation `none`.

## 4. Root-SSH deployment gate

Root SSH is the only authority allowed to fetch and fast-forward `/srv/aiv3/repo`, perform the release build and restart the selected units. Before mutation it must prove:

```text
root git status == clean
server HEAD is an ancestor of exact origin/main
1edff9cd and 6bc3ac45 are ancestors of the target
/etc/aiv3/aiv3.env == root:root 600
feature-flag snapshot can be compared without printing values
```

The connection-redaction helper is statically linked, so the 2026-07-16 runtime release rebuilt these fourteen Cargo packages, covering all sixteen Rust services:

```text
platform-api, assistant-run-worker, chat-session-worker, codex-host-agent,
dataset-output-worker, report-planner-worker, report-render-worker,
retrieval-worker, memory-worker, external-source-worker,
external-action-worker, media-worker, ingest-worker, static-page-worker
```

The deployment audit must read journald as JSON and emit counts only. It covers PostgreSQL and NATS userinfo, including token-only userinfo, plus secret-like query, fragment and DSN keys. A text-only `grep` over rendered messages is not sufficient. The selected `--since`/`--until` values must first be accepted by the host's `journalctl`; stderr and the producer exit code must not be hidden or neutralized by `|| true`. Record the number of JSON records scanned so an empty/error stream cannot masquerade as a zero-match pass.

After restart, prove within a bounded fresh window:

```text
all 18 AIV3 services == active/running
all 18 AIV3 services NRestarts == 0
healthz == 200
readyz == 200
https://doc.elepcloud.com/ == 200
https://v3.elepcloud.com/ == 200
credential-userinfo matches == 0
secret-parameter matches == 0
feature flags == unchanged
```

No provider call, migration command, database mutation or feature-flag edit belongs to this release gate.

## 5. PostgreSQL credential classification

The Task 1 retained-journal scan reported 1,610 historical PostgreSQL-userinfo matches and provisionally classified PostgreSQL rotation as required. Task 2 rechecked the current secret and authentication state before any destructive action and supersedes that provisional classification for the live platform database:

| Check | Current result |
|---|---|
| PostgreSQL version | `18.4` |
| `PLATFORM_DATABASE_URL` entries | exactly one |
| Expected role / database | `ai_platform_v3` / `ai_data_platform_v3` |
| URL endpoint | loopback, password component absent |
| Role can log in | yes |
| Role password verifier | `NULL` |
| Loopback host authentication | `trust` |
| Current retained-journal rerun | zero matching records across all 18 journals |

Therefore the live classification is `platform_rotation_not_applicable`. No active platform password exists to rotate. Creating a password now would introduce a new secret while the active loopback `trust` rules would not enforce it, so Task 2 must not run the prepared rotation script or restart the seventeen environment dependents for a no-op credential event.

Three one-time disposable-role fixture attempts correctly failed closed: both the candidate and intentionally wrong password authenticated under `trust`, so `fixture_ok=false`. Every attempt reported `cleanup_ok=true`; an independent postcheck found zero temporary roles, databases, environment directories and state directories. This is evidence that the proposed rotation proof is invalid under the current HBA configuration, not a passed rotation test.

The earlier 1,610-count observation remains part of the historical audit record, but an exact rerun over the currently retained eighteen journals now returns zero. No journal deletion or vacuum was performed during Task 2, and the current journal still retains records from May 20 onward. The discrepancy cannot be safely attributed or used to claim that any historical secret was rotated. If those earlier records represented an external datasource rather than the passwordless platform role, owner and secret inventory must first be recovered through the approved secret/configuration channel; that is separate work.

Replacing loopback `trust` with enforced authentication is a production-readiness security debt. It requires a dedicated plan with application compatibility, HBA ordering, rollback and a real old/new-password rejection fixture. It is not an opportunistic Task 2 mutation.

## 6. 2026-07-16 runtime release receipt

### GitHub lineage

| Stage | Exact identity | Result |
|---|---|---|
| Reconciliation PR | PR `#1`, head `5e420a375d12e9472b9af7b01b92327b88c684c6`, run `29466636362` | all three jobs passed |
| First merged main | `883f5f3e3d1367d1e7eb58968365f51c13b2ec5d`, run `29467393919` | all three jobs passed |
| Node-path hotfix | PR `#2`, head `3631ce96ee1fb5862a9e4dea7dc0bfe9ce826d3a`, run `29468730320` | all three jobs passed |
| Second merged main | `38c84d4a8b91dec0651b5bd0c64e1c18042a340b`, run `29470149051` | all three jobs passed |
| Ownership-boundary hotfix | PR `#3`, head `6412aef1654a7e75588af8d6ec3f31c031a1ac2d`, run `29471314556` | all three jobs passed |
| Runtime main | `ff60d0deee81a2cbb98a70a4cbda9693962349d0`, run `29472517847` | all three jobs passed |
| Runtime identity | run `29473260743`, job `87540637627` | success, mutation none |

Two earlier read-only identity attempts failed safely before deployment mutation: run `29468579036` found the missing non-interactive Node path; run `29471081803` found Git's dubious-ownership boundary. The hotfixes made the Node path explicit and delegated the root-only clean check honestly; they did not relax file permissions or global Git safety.

### 8-server deployment

| Evidence | Result |
|---|---|
| Pre-deploy SHA | `1edff9cdac585671f2ed64ed25690f2be121c750` |
| Runtime SHA | `ff60d0deee81a2cbb98a70a4cbda9693962349d0` |
| Frozen/redaction ancestry | both true |
| Release build | success, `2026-07-16T13:13:51+08:00` to `13:23:34+08:00` |
| Fresh audit window | corrected host-local form `2026-07-16 13:23:34` to `13:24:04` |
| JSON records scanned | `808` |
| Credential-userinfo total | `0` across all 18 services |
| Secret-parameter total | `0` across all 18 services |
| Service state | 18 active, 18 running, 18 with `NRestarts=0` |
| HTTP gate | local health `200`, local ready `200`, public docs `200`, public V3 `200` |
| Environment permissions | `root:root 600` |
| Feature flags | unchanged |
| Independent runtime result | green after corrected journal rescan |

The deployment helper's original ISO-8601 timestamp was rejected by the host `journalctl` with a parse error; hidden stderr plus `|| true` made its initial zero counts invalid. The independent read-only rescan above used an accepted host-local format and is the authoritative fresh-window evidence. Extending the end to `13:35:12` still scanned the same 808 records and returned zero for both match classes; the responses shim independently had zero records and zero matches in both windows.

The deployment did not rebuild Web because the runtime delta from the frozen deployed ancestor had no Web source change. It did not run a provider call, migration, database mutation, secret rotation, journal cleanup or feature-flag change.

## 7. Docs-only receipt and final identity

Runtime evidence cannot be committed before it exists, so Task 2 closes through one docs-only PR from runtime `main`. Its delta is restricted to these four paths:

```text
docs/operations/datamax-release-line.md
docs/plans/2026-07-16-datamax-v3-quality-and-production-readiness.md
docs/plans/datamax-active-execution-plan.md
docs/validation/2026-07-16-datamax-v3-quality-readiness.md
```

The receipt PR and merged `main` must each pass the same exact three jobs. Before the server moves, root SSH proves that `RUNTIME_SHA..RECEIPT_SHA` contains only those four paths, then fast-forwards without a build or restart. Local `main` is fast-forwarded while preserving unrelated untracked user files.

The final read-only release-identity run is dispatched only after the server docs-only fast-forward. Its run id and resulting `RECEIPT_SHA` cannot be embedded in the commit they identify without creating an infinite receipt loop; GitHub's merge object, exact CI runs and release-identity run are the immutable external evidence. Do not open a third PR merely to write those identifiers back into this document.

Task 2 closes when all of these are true:

```text
local main == GitHub origin/main == /srv/aiv3/repo == RECEIPT_SHA
RUNTIME_SHA is an ancestor of RECEIPT_SHA
RUNTIME_SHA..RECEIPT_SHA changes only the four allowed documentation paths
receipt PR and receipt-main each passed No-Credential Smoke, Rust P0 and Web
final release-identity run succeeded on RECEIPT_SHA
18 services and four HTTP probes remained healthy without restart
fresh journal count-only recheck remained zero
temporary runner variables and registration were removed
```

Only after that external gate may the active pointer advance to Task 3. Task 3 implementation does not belong to this Task 2 receipt operation.

## 8. Rollback and follow-up rules

- Before a future database authentication change, preserve a reviewed release containing the redaction floor and build a tested pairwise database/environment rollback. Never change only one side.
- The pre-Task-2 binary line is not a preferred rollback because it lacks the complete CI and release-identity hardening. Prepare forward hotfixes from `main` unless a reviewed incident procedure says otherwise.
- A nonzero fresh sensitive-log count, unhealthy service, dirty checkout, ancestry mismatch or CI identity mismatch closes the gate. Keep the last healthy redacted runtime and investigate without printing the record.
- HBA authentication hardening and any attributable external-datasource credential rotation require their own explicitly scoped tasks.
