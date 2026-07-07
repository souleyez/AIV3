use contracts::CreateAssistantRunRequest;
use serde_json::Value;

use crate::{
    assistant_run_scope_selection_support::{
        selected_dataset_ids_from_scope, selected_document_ids_from_scope,
    },
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
    static_page_revision_artifact_support::static_page_public_url_from_current_artifact,
};

pub(crate) fn assistant_run_customer_codex_context_present(
    request: &CreateAssistantRunRequest,
    selected_scope: &Value,
    compact_prompt: &str,
    lower_prompt: &str,
) -> bool {
    request.current_artifact.is_some()
        || !selected_dataset_ids_from_scope(selected_scope).is_empty()
        || !selected_document_ids_from_scope(selected_scope).is_empty()
        || selected_scope.get("database_sources").is_some()
        || selected_scope.get("databaseSources").is_some()
        || selected_scope.get("type").and_then(Value::as_str) == Some("external_channel")
        || request_like_prompt_mentions_data_context(compact_prompt, lower_prompt)
}

pub(crate) fn assistant_run_prompt_or_context_has_report_artifact(
    prompt: &str,
    request: &CreateAssistantRunRequest,
    selected_scope: &Value,
) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let lower = compact.to_ascii_lowercase();
    let prompt_has_report_context = prompt_contains_any(
        &compact,
        &[
            "报表",
            "报告",
            "看板",
            "仪表盘",
            "可视化",
            "图表",
            "页面",
            "静态页",
            "月报",
            "周报",
            "日报",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "report",
            "dashboard",
            "visualization",
            "visualisation",
            "chart",
        ],
    );
    if prompt_has_report_context {
        return true;
    }
    if request
        .current_artifact
        .as_ref()
        .is_some_and(assistant_run_current_artifact_is_reportish)
    {
        return true;
    }
    [
        "report_plan_id",
        "reportPlanId",
        "report_entry",
        "reportEntry",
        "static_page_draft_id",
        "staticPageDraftId",
        "static_page",
        "staticPage",
        "current_report",
        "currentReport",
    ]
    .iter()
    .any(|key| selected_scope.get(*key).is_some())
}

fn assistant_run_current_artifact_is_reportish(current_artifact: &Value) -> bool {
    if static_page_public_url_from_current_artifact(current_artifact).is_some() {
        return true;
    }
    [
        "type",
        "artifact_type",
        "artifactType",
        "artifact_kind",
        "artifactKind",
        "kind",
    ]
    .iter()
    .filter_map(|key| current_artifact.get(*key).and_then(Value::as_str))
    .any(|value| {
        let lower = value.to_ascii_lowercase();
        value.contains("报表")
            || value.contains("报告")
            || value.contains("看板")
            || lower.contains("report")
            || lower.contains("static_page")
            || lower.contains("dashboard")
    })
}

pub(crate) fn request_like_prompt_mentions_data_context(
    compact_prompt: &str,
    lower_prompt: &str,
) -> bool {
    prompt_contains_any(
        compact_prompt,
        &[
            "数据",
            "数据集",
            "数据库",
            "文档",
            "报表",
            "新百",
            "门店",
            "经营",
        ],
    ) || ascii_prompt_contains_any(lower_prompt, &["data", "dataset", "database", "document"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_request(prompt: &str) -> CreateAssistantRunRequest {
        CreateAssistantRunRequest {
            prompt: prompt.to_string(),
            local_thread_id: None,
            startup_briefing: None,
            selected_scope: None,
            scope_candidates: Vec::new(),
            context_policy_hint: None,
            current_artifact: None,
            messages: Vec::new(),
        }
    }

    #[test]
    fn customer_codex_context_detects_scope_and_prompt_data_context() {
        let request = sample_request("cc 做经营分析");
        assert!(assistant_run_customer_codex_context_present(
            &request,
            &json!({"type": "external_channel"}),
            "cc做经营分析",
            "cc做经营分析",
        ));
        assert!(assistant_run_customer_codex_context_present(
            &request,
            &json!({}),
            "门店经营数据",
            "门店经营数据",
        ));
        assert!(!assistant_run_customer_codex_context_present(
            &request,
            &json!({}),
            "普通闲聊",
            "普通闲聊",
        ));
    }

    #[test]
    fn report_artifact_context_detects_prompt_current_artifact_and_scope_keys() {
        let request = sample_request("cc 输出报表");
        assert!(assistant_run_prompt_or_context_has_report_artifact(
            &request.prompt,
            &request,
            &json!({})
        ));

        let mut request = sample_request("cc 继续修改");
        request.current_artifact = Some(json!({"type": "dashboard"}));
        assert!(assistant_run_prompt_or_context_has_report_artifact(
            &request.prompt,
            &request,
            &json!({})
        ));

        let request = sample_request("cc 继续修改");
        assert!(assistant_run_prompt_or_context_has_report_artifact(
            &request.prompt,
            &request,
            &json!({"staticPageDraftId": "draft-1"})
        ));
    }

    #[test]
    fn report_artifact_context_uses_static_page_public_url() {
        let mut request = sample_request("cc 继续修改");
        request.current_artifact = Some(json!({
            "finalPage": {
                "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html"
            }
        }));
        assert!(assistant_run_prompt_or_context_has_report_artifact(
            &request.prompt,
            &request,
            &json!({})
        ));
    }

    #[test]
    fn report_artifact_context_rejects_plain_chat_without_scope_or_artifact() {
        let request = sample_request("cc 解释一下");
        assert!(!assistant_run_prompt_or_context_has_report_artifact(
            &request.prompt,
            &request,
            &json!({})
        ));
    }
}
