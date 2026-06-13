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
}
