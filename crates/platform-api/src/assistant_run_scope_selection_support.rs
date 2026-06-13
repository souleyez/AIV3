use domain_model::{DatasetId, DocumentId};
use serde_json::Value;
use uuid::Uuid;

use crate::assistant_run_conversation_memory_support::selected_scope_requests_conversation_memory;
use crate::assistant_run_scope_policy_support::{
    assistant_run_scope_intent, assistant_run_scope_policy_string,
    assistant_run_scope_prefers_detail, assistant_run_scope_supply_policy,
};

pub(crate) fn selected_dataset_id_from_scope(scope: &Value) -> Option<DatasetId> {
    selected_dataset_ids_from_scope(scope).into_iter().next()
}

pub(crate) fn selected_dataset_ids_from_scope(scope: &Value) -> Vec<DatasetId> {
    let Some(object) = scope.as_object() else {
        return Vec::new();
    };

    let mut dataset_ids = Vec::new();
    for key in ["datasets", "selected"] {
        let Some(items) = object.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(dataset_id) = dataset_id_from_scope_item(item) else {
                continue;
            };
            if !dataset_ids.contains(&dataset_id) {
                dataset_ids.push(dataset_id);
            }
        }
    }
    dataset_ids
}

pub(crate) fn selected_document_ids_from_scope(scope: &Value) -> Vec<DocumentId> {
    let Some(object) = scope.as_object() else {
        return Vec::new();
    };

    let mut document_ids = Vec::new();
    for key in ["documents", "selected"] {
        let Some(items) = object.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(document_id) = document_id_from_scope_item(item) else {
                continue;
            };
            if !document_ids.contains(&document_id) {
                document_ids.push(document_id);
            }
        }
    }
    document_ids
}

pub(crate) fn selected_scope_document_template_document_ids(scope: &Value) -> Vec<DocumentId> {
    scope
        .get("document_template_skills")
        .and_then(Value::as_array)
        .map(|items| {
            let mut document_ids = Vec::new();
            for item in items {
                let Some(document_id) = item
                    .get("template_document")
                    .and_then(|document| document.get("document_id"))
                    .and_then(Value::as_str)
                    .and_then(|raw| Uuid::parse_str(raw.trim()).ok())
                    .map(DocumentId)
                else {
                    continue;
                };
                if !document_ids.contains(&document_id) {
                    document_ids.push(document_id);
                }
            }
            document_ids
        })
        .unwrap_or_default()
}

pub(crate) fn selected_document_ids_for_evidence_from_scope(scope: &Value) -> Vec<DocumentId> {
    let template_document_ids = selected_scope_document_template_document_ids(scope);
    let mut document_ids = selected_document_ids_from_scope(scope);
    document_ids.retain(|document_id| !template_document_ids.contains(document_id));
    document_ids
}

pub(crate) fn selected_scope_temporary_dataset_id(scope: &Value) -> Option<DatasetId> {
    scope
        .get("temporary_dataset")
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .and_then(|raw| Uuid::parse_str(raw.trim()).ok())
        .map(DatasetId)
}

pub(crate) fn selected_scope_attachment_title_document_ids(scope: &Value) -> Vec<DocumentId> {
    let policy = assistant_run_scope_supply_policy(scope);
    let mut ids = Vec::new();
    for value in [
        policy.get("attachmentTitleDocumentIds"),
        policy.get("attachment_title_document_ids"),
        scope.pointer("/attachment_title_resolution/matched_documents"),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(items) = value.as_array() {
            for item in items {
                let raw = item
                    .as_str()
                    .or_else(|| item.get("id").and_then(Value::as_str))
                    .or_else(|| item.get("document_id").and_then(Value::as_str))
                    .or_else(|| item.get("documentId").and_then(Value::as_str));
                if let Some(document_id) = raw
                    .and_then(|raw| Uuid::parse_str(raw.trim()).ok())
                    .map(DocumentId)
                {
                    if !ids.contains(&document_id) {
                        ids.push(document_id);
                    }
                }
            }
        }
    }
    ids
}

pub(crate) fn assistant_run_scope_action_policy(scope: &Value) -> String {
    assistant_run_scope_policy_string(
        scope,
        &["actionPolicy", "action_policy"],
        "model_may_request_controlled_actions_host_validates",
    )
}

pub(crate) fn assistant_run_scope_context_budget_policy(scope: &Value) -> String {
    let default_value = if assistant_run_scope_prefers_detail(scope)
        || selected_scope_requests_conversation_memory(scope)
    {
        "quality_first_token_tolerant"
    } else {
        "compact_until_retrieval_needed"
    };
    assistant_run_scope_policy_string(
        scope,
        &["contextBudgetPolicy", "context_budget_policy"],
        default_value,
    )
}

pub(crate) fn assistant_run_scope_candidate_policy(scope: &Value) -> String {
    let default_value = if selected_dataset_ids_from_scope(scope).is_empty() {
        "ordinary_chat_without_forced_dataset"
    } else {
        "selected_or_inferred_visible_datasets_only"
    };
    assistant_run_scope_policy_string(
        scope,
        &["candidatePolicy", "candidate_policy"],
        default_value,
    )
}

pub(crate) fn assistant_run_scope_recommended_tool_actions(scope: &Value) -> Vec<String> {
    let policy = assistant_run_scope_supply_policy(scope);
    for field_name in ["recommendedActions", "recommended_actions"] {
        if let Some(actions) = policy.get(field_name).and_then(Value::as_array) {
            let values = actions
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .take(5)
                .collect::<Vec<_>>();
            if !values.is_empty() {
                return values;
            }
        }
    }

    let mut actions = Vec::new();
    let has_dataset = !selected_dataset_ids_from_scope(scope).is_empty();
    if has_dataset {
        actions.push("retrieval.search".to_string());
    }
    if assistant_run_scope_prefers_detail(scope) {
        actions.push("retrieval.read_detail".to_string());
    }
    if assistant_run_scope_requests_dataset_entity_scan(scope) {
        actions.push("retrieval.scan_documents".to_string());
    }
    match assistant_run_scope_intent(scope) {
        "static_page" => actions.push("static_page.plan".to_string()),
        "report" => actions.push("report.plan".to_string()),
        _ => {}
    }
    if actions.is_empty() {
        actions.push("ordinary_chat.answer".to_string());
    }
    actions.truncate(5);
    actions
}

pub(crate) fn assistant_run_scope_requests_dataset_entity_scan(scope: &Value) -> bool {
    let policy = assistant_run_scope_supply_policy(scope);
    policy
        .get("coveragePolicy")
        .or_else(|| policy.get("coverage_policy"))
        .and_then(Value::as_str)
        .is_some_and(|value| value == "document_entity_scan")
        || policy
            .get("recommendedActions")
            .or_else(|| policy.get("recommended_actions"))
            .and_then(Value::as_array)
            .map(|actions| {
                actions.iter().any(|action| {
                    action
                        .as_str()
                        .map(str::trim)
                        .is_some_and(|value| value == "retrieval.scan_documents")
                })
            })
            .unwrap_or(false)
}

pub(crate) fn dataset_id_from_scope_item(item: &Value) -> Option<DatasetId> {
    if item
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|item_type| item_type != "dataset")
    {
        return None;
    }
    let raw = item.as_str().or_else(|| {
        item.as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
    })?;
    Uuid::parse_str(raw).ok().map(DatasetId)
}

pub(crate) fn document_id_from_scope_item(item: &Value) -> Option<DocumentId> {
    if item
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|item_type| item_type != "document")
    {
        return None;
    }
    let raw = item.as_str().or_else(|| {
        item.as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
    })?;
    Uuid::parse_str(raw).ok().map(DocumentId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selected_dataset_and_document_ids_keep_order_and_dedupe() {
        let first_dataset = DatasetId::new();
        let second_dataset = DatasetId::new();
        let first_document = DocumentId::new();
        let second_document = DocumentId::new();
        let scope = json!({
            "datasets": [
                first_dataset.to_string(),
                {"type": "dataset", "id": second_dataset.to_string()},
                {"type": "document", "id": DocumentId::new().to_string()},
                first_dataset.to_string()
            ],
            "documents": [
                first_document.to_string(),
                {"type": "document", "id": second_document.to_string()},
                {"type": "dataset", "id": DatasetId::new().to_string()},
                first_document.to_string()
            ],
            "selected": [
                {"type": "dataset", "id": second_dataset.to_string()},
                {"type": "document", "id": second_document.to_string()}
            ]
        });

        assert_eq!(
            selected_dataset_ids_from_scope(&scope),
            vec![first_dataset, second_dataset]
        );
        assert_eq!(
            selected_document_ids_from_scope(&scope),
            vec![first_document, second_document]
        );
        assert_eq!(selected_dataset_id_from_scope(&scope), Some(first_dataset));
    }

    #[test]
    fn template_documents_are_excluded_from_evidence_ids() {
        let business_document = DocumentId::new();
        let template_document = DocumentId::new();
        let scope = json!({
            "documents": [
                {"type": "document", "id": business_document.to_string()},
                {"type": "document", "id": template_document.to_string()}
            ],
            "document_template_skills": [{
                "template_document": {"document_id": template_document.to_string()}
            }]
        });

        assert_eq!(
            selected_scope_document_template_document_ids(&scope),
            vec![template_document]
        );
        assert_eq!(
            selected_document_ids_for_evidence_from_scope(&scope),
            vec![business_document]
        );
    }

    #[test]
    fn temporary_dataset_and_attachment_title_ids_read_aliases() {
        let dataset_id = DatasetId::new();
        let first_document = DocumentId::new();
        let second_document = DocumentId::new();
        let third_document = DocumentId::new();
        let scope = json!({
            "temporary_dataset": {"id": dataset_id.to_string()},
            "supply_policy": {
                "attachmentTitleDocumentIds": [
                    first_document.to_string(),
                    {"documentId": second_document.to_string()},
                    first_document.to_string()
                ]
            },
            "attachment_title_resolution": {
                "matched_documents": [{"document_id": third_document.to_string()}]
            }
        });

        assert_eq!(
            selected_scope_temporary_dataset_id(&scope),
            Some(dataset_id)
        );
        assert_eq!(
            selected_scope_attachment_title_document_ids(&scope),
            vec![first_document, second_document, third_document]
        );
    }

    #[test]
    fn scope_policies_preserve_defaults_and_overrides() {
        let dataset_id = DatasetId::new();
        let plain_scope = json!({});
        assert_eq!(
            assistant_run_scope_action_policy(&plain_scope),
            "model_may_request_controlled_actions_host_validates"
        );
        assert_eq!(
            assistant_run_scope_context_budget_policy(&plain_scope),
            "compact_until_retrieval_needed"
        );
        assert_eq!(
            assistant_run_scope_candidate_policy(&plain_scope),
            "ordinary_chat_without_forced_dataset"
        );

        let selected_scope = json!({
            "datasets": [dataset_id.to_string()],
            "supply_policy": {
                "actionPolicy": " controlled ",
                "contextBudgetPolicy": " verbose ",
                "candidatePolicy": " exact "
            }
        });
        assert_eq!(
            assistant_run_scope_action_policy(&selected_scope),
            "controlled"
        );
        assert_eq!(
            assistant_run_scope_context_budget_policy(&selected_scope),
            "verbose"
        );
        assert_eq!(
            assistant_run_scope_candidate_policy(&selected_scope),
            "exact"
        );
    }

    #[test]
    fn recommended_actions_use_explicit_policy_or_scope_defaults() {
        let dataset_id = DatasetId::new();
        assert_eq!(
            assistant_run_scope_recommended_tool_actions(&json!({
                "datasets": [dataset_id.to_string()],
                "intent": "static_page",
                "supply_policy": {"preferDetail": true}
            })),
            vec![
                "retrieval.search",
                "retrieval.read_detail",
                "static_page.plan"
            ]
        );
        assert_eq!(
            assistant_run_scope_recommended_tool_actions(&json!({
                "supply_policy": {"recommendedActions": [
                    " retrieval.search ",
                    "",
                    "retrieval.read_detail",
                    "retrieval.scan_documents",
                    "report.plan",
                    "ignored.extra"
                ]}
            })),
            vec![
                "retrieval.search",
                "retrieval.read_detail",
                "retrieval.scan_documents",
                "report.plan",
                "ignored.extra"
            ]
        );
        assert_eq!(
            assistant_run_scope_recommended_tool_actions(&json!({})),
            vec!["ordinary_chat.answer"]
        );
    }

    #[test]
    fn dataset_entity_scan_request_reads_coverage_and_action_aliases() {
        assert!(assistant_run_scope_requests_dataset_entity_scan(&json!({
            "supply_policy": {"coveragePolicy": "document_entity_scan"}
        })));
        assert!(assistant_run_scope_requests_dataset_entity_scan(&json!({
            "supply_policy": {"recommended_actions": [" retrieval.scan_documents "]}
        })));
        assert!(!assistant_run_scope_requests_dataset_entity_scan(&json!({
            "supply_policy": {"coveragePolicy": "search"}
        })));
    }
}
