use domain_model::{DatasetOutput, MemoryDirectory, WorkflowExecution};
use serde_json::{json, Value};

pub(crate) fn chat_session_has_in_progress_turn(session_manifest: &Value) -> bool {
    let status = session_manifest
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if status != "pending_assistant_reply" {
        return false;
    }

    let turn_status = session_manifest
        .get("last_turn")
        .and_then(|turn| turn.get("status"))
        .and_then(Value::as_str);
    !matches!(turn_status, Some("completed" | "failed"))
}

pub(crate) fn build_pending_chat_session_turn_manifest(
    current_manifest: &Value,
    execution: &WorkflowExecution,
    prompt: &str,
    latest_memory_directory: Option<&MemoryDirectory>,
    latest_dataset_output: Option<&DatasetOutput>,
    chat_turn_id: &str,
) -> Value {
    let mut manifest = current_manifest.as_object().cloned().unwrap_or_default();
    manifest.insert(
        "generator".to_string(),
        Value::String("chat-session-workflow".to_string()),
    );
    manifest.insert(
        "schema_version".to_string(),
        Value::String("0.3.0".to_string()),
    );
    manifest.insert(
        "status".to_string(),
        Value::String("pending_assistant_reply".to_string()),
    );
    manifest
        .entry("initial_prompt".to_string())
        .or_insert_with(|| Value::String(prompt.trim().to_string()));
    manifest.insert(
        "last_prompt".to_string(),
        Value::String(prompt.trim().to_string()),
    );
    manifest.insert(
        "last_turn_kind".to_string(),
        Value::String("placeholder_orchestration".to_string()),
    );
    manifest
        .entry("context_binding".to_string())
        .or_insert_with(|| Value::String("creation_time".to_string()));
    manifest.insert(
        "latest_memory_directory_id".to_string(),
        json!(latest_memory_directory.map(|entry| entry.id)),
    );
    manifest.insert(
        "latest_memory_directory_version_no".to_string(),
        json!(latest_memory_directory.map(|entry| entry.version_no)),
    );
    manifest.insert(
        "latest_dataset_output_id".to_string(),
        json!(latest_dataset_output.map(|entry| entry.id)),
    );
    manifest.insert(
        "last_turn".to_string(),
        json!({
            "turn_id": chat_turn_id,
            "status": "pending",
            "stream_mode": "buffered",
            "provider_request_id": Value::Null,
            "provider_status": "pending",
            "finish_reason": Value::Null,
            "assistant_message_id": Value::Null,
            "tool_trace_count": 0,
            "events": [{
                "kind": "turn_started",
                "at": execution.created_at,
                "provider_request_id": Value::Null,
                "finish_reason": Value::Null,
                "tool_trace_count": Value::Null,
            }],
            "started_at": execution.created_at,
            "completed_at": Value::Null,
        }),
    );
    Value::Object(manifest)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use domain_model::{
        DatasetId, DatasetOutputId, MemoryDirectoryId, TenantId, UserId, WorkflowExecutionId,
        WorkflowKind, WorkflowStatus,
    };

    use super::*;

    fn workflow_execution(now: chrono::DateTime<Utc>) -> WorkflowExecution {
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: Some(DatasetId::new()),
            report_plan_id: None,
            kind: WorkflowKind::ChatSession,
            version: "0.3.0".to_string(),
            stage: "created".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 1,
            context: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    fn memory_directory(
        dataset_id: DatasetId,
        id: MemoryDirectoryId,
        execution_id: WorkflowExecutionId,
        now: chrono::DateTime<Utc>,
    ) -> MemoryDirectory {
        MemoryDirectory {
            id,
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id,
            owner_user_id: Some(UserId::new()),
            source_document_ids: Vec::new(),
            version_no: 7,
            directory_nodes: 3,
            refreshed_chunks: 9,
            directory_manifest: json!({}),
            created_at: now,
        }
    }

    fn dataset_output(
        dataset_id: DatasetId,
        id: DatasetOutputId,
        execution_id: WorkflowExecutionId,
        now: chrono::DateTime<Utc>,
    ) -> DatasetOutput {
        DatasetOutput {
            id,
            tenant_id: TenantId::new(),
            execution_id,
            dataset_id,
            owner_user_id: None,
            prompt: "old".to_string(),
            output_text: "output".to_string(),
            memory_directory_id: None,
            retrieval_evidence_ids: Vec::new(),
            output_manifest: json!({}),
            created_at: now,
        }
    }

    #[test]
    fn in_progress_turn_detects_only_pending_unfinished_sessions() {
        assert!(!chat_session_has_in_progress_turn(&json!({})));
        assert!(!chat_session_has_in_progress_turn(&json!({
            "status": "assistant_replied",
            "last_turn": { "status": "pending" }
        })));
        assert!(chat_session_has_in_progress_turn(&json!({
            "status": "pending_assistant_reply"
        })));
        assert!(chat_session_has_in_progress_turn(&json!({
            "status": "pending_assistant_reply",
            "last_turn": { "status": "pending" }
        })));
        assert!(!chat_session_has_in_progress_turn(&json!({
            "status": "pending_assistant_reply",
            "last_turn": { "status": "completed" }
        })));
        assert!(!chat_session_has_in_progress_turn(&json!({
            "status": "pending_assistant_reply",
            "last_turn": { "status": "failed" }
        })));
    }

    #[test]
    fn pending_turn_manifest_preserves_existing_context_and_sets_latest_scope() {
        let now = Utc::now();
        let execution = workflow_execution(now);
        let dataset_id = execution.dataset_id.expect("test execution has dataset");
        let memory_directory_id = MemoryDirectoryId::new();
        let dataset_output_id = DatasetOutputId::new();
        let memory_directory = memory_directory(dataset_id, memory_directory_id, execution.id, now);
        let output = dataset_output(dataset_id, dataset_output_id, execution.id, now);

        let manifest = build_pending_chat_session_turn_manifest(
            &json!({
                "initial_prompt": "original prompt",
                "context_binding": "custom_context",
                "thread_binding": { "local_thread_id": "local-1" }
            }),
            &execution,
            "  follow up prompt  ",
            Some(&memory_directory),
            Some(&output),
            "turn_2",
        );

        assert_eq!(manifest["generator"], json!("chat-session-workflow"));
        assert_eq!(manifest["schema_version"], json!("0.3.0"));
        assert_eq!(manifest["status"], json!("pending_assistant_reply"));
        assert_eq!(manifest["initial_prompt"], json!("original prompt"));
        assert_eq!(manifest["last_prompt"], json!("follow up prompt"));
        assert_eq!(
            manifest["last_turn_kind"],
            json!("placeholder_orchestration")
        );
        assert_eq!(manifest["context_binding"], json!("custom_context"));
        assert_eq!(
            manifest["thread_binding"]["local_thread_id"],
            json!("local-1")
        );
        assert_eq!(
            manifest["latest_memory_directory_id"],
            json!(memory_directory_id)
        );
        assert_eq!(manifest["latest_memory_directory_version_no"], json!(7));
        assert_eq!(
            manifest["latest_dataset_output_id"],
            json!(dataset_output_id)
        );
        assert_eq!(manifest["last_turn"]["turn_id"], json!("turn_2"));
        assert_eq!(manifest["last_turn"]["status"], json!("pending"));
        assert_eq!(manifest["last_turn"]["provider_status"], json!("pending"));
        assert_eq!(
            manifest["last_turn"]["events"][0]["kind"],
            json!("turn_started")
        );
        assert_eq!(manifest["last_turn"]["events"][0]["at"], json!(now));
        assert!(manifest["last_turn"]["completed_at"].is_null());
    }

    #[test]
    fn pending_turn_manifest_defaults_initial_prompt_context_and_absent_latest_scope() {
        let now = Utc::now();
        let execution = workflow_execution(now);

        let manifest = build_pending_chat_session_turn_manifest(
            &json!({ "unrelated": true }),
            &execution,
            "  first prompt  ",
            None,
            None,
            "turn_1",
        );

        assert_eq!(manifest["initial_prompt"], json!("first prompt"));
        assert_eq!(manifest["last_prompt"], json!("first prompt"));
        assert_eq!(manifest["context_binding"], json!("creation_time"));
        assert_eq!(manifest["latest_memory_directory_id"], Value::Null);
        assert_eq!(manifest["latest_memory_directory_version_no"], Value::Null);
        assert_eq!(manifest["latest_dataset_output_id"], Value::Null);
        assert_eq!(manifest["last_turn"]["tool_trace_count"], json!(0));
        assert_eq!(manifest["unrelated"], json!(true));
    }
}
