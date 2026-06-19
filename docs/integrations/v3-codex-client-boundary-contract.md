# V3 and Codex Client Boundary Contract

## Purpose

This document is the shared contract between the DataMax V3 thread and the `codex-web` / Codex client thread.

The goal is to let customer employees use an enterprise Codex client for local business work while V3 remains the enterprise authority for users, permissions, datasets, asset libraries, task cards, artifact storage, parsing, publishing, and audit.

## Canonical Ownership

| Area | Owner | Rule |
| --- | --- | --- |
| Tenant, user, account, roles | V3 | `codex-web` must not create a parallel enterprise identity system for V3 clients. |
| Dataset and asset library model | V3 | `codex-web` receives scoped references only. It must not model enterprise datasets or asset libraries as its own source of truth. |
| Client configuration package | V3 defines, `codex-web` consumes | V3 creates the package contract; `codex-web` implements installer/configurator support. |
| Codex runtime, bridge, Cloudflare/local target | `codex-web` | V3 must not duplicate Codex runtime control, bridge, or orchestrator internals. |
| Codex execution quota and terminal seats | `codex-web` | V3 may display or call quota status, but `codex-web` owns Codex execution quota, personal recharge records, reservations, refunds, and terminal seat usage. |
| Local execution | Codex client / `codex-web` | Local file processing, Office work, browser automation, and multi-step Codex tasks run outside V3. |
| Artifact table and publishing | V3 | `codex-web` may keep temporary artifacts; V3 owns enterprise artifact records and final publication. |
| Task card display | V3 | V3 task cards render client-generated artifacts after upload. |
| Download mirror | V3 operations + `codex-web` package outputs | V3 hosts approved client packages; `codex-web` produces package contents. |

## Non-Goals

`codex-web` must not implement:

1. A second V3 dataset permission model.
2. A second V3 asset library model.
3. A second enterprise artifact table.
4. A second report/static-page publishing authority for V3 customer artifacts.
5. A permanent enterprise data sync engine that bypasses V3.
6. Direct production database writes for customer data.

V3 must not implement:

1. Codex bridge runtime control.
2. Cloudflare workstation internals.
3. Local Codex job execution.
4. Local client installer internals.
5. `codex-web` project workspace and GitHub sync internals.
6. Codex execution quota ledger or terminal seat source of truth.

## Target Flow

```text
V3 user selects task + dataset / asset library
-> V3 creates client configuration package or task package
-> Codex enterprise client executes locally
-> Codex client writes result manifest + files
-> V3 receives client artifact
-> V3 stores, parses, indexes, and attaches artifact
-> V3 task card shows progress and result
-> user continues editing or publishes in V3
```

## V3-Side API Contract

The following endpoints are V3-owned. `codex-web` should consume them once implemented.

```text
POST /v1/client-config-packages
GET  /v1/client-config-packages/{package_id}
POST /v1/client-artifacts
GET  /v1/client-artifacts/{artifact_id}
GET  /v1/client-artifacts/{artifact_id}/files/{file_index}
GET  /v1/client-artifacts/{artifact_id}/files/{file_index}/preview
POST /v1/client-artifacts/{artifact_id}/attach-to-dataset
POST /v1/client-artifacts/{artifact_id}/attach-to-asset-library
POST /v1/client-artifacts/{artifact_id}/publish
POST /v1/client-artifacts/{artifact_id}/publish-public
```

The first implementation can use a simpler operator-only path, but field names and manifest shape should stay compatible with this contract.

### Implementation Status: 2026-06-17

Implemented in V3 `platform-api`:

- `POST /v1/client-config-packages`
- `GET /v1/client-config-packages/{package_id}`
- `GET /v1/client-artifacts`
- `POST /v1/client-artifacts`
- `GET /v1/client-artifacts/{artifact_id}`
- `GET /v1/client-artifacts/{artifact_id}/files/{file_index}`
- `GET /v1/client-artifacts/{artifact_id}/files/{file_index}/preview`
- `POST /v1/client-artifacts/{artifact_id}/attach-to-dataset`
- `POST /v1/client-artifacts/{artifact_id}/attach-to-asset-library`
- `POST /v1/client-artifacts/{artifact_id}/publish`
- `POST /v1/client-artifacts/{artifact_id}/publish-public`

Current upload format:

- `POST /v1/client-artifacts` accepts `multipart/form-data`.
- Field `manifest` must be JSON matching `v3.client_artifact_manifest.v1`.
- Field `files` is repeatable and must match `manifest.files` order and filenames.
- Upload authorization accepts an existing V3 session or `Authorization: Bearer <token>` where the server token comes from `V3_CLIENT_ARTIFACT_TOKEN`.

Current storage:

- Config packages are stored in `v3_client_config_packages`.
- Client artifact records are stored in `v3_client_artifacts`.
- Uploaded file metadata is stored in `v3_client_artifact_files`.
- Small uploaded file snapshots stay in the database `bytes` column.
- When `V3_CLIENT_ARTIFACT_OBJECT_DIR` is configured, larger uploaded files can be stored under a V3-managed filesystem object directory with `storage_kind=filesystem`.
- Filesystem object locators are internal only and must not be returned in API responses, task cards, logs, validation records, or customer-facing documents.
- Dataset and asset library references remain V3-owned references. UUID references are checked against visible V3 records; non-UUID external references are accepted for compatibility with scoped packages.
- File records include `download_url` values under `/v1/client-artifacts/{artifact_id}/files/{file_index}`. The V3 web app accesses these through its `/api/v3/...` proxy.
- Published HTML file records include `preview_url` values under `/v1/client-artifacts/{artifact_id}/files/{file_index}/preview`.
- `POST /v1/client-artifacts/{artifact_id}/publish` requires a V3 user session, marks the artifact as `published`, and records `manifest.metadata.v3_publish.status=published_private`.
- First publish implementation is enterprise-private: it does not produce an anonymous public URL and sets `manifest.metadata.v3_publish.public_url=null`.
- HTML preview is an enterprise-private sandbox preview. The preview endpoint requires a V3 user session, requires the artifact to be published, strips active content and external-resource hooks, wraps the result in an empty-sandbox iframe, and does not produce a public URL.
- `POST /v1/client-artifacts/{artifact_id}/publish-public` requires a V3 user session and requires the artifact to already be `published`.
- Public HTML publication selects the primary HTML file, sanitizes active content, writes a DataMax-owned sandbox wrapper into the configured generated-artifacts directory, registers a `html_artifacts` record, and updates `manifest.metadata.v3_public_html.public_url`.
- Public HTML publication does not expose the original uploaded file bytes, DB payload, filesystem locator, or raw object path.

Current V3 web display:

- `GET /v1/client-artifacts` is polled by the V3 home page.
- Uploaded client artifacts are normalized into the existing artifact task-card shelf as `v3_client_artifact`.
- File entries are shown as downloadable files.
- Published client artifacts show the task-card publish stage as complete.
- Published HTML files can be opened from the task card through the public generated-artifact URL when `public_url` is present, otherwise through the V3 private sandbox preview URL.

Not implemented yet:

- Raw/unsandboxed public HTML publication for client artifacts
- Real end-to-end joint smoke with the codex-web client/configurator thread

## Codex-Owned Quota and Terminal Contract

codex-web owns the Codex execution quota control plane. It references V3 `tenant_id`, `user_id`, and `client_id`, but does not create V3 data-permission authority.

Current codex-web internal route shape:

```text
POST /api/codex/clients/activate
POST /api/codex/clients/heartbeat
POST /api/codex/clients/revoke
POST /api/codex/clients/self-revoke
GET  /api/codex/clients/terminal-summary
GET  /api/codex/clients/me

GET  /api/codex/quotas/plans
POST /api/codex/quotas/reserve
POST /api/codex/quotas/commit
POST /api/codex/quotas/refund
GET  /api/codex/quotas/admin/overview
POST /api/codex/quotas/admin/grant-enterprise
POST /api/codex/quotas/personal/recharge
POST /api/codex/quotas/personal/recharge/wechat-native
POST /api/codex/quotas/self/recharge/wechat-native
GET  /api/codex/quotas/summary
GET  /api/codex/quotas/usage-basic
```

Public or gateway-facing paths may later map these to `/v1/codex-clients/*` and `/v1/codex-quotas/*`.

Rules:

1. Quota reservation must happen before a Codex bridge job is created.
2. Duplicate `request_id` retries must not double-deduct quota.
3. Refund must return reserved quota at most once.
4. Terminal activation must enforce the tenant terminal limit.
5. Dataset and asset-library scope in entitlement responses must come from V3 config or V3 entitlement lookup, not from a codex-web-owned model.
6. Codex terminal session tokens are execution-side credentials only. They can identify a V3 tenant/user/client reference for quota enforcement, but they must not become V3 dataset, asset library, artifact, role, or publish permission tokens.

## Client Configuration Package

The configuration package is generated by V3 and consumed by the Codex enterprise client.

```json
{
  "schema": "v3.codex_client_config.v1",
  "tenant_id": "tenant-001",
  "user_id": "user-001",
  "client_id": "device-001",
  "v3_base_url": "https://v3.elepcloud.com",
  "asset_library_ids": ["fashion-design-main"],
  "dataset_ids": ["design-images", "supply-chain"],
  "skill_packs": ["fashion-design", "static-page-edit"],
  "artifact_upload": {
    "mode": "session_token",
    "endpoint": "/v1/client-artifacts"
  },
  "codex_control": {
    "base_url": "https://ad.goods-editor.com",
    "activation_endpoint": "/api/codex/clients/activate",
    "heartbeat_endpoint": "/api/codex/clients/heartbeat",
    "revoke_endpoint": "/api/codex/clients/self-revoke",
    "plans_endpoint": "/api/codex/quotas/plans",
    "quota_summary_endpoint": "/api/codex/quotas/summary",
    "terminal_summary_endpoint": "/api/codex/clients/terminal-summary",
    "billing_url": "https://ad.goods-editor.com/codex/billing",
    "activation_token_env": "CODEX_CLIENT_ACTIVATION_TOKEN",
    "terminal_id": "term-001",
    "terminal_label": "finance-pc-01",
    "session_ttl_seconds": 2592000
  },
  "expires_at": "2026-06-17T18:00:00Z"
}
```

Rules:

1. The package gives short-lived or device-bound access, not an all-powerful token.
2. Dataset and asset library entries are references, not full data dumps.
3. Skill packs contain templates, prompts, schemas, and tool descriptions, not production secrets.
4. Local files remain local unless the user or task explicitly selects them for upload.
5. `codex_control` is optional Codex execution metadata. It can activate a terminal and issue a Codex session token, but it does not grant V3 dataset, asset-library, artifact, role, or publish authority.
6. `activation_token` should not be embedded in normal customer packages. Prefer `activation_token_env` or a short-lived V3 delivery flow.

## V3 Display Integration

V3 can display the codex-web package and operator summary, but it must not become a second quota ledger.

Current V3 web proxy routes:

```text
GET /api/codex-control/plans
GET /api/codex-control/overview
```

Rules:

1. `/api/codex-control/plans` reads public package definitions from codex-web and exposes billing/admin links for the V3 page.
2. `/api/codex-control/overview` forwards to `GET /api/codex/quotas/admin/overview` only when `V3_CODEX_QUOTA_SERVICE_TOKEN` is configured on the V3 server.
3. If the service token is absent, V3 displays public packages only and leaves terminal, recharge, and usage details in codex-web.
4. `GET /api/codex/clients/terminal-summary` remains terminal-session scoped and should be called by the enterprise client with `cxt_*`, not by V3 with a service token.
5. V3 pages may link to `billing_url`, but recharge order creation and payment confirmation remain codex-web-owned.

## Enterprise Terminal Activation Flow

The current Codex client can consume the optional `codex_control` section:

```text
V3 config package
-> local V3企业定制agent终端
-> POST /api/codex/clients/activate on codex-web
-> codex-web returns cxt_* Codex session token
-> client verifies token with POST /api/codex/clients/heartbeat
-> client injects token into private Codex runtime as SOULEYE_CODEX_EDGE_TOKEN
-> Cloudflare /responses resolves token and enforces quota
```

Boundary rules:

1. V3 owns config package creation and the V3 references inside it.
2. codex-web owns terminal activation, seat usage, session-token issuance, and quota ledger.
3. The Codex terminal session token is accepted only for Codex execution/quota paths.
4. V3 artifact upload still uses V3 authorization, currently session auth or `V3_CLIENT_ARTIFACT_TOKEN`.
5. Revoking a Codex terminal must stop Codex execution quota access, but it must not delete V3 artifacts or change V3 dataset permissions.
6. The client must heartbeat before reusing a cached Codex terminal session token; heartbeat failure should trigger reactivation or stop launch.

## Client Artifact Manifest

The Codex client uploads a manifest and files. V3 stores the files and uses the manifest to attach the artifact to the right enterprise context.

```json
{
  "schema": "v3.client_artifact_manifest.v1",
  "source": "enterprise-codex-client",
  "tenant_id": "tenant-001",
  "user_id": "user-001",
  "client_id": "device-001",
  "task_id": "client-task-001",
  "asset_library_ids": ["fashion-design-main"],
  "dataset_ids": ["design-images"],
  "title": "春夏连衣裙选款分析",
  "artifact_type": "static_page",
  "files": [
    {
      "filename": "index.html",
      "content_type": "text/html",
      "role": "primary_html"
    },
    {
      "filename": "report.md",
      "content_type": "text/markdown",
      "role": "source_summary"
    }
  ],
  "evidence_refs": [
    {
      "kind": "dataset",
      "id": "design-images"
    }
  ],
  "created_at": "2026-06-17T12:00:00Z"
}
```

V3 processing after upload:

1. Verify tenant, user, device, token, and scope.
2. Validate file type, file size, filename, archive contents, and HTML safety.
3. Store files in V3 artifact storage.
4. Create enterprise artifact records.
5. Attach artifact to datasets and asset libraries.
6. Parse supported formats into search/report evidence.
7. Surface the artifact in the V3 task card.
8. Allow V3-side publish, revoke, and continued editing.

## Status Mapping

`codex-web` orchestrator already has richer execution states. V3 task cards should consume a safe public mapping.

| Codex internal status | V3 public task status |
| --- | --- |
| `queued` | `processing` |
| `retry_wait` | `processing` |
| `leased` | `processing` |
| `waking_runtime` | `processing` |
| `submitted` | `processing` |
| `running` | `processing` |
| `completed` with usable artifact | `completed` |
| `completed` without required artifact | `processing` or `needs_retry` |
| `cancelled` | `cancelled` |
| terminal `failed` | `failed` |

For image/static-page visual tasks, missing required image artifact should not become a user-visible terminal failure by default. It should stay retryable or actionable.

## Change Control

1. V3 owns this contract.
2. `codex-web` may propose changes, but should not silently introduce new V3-facing fields.
3. Any change to required fields, authentication, upload semantics, dataset scope, asset library scope, or artifact publication must update this document first.
4. Public external APIs must not change as a side effect of Codex client work.
5. Client package download URLs and checksums must be documented in V3 operations docs.

## Minimal Joint Smoke

The first shared smoke should prove:

```text
V3 creates test config package
-> Codex client reads package
-> local Codex creates index.html + report.md
-> client uploads manifest + files
-> V3 creates enterprise artifact
-> V3 attaches artifact to dataset / asset library
-> V3 task card shows completed artifact
-> user opens artifact and requests "修改报表"
```

No Cloudflare runtime, Mac mini pool, or automatic task pulling is required for the first smoke.

V3-side harness:

```bash
npm run smoke:v3-client-artifact-joint -- --self-test
npm run smoke:v3-client-artifact-joint -- --preflight --base-url https://v3.elepcloud.com
npm run smoke:v3-client-artifact-joint -- --execute --ack-controlled-live \
  --base-url https://v3.elepcloud.com \
  --cookie "<V3 session cookie>"
```

Rules:

1. `--self-test` is deterministic and does not call V3.
2. `--preflight` only checks service readiness.
3. `--execute --ack-controlled-live` is the mutating joint smoke and creates a config package plus a client artifact.
4. The receipt must not contain cookies, bearer tokens, filesystem locators, raw object paths, provider keys, source database rows, or customer content.
5. Public HTML publication in this smoke uses the V3 generated-artifact sandbox path, not raw/unsandboxed client HTML.

## Recommended Thread Coordination

V3 thread:

1. Own this document.
2. Implement config package API.
3. Implement client artifact upload API.
4. Implement task card display and continued editing.
5. Implement artifact parsing, dataset attachment, asset library attachment, publishing, and audit.

`codex-web` thread:

1. Read this document before implementing client work.
2. Implement enterprise client configuration consumption.
3. Implement local manifest generation.
4. Implement V3 artifact upload client.
5. Keep orchestrator/runtime/artifact temporary storage inside `codex-web`.
6. Avoid duplicating V3 enterprise data models.
