# Codex Host Workspace Retention Smoke

This smoke validates the cleanup policy for Codex Host task workspaces. It is intentionally read-only for cleanup: it may create local fixtures under `target/`, but it never deletes workspace directories.

The contract is:

- each fixed-task workspace should include `runtime.json.retention_policy`;
- the default retention window is 168 hours unless `CODEX_HOST_AGENT_TASK_WORKSPACE_RETENTION_HOURS` overrides it;
- cleanup requires an explicit operator action;
- cleanup must be backup-first;
- normal task polling, third-party reply polling, and model status checks must not delete workspaces.

## Local Command

```bash
bash scripts/run-codex-host-workspace-retention-smoke.sh
```

Default report output:

```text
target/codex-host-workspace-retention-smoke/<timestamp>.json
target/codex-host-workspace-retention-smoke/<timestamp>.md
```

## Deployment Scan

On a deployment target, scan the configured workspace root without creating fixtures:

```bash
CODEX_HOST_WORKSPACE_RETENTION_SCAN_ONLY=true \
CODEX_HOST_WORKSPACE_RETENTION_ROOT=/srv/aiv3/codex-host/tasks \
bash scripts/run-codex-host-workspace-retention-smoke.sh
```

Expected:

- `destructive_operation_performed=false`;
- expired candidates are listed, not removed;
- candidates require `cleanup_requires_operator=true` and `backup_before_delete=true`;
- any directory missing `runtime.json` is reported as skipped.

## 2026-05-27 8-Server Pre-Deploy Probe

Read-only probe against `8服务器`:

- Repository: `/srv/aiv3/repo`
- Server HEAD: `50fd00d18`
- Services checked active: `aiv3-platform-api.service`, `aiv3-web.service`, `aiv3-codex-host-agent.service`, `aiv3-external-source-worker.service`, `aiv3-ingest-worker.service`, `aiv3-retrieval-worker.service`
- Scan: `find /srv/aiv3 -maxdepth 6 -name runtime.json`
- Result: no Codex Host task workspace `runtime.json` files were present before this local retention-policy change was deployed.
- Recent generated artifacts existed under `/srv/aiv3/shared/objects/generated-artifacts/`, so artifact retention and task-workspace retention remain separate concerns.

Formal deployment evidence should be recorded after the current local retention-policy changes are released to the target host.

## Cleanup Policy

This smoke does not perform cleanup. On this local Windows workspace, any Codex-driven cleanup must use `Safe-RemoveToBackup.ps1`. On Linux deployment targets, cleanup should be a separate operator command that archives the workspace first, records the archive path, and only then removes the original directory.
