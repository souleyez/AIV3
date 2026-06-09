# Customer Web Codex Executor Smoke

This smoke is the local and deployment-target release gate for the customer-facing Web Codex executor. It validates that the feature is a general customer Codex lane, not a static-page-only editor, while preserving the V3 product boundary.

The default command is non-destructive:

```bash
npm run smoke:customer-web-codex-executor
```

It writes a JSON and Markdown receipt under:

```text
target/customer-web-codex-executor-smoke/
```

## Covered Local Contract

The smoke runs the same checks that protect the customer Web Codex chain:

- `cargo fmt --check`
- `cargo test -p platform-api customer_codex --lib`
- `cargo test -p platform-api codex_host --lib`
- `cargo test -p platform-api external_channel_static_page_dataset_template --lib`
- `cargo test -p codex-host-agent`
- `node --test apps/web/app/lib/codex-customer-artifacts.test.mjs`
- `npm run smoke:customer-web-codex-readiness -- --self-test`
- `bash scripts/run-customer-web-codex-readiness.sh --self-test --json-stdout`
- `bash scripts/run-customer-web-codex-readiness.sh --json-stdout --allow-not-ready` against a local not-ready fixture
- `npm run smoke:customer-web-codex-live -- --self-test`
- `npm --prefix apps/web run build`
- `git diff --check`

These checks cover:

- `customer_complex_request` routes to read-only Codex execution and can surface a safe `customer_result_summary`.
- `customer_artifact_request` routes customer documents, reports, scripts, pages, and artifact packages to an isolated task workspace.
- `generated_static_page_edit` uses a seeded copy of the current V3-generated page and rejects unchanged republish.
- `generated_static_page_publish` produces customer artifact manifests and preserves the publish route through host, Platform API, and web normalizers.
- `v3_product_change_request` is blocked into operator review and does not become a writable customer task.
- browser-visible and runtime-inspect-visible fields do not expose secrets, raw prompts/logs, absolute paths, `/srv/aiv3/repo`, `/srv/aiv3/shared`, `.env`, or arbitrary external URLs.
- the right-side `Codex 执行与产物` shelf can show both task cards and artifact bundles.
- readiness parsing treats self-test fixtures and parsed env-file values as authoritative, so conflicting shell env variables cannot make a deployment-target check look ready.
- the guarded live-smoke harness self-test validates approval gates and a synthetic five-case evidence matrix for task cards, artifact bundles, and product-change blocked cards before any live API call is allowed.
- the guarded live-smoke harness self-test writes a redacted `Synthetic Shelf Evidence` receipt section with task-card, artifact-bundle, blocked-task, and product-change no-artifact counts for the right-side shelf contract.
- the guarded live-smoke harness preflight writes `controlledLiveInputChecklist` and `approvalRequestSummary`, so operators can see required live inputs, selected-case requirements, and missing gates without exposing cookies, bearer values, approval text, artifact URLs, raw prompts, local paths, or current artifact JSON.
- the guarded live-smoke harness supports `--allow-missing-gates` for preflight-only missing-input collection; this changes only the preflight exit code and does not mark `readyToExecute=true` or relax live `--execute` gate enforcement.
- the guarded live-smoke harness execute receipt includes a redacted `preflightSnapshot`, so a real controlled-live result can prove the gates, selected cases, checklist, and approval-request summary were ready before live writes.
- the top-level executor smoke receipt writes `acceptance_status.schema=v3.customer_web_codex_executor_acceptance_status.v1`, so reviewers can distinguish passed no-live evidence from pending controlled-live gates.
- the top-level executor smoke writes the live-smoke self-test into a per-run child report directory, reads it back, and only marks live-readiness evidence ready when approval, redacted command-template, current-artifact, SSE parsing, synthetic shelf, product-change blocking, and redaction checks are present in that child report.

## Optional Video PPT Regression Rollup

Before a release approval, include the no-live video/PPT regression gate:

```bash
CUSTOMER_WEB_CODEX_SMOKE_INCLUDE_VIDEO_PPT_ROLLUP=true \
npm run smoke:customer-web-codex-executor
```

This adds:

```bash
npm run smoke:video-ppt-no-live-rollup -- --self-test
```

The rollup remains no-live. It must not upload files, call live endpoints, fetch videos, record screens, run FFmpeg capture, deploy services, or touch any server.

## Deployment Gates

The local smoke does not prove deployment readiness by itself. Before updating the 8 server, the operator must still verify:

```bash
CUSTOMER_WEB_CODEX_ENV_FILE=/etc/aiv3/aiv3.env \
npm run smoke:customer-web-codex-readiness
```

This readiness command is read-only. It parses the env file without sourcing it and does not print provider key values. When the env file is readable, the parsed file values are authoritative for readiness checks; the current shell environment is only a fallback when no env file is read.

For an SSH/stdin check that leaves no report file on the deployment host, run the local script through the remote shell and save the redacted JSON locally:

```bash
ssh root@8.155.8.7 \
  'cd /srv/aiv3/repo && CUSTOMER_WEB_CODEX_REPO_ROOT=/srv/aiv3/repo bash -s -- --env-file /etc/aiv3/aiv3.env --json-stdout --allow-not-ready' \
  < scripts/run-customer-web-codex-readiness.sh \
  > target/customer-web-codex-readiness/customer-web-codex-readiness-8server.json
```

When the target is not ready, the JSON report must remain non-secret but actionable:

- `missing_task_allowlist_capabilities` lists customer Web Codex capabilities absent from `CODEX_HOST_TASK_ALLOWLIST`.
- `missing_profile_allowed_capabilities` lists customer Web Codex capabilities absent from `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES`.
- `remediation.required_env_updates` lists the non-secret env/profile changes that still require production write approval.
- `remediation.raw_secret_values_included` must stay `false`.

- GitHub `main` contains the approved commit.
- `CODEX_HOST_TASK_ALLOWLIST` includes:
  - `customer_complex_request`
  - `customer_artifact_request`
  - `generated_static_page_edit`
  - `generated_static_page_publish`
- `CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES` includes the same customer Web Codex capabilities.
- `CODEX_HOST_AGENT_PROFILE_ENV_KEY=RIGHTCODE_API_KEY_MAIN`.
- `RIGHTCODE_API_KEY_MAIN` is present without printing the secret.
- `CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT` is configured and is not `/srv/aiv3/repo` or a child of that repo.
- deployment scope is limited to affected services only.
- 120 is not touched.
- screen recording remains disabled unless explicitly approved for a separate login-gated video workflow.

The top-level executor smoke JSON includes an `acceptance_status` object with:

- `full_acceptance_ready=false` until controlled-live smoke is executed with the required test account, dataset, current artifact, and approval id.
- `no_live_gate.status=passed` when local/deployment-target checks pass.
- `no_live_gate.live_self_test_evidence_ready=true` only after the executor smoke parses the per-run live-smoke self-test child report.
- `live_gate_readiness_summary.required_inputs` listing the four controlled-live inputs required before execution.
- `live_gate_readiness_summary.preflight_allow_missing_gates_ready=true` proving the live-smoke self-test covered the preflight-only checklist exit-code mode without marking missing live inputs ready.
- `live_gate_readiness_summary.next_command_template_redacted=true` proving the live-smoke self-test covered the redacted command-template contract for cookie and bearer auth shapes.
- `live_gate_readiness_summary.controlled_live_input_checklist_ready=true` proving the live-smoke self-test covered the machine-readable checklist, including the product-change-only case that does not require dataset or current artifact inputs.
- `live_gate_readiness_summary.execute_report_preflight_snapshot_ready=true` proving the live-smoke self-test covered execute receipts that preserve a redacted ready preflight snapshot for all cases and the product-change-only narrowed run.
- `live_gate_readiness_summary.approval_request_summary` listing the required operator input categories and `--execute` / `--ack-controlled-live` gates without recording their values.
- `live_gate_readiness_summary.synthetic_shelf_evidence_summary` containing task-card, artifact-bundle, blocked-task, and product-change no-artifact counts.
- `pending_gate_requirements_summary.no_live_substitute_available_for_pending_gates=true`, so no-live receipts cannot be used as a substitute for live Customer Web Codex evidence.

## Post-Deploy Live Smoke

Run live smoke only after explicit deployment approval.

A guarded live-smoke harness is available, but it defaults to preflight/dry-run behavior and never calls the live API unless `--execute` is passed:

```bash
npm run smoke:customer-web-codex-live -- --preflight \
  --base-url https://v3.elepcloud.com \
  --dataset-id <controlled_test_dataset_id> \
  --current-artifact-file <current_static_page_artifact.json>
```

When the operator already has the current rendered page URL, the file can be replaced by a shorthand URL input:

```bash
npm run smoke:customer-web-codex-live -- --preflight \
  --base-url https://v3.elepcloud.com \
  --dataset-id <controlled_test_dataset_id> \
  --current-artifact-public-url https://v3.elepcloud.com/generated-artifacts/<artifact>/index.html
```

The preflight writes a redacted receipt under:

```text
target/customer-web-codex-live-smoke/
```

It checks whether the operator has supplied the required controlled-live inputs without printing cookies, bearer tokens, approval text, raw prompts, generated-artifact URLs, local paths, or current artifact JSON. The JSON receipt includes:

- `controlledLiveInputChecklist.schema=v3.customer_web_codex_live_input_checklist.v1`
- `controlledLiveInputChecklist.requiredOperatorInputs`, with only input categories, present/missing booleans, selected case ids, and missing gate names.
- `controlledLiveInputChecklist.caseRequirements`, so a narrowed run such as `--case v3_product_change_request` can prove it does not need dataset or current artifact inputs.
- `approvalRequestSummary.schema=v3.customer_web_codex_controlled_live_approval_request.v1`
- `approvalRequestSummary.executionGates`, listing `--execute` and `--ack-controlled-live` without recording live values.

If the operator only wants to collect the missing-input checklist before credentials or approval exist, add `--allow-missing-gates`. The command still writes `readyToExecute=false` and lists the missing gates, but exits 0 so deployment checklists can archive the receipt without treating the missing-input state as a script failure:

```bash
npm run smoke:customer-web-codex-live -- --preflight --allow-missing-gates \
  --base-url https://v3.elepcloud.com
```

For the `generated_static_page_edit` case, preflight also validates the current static-page artifact shape without recording the actual URL or JSON: the context must look like a V3 static page, be finally rendered, expose an allowed generated-artifact URL, and include an absolute `https://v3.elepcloud.com/generated-artifacts/...` URL that the host agent can copy into the task workspace as the existing page seed.

To execute against the approved test account, the command must include all live-write gates:

```bash
npm run smoke:customer-web-codex-live -- --execute --ack-controlled-live \
  --approval-id <operator_approval_ref> \
  --base-url https://v3.elepcloud.com \
  --cookie "<redacted_test_session_cookie>" \
  --dataset-id <controlled_test_dataset_id> \
  --current-artifact-file <current_static_page_artifact.json>
```

`--bearer <token>` may be used instead of `--cookie` when the target auth mode supports it. `generated_static_page_edit` requires the current rendered V3-generated static page context through exactly one of `--current-artifact-json`, `--current-artifact-file`, or `--current-artifact-public-url`; the script intentionally does not invent this context. A placeholder such as `{ "artifact_id": "...", "files": [...] }` is not sufficient for this live smoke because it cannot prove that the old generated page will be available for changed-page comparison.

The execute JSON receipt includes `preflightSnapshot.schema=v3.customer_web_codex_execute_preflight_snapshot.v1`. It records only safe readiness evidence: target booleans, selected case metadata, missing gate names, `controlledLiveInputChecklist`, and `approvalRequestSummary`. It must not record raw cookies, bearer values, approval text, generated-artifact URLs, local paths, raw prompts, or current artifact JSON.

Required cases:

- `customer_complex_request`: prompt a complex customer analysis task; expect read-only Codex execution, safe result summary, and no generated artifact requirement.
- `customer_artifact_request`: prompt creation of a management report/page/script package; expect workspace-write only under the task workspace and safe generated-artifact URLs if published.
- `generated_static_page_edit`: prompt a revision of the current V3-generated report page; expect a changed page package, template reuse, and no V3 repo mutation.
- `generated_static_page_publish`: prompt creation of a new customer page/dashboard package; expect the generic `assistant_run.customer_artifact_request_artifacts_ready` event with `capability=generated_static_page_publish` and `route=generated_static_page_publish`.
- `v3_product_change_request`: prompt a V3 login/API/deploy/source-code change; expect `needs_operator_review`, no writable task, and no customer artifact bundle.
- right-side shelf: verify task cards and artifact bundles render without raw logs, prompts, secrets, absolute paths, or unsafe URLs.
- regression: run the approved video/PPT upload-main and handoff smoke selected for that release window.

Each live smoke receipt must record:

- prompt class, route, capability, status, workflow execution id;
- sandbox or permission scope;
- task workspace root check;
- whether a customer artifact manifest was produced;
- whether the V3 product repo stayed clean;
- whether the main AssistantRun answer path was preserved;
- any generated-artifact URL exposed to the browser.

## Rollback

Fast rollback is config-only:

```text
remove customer_* / generated_static_page_* from CODEX_HOST_TASK_ALLOWLIST
remove customer_* / generated_static_page_* from CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES
restart aiv3-platform-api.service and aiv3-codex-host-agent.service
```

The rollback should keep existing generated artifacts readable. It only disables new customer Web Codex task routing.
