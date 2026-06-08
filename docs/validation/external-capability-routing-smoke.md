# External Capability Routing Smoke

## 2026-06-06 Current-Head Report-Material Routing Rollout

- Host: `8服务器`
- Public endpoint: `https://v3.elepcloud.com`
- Final deployed commit: `bee7e08`
- Service: `aiv3-platform-api.service`
- Service status: `active`
- Dataset scope for selected Xinbai cases: `64fff6c8-10e2-4ee8-8243-23166cce3abc`

Changes covered:

- Model-visible capability guidance now explicitly distinguishes scoped temporary report materials from permanent document-processing tasks.
- Template/reference files, temporary contracts, store-area evidence, traffic-stat files, and customer report style requirements can route to the report/static-page workflow.
- Xinbai report focus routing maps contract area, 坪效, 客流统计, 客流同比, 租售比, and health/trend wording to `经营总览` before generic contract-detail routing.
- Accepted template-baseline links are returned immediately for report-material updates while background refresh may continue.
- Explicit existing-page repair prompts still hide the baseline link while the repair/revision task proceeds.

Local checks before final rollout:

- `cargo test -p platform-api external_channel_capability_routing_fixture --lib`
- `cargo test -p platform-api static_page_public_url_with_prompt_focus_adds_module_query --lib`
- `cargo test -p platform-api external_channel_static_page_artifact_detects_xinbai_business_report_modules --lib`
- `cargo test -p platform-api external_channel_model_tool_request --lib`
- `cargo test -p platform-api external_channel_static_page_reply_restores_visual_contract_template_link --lib`
- `cargo test -p platform-api external_channel_static_page_reply_prefers_accepted_template_link_over_fixed_task_queue --lib`
- `cargo test -p platform-api external_channel_static_page_dataset_template_overlap_skips_image2_and_queues_codex --lib`
- `cargo test -p platform-api static_page_stable_artifact_key_uses_canonical_dataset_not_temporary_dataset --lib`
- `cargo fmt --check -p platform-api`
- `cargo check -p platform-api`
- `git diff --check`

8-server selected SSE smoke passed for 6 cases after rollout:

| Case | Expected | Result |
| --- | --- | --- |
| `static_page_xinbai_template_reference` | report/static page, focus `取高机会` | Passed; 1 artifact link, focus matched |
| `static_page_xinbai_temp_contract_area` | report/static page, focus `经营总览` | Passed; 1 artifact link, focus matched |
| `static_page_xinbai_traffic_stats` | report/static page, focus `经营总览` | Passed; 1 artifact link, focus matched |
| `plain_qa_bedridden_turning` | ordinary Q&A | Passed; completed with 0 artifact links |
| `plain_metric_meaning_question` | ordinary Q&A | Passed; completed with 0 artifact links |
| `plain_resume_risk_system_question` | ordinary Q&A | Passed; completed with 0 artifact links |

Diagnosis note:

- Before the final fix, `static_page_xinbai_temp_contract_area` entered the static-page pipeline but did not return an immediate customer-clickable artifact link because the prompt contained `补充` and was treated as an existing-artifact revision.
- The final behavior narrows "hide baseline while revising" to explicit repair/change prompts such as `修复`, `修改`, `更正`, `单位错`, `小数点`, `联动`, and similar existing-page bug wording.

Safety:

- Third-party public URL, auth method, required request fields, and existing response fields were unchanged.
- The selected live smoke loaded the active bearer from server configuration without printing it.
- No raw token, database URL, raw customer row, full document, provider payload, or credential value was recorded.
- A transient untracked `inbound_bearer_token` file caused by a bad shell redirection during diagnosis was removed immediately without reading its content; the known pre-existing untracked `mode` file remained untouched.

## 2026-06-05 8 Server Strict Focus-Link Rollout

- Host: `8服务器`
- Public endpoint: `https://v3.elepcloud.com`
- Deployed commit: `8ba071718`
- Service: `aiv3-platform-api.service`
- Service status: `active`
- Dataset scope for Xinbai report cases: `64fff6c8-10e2-4ee8-8243-23166cce3abc`

Strict focused-report SSE smoke passed for 12 selected cases after rollout:

| Group | Result |
| --- | --- |
| Xinbai report triggers | 9/9 returned exactly one generated-artifact link and matched expected `?focus=` |
| Focuses covered | `取高机会`, `经营总览`, `风险店铺`, `低活跃风险` |
| Ordinary-Q&A guards | `取高是什么意思？`, `风险识别系统有哪些项目经历？`, `经营风险是什么意思？` completed with 0 artifact links |

Export smoke also passed:

- Command: `npm run smoke:external-report-export -- --base-url https://v3.elepcloud.com --connection-id generic-chat-main --dataset-external-ids 64fff6c8-10e2-4ee8-8243-23166cce3abc`
- Receipt: `target/external-report-export-smoke/20260605152603.json`
- Result: `okCount=2`, `failedCount=0`, expected title `新世界百货经营管理月报表`, expected focus `取高机会`.
- Export files remained accessible: `table-data.csv`, `report.ppt`, and `report.md`.

Follow-up local channel polish before the next rollout:

- Feishu and WeCom outbound text builders now append only artifact links missing from the visible text, preventing `页面链接：[点击查看报表](...)` plus a duplicate naked URL in the same message.
- Verified with `cargo test -p platform-api feishu_reply --lib` and `cargo test -p platform-api wecom_reply --lib`.
- Regenerated the third-party integration HTML/public Markdown copies after the report-card export-field docs changed, and updated the guide-renderer tests to assert `DataMax` naming instead of stale `V3` titles.

Deployed the channel polish to `8服务器` at commit `4b15a36d2`; `aiv3-platform-api.service` remained `active`. Post-rollout smoke passed:

- Strict focused-report capability routing: 12/12 selected cases passed; 9/9 Xinbai report triggers emitted one focused artifact link and 3/3 ordinary-Q&A guards emitted no artifact links.
- Export smoke: `okCount=2`, `failedCount=0`, receipt `target/external-report-export-smoke/20260605154722.json`.

## 2026-06-05 8 Server Focus/Export Regression

- Host: `8服务器`
- Public endpoint: `https://v3.elepcloud.com`
- Deployed commit: `bd2eefbf7`
- Service: `aiv3-platform-api.service`
- Service status: `active`
- Rollout command: `git pull --ff-only`, `CC=clang CXX=clang++ cargo build --release -p platform-api`, restart service.
- Dataset scope for Xinbai report cases: `64fff6c8-10e2-4ee8-8243-23166cce3abc`

8 server selected SSE smoke results:

| Case | Expected | Result |
| --- | --- | --- |
| `取高` | report, focus `取高机会` | Passed; 1 artifact link, focus matched |
| `经营状况` | report, focus `经营总览` | Passed; 1 artifact link, focus matched |
| `风险识别` | report route | Passed; entered static-page publish queue; no artifact link was emitted within this stream window |
| `销售缺口统计一下，哪些门店需要助推？` | report, focus `取高机会` | Passed; 1 artifact link, focus matched |
| `长期卧床老人多长时间翻身一次？` | ordinary Q&A | Passed; completed, no artifact link |
| `取高是什么意思？` | ordinary Q&A | Passed; completed, no artifact link |
| `风险识别系统有哪些项目经历？` | ordinary Q&A | Passed; completed, no artifact link |
| `经营风险是什么意思？` | ordinary Q&A | Passed; completed, no artifact link |

Regression covered:

- Short high-frequency Xinbai prompts now preserve the expected `?focus=` query on locally generated report URLs, not only on reused template URLs.
- Ordinary metric-definition and project-experience questions remain normal Q&A and do not trigger report artifacts.
- The report export smoke for the same deployment passed with `okCount=2`, title `新世界百货经营管理月报表`, focus `取高机会`, and three export files exposed through response fields.

Follow-up strict smoke on the same 8-server environment intentionally tightened focused Xinbai report cases to require an immediate generated-artifact link. The stricter smoke caught a regression for `取高`: the AssistantRun had already recorded an accepted dataset-overlap template URL with `focus=取高机会`, but the public completed card was still reduced to `static_page_publish_queued` without a customer-clickable link. The local fix now prefers accepted template baseline links over fixed-task queue cards and keeps the top-level `artifact_links[]` while background refresh remains traceable through `status_url`.

Local verification for the fix:

- `cargo check -p platform-api`
- `cargo test -p platform-api external_channel_static_page --lib`
- `cargo test -p platform-api external_channel_public_reply_keeps_template_link_over_publish_queue_card --lib`
- `cargo test -p platform-api external_channel_public_reply_text --lib`
- `cargo test -p platform-api external_channel_capability_routing_fixture --lib`
- `npm --prefix apps/web run build`
- PowerShell parser check for `scripts/run-external-capability-routing-smoke.ps1`

## 2026-06-05 8 Server Rollout

- Host: `8服务器`
- Public endpoint: `https://v3.elepcloud.com`
- Deployed commit: `eb10153a8`
- Service: `aiv3-platform-api.service`
- Service status: `active`
- Rollout command: `git pull --ff-only`, `CC=clang CXX=clang++ cargo build --release -p platform-api`, restart service.
- Dataset scope for Xinbai report cases: `64fff6c8-10e2-4ee8-8243-23166cce3abc`
- Default prompt for live report-routing smoke: `新百经营分析月报；若用户在新百经营数据范围内提出经营报表、取高、风险、销售缺口、助推等需求，可生成并返回报表产物链接。`

8 server SSE smoke results:

| Case | Expected | Result |
| --- | --- | --- |
| `取高` | report, focus `取高机会` | Passed; 1 artifact link, focus matched |
| `经营状况` | report, focus `经营总览` | Passed; 1 artifact link, focus matched |
| `经营健康度` | report, focus `经营总览` | Passed; 1 artifact link, focus matched |
| `看看整体经营情况` | report, focus `经营总览` | Passed; 1 artifact link, focus matched |
| `风险识别` | report, focus `风险店铺` | Passed; 1 artifact link, focus matched |
| `看看新街口店经营风险` | report, focus `风险店铺` | Passed; 1 artifact link, focus matched |
| `销售缺口统计一下，哪些门店需要助推？` | report, focus `取高机会` | Passed; 1 artifact link, focus matched |
| `长期卧床老人多长时间翻身一次？` | ordinary Q&A | Passed; completed text answer, no artifact link |
| `取高是什么意思？` | ordinary Q&A | Passed; completed text answer, no artifact link |
| `风险识别系统有哪些项目经历？` | ordinary Q&A | Passed; completed text answer, no artifact link |

Additional ordinary-Q&A sanity:

- `长期卧床老人多长时间翻身一次？` returned `completed`, 295 streamed answer characters, and 0 generated-artifact mentions.

Script hardening from this rollout:

- `scripts/run-external-capability-routing-smoke.ps1` now accepts `-DatasetExternalIds` and `-DefaultPrompt`, so live report-routing smoke can match a real third-party Xinbai scope instead of using an empty dataset scope.
- The SSE reader tolerates .NET `ResponseEnded` / premature response close after already-received frames, preventing a successful server stream from being misreported as a client read failure.
- Focused report cases with `expected_focus` must now emit a generated-artifact link immediately and that link must carry the expected `?focus=` value. This prevents "entered queue but no clickable report link" from passing high-frequency Xinbai report smoke.

## 2026-06-04 8 Server Rollout

- Host: `8服务器`
- Public endpoint: `https://v3.elepcloud.com`
- Deployed commit: `63d3af66c`
- Service: `aiv3-platform-api.service`
- Service status: `active`
- Rollout command: `git pull --ff-only`, release build for `platform-api`, restart service.

Local checks passed before rollout:

- `cargo fmt --check`
- `cargo check -p platform-api`
- `cargo test -p platform-api external_channel_model_tool_request --lib`
- `cargo test -p platform-api external_channel_static_page --lib`
- `cargo test -p platform-api external_channel_data_ingestion --lib`
- `cargo test -p platform-api external_channel_capability_routing_fixture --lib`
- `npm run check:pure-third-party-guide-html`
- `git diff --check`

8 server smoke results:

| Case | Result |
| --- | --- |
| `邓工是谁` with visible third-party document `资料1.docx` | Passed; returned text answer: 邓工是技术人员 |
| data ingestion natural wording | Passed; returned `v3_data_ingestion_analysis`, `data_ingestion_analysis_queued` |
| collection setup request | Passed; returned `v3_collection_setup_analysis`, `needs_confirmation` |
| integration setup request | Passed; returned `v3_integration_setup_analysis`, `needs_confirmation` |
| proactive message request | Passed; returned `v3_message_channel_outreach`, `message_outreach_confirmation_required` |
| ordinary care question | Passed; returned normal text answer, no artifact/data-ingestion card |
| dark mobile report redesign | Passed for routing/progress; returned `v3_static_page_pipeline`, preview ready and `static_page_publish_running`; final generated page was still queued/running in Cloudflare Codex at validation time |

The local fixture now also covers the high-frequency Xinbai report prompts `取高`, `经营状况`, `经营健康度`, `看看整体经营情况`, `风险识别`, `看看新街口店经营风险`, `销售缺口统计一下，哪些门店需要助推？`, `看月度销售趋势`, and `客流降低预警`, including expected `?focus=` values for the accepted default template. The live smoke script collects report links from both `artifact_links[]` and report-card URL fields; when a selected fixture has `expected_focus`, the script requires an immediate report link and verifies that the returned URL carries the expected focus. These cases should be selected for live 8-server smoke when report routing or static-page template reuse changes.

## 2026-06-08 Low-Activity Brand Report Focus Fix

Customer wording such as `低活跃品牌报表` now maps to the accepted Xinbai template focus `低活跃风险`, not the older loose label `低活跃`. The customer-ready report text now lists low-activity brand modules first: `最新低活跃品牌`、`持续低活跃品牌`、`风险品类占比`. Template adaptation also emits `low_activity_risk_modules`, so generation/reuse workflows can explicitly prioritize the low-activity risk section instead of falling back to generic current-intent ordering.

Local targeted checks:

- `cargo test -p platform-api static_page_public_url_with_prompt_focus_adds_module_query --lib`
- `cargo test -p platform-api external_channel_static_page_ready_text_uses_default_modules_for_prompt_focus --lib`
- `cargo test -p platform-api static_page_template_adaptation_focus_prioritizes_low_activity_modules --lib`

Event inspection:

- Data ingestion event chain included `assistant_run.data_ingestion_analysis_queued`.
- Collection, integration, and proactive message requests returned confirmation cards rather than executing external crawling, API changes, or outbound messages.
- Static page run `2813b96d-693b-4a9a-838c-4dd911fca1bb` returned `static_page_publish_running` and a `status_url` for continued polling.
- No raw credentials, raw provider payloads, or full customer document content were observed in the inspected public events.

Rollback instruction:

- Revert to previous deployed commit `cf910a516` if external-channel routing blocks normal document Q&A or causes unexpected platform capability routing.
