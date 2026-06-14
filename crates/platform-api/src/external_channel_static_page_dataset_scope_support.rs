use domain_model::{AssistantRun, TenantId};
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::external_bot_message_payload_support::external_string_ids_from_payload_value;
use crate::external_channel_support::{
    external_channel_allowed_database_source_ids, external_channel_default_source_id_from_config,
};
use crate::text_normalization::non_empty_trimmed_string;
use crate::{
    selected_dataset_ids_from_scope, selected_document_ids_from_scope,
    ExternalChannelConnectionSummary,
};

pub(crate) fn collect_external_static_page_database_source_ids(
    connection: &ExternalChannelConnectionSummary,
    selected_scope: &Value,
    evidence_state: &Value,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    let mut selected_scope_database_ids = BTreeSet::new();
    let fixed_report_scope = selected_scope
        .get("database_report_scope_policy")
        .and_then(Value::as_str)
        == Some("fixed_dataset_independent_of_chat_selection");

    for key in [
        "database_source_id",
        "databaseSourceId",
        "source_database_id",
        "sourceDatabaseId",
    ] {
        if let Some(value) = selected_scope
            .get(key)
            .and_then(Value::as_str)
            .and_then(non_empty_trimmed_string)
        {
            selected_scope_database_ids.insert(value);
        }
    }
    for key in ["database_source_ids", "databaseSourceIds"] {
        for raw in selected_scope
            .get(key)
            .cloned()
            .map(external_string_ids_from_payload_value)
            .unwrap_or_default()
        {
            if let Some(value) = non_empty_trimmed_string(&raw) {
                selected_scope_database_ids.insert(value);
            }
        }
    }
    if !fixed_report_scope {
        if let Some(default_source_id) =
            external_channel_default_source_id_from_config(&connection.config_redacted)
        {
            ids.insert(default_source_id);
        }
        ids.extend(external_channel_allowed_database_source_ids(
            &connection.config_redacted,
        ));
    }
    ids.extend(selected_scope_database_ids);

    if !fixed_report_scope {
        for item in evidence_state
            .get("supplied_items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if matches!(
                item.get("type").and_then(Value::as_str),
                Some("database_schema_context" | "database_aggregate")
            ) {
                if let Some(value) = item
                    .get("source_id")
                    .and_then(Value::as_str)
                    .and_then(non_empty_trimmed_string)
                {
                    ids.insert(value);
                }
            }
        }
    }

    ids.into_iter().collect()
}

pub(crate) fn external_channel_static_page_dataset_scope(
    tenant_id: TenantId,
    connection: &ExternalChannelConnectionSummary,
    run: &AssistantRun,
) -> Value {
    json!({
        "tenant_id": tenant_id.to_string(),
        "dataset_ids": selected_dataset_ids_from_scope(&run.selected_scope)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "database_source_ids": collect_external_static_page_database_source_ids(
            connection,
            &run.selected_scope,
            &run.evidence_state,
        ),
        "selected_document_ids": selected_document_ids_from_scope(&run.selected_scope)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "available_document_source_id": run
            .selected_scope
            .get("available_document_source_id")
            .and_then(Value::as_str),
        "scope_source": "v3_external_channel_selected_scope",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::ExternalChannelPlatformView;
    use domain_model::{AssistantRunId, DatasetId, DocumentId};

    fn sample_connection() -> ExternalChannelConnectionSummary {
        ExternalChannelConnectionSummary {
            platform: ExternalChannelPlatformView::GenericChat,
            status: "enabled".to_string(),
            config_redacted: json!({
                "default_source_id": "default-doc-source",
                "allowed_database_source_ids": ["db-allowed", "db-shared"]
            }),
        }
    }

    fn external_run_for_scope(selected_scope: Value, evidence_state: Value) -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: None,
            user_prompt: "生成经营报表".to_string(),
            startup_briefing: json!({}),
            selected_scope,
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state,
            service_lane: "external_channel".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn external_database_source_ids_merge_connection_scope_and_evidence() {
        let connection = sample_connection();
        let selected_scope = json!({
            "database_source_id": "db-scope",
            "databaseSourceIds": ["db-array", "db-shared", " "]
        });
        let evidence_state = json!({
            "supplied_items": [
                {"type": "database_schema_context", "source_id": "db-schema"},
                {"type": "database_aggregate", "source_id": "db-aggregate"},
                {"type": "retrieval_chunk", "source_id": "db-ignored"}
            ]
        });

        assert_eq!(
            collect_external_static_page_database_source_ids(
                &connection,
                &selected_scope,
                &evidence_state
            ),
            vec![
                "db-aggregate".to_string(),
                "db-allowed".to_string(),
                "db-array".to_string(),
                "db-schema".to_string(),
                "db-scope".to_string(),
                "db-shared".to_string(),
                "default-doc-source".to_string(),
            ]
        );
    }

    #[test]
    fn fixed_report_scope_ignores_connection_defaults_and_evidence() {
        let connection = sample_connection();
        let selected_scope = json!({
            "database_report_scope_policy": "fixed_dataset_independent_of_chat_selection",
            "database_source_ids": ["db-report"]
        });
        let evidence_state = json!({
            "supplied_items": [
                {"type": "database_aggregate", "source_id": "db-evidence"}
            ]
        });

        assert_eq!(
            collect_external_static_page_database_source_ids(
                &connection,
                &selected_scope,
                &evidence_state
            ),
            vec!["db-report".to_string()]
        );
    }

    #[test]
    fn external_dataset_scope_preserves_external_channel_fields() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let run = external_run_for_scope(
            json!({
                "datasets": [{"type": "dataset", "id": dataset_id.to_string()}],
                "documents": [{"type": "document", "id": document_id.to_string()}],
                "available_document_source_id": "third-party-doc-source",
                "database_source_id": "db-scope"
            }),
            json!({}),
        );

        let scope =
            external_channel_static_page_dataset_scope(tenant_id, &sample_connection(), &run);

        assert_eq!(scope["tenant_id"], json!(tenant_id.to_string()));
        assert_eq!(scope["dataset_ids"], json!([dataset_id.to_string()]));
        assert_eq!(
            scope["selected_document_ids"],
            json!([document_id.to_string()])
        );
        assert_eq!(
            scope["available_document_source_id"],
            json!("third-party-doc-source")
        );
        assert_eq!(
            scope["scope_source"],
            json!("v3_external_channel_selected_scope")
        );
        assert_eq!(
            scope["database_source_ids"],
            json!(["db-allowed", "db-scope", "db-shared", "default-doc-source"])
        );
    }
}
