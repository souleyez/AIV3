use contracts::{CreateStaticPageRenderResponse, StaticPageDraftView, StaticPageRenderOutputView};
use domain_model::{StaticPageDraft, StaticPageDraftStatus, StaticPageRenderOutput};
use serde_json::{json, Value};

use crate::static_page_operation_apply_support::{
    append_static_page_operations_metadata, apply_static_page_operations_to_payload,
};
use crate::static_page_view_support::{
    to_static_page_draft_view, to_static_page_render_output_view,
};

const STATIC_PAGE_RENDER_CREATED_EVENT: &str = "static_page_render.created";
const STATIC_PAGE_FINAL_RENDER_OPERATION_TYPE: &str = "request_final_render";
const STATIC_PAGE_FINAL_RENDER_STATUS: &str = "rendered";
const STATIC_PAGE_FINAL_RENDER_DIRECT_HTML_SUMMARY: &str =
    "最终静态页已按快速 HTML 交付模式生成，未经过可视化确认。";
const STATIC_PAGE_FINAL_RENDER_VISUAL_SUMMARY: &str = "最终静态页已根据可视化和模块规划生成。";

pub(crate) fn static_page_render_created_event_name() -> &'static str {
    STATIC_PAGE_RENDER_CREATED_EVENT
}

pub(crate) fn static_page_final_render_operation_type() -> &'static str {
    STATIC_PAGE_FINAL_RENDER_OPERATION_TYPE
}

pub(crate) fn static_page_final_render_status() -> &'static str {
    STATIC_PAGE_FINAL_RENDER_STATUS
}

pub(crate) fn static_page_final_render_direct_html_summary() -> &'static str {
    STATIC_PAGE_FINAL_RENDER_DIRECT_HTML_SUMMARY
}

pub(crate) fn static_page_final_render_visual_summary() -> &'static str {
    STATIC_PAGE_FINAL_RENDER_VISUAL_SUMMARY
}

pub(crate) fn static_page_final_render_draft_status() -> StaticPageDraftStatus {
    StaticPageDraftStatus::Rendered
}

pub(crate) fn static_page_render_response(
    draft: StaticPageDraft,
    render_output: StaticPageRenderOutput,
) -> CreateStaticPageRenderResponse {
    CreateStaticPageRenderResponse {
        render_output: static_page_render_output_view_for_response(&draft, render_output),
        draft: static_page_draft_view_for_response(draft),
    }
}

pub(crate) fn static_page_draft_view_for_response(draft: StaticPageDraft) -> StaticPageDraftView {
    to_static_page_draft_view(draft)
}

pub(crate) fn static_page_render_output_view_for_response(
    draft: &StaticPageDraft,
    render_output: StaticPageRenderOutput,
) -> StaticPageRenderOutputView {
    to_static_page_render_output_view(render_output, Some(&draft.selected_scope))
}

pub(crate) fn apply_static_page_final_render_to_draft(
    mut draft: StaticPageDraft,
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) -> StaticPageDraft {
    static_page_final_render_complete_draft(&mut draft, render_output, direct_html);
    draft
}

pub(crate) fn static_page_final_render_complete_draft(
    draft: &mut StaticPageDraft,
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) {
    let (operations, render_summary) =
        static_page_final_render_operations_and_summary(render_output, direct_html);
    static_page_final_render_update_draft_payload(draft, &operations, render_summary);
    static_page_final_render_mark_draft_status(draft);
}

pub(crate) fn static_page_final_render_mark_draft_status(draft: &mut StaticPageDraft) {
    draft.status = static_page_final_render_draft_status();
}

pub(crate) fn static_page_final_render_apply_payload(
    draft_payload: Value,
    operations: &[Value],
    render_summary: &str,
) -> Value {
    apply_static_page_operations_to_payload(draft_payload, operations, Some(render_summary))
}

pub(crate) fn static_page_final_render_append_metadata(
    draft_payload: &mut Value,
    operations: &[Value],
    render_summary: &str,
) {
    append_static_page_operations_metadata(draft_payload, operations, None, render_summary);
}

pub(crate) fn static_page_final_render_update_draft_payload(
    draft: &mut StaticPageDraft,
    operations: &[Value],
    render_summary: &str,
) {
    let draft_payload = std::mem::replace(&mut draft.draft_payload, Value::Null);
    draft.draft_payload =
        static_page_final_render_apply_payload(draft_payload, operations, render_summary);
    static_page_final_render_append_metadata(&mut draft.draft_payload, operations, render_summary);
}

pub(crate) fn static_page_render_created_event_payload(
    draft: &StaticPageDraft,
    render_output: &StaticPageRenderOutput,
) -> Value {
    static_page_render_created_event_ids_payload(
        draft.id,
        render_output.id,
        render_output.image_job_id,
    )
}

pub(crate) fn static_page_render_created_event_ids_payload(
    draft_id: domain_model::StaticPageDraftId,
    render_output_id: domain_model::StaticPageRenderOutputId,
    image_job_id: Option<domain_model::StaticPageImageJobId>,
) -> Value {
    json!({
        "draft_id": draft_id,
        "render_output_id": render_output_id,
        "image_job_id": image_job_id,
    })
}

pub(crate) fn static_page_final_render_page_payload(
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) -> Value {
    json!({
        "status": static_page_final_render_status(),
        "renderOutputId": render_output.id,
        "assetManifest": render_output.asset_manifest,
        "directHtml": direct_html,
    })
}

pub(crate) fn static_page_final_render_operation_payload(
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) -> Value {
    json!({
        "type": static_page_final_render_operation_type(),
        "finalPage": static_page_final_render_page_payload(render_output, direct_html),
    })
}

pub(crate) fn static_page_final_render_operations(
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) -> Vec<Value> {
    vec![static_page_final_render_operation_payload(
        render_output,
        direct_html,
    )]
}

pub(crate) fn static_page_final_render_operations_and_summary(
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) -> (Vec<Value>, &'static str) {
    (
        static_page_final_render_operations(render_output, direct_html),
        static_page_final_render_summary(direct_html),
    )
}

pub(crate) fn static_page_final_render_summary(direct_html: bool) -> &'static str {
    if direct_html {
        static_page_final_render_direct_html_summary()
    } else {
        static_page_final_render_visual_summary()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use contracts::{StaticPageDraftStatusView, StaticPageRenderOutputStatusView};
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageRenderOutputId, StaticPageRenderOutputStatus, TenantId,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn render_created_event_name_matches_existing_render_event() {
        assert_eq!(
            static_page_render_created_event_name(),
            "static_page_render.created"
        );
    }

    #[test]
    fn final_render_operation_type_matches_existing_operation() {
        assert_eq!(
            static_page_final_render_operation_type(),
            "request_final_render"
        );
    }

    #[test]
    fn final_render_status_matches_existing_final_page_status() {
        assert_eq!(static_page_final_render_status(), "rendered");
    }

    #[test]
    fn final_render_summary_copy_helpers_match_existing_copy() {
        assert_eq!(
            static_page_final_render_direct_html_summary(),
            "最终静态页已按快速 HTML 交付模式生成，未经过可视化确认。"
        );
        assert_eq!(
            static_page_final_render_visual_summary(),
            "最终静态页已根据可视化和模块规划生成。"
        );
    }

    #[test]
    fn final_render_draft_status_matches_existing_rendered_status() {
        assert_eq!(
            static_page_final_render_draft_status(),
            StaticPageDraftStatus::Rendered
        );
    }

    fn draft() -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "经营分析".to_string(),
            status: StaticPageDraftStatus::Confirmed,
            selected_scope: json!({"dataset_ids": ["dataset-1"]}),
            visibility_snapshot: json!({}),
            source_refs: json!([]),
            draft_payload: json!({
                "modules": [
                    {"id": "hero", "title": "总览"}
                ]
            }),
            created_at: now,
            updated_at: now,
        }
    }

    fn render_output() -> StaticPageRenderOutput {
        StaticPageRenderOutput {
            id: StaticPageRenderOutputId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            image_job_id: None,
            status: StaticPageRenderOutputStatus::Rendered,
            html: "<html>ok</html>".to_string(),
            asset_manifest: json!({
                "files": ["index.html"],
                "renderer": "static-page-renderer-v1"
            }),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn draft_view_for_response_preserves_draft_fields() {
        let draft = draft();
        let draft_id = draft.id;
        let assistant_run_id = draft.assistant_run_id;
        let title = draft.title.clone();
        let selected_scope = draft.selected_scope.clone();
        let visibility_snapshot = draft.visibility_snapshot.clone();
        let source_refs = draft.source_refs.clone();
        let draft_payload = draft.draft_payload.clone();

        let view = static_page_draft_view_for_response(draft);

        assert_eq!(view.id, draft_id);
        assert_eq!(view.assistant_run_id, assistant_run_id);
        assert_eq!(view.title, title);
        assert_eq!(view.status, StaticPageDraftStatusView::Confirmed);
        assert_eq!(view.selected_scope, selected_scope);
        assert_eq!(view.visibility_snapshot, visibility_snapshot);
        assert_eq!(view.source_refs, source_refs);
        assert_eq!(view.draft_payload, draft_payload);
    }

    #[test]
    fn final_render_mark_draft_status_sets_rendered_status() {
        let mut draft = draft();
        draft.status = StaticPageDraftStatus::Confirmed;

        static_page_final_render_mark_draft_status(&mut draft);

        assert_eq!(draft.status, StaticPageDraftStatus::Rendered);
    }

    #[test]
    fn final_render_operations_preserve_output_manifest_and_direct_html_flag() {
        let output = render_output();

        let operations = static_page_final_render_operations(&output, true);

        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0]["type"], json!("request_final_render"));
        assert_eq!(operations[0]["finalPage"]["status"], json!("rendered"));
        assert_eq!(
            operations[0]["finalPage"]["renderOutputId"],
            json!(output.id)
        );
        assert_eq!(
            operations[0]["finalPage"]["assetManifest"],
            output.asset_manifest
        );
        assert_eq!(operations[0]["finalPage"]["directHtml"], json!(true));
    }

    #[test]
    fn final_render_page_payload_preserves_status_manifest_and_direct_html_flag() {
        let output = render_output();

        let final_page = static_page_final_render_page_payload(&output, false);

        assert_eq!(final_page["status"], json!("rendered"));
        assert_eq!(final_page["renderOutputId"], json!(output.id));
        assert_eq!(final_page["assetManifest"], output.asset_manifest);
        assert_eq!(final_page["directHtml"], json!(false));
    }

    #[test]
    fn final_render_operation_payload_preserves_type_and_final_page() {
        let output = render_output();

        let operation = static_page_final_render_operation_payload(&output, true);

        assert_eq!(operation["type"], json!("request_final_render"));
        assert_eq!(operation["finalPage"]["status"], json!("rendered"));
        assert_eq!(operation["finalPage"]["renderOutputId"], json!(output.id));
        assert_eq!(
            operation["finalPage"]["assetManifest"],
            output.asset_manifest
        );
        assert_eq!(operation["finalPage"]["directHtml"], json!(true));
    }

    #[test]
    fn final_render_operations_and_summary_preserves_direct_html_choice() {
        let output = render_output();

        let (visual_operations, visual_summary) =
            static_page_final_render_operations_and_summary(&output, false);
        let (direct_operations, direct_summary) =
            static_page_final_render_operations_and_summary(&output, true);

        assert_eq!(
            visual_operations[0]["finalPage"]["directHtml"],
            json!(false)
        );
        assert_eq!(visual_summary, "最终静态页已根据可视化和模块规划生成。");
        assert_eq!(direct_operations[0]["finalPage"]["directHtml"], json!(true));
        assert_eq!(
            direct_summary,
            "最终静态页已按快速 HTML 交付模式生成，未经过可视化确认。"
        );
    }

    #[test]
    fn final_render_apply_payload_preserves_final_page_and_summary() {
        let draft = draft();
        let output = render_output();
        let operations = static_page_final_render_operations(&output, false);
        let summary = static_page_final_render_summary(false);

        let payload =
            static_page_final_render_apply_payload(draft.draft_payload, &operations, summary);

        assert_eq!(payload["finalPage"]["renderOutputId"], json!(output.id));
        assert_eq!(payload["finalPage"]["assetManifest"], output.asset_manifest);
        assert_eq!(payload["finalPage"]["directHtml"], json!(false));
        assert_eq!(payload["modelSummary"], json!(summary));
        assert_eq!(payload["model_summary"], json!(summary));
    }

    #[test]
    fn final_render_append_metadata_preserves_operations_and_summary() {
        let output = render_output();
        let operations = static_page_final_render_operations(&output, true);
        let summary = static_page_final_render_summary(true);
        let mut payload = json!({});

        static_page_final_render_append_metadata(&mut payload, &operations, summary);

        assert_eq!(payload["lastOperationSummary"], json!(summary));
        assert_eq!(
            payload["operations"][0]["type"],
            json!("request_final_render")
        );
        assert_eq!(
            payload["operations"][0]["finalPage"]["renderOutputId"],
            json!(output.id)
        );
    }

    #[test]
    fn final_render_update_draft_payload_preserves_payload_metadata_and_status() {
        let mut draft = draft();
        let output = render_output();
        let operations = static_page_final_render_operations(&output, false);
        let summary = static_page_final_render_summary(false);

        static_page_final_render_update_draft_payload(&mut draft, &operations, summary);

        assert_eq!(draft.status, StaticPageDraftStatus::Confirmed);
        assert_eq!(
            draft.draft_payload["finalPage"]["renderOutputId"],
            json!(output.id)
        );
        assert_eq!(
            draft.draft_payload["finalPage"]["assetManifest"],
            output.asset_manifest
        );
        assert_eq!(draft.draft_payload["finalPage"]["directHtml"], json!(false));
        assert_eq!(draft.draft_payload["modelSummary"], json!(summary));
        assert_eq!(draft.draft_payload["lastOperationSummary"], json!(summary));
        assert_eq!(
            draft.draft_payload["operations"][0]["type"],
            json!("request_final_render")
        );
    }

    #[test]
    fn final_render_complete_draft_preserves_payload_metadata_and_rendered_status() {
        let mut draft = draft();
        let output = render_output();

        static_page_final_render_complete_draft(&mut draft, &output, true);

        assert_eq!(draft.status, StaticPageDraftStatus::Rendered);
        assert_eq!(
            draft.draft_payload["finalPage"]["renderOutputId"],
            json!(output.id)
        );
        assert_eq!(
            draft.draft_payload["finalPage"]["assetManifest"],
            output.asset_manifest
        );
        assert_eq!(draft.draft_payload["finalPage"]["directHtml"], json!(true));
        assert_eq!(
            draft.draft_payload["lastOperationSummary"],
            json!("最终静态页已按快速 HTML 交付模式生成，未经过可视化确认。")
        );
        assert_eq!(
            draft.draft_payload["operations"][0]["type"],
            json!("request_final_render")
        );
    }

    #[test]
    fn final_render_summary_distinguishes_fast_html_from_visual_render() {
        assert_eq!(
            static_page_final_render_summary(true),
            "最终静态页已按快速 HTML 交付模式生成，未经过可视化确认。"
        );
        assert_eq!(
            static_page_final_render_summary(false),
            "最终静态页已根据可视化和模块规划生成。"
        );
    }

    #[test]
    fn final_render_to_draft_applies_operation_metadata_and_rendered_status() {
        let draft = draft();
        let output = render_output();

        let draft = apply_static_page_final_render_to_draft(draft, &output, false);

        assert_eq!(draft.status, StaticPageDraftStatus::Rendered);
        assert_eq!(
            draft.draft_payload["finalPage"]["renderOutputId"],
            json!(output.id)
        );
        assert_eq!(
            draft.draft_payload["finalPage"]["assetManifest"],
            output.asset_manifest
        );
        assert_eq!(draft.draft_payload["finalPage"]["directHtml"], json!(false));
        assert_eq!(
            draft.draft_payload["lastOperationSummary"],
            json!("最终静态页已根据可视化和模块规划生成。")
        );
        assert_eq!(
            draft.draft_payload["operations"][0]["type"],
            json!("request_final_render")
        );
    }

    #[test]
    fn render_created_event_payload_preserves_draft_output_and_image_job_ids() {
        let draft = draft();
        let mut output = render_output();
        let image_job_id = StaticPageImageJobId::new();
        output.image_job_id = Some(image_job_id);

        let payload = static_page_render_created_event_payload(&draft, &output);

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["render_output_id"], json!(output.id));
        assert_eq!(payload["image_job_id"], json!(image_job_id));
    }

    #[test]
    fn render_created_event_ids_payload_preserves_event_field_names() {
        let draft_id = StaticPageDraftId::new();
        let render_output_id = StaticPageRenderOutputId::new();
        let image_job_id = StaticPageImageJobId::new();

        let payload = static_page_render_created_event_ids_payload(
            draft_id,
            render_output_id,
            Some(image_job_id),
        );

        assert_eq!(payload["draft_id"], json!(draft_id));
        assert_eq!(payload["render_output_id"], json!(render_output_id));
        assert_eq!(payload["image_job_id"], json!(image_job_id));
    }

    #[test]
    fn render_output_view_for_response_uses_draft_scope_for_external_urls() {
        let mut draft = draft();
        draft.selected_scope = json!({
            "type": "external_channel",
            "channelConnectionId": "generic-chat-main"
        });
        let mut output = render_output();
        output.draft_id = draft.id;
        output.assistant_run_id = draft.assistant_run_id;
        let output_id = output.id;

        let view = static_page_render_output_view_for_response(&draft, output);

        assert_eq!(
            view.html_download_url,
            Some(format!(
                "/v1/external/channels/generic-chat-main/static-page-renders/{}/download",
                output_id
            ))
        );
        assert_eq!(
            view.html_preview_url,
            Some(format!(
                "/v1/external/channels/generic-chat-main/static-page-renders/{}/preview",
                output_id
            ))
        );
    }

    #[test]
    fn render_response_uses_draft_scope_for_render_output_view() {
        let draft = draft();
        let mut output = render_output();
        output.draft_id = draft.id;
        output.assistant_run_id = draft.assistant_run_id;
        let draft_id = draft.id;
        let output_id = output.id;

        let response = static_page_render_response(draft, output);

        assert_eq!(response.draft.id, draft_id);
        assert_eq!(response.render_output.id, output_id);
        assert_eq!(response.render_output.draft_id, draft_id);
        assert_eq!(
            response.render_output.status,
            StaticPageRenderOutputStatusView::Rendered
        );
    }
}
