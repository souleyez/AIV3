use contracts::CreateStaticPageRenderResponse;
use domain_model::{StaticPageDraft, StaticPageDraftStatus, StaticPageRenderOutput};
use serde_json::{json, Value};

use crate::static_page_operation_apply_support::{
    append_static_page_operations_metadata, apply_static_page_operations_to_payload,
};
use crate::static_page_view_support::{
    to_static_page_draft_view, to_static_page_render_output_view,
};

pub(crate) fn static_page_render_response(
    draft: StaticPageDraft,
    render_output: StaticPageRenderOutput,
) -> CreateStaticPageRenderResponse {
    CreateStaticPageRenderResponse {
        render_output: to_static_page_render_output_view(
            render_output,
            Some(&draft.selected_scope),
        ),
        draft: to_static_page_draft_view(draft),
    }
}

pub(crate) fn apply_static_page_final_render_to_draft(
    mut draft: StaticPageDraft,
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) -> StaticPageDraft {
    let operations = static_page_final_render_operations(render_output, direct_html);
    let render_summary = static_page_final_render_summary(direct_html);
    draft.draft_payload = apply_static_page_operations_to_payload(
        draft.draft_payload,
        &operations,
        Some(render_summary),
    );
    append_static_page_operations_metadata(
        &mut draft.draft_payload,
        &operations,
        None,
        render_summary,
    );
    draft.status = StaticPageDraftStatus::Rendered;
    draft
}

pub(crate) fn static_page_render_created_event_payload(
    draft: &StaticPageDraft,
    render_output: &StaticPageRenderOutput,
) -> Value {
    json!({
        "draft_id": draft.id,
        "render_output_id": render_output.id,
        "image_job_id": render_output.image_job_id,
    })
}

pub(crate) fn static_page_final_render_operations(
    render_output: &StaticPageRenderOutput,
    direct_html: bool,
) -> Vec<Value> {
    vec![json!({
        "type": "request_final_render",
        "finalPage": {
            "status": "rendered",
            "renderOutputId": render_output.id,
            "assetManifest": render_output.asset_manifest,
            "directHtml": direct_html,
        }
    })]
}

pub(crate) fn static_page_final_render_summary(direct_html: bool) -> &'static str {
    if direct_html {
        "最终静态页已按快速 HTML 交付模式生成，未经过可视化确认。"
    } else {
        "最终静态页已根据可视化和模块规划生成。"
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use contracts::StaticPageRenderOutputStatusView;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageRenderOutputId, StaticPageRenderOutputStatus, TenantId,
    };
    use serde_json::json;

    use super::*;

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
