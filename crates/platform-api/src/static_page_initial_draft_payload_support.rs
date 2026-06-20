use domain_model::AssistantRun;
use serde_json::{json, Value};

use crate::{
    build_static_page_data_snapshot_with_evidence, build_static_page_preview_contract,
    build_static_page_render_spec, build_static_page_visual_spec, derive_static_page_draft_title,
};

pub(crate) fn build_initial_static_page_draft_payload(run: &AssistantRun, prompt: &str) -> Value {
    let style_direction = "client-delivery";
    let visual_spec = build_static_page_visual_spec(style_direction);
    let render_spec = build_static_page_render_spec();
    let data_snapshot = build_static_page_data_snapshot_with_evidence(
        &json!({ "modules": [] }),
        &run.selected_scope,
        Some(&run.evidence_state),
        "assistant_run",
    );
    let preview_contract = build_static_page_preview_contract(
        style_direction,
        &Value::Array(Vec::new()),
        &render_spec,
        &Value::Array(Vec::new()),
        None,
    );
    json!({
        "version": 1,
        "status": "draft",
        "title": derive_static_page_draft_title(prompt),
        "prompt": prompt,
        "modules": [],
        "mobileOrder": [],
        "styleDirection": style_direction,
        "style_direction": style_direction,
        "visualSpec": visual_spec.clone(),
        "visual_spec": visual_spec,
        "renderSpec": render_spec.clone(),
        "render_spec": render_spec,
        "dataSnapshot": data_snapshot.clone(),
        "data_snapshot": data_snapshot,
        "previewContract": preview_contract.clone(),
        "preview_contract": preview_contract,
        "data_bindings": [],
        "assistant_context": {
            "assistant_run_id": run.id,
            "selected_scope": run.selected_scope,
            "evidence_state": run.evidence_state,
        },
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{AssistantRun, AssistantRunId, TenantId};
    use serde_json::json;

    use super::*;

    #[test]
    fn initial_static_page_draft_payload_keeps_contract_aliases_and_context() {
        let run_id = AssistantRunId::new();
        let run = AssistantRun {
            id: run_id,
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("thread-1".to_string()),
            user_prompt: "生成经营报表".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({
                "datasets": [],
                "intent": "static_page"
            }),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state: json!({
                "status": "ready",
                "updated_at": "2026-06-20T00:00:00Z",
                "supplied_items": []
            }),
            service_lane: "static_page".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let payload = build_initial_static_page_draft_payload(&run, " 新百 经营总览 ");

        assert_eq!(payload["version"], json!(1));
        assert_eq!(payload["status"], json!("draft"));
        assert_eq!(payload["title"], json!("静态页：新百 经营总览"));
        assert_eq!(payload["prompt"], json!(" 新百 经营总览 "));
        assert_eq!(payload["styleDirection"], payload["style_direction"]);
        assert_eq!(payload["visualSpec"], payload["visual_spec"]);
        assert_eq!(payload["renderSpec"], payload["render_spec"]);
        assert_eq!(payload["dataSnapshot"], payload["data_snapshot"]);
        assert_eq!(payload["previewContract"], payload["preview_contract"]);
        assert_eq!(
            payload["assistant_context"]["assistant_run_id"],
            json!(run_id)
        );
        assert_eq!(payload["dataSnapshot"]["source"], json!("assistant_run"));
        assert_eq!(payload["dataSnapshot"]["evidence_status"], json!("ready"));
        assert_eq!(
            payload["previewContract"]["kind"],
            json!("static-page-preview-contract")
        );
        assert_eq!(payload["previewContract"]["status"], json!("not_requested"));
    }
}
