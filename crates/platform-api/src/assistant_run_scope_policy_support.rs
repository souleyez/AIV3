use serde_json::{json, Value};

pub(crate) fn assistant_run_scope_intent(scope: &Value) -> &str {
    scope
        .get("intent")
        .or_else(|| scope.get("assistant_intent"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("ordinary_chat")
}

pub(crate) fn assistant_run_scope_supply_policy(scope: &Value) -> Value {
    scope
        .get("supply_policy")
        .or_else(|| scope.get("supplyPolicy"))
        .cloned()
        .unwrap_or_else(|| json!({}))
}

pub(crate) fn assistant_run_scope_suppresses_default_database_supply(scope: &Value) -> bool {
    let policy = assistant_run_scope_supply_policy(scope);
    policy
        .get("suppressDefaultDatabaseSupply")
        .or_else(|| policy.get("suppress_default_database_supply"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn assistant_run_scope_policy_string(
    scope: &Value,
    field_names: &[&str],
    default_value: &str,
) -> String {
    let policy = assistant_run_scope_supply_policy(scope);
    field_names
        .iter()
        .find_map(|field_name| policy.get(*field_name).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_value)
        .to_string()
}

pub(crate) fn assistant_run_scope_prefers_detail(scope: &Value) -> bool {
    let policy = assistant_run_scope_supply_policy(scope);
    policy
        .get("preferDetail")
        .or_else(|| policy.get("prefer_detail"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || policy
            .get("retrievalPolicy")
            .or_else(|| policy.get("retrieval_policy"))
            .and_then(Value::as_str)
            .is_some_and(|value| value == "detail_first")
}

pub(crate) fn assistant_run_scope_is_external_channel(scope: Option<&Value>) -> bool {
    let Some(scope) = scope else {
        return false;
    };
    scope
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "external_channel")
        || scope
            .get("mode")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "external_channel")
}

pub(crate) fn assistant_run_preferred_dataset_id_strings_from_scope(scope: &Value) -> Vec<String> {
    scope
        .get("preferred_dataset_ids")
        .or_else(|| scope.get("preferredDatasetIds"))
        .and_then(Value::as_array)
        .map(|items| {
            let mut ids = Vec::new();
            for item in items {
                let Some(raw) = item.as_str().map(str::trim) else {
                    continue;
                };
                if raw.is_empty() || ids.iter().any(|id| id == raw) {
                    continue;
                }
                ids.push(raw.to_string());
            }
            ids
        })
        .unwrap_or_default()
}

pub(crate) fn assistant_run_requested_dataset_supply_policy(intent: &str) -> Value {
    json!({
        "intent": if intent.trim().is_empty() { "data_question" } else { intent.trim() },
        "retrievalPolicy": "standard",
        "preferDetail": false,
        "noFakeData": true,
        "answerPolicy": "model_authored_host_supplied",
        "candidatePolicy": "selected_or_inferred_visible_datasets_only",
        "recommendedActions": ["retrieval.search"],
    })
}

pub(crate) fn assistant_run_active_static_page_supply_policy(
    has_dataset: bool,
    has_memory: bool,
) -> Value {
    let mut recommended_actions = Vec::new();
    if has_dataset {
        recommended_actions.push("retrieval.search");
        recommended_actions.push("retrieval.read_detail");
    }
    recommended_actions.push("static_page.update_draft");

    json!({
        "intent": "static_page",
        "answerPolicy": "model_authored_host_supplied",
        "currentArtifactPolicy": "active_static_page_draft",
        "actionPolicy": "model_may_request_controlled_actions_host_validates",
        "contextBudgetPolicy": if has_dataset || has_memory {
            "quality_first_token_tolerant"
        } else {
            "compact_until_retrieval_needed"
        },
        "candidatePolicy": if has_dataset {
            "selected_or_inferred_visible_datasets_only"
        } else {
            "ordinary_chat_without_forced_dataset"
        },
        "historyPolicy": if has_memory {
            "intent_gated_selected"
        } else {
            "intent_gated"
        },
        "retrievalPolicy": if has_dataset {
            "detail_first"
        } else {
            "not_requested"
        },
        "preferDetail": has_dataset,
        "recommendedActions": recommended_actions,
        "noFakeData": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scope_intent_prefers_primary_and_falls_back_to_assistant_alias() {
        assert_eq!(
            assistant_run_scope_intent(&json!({
                "intent": " static_page ",
                "assistant_intent": "report"
            })),
            "static_page"
        );
        assert_eq!(
            assistant_run_scope_intent(&json!({"assistant_intent": " report "})),
            "report"
        );
        assert_eq!(
            assistant_run_scope_intent(&json!({"intent": ""})),
            "ordinary_chat"
        );
    }

    #[test]
    fn scope_supply_policy_accepts_snake_and_camel_aliases() {
        assert_eq!(
            assistant_run_scope_supply_policy(&json!({
                "supply_policy": {"preferDetail": true}
            }))["preferDetail"],
            json!(true)
        );
        assert_eq!(
            assistant_run_scope_supply_policy(&json!({
                "supplyPolicy": {"prefer_detail": true}
            }))["prefer_detail"],
            json!(true)
        );
        assert_eq!(assistant_run_scope_supply_policy(&json!({})), json!({}));
    }

    #[test]
    fn scope_suppress_default_database_supply_reads_both_aliases() {
        assert!(assistant_run_scope_suppresses_default_database_supply(
            &json!({"supply_policy": {"suppressDefaultDatabaseSupply": true}})
        ));
        assert!(assistant_run_scope_suppresses_default_database_supply(
            &json!({"supply_policy": {"suppress_default_database_supply": true}})
        ));
        assert!(!assistant_run_scope_suppresses_default_database_supply(
            &json!({"supply_policy": {"suppressDefaultDatabaseSupply": false}})
        ));
    }

    #[test]
    fn scope_policy_string_trims_and_uses_default_for_empty_values() {
        assert_eq!(
            assistant_run_scope_policy_string(
                &json!({"supply_policy": {"actionPolicy": " controlled "}}),
                &["actionPolicy", "action_policy"],
                "default"
            ),
            "controlled"
        );
        assert_eq!(
            assistant_run_scope_policy_string(
                &json!({"supply_policy": {"actionPolicy": ""}}),
                &["actionPolicy", "action_policy"],
                "default"
            ),
            "default"
        );
    }

    #[test]
    fn scope_prefers_detail_reads_flag_and_retrieval_policy_aliases() {
        assert!(assistant_run_scope_prefers_detail(
            &json!({"supply_policy": {"preferDetail": true}})
        ));
        assert!(assistant_run_scope_prefers_detail(
            &json!({"supply_policy": {"prefer_detail": true}})
        ));
        assert!(assistant_run_scope_prefers_detail(
            &json!({"supply_policy": {"retrievalPolicy": "detail_first"}})
        ));
        assert!(assistant_run_scope_prefers_detail(
            &json!({"supply_policy": {"retrieval_policy": "detail_first"}})
        ));
        assert!(!assistant_run_scope_prefers_detail(
            &json!({"supply_policy": {"retrievalPolicy": "search"}})
        ));
    }

    #[test]
    fn scope_is_external_channel_reads_type_or_mode() {
        assert!(assistant_run_scope_is_external_channel(Some(
            &json!({"type": "external_channel"})
        )));
        assert!(assistant_run_scope_is_external_channel(Some(
            &json!({"mode": "external_channel"})
        )));
        assert!(!assistant_run_scope_is_external_channel(Some(
            &json!({"type": "ordinary_chat", "mode": "normal"})
        )));
        assert!(!assistant_run_scope_is_external_channel(None));
    }

    #[test]
    fn preferred_dataset_ids_trim_dedupe_and_read_aliases() {
        assert_eq!(
            assistant_run_preferred_dataset_id_strings_from_scope(&json!({
                "preferred_dataset_ids": [" dataset-a ", "", "dataset-a", 42, "dataset-b"]
            })),
            vec!["dataset-a".to_string(), "dataset-b".to_string()]
        );
        assert_eq!(
            assistant_run_preferred_dataset_id_strings_from_scope(&json!({
                "preferredDatasetIds": ["dataset-c"]
            })),
            vec!["dataset-c".to_string()]
        );
        assert!(assistant_run_preferred_dataset_id_strings_from_scope(&json!({})).is_empty());
    }

    #[test]
    fn requested_dataset_supply_policy_uses_selected_dataset_contract() {
        let static_policy = assistant_run_requested_dataset_supply_policy(" static_page ");
        assert_eq!(static_policy["intent"], json!("static_page"));
        assert_eq!(static_policy["retrievalPolicy"], json!("standard"));
        assert_eq!(
            static_policy["candidatePolicy"],
            json!("selected_or_inferred_visible_datasets_only")
        );
        assert_eq!(
            static_policy["answerPolicy"],
            json!("model_authored_host_supplied")
        );
        assert_eq!(static_policy["preferDetail"], json!(false));

        let default_policy = assistant_run_requested_dataset_supply_policy(" ");
        assert_eq!(default_policy["intent"], json!("data_question"));
    }

    #[test]
    fn active_static_page_supply_policy_tracks_dataset_and_memory_context() {
        let artifact_only = assistant_run_active_static_page_supply_policy(false, false);
        assert_eq!(artifact_only["retrievalPolicy"], json!("not_requested"));
        assert_eq!(artifact_only["historyPolicy"], json!("intent_gated"));
        assert_eq!(
            artifact_only["contextBudgetPolicy"],
            json!("compact_until_retrieval_needed")
        );
        assert_eq!(
            artifact_only["currentArtifactPolicy"],
            json!("active_static_page_draft")
        );
        assert_eq!(
            artifact_only["candidatePolicy"],
            json!("ordinary_chat_without_forced_dataset")
        );

        let with_dataset_and_memory = assistant_run_active_static_page_supply_policy(true, true);
        assert_eq!(
            with_dataset_and_memory["retrievalPolicy"],
            json!("detail_first")
        );
        assert_eq!(
            with_dataset_and_memory["historyPolicy"],
            json!("intent_gated_selected")
        );
        assert_eq!(with_dataset_and_memory["preferDetail"], json!(true));
        assert_eq!(
            with_dataset_and_memory["recommendedActions"],
            json!([
                "retrieval.search",
                "retrieval.read_detail",
                "static_page.update_draft"
            ])
        );
    }
}
