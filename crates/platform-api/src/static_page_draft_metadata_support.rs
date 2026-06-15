use domain_model::AssistantRun;
use serde_json::{json, Value};

use crate::assistant_run_evidence_state_support::assistant_run_evidence_supplied_count;
use crate::value_array;

pub(crate) fn derive_static_page_draft_title(prompt: &str) -> String {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "静态页草稿".to_string();
    }
    let total_chars = normalized.chars().count();
    let mut title = normalized.chars().take(42).collect::<String>();
    if total_chars > 42 {
        title.push_str("...");
    }
    format!("静态页：{title}")
}

pub(crate) fn build_static_page_source_refs(run: &AssistantRun) -> Value {
    json!({
        "source": "local_chat_static_page_image2_pipeline",
        "assistant_run_id": run.id,
        "local_thread_id": run.local_thread_id,
        "output_artifact_count": value_array(run.output_artifacts.clone()).len(),
        "evidence_status": run.evidence_state.get("status").cloned().unwrap_or(Value::Null),
        "supplied_evidence_count": assistant_run_evidence_supplied_count(&run.evidence_state),
        "auto_publish_generated_artifact": true,
        "effect_image_confirmation_required": false,
        "continue_to_publish_after_effect_image": true,
        "fixed_task_template_id": "static_page_image2_data_publish",
        "customer_preview_delivery": "stream_event_or_status_card",
    })
}

pub(crate) fn build_static_page_visibility_snapshot(
    run: &AssistantRun,
    selected_scope: &Value,
) -> Value {
    json!({
        "assistant_run_id": run.id,
        "selected_scope": selected_scope,
        "policy": "assistant_run_scope_snapshot",
        "created_from_evidence_state": run.evidence_state.get("status").and_then(Value::as_str).unwrap_or("unknown"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, TenantId};

    fn test_run() -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("local-thread-1".to_string()),
            user_prompt: "生成经营报表".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({"dataset_ids": ["dataset-1"]}),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state: json!({
                "status": "supplied",
                "supplied_items": [{}, {}, {}]
            }),
            service_lane: "assistant_run".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([
                {"kind": "static_page"},
                {"kind": "markdown"}
            ]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn draft_title_uses_default_for_blank_prompt() {
        assert_eq!(derive_static_page_draft_title(" \n\t "), "静态页草稿");
    }

    #[test]
    fn draft_title_normalizes_whitespace_and_prefixes_title() {
        assert_eq!(
            derive_static_page_draft_title(" 新百   经营\n月报 "),
            "静态页：新百 经营 月报"
        );
    }

    #[test]
    fn draft_title_truncates_after_forty_two_chars() {
        let prompt = "一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五";
        let title = derive_static_page_draft_title(prompt);

        assert!(title.starts_with("静态页："));
        assert!(title.ends_with("..."));
        assert_eq!(title.trim_start_matches("静态页：").chars().count(), 45);
    }

    #[test]
    fn source_refs_preserve_static_page_pipeline_contract() {
        let run = test_run();
        let source_refs = build_static_page_source_refs(&run);

        assert_eq!(
            source_refs["source"],
            json!("local_chat_static_page_image2_pipeline")
        );
        assert_eq!(source_refs["assistant_run_id"], json!(run.id));
        assert_eq!(source_refs["local_thread_id"], json!("local-thread-1"));
        assert_eq!(source_refs["output_artifact_count"], json!(2));
        assert_eq!(source_refs["evidence_status"], json!("supplied"));
        assert_eq!(source_refs["supplied_evidence_count"], json!(3));
        assert_eq!(
            source_refs["fixed_task_template_id"],
            json!("static_page_image2_data_publish")
        );
        assert_eq!(
            source_refs["customer_preview_delivery"],
            json!("stream_event_or_status_card")
        );
    }

    #[test]
    fn visibility_snapshot_keeps_selected_scope_and_evidence_status() {
        let run = test_run();
        let selected_scope = json!({"dataset_ids": ["selected"]});
        let snapshot = build_static_page_visibility_snapshot(&run, &selected_scope);

        assert_eq!(snapshot["assistant_run_id"], json!(run.id));
        assert_eq!(snapshot["selected_scope"], selected_scope);
        assert_eq!(snapshot["policy"], json!("assistant_run_scope_snapshot"));
        assert_eq!(snapshot["created_from_evidence_state"], json!("supplied"));
    }

    #[test]
    fn visibility_snapshot_defaults_unknown_evidence_status() {
        let mut run = test_run();
        run.evidence_state = json!({});
        let snapshot = build_static_page_visibility_snapshot(&run, &json!({}));

        assert_eq!(snapshot["created_from_evidence_state"], json!("unknown"));
    }
}
