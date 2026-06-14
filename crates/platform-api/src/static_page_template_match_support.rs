use domain_model::StaticPageDraft;
use serde_json::Value;
use std::collections::BTreeSet;

use crate::external_bot_message_payload_support::external_string_ids_from_payload_value;
use crate::{dataset_id_from_scope_item, selected_dataset_ids_from_scope};

pub(crate) fn static_page_stable_key_token(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>();
    if normalized.is_empty() {
        return None;
    }
    Some(
        normalized
            .replace('\\', "\\\\")
            .replace('|', "\\|")
            .replace(':', "\\:"),
    )
}

pub(crate) fn static_page_stable_key_insert_value(
    output: &mut BTreeSet<String>,
    prefix: &str,
    value: Option<&Value>,
) {
    let Some(value) = value else {
        return;
    };
    for raw in external_string_ids_from_payload_value(value.clone()) {
        if let Some(token) = static_page_stable_key_token(&raw) {
            output.insert(format!("{prefix}:{token}"));
        }
    }
}

pub(crate) fn static_page_stable_key_insert_scope_item_id(
    output: &mut BTreeSet<String>,
    prefix: &str,
    item: &Value,
) {
    if let Some(object) = item.as_object() {
        for key in ["id", "dataset_id", "datasetId", "document_id", "documentId"] {
            if let Some(value) = object
                .get(key)
                .and_then(Value::as_str)
                .and_then(static_page_stable_key_token)
            {
                output.insert(format!("{prefix}:{value}"));
                return;
            }
        }
    }
}

pub(crate) fn static_page_stable_key_insert_scope_item_external_id(
    output: &mut BTreeSet<String>,
    prefix: &str,
    item: &Value,
) {
    if let Some(object) = item.as_object() {
        for key in [
            "document_external_id",
            "documentExternalId",
            "external_document_id",
            "externalDocumentId",
        ] {
            if let Some(value) = object
                .get(key)
                .and_then(Value::as_str)
                .and_then(static_page_stable_key_token)
            {
                output.insert(format!("{prefix}:{value}"));
                return;
            }
        }
    }
}

fn static_page_template_match_insert_external_tokens(
    output: &mut BTreeSet<String>,
    value: Option<&Value>,
) {
    for raw in value
        .cloned()
        .map(external_string_ids_from_payload_value)
        .unwrap_or_default()
    {
        if let Some(token) = static_page_stable_key_token(&raw) {
            output.insert(format!("dataset_external:{token}"));
        }
    }
}

pub(crate) fn static_page_template_match_tokens(
    selected_scope: &Value,
    source_refs: &Value,
) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    for key in [
        "dataset_external_id",
        "datasetExternalId",
        "dataset_external_ids",
        "datasetExternalIds",
        "requested_dataset_external_id",
        "requestedDatasetExternalId",
        "requested_dataset_external_ids",
        "requestedDatasetExternalIds",
        "available_dataset_external_id",
        "availableDatasetExternalId",
        "available_dataset_external_ids",
        "availableDatasetExternalIds",
    ] {
        static_page_template_match_insert_external_tokens(&mut tokens, selected_scope.get(key));
        static_page_template_match_insert_external_tokens(&mut tokens, source_refs.get(key));
    }
    if let Some(dataset_scope) = selected_scope.get("dataset_document_scope") {
        for key in [
            "dataset_external_id",
            "datasetExternalId",
            "dataset_external_ids",
            "datasetExternalIds",
            "requested_dataset_external_id",
            "requestedDatasetExternalId",
            "requested_dataset_external_ids",
            "requestedDatasetExternalIds",
        ] {
            static_page_template_match_insert_external_tokens(&mut tokens, dataset_scope.get(key));
        }
    }
    for item in selected_scope
        .get("canonical_datasets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(dataset_id) = dataset_id_from_scope_item(item) {
            tokens.insert(format!("dataset_canonical:{dataset_id}"));
        }
    }
    for dataset_id in selected_dataset_ids_from_scope(selected_scope) {
        tokens.insert(format!("dataset_id:{dataset_id}"));
    }
    for key in [
        "database_source_id",
        "databaseSourceId",
        "database_source_ids",
        "databaseSourceIds",
    ] {
        for value in selected_scope
            .get(key)
            .into_iter()
            .chain(source_refs.get(key))
        {
            for raw in external_string_ids_from_payload_value(value.clone()) {
                if let Some(token) = static_page_stable_key_token(&raw) {
                    tokens.insert(format!("database_source:{token}"));
                }
            }
        }
    }
    tokens
}

pub(crate) fn static_page_template_tokens_intersect(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let left_has_material_scope = static_page_template_tokens_have_material_scope(left);
    let right_has_material_scope = static_page_template_tokens_have_material_scope(right);
    if left_has_material_scope || right_has_material_scope {
        return left
            .iter()
            .filter(|token| static_page_template_token_is_material_scope(token))
            .any(|token| right.contains(token));
    }
    left.iter().any(|token| right.contains(token))
}

fn static_page_template_tokens_have_material_scope(tokens: &BTreeSet<String>) -> bool {
    tokens
        .iter()
        .any(|token| static_page_template_token_is_material_scope(token))
}

pub(crate) fn static_page_template_token_is_material_scope(token: &str) -> bool {
    token.starts_with("dataset_external:")
        || token.starts_with("dataset_canonical:")
        || token.starts_with("dataset_id:")
}

fn static_page_default_prompt_from_answer_policy(answer_policy: &Value) -> Option<&str> {
    answer_policy
        .get("default_prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn static_page_default_prompt_from_scope_or_refs<'a>(
    selected_scope: &'a Value,
    source_refs: &'a Value,
) -> Option<&'a str> {
    source_refs
        .get("answer_policy")
        .and_then(static_page_default_prompt_from_answer_policy)
        .or_else(|| {
            selected_scope
                .get("answer_policy")
                .and_then(static_page_default_prompt_from_answer_policy)
        })
}

fn static_page_default_prompt_reuse_class(prompt: &str) -> Option<&'static str> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return None;
    }
    if prompt.contains("简历")
        || prompt.contains("招聘")
        || prompt.to_ascii_lowercase().contains("resume")
    {
        return Some("resume");
    }
    if prompt.contains("经营")
        || prompt.contains("报表")
        || prompt.contains("业务")
        || prompt.contains("数据")
        || prompt.contains("数据库")
        || prompt.contains("数据源")
        || prompt.contains("文档")
        || prompt.to_ascii_lowercase().contains("report")
        || prompt.to_ascii_lowercase().contains("dashboard")
    {
        return Some("business_report");
    }
    None
}

pub(crate) fn static_page_template_intent_reuse_class(text: &str) -> Option<&'static str> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    if text.contains("简历") || text.contains("招聘") || lower.contains("resume") {
        return Some("resume");
    }
    if text.contains("经营")
        || text.contains("报表")
        || text.contains("业务")
        || text.contains("数据")
        || text.contains("数据库")
        || text.contains("数据源")
        || lower.contains("report")
        || lower.contains("dashboard")
    {
        return Some("business_report");
    }
    None
}

fn static_page_template_draft_intent_reuse_class(draft: &StaticPageDraft) -> Option<&'static str> {
    [
        draft.draft_payload.get("prompt").and_then(Value::as_str),
        draft.draft_payload.get("title").and_then(Value::as_str),
        Some(draft.title.as_str()),
    ]
    .into_iter()
    .flatten()
    .find_map(static_page_template_intent_reuse_class)
}

pub(crate) fn static_page_template_intent_compatible(
    current_prompt: Option<&str>,
    draft: &StaticPageDraft,
) -> bool {
    let Some(current_class) = current_prompt.and_then(static_page_template_intent_reuse_class)
    else {
        return true;
    };
    static_page_template_draft_intent_reuse_class(draft)
        .is_none_or(|draft_class| draft_class == current_class)
}

fn static_page_default_prompt_tokens_compatible(
    left_prompt: Option<&str>,
    right_prompt: Option<&str>,
) -> bool {
    let left_token = left_prompt.and_then(static_page_stable_key_token);
    let right_token = right_prompt.and_then(static_page_stable_key_token);
    if left_token == right_token {
        return true;
    }
    let Some(left_prompt) = left_prompt else {
        return false;
    };
    let Some(right_prompt) = right_prompt else {
        return false;
    };
    let Some(left_class) = static_page_default_prompt_reuse_class(left_prompt) else {
        return false;
    };
    static_page_default_prompt_reuse_class(right_prompt) == Some(left_class)
}

pub(crate) fn static_page_default_prompt_template_token(
    selected_scope: &Value,
    source_refs: &Value,
) -> String {
    if let Some(prompt) = static_page_default_prompt_from_scope_or_refs(selected_scope, source_refs)
    {
        if let Some(class) = static_page_default_prompt_reuse_class(prompt) {
            return format!("class:{class}");
        }
        if let Some(token) = static_page_stable_key_token(prompt) {
            return token;
        }
    }
    "none".to_string()
}

pub(crate) fn static_page_template_stability_key_with_default_prompt(
    template_stability_key: impl Into<String>,
    default_prompt: Option<&str>,
) -> String {
    let mut key = template_stability_key.into();
    if let Some(token) = default_prompt.and_then(static_page_stable_key_token) {
        key.push_str("|default-prompt:");
        key.push_str(&token);
    }
    key
}

pub(crate) fn static_page_default_prompt_tokens_match(
    selected_scope: &Value,
    source_refs: &Value,
    baseline_selected_scope: &Value,
    baseline_source_refs: &Value,
) -> bool {
    static_page_default_prompt_tokens_compatible(
        static_page_default_prompt_from_scope_or_refs(selected_scope, source_refs),
        static_page_default_prompt_from_scope_or_refs(
            baseline_selected_scope,
            baseline_source_refs,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, DatasetId, StaticPageDraftId, StaticPageDraftStatus, TenantId, UserId,
    };
    use serde_json::json;

    fn rendered_draft_with_title(title: &str) -> StaticPageDraft {
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: Some(UserId::new()),
            title: title.to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope: json!({}),
            visibility_snapshot: json!({}),
            source_refs: json!({}),
            draft_payload: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn stable_key_token_trims_and_escapes_reserved_separators() {
        assert_eq!(
            static_page_stable_key_token("  dataset\\a|b:c\u{0000}  ").as_deref(),
            Some("dataset\\\\a\\|b\\:c")
        );
        assert_eq!(static_page_stable_key_token(" \n\t ").as_deref(), None);
    }

    #[test]
    fn template_match_tokens_collect_aliases_and_material_scope() {
        let dataset_id = DatasetId::new();
        let selected_scope = json!({
            "requestedDatasetExternalIds": ["dataset-a", "dataset-b"],
            "canonical_datasets": [{"id": dataset_id.to_string()}],
            "databaseSourceIds": ["db-main"]
        });
        let source_refs = json!({
            "dataset_external_id": "dataset-c",
            "database_source_id": "db-source"
        });

        let tokens = static_page_template_match_tokens(&selected_scope, &source_refs);

        assert!(tokens.contains("dataset_external:dataset-a"));
        assert!(tokens.contains("dataset_external:dataset-b"));
        assert!(tokens.contains("dataset_external:dataset-c"));
        assert!(tokens.contains(&format!("dataset_canonical:{dataset_id}")));
        assert!(tokens.contains("database_source:db-main"));
        assert!(tokens.contains("database_source:db-source"));
        assert!(static_page_template_tokens_intersect(
            &tokens,
            &BTreeSet::from(["dataset_external:dataset-b".to_string()])
        ));
        assert!(!static_page_template_tokens_intersect(
            &tokens,
            &BTreeSet::from(["database_source:db-main".to_string()])
        ));
    }

    #[test]
    fn default_prompt_match_accepts_same_or_business_class_and_blocks_resume() {
        let selected_scope = json!({});
        let business_refs = json!({
            "answer_policy": {
                "default_prompt": "请面向业务用户，按经营月报口径输出。"
            }
        });
        let compatible_business_refs = json!({
            "answer_policy": {
                "default_prompt": "请面向业务用户，基于数据库生成 dashboard。"
            }
        });
        let resume_refs = json!({
            "answer_policy": {
                "default_prompt": "请按招聘简历项目经历口径输出。"
            }
        });

        assert!(static_page_default_prompt_tokens_match(
            &selected_scope,
            &business_refs,
            &selected_scope,
            &compatible_business_refs
        ));
        assert!(!static_page_default_prompt_tokens_match(
            &selected_scope,
            &business_refs,
            &selected_scope,
            &resume_refs
        ));
        assert_eq!(
            static_page_default_prompt_template_token(&selected_scope, &business_refs),
            "class:business_report"
        );
    }

    #[test]
    fn template_intent_compatibility_blocks_cross_domain_template_reuse() {
        let resume_draft = rendered_draft_with_title("静态页：请生成一页简历库统计静态页报表");

        assert_eq!(
            static_page_template_intent_reuse_class("经营分析报表"),
            Some("business_report")
        );
        assert_eq!(
            static_page_template_intent_reuse_class("resume dashboard"),
            Some("resume")
        );
        assert!(!static_page_template_intent_compatible(
            Some("经营分析报表"),
            &resume_draft
        ));
        assert!(static_page_template_intent_compatible(
            Some("简历项目经历统计报表"),
            &resume_draft
        ));
        assert!(static_page_template_intent_compatible(None, &resume_draft));
    }
}
