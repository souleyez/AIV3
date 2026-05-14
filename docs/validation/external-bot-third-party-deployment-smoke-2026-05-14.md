# External Bot And Third-Party Deployment Smoke - 2026-05-14

## Target

- Deployment target: `8服务器`
- Repository path: `/srv/aiv3/repo`
- Deployed commit: `fac2178`
- Public panel URL: `https://v3.elepcloud.com/external-integrations`
- Smoke source: `docs/validation/external-bot-third-party-smoke.md`

## Result

Status: passed.

The deployment repository was fast-forwarded from `a00c3b9` to `fac2178` before validation. The worktree was clean after the run.

The target host does not currently have PowerShell installed, so the smoke was run with the same cargo and Node steps as the checked-in PowerShell entrypoint. Database-backed steps were first observed to skip under the default local fixture user, then rerun with the platform service environment loaded from `/etc/aiv3/aiv3.env`. No secrets were printed or recorded.

## Passed Checks

- `assistant-runtime` low-risk external artifact publish policy.
- `assistant-runtime` high-risk external artifact revoke confirmation policy.
- `assistant-runtime` cross-system external business action confirmation policy.
- `platform-api` external artifact observability summary.
- `platform-api` external channel ACL filtering for the same question by principal.
- `platform-api` generic external source sync workflow creation.
- `platform-api` Feishu/Lark encrypted callback normalization.
- `platform-api` WeCom encrypted callback normalization.
- `platform-api` high-risk external channel action confirmation flow.
- `apps/web` external integrations panel contract tests.

## Public Panel Probe

The public deployment probe returned:

```text
status=200
content_type=text/html; charset=utf-8
final_url=https://v3.elepcloud.com/external-integrations
```

Additional panel checks:

- direct home navigation link `href="/"`: not found;
- default third-party domain `v3.elepcloud.com`: found;
- external integration page marker: found.

## Notes

- The Node contract test emitted a module-type warning for `external-integrations.js`; it did not affect the pass result.
- This is a deterministic contract smoke plus target-panel availability probe. It is not a live customer-system integration test.
- Next validation layer should use a third-party mock or sandbox endpoint to exercise signed outbound dispatch and customer-hosted chat page behavior.
