# External Capability Routing Smoke

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

The local fixture now also covers the high-frequency Xinbai report prompts `取高`, `经营状况`, `风险识别`, `销售缺口统计一下，哪些门店需要助推？`, `看月度销售趋势`, and `客流降低预警`, including expected `?focus=` values for the accepted default template. These should be selected for live 8-server smoke when report routing or static-page template reuse changes.

Event inspection:

- Data ingestion event chain included `assistant_run.data_ingestion_analysis_queued`.
- Collection, integration, and proactive message requests returned confirmation cards rather than executing external crawling, API changes, or outbound messages.
- Static page run `2813b96d-693b-4a9a-838c-4dd911fca1bb` returned `static_page_publish_running` and a `status_url` for continued polling.
- No raw credentials, raw provider payloads, or full customer document content were observed in the inspected public events.

Rollback instruction:

- Revert to previous deployed commit `cf910a516` if external-channel routing blocks normal document Q&A or causes unexpected platform capability routing.
