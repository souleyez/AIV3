use domain_model::{AssistantRun, StaticPageDraft, TenantId};
#[cfg(test)]
use domain_model::{AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, UserId};
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::{selected_dataset_ids_from_scope, selected_document_ids_from_scope};

fn non_empty_trimmed_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn collect_static_page_string_values(value: Option<&Value>, output: &mut BTreeSet<String>) {
    match value {
        Some(Value::String(text)) => {
            for part in text.split(',') {
                if let Some(value) = non_empty_trimmed_string(part) {
                    output.insert(value);
                }
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(value) = item.as_str().and_then(non_empty_trimmed_string) {
                    output.insert(value);
                }
            }
        }
        _ => {}
    }
}

fn collect_static_page_database_source_ids_from_value(value: &Value, ids: &mut BTreeSet<String>) {
    for key in [
        "database_source_id",
        "databaseSourceId",
        "source_database_id",
        "sourceDatabaseId",
    ] {
        if let Some(value) = value
            .get(key)
            .and_then(Value::as_str)
            .and_then(non_empty_trimmed_string)
        {
            ids.insert(value);
        }
    }
    for key in [
        "database_source_ids",
        "databaseSourceIds",
        "source_database_ids",
        "sourceDatabaseIds",
        "allowed_database_source_ids",
        "allowedDatabaseSourceIds",
    ] {
        collect_static_page_string_values(value.get(key), ids);
    }
}

fn collect_static_page_database_source_ids_from_evidence(
    evidence_state: &Value,
    ids: &mut BTreeSet<String>,
) {
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

pub(crate) fn assistant_run_static_page_database_source_ids(
    run: &AssistantRun,
    draft: &StaticPageDraft,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    collect_static_page_database_source_ids_from_value(&run.selected_scope, &mut ids);
    collect_static_page_database_source_ids_from_value(&draft.selected_scope, &mut ids);
    collect_static_page_database_source_ids_from_value(&draft.source_refs, &mut ids);
    collect_static_page_database_source_ids_from_evidence(&run.evidence_state, &mut ids);
    ids.into_iter().collect()
}

pub(crate) fn assistant_run_static_page_dataset_scope(
    tenant_id: TenantId,
    run: &AssistantRun,
    draft: &StaticPageDraft,
) -> Value {
    json!({
        "tenant_id": tenant_id.to_string(),
        "dataset_ids": selected_dataset_ids_from_scope(&run.selected_scope)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "database_source_ids": assistant_run_static_page_database_source_ids(run, draft),
        "selected_document_ids": selected_document_ids_from_scope(&run.selected_scope)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "local_thread_id": run.local_thread_id,
        "scope_source": "v3_main_assistant_selected_scope",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, DocumentId};

    fn assistant_run_for_scope(selected_scope: Value, evidence_state: Value) -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: Some(UserId::new()),
            local_thread_id: Some("local-thread-static-page".to_string()),
            user_prompt: "生成经营报表".to_string(),
            startup_briefing: json!({}),
            selected_scope,
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state,
            service_lane: "ordinary_chat".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    fn static_page_draft_for_scope(selected_scope: Value, source_refs: Value) -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "经营报表".to_string(),
            status: StaticPageDraftStatus::Queued,
            selected_scope,
            visibility_snapshot: json!({}),
            source_refs,
            draft_payload: json!({"modules": []}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn database_source_ids_merge_scope_source_refs_and_evidence() {
        let run = assistant_run_for_scope(
            json!({
                "database_source_id": "db-run",
                "database_source_ids": "db-run-list, db-shared"
            }),
            json!({
                "supplied_items": [
                    {"type": "database_schema_context", "source_id": "db-schema"},
                    {"type": "database_aggregate", "source_id": "db-aggregate"},
                    {"type": "retrieval_chunk", "source_id": "db-ignored"}
                ]
            }),
        );
        let draft = static_page_draft_for_scope(
            json!({"databaseSourceIds": ["db-draft", "db-shared", " "]}),
            json!({"allowed_database_source_ids": ["db-allowed"]}),
        );

        assert_eq!(
            assistant_run_static_page_database_source_ids(&run, &draft),
            vec![
                "db-aggregate".to_string(),
                "db-allowed".to_string(),
                "db-draft".to_string(),
                "db-run".to_string(),
                "db-run-list".to_string(),
                "db-schema".to_string(),
                "db-shared".to_string(),
            ]
        );
    }

    #[test]
    fn dataset_scope_preserves_main_assistant_scope_fields() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let run = assistant_run_for_scope(
            json!({
                "datasets": [{"type": "dataset", "id": dataset_id.to_string()}],
                "documents": [{"type": "document", "id": document_id.to_string()}],
                "database_source_id": "db-main"
            }),
            json!({}),
        );
        let draft = static_page_draft_for_scope(json!({}), json!({}));

        let scope = assistant_run_static_page_dataset_scope(tenant_id, &run, &draft);

        assert_eq!(scope["tenant_id"], json!(tenant_id.to_string()));
        assert_eq!(scope["dataset_ids"], json!([dataset_id.to_string()]));
        assert_eq!(
            scope["selected_document_ids"],
            json!([document_id.to_string()])
        );
        assert_eq!(scope["database_source_ids"], json!(["db-main"]));
        assert_eq!(scope["local_thread_id"], json!("local-thread-static-page"));
        assert_eq!(
            scope["scope_source"],
            json!("v3_main_assistant_selected_scope")
        );
    }
}
