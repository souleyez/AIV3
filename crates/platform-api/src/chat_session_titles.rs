use domain_model::ChatSession;

pub(crate) fn derive_chat_session_title(prompt: &str) -> String {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let total_chars = normalized.chars().count();
    let mut title = normalized.chars().take(72).collect::<String>();
    if total_chars > 72 {
        title.push_str("...");
    }

    if title.is_empty() {
        "Untitled chat session".to_string()
    } else {
        title
    }
}

pub(crate) fn derive_chat_session_report_plan_title(session: &ChatSession) -> String {
    let base = session.title.trim();
    if base.is_empty() {
        return "Dataset Report".to_string();
    }

    let lowercase = base.to_ascii_lowercase();
    if lowercase.contains("report") {
        base.to_string()
    } else {
        format!("{base} Report")
    }
}

pub(crate) fn derive_chat_session_report_plan_objective(session: &ChatSession) -> String {
    let prompt = session
        .session_manifest
        .as_object()
        .and_then(|manifest| {
            manifest
                .get("last_prompt")
                .and_then(serde_json::Value::as_str)
                .or_else(|| {
                    manifest
                        .get("initial_prompt")
                        .and_then(serde_json::Value::as_str)
                })
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(session.title.trim());

    if prompt.is_empty() {
        "Turn the current dataset context into a report-ready output.".to_string()
    } else {
        format!("Turn the current dataset context into a report-ready output for: {prompt}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        ChatSessionId, DatasetId, MemoryDirectoryId, TenantId, UserId, WorkflowExecutionId,
    };
    use serde_json::{json, Value};

    fn sample_chat_session(title: &str, session_manifest: Value) -> ChatSession {
        ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            user_id: Some(UserId::new()),
            execution_id: WorkflowExecutionId::new(),
            title: title.to_string(),
            latest_memory_directory_id: Some(MemoryDirectoryId::new()),
            latest_dataset_output_id: None,
            session_manifest,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn chat_session_title_keeps_existing_whitespace_and_truncation_semantics() {
        assert_eq!(
            derive_chat_session_title("  月报\n\n  取高   机会  "),
            "月报 取高 机会"
        );
        assert_eq!(derive_chat_session_title("   "), "Untitled chat session");

        let title = derive_chat_session_title(&"一".repeat(73));
        assert_eq!(title.chars().count(), 75);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn report_plan_title_preserves_existing_report_suffix_semantics() {
        assert_eq!(
            derive_chat_session_report_plan_title(&sample_chat_session("经营分析", json!({}))),
            "经营分析 Report"
        );
        assert_eq!(
            derive_chat_session_report_plan_title(&sample_chat_session("Risk Report", json!({}))),
            "Risk Report"
        );
        assert_eq!(
            derive_chat_session_report_plan_title(&sample_chat_session("   ", json!({}))),
            "Dataset Report"
        );
    }

    #[test]
    fn report_plan_objective_prefers_last_prompt_then_initial_prompt_then_title() {
        assert_eq!(
            derive_chat_session_report_plan_objective(&sample_chat_session(
                "标题",
                json!({"initial_prompt": "初始", "last_prompt": " 最近 "})
            )),
            "Turn the current dataset context into a report-ready output for: 最近"
        );
        assert_eq!(
            derive_chat_session_report_plan_objective(&sample_chat_session(
                "标题",
                json!({"initial_prompt": " 初始 "})
            )),
            "Turn the current dataset context into a report-ready output for: 初始"
        );
        assert_eq!(
            derive_chat_session_report_plan_objective(&sample_chat_session("标题", json!({}))),
            "Turn the current dataset context into a report-ready output for: 标题"
        );
        assert_eq!(
            derive_chat_session_report_plan_objective(&sample_chat_session("  ", json!({}))),
            "Turn the current dataset context into a report-ready output."
        );
    }
}
