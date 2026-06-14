use contracts::ExternalIntegrationAuditItemView;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ApiError;

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct ExternalIntegrationAuditQuery {
    pub(crate) item_type: Option<String>,
    pub(crate) action_state: Option<String>,
    pub(crate) action_id: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExternalIntegrationAuditFilter {
    pub(crate) item_type: Option<String>,
    pub(crate) action_state: Option<String>,
    pub(crate) action_id: Option<String>,
    pub(crate) limit: usize,
}

pub(crate) fn external_integration_audit_filter(
    query: ExternalIntegrationAuditQuery,
) -> std::result::Result<ExternalIntegrationAuditFilter, ApiError> {
    let item_type = query
        .item_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(|value| value.to_ascii_lowercase())
        .map(|value| match value.as_str() {
            "message" | "action" | "sync" | "search_evidence" | "outbound_reply" => Ok(value),
            _ => Err(ApiError::bad_request_with_details(
                "external_integration_audit_item_type_invalid",
                "audit item_type must be one of message, action, search_evidence, outbound_reply, sync, or all"
                    .to_string(),
                json!({ "item_type": value }),
            )),
        })
        .transpose()?;
    let action_state = query
        .action_state
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(|value| value.to_ascii_lowercase())
        .map(|value| match value.as_str() {
            "result_callback"
            | "waiting_result"
            | "failed"
            | "blocked"
            | "pending_confirmation" => Ok(value),
            _ => Err(ApiError::bad_request_with_details(
                "external_integration_audit_action_state_invalid",
                "audit action_state must be one of result_callback, waiting_result, failed, blocked, pending_confirmation, or all".to_string(),
                json!({ "action_state": value }),
            )),
        })
        .transpose()?;
    if action_state.is_some() && item_type.as_deref().is_some_and(|value| value != "action") {
        return Err(ApiError::bad_request(
            "external_integration_audit_filter_conflict",
            "action_state can only be combined with item_type=action or all".to_string(),
        ));
    }
    let action_id = query
        .action_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if action_id.is_some() && item_type.as_deref().is_some_and(|value| value != "action") {
        return Err(ApiError::bad_request(
            "external_integration_audit_filter_conflict",
            "action_id can only be combined with item_type=action or all".to_string(),
        ));
    }
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    Ok(ExternalIntegrationAuditFilter {
        item_type,
        action_state,
        action_id,
        limit,
    })
}

pub(crate) fn external_integration_audit_item_matches(
    item: &ExternalIntegrationAuditItemView,
    filter: &ExternalIntegrationAuditFilter,
) -> bool {
    if let Some(item_type) = filter.item_type.as_deref() {
        if item.item_type != item_type {
            return false;
        }
    }
    if let Some(action_id) = filter.action_id.as_deref() {
        if item.item_type != "action" || item.action_id.as_deref() != Some(action_id) {
            return false;
        }
    }
    let Some(action_state) = filter.action_state.as_deref() else {
        return true;
    };
    if item.item_type != "action" {
        return false;
    }
    let lifecycle_status = item
        .summary
        .get("action_lifecycle_status")
        .and_then(Value::as_str)
        .or(item.status.as_deref())
        .unwrap_or_default();
    let confirmation_state = item
        .summary
        .get("confirmation_state")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match action_state {
        "result_callback" => item
            .summary
            .get("result_callback_received")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "waiting_result" => lifecycle_status == "dispatched",
        "failed" => {
            item.failure_kind.is_some()
                || matches!(
                    lifecycle_status,
                    "dispatch_failed"
                        | "external_action_failed"
                        | "external_action_cancelled"
                        | "external_action_rejected"
                )
        }
        "blocked" => lifecycle_status == "dispatch_blocked",
        "pending_confirmation" => confirmation_state == "pending",
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::AssistantRunId;
    use serde_json::json;

    fn audit_item(item_type: &str, summary: Value) -> ExternalIntegrationAuditItemView {
        ExternalIntegrationAuditItemView {
            item_type: item_type.to_string(),
            created_at: Utc::now(),
            assistant_run_id: Some(AssistantRunId::new()),
            action_id: None,
            status: None,
            failure_kind: None,
            summary,
        }
    }

    #[test]
    fn audit_filter_accepts_lowercase_all_values_and_clamps_limit() {
        let filter = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some(" all ".to_string()),
            action_state: Some(" all ".to_string()),
            action_id: Some("  ".to_string()),
            limit: Some(500),
        })
        .expect("all values should be accepted as no filter");

        assert_eq!(filter.item_type, None);
        assert_eq!(filter.action_state, None);
        assert_eq!(filter.action_id, None);
        assert_eq!(filter.limit, 100);
    }

    #[test]
    fn audit_filter_rejects_unknown_item_type() {
        let error = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some("unknown".to_string()),
            action_state: None,
            action_id: None,
            limit: None,
        })
        .expect_err("unknown item type should fail");

        assert_eq!(
            error.payload.code,
            "external_integration_audit_item_type_invalid"
        );
    }

    #[test]
    fn audit_filter_rejects_action_filters_for_non_action_items() {
        let action_state_error = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some("message".to_string()),
            action_state: Some("blocked".to_string()),
            action_id: None,
            limit: None,
        })
        .expect_err("action_state should only apply to action items");
        let action_id_error = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some("sync".to_string()),
            action_state: None,
            action_id: Some("act-001".to_string()),
            limit: None,
        })
        .expect_err("action_id should only apply to action items");

        assert_eq!(
            action_state_error.payload.code,
            "external_integration_audit_filter_conflict"
        );
        assert_eq!(
            action_id_error.payload.code,
            "external_integration_audit_filter_conflict"
        );
    }

    #[test]
    fn audit_item_matches_action_lifecycle_states() {
        let waiting_filter = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some("action".to_string()),
            action_state: Some("waiting_result".to_string()),
            action_id: None,
            limit: None,
        })
        .expect("waiting filter should parse");
        let pending_filter = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some("action".to_string()),
            action_state: Some("pending_confirmation".to_string()),
            action_id: None,
            limit: None,
        })
        .expect("pending filter should parse");
        let blocked_filter = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some("action".to_string()),
            action_state: Some("blocked".to_string()),
            action_id: None,
            limit: None,
        })
        .expect("blocked filter should parse");

        let waiting = audit_item("action", json!({ "action_lifecycle_status": "dispatched" }));
        let pending = audit_item(
            "action",
            json!({
                "confirmation_state": "pending",
                "action_lifecycle_status": "pending_confirmation"
            }),
        );
        let blocked = audit_item(
            "action",
            json!({ "action_lifecycle_status": "dispatch_blocked" }),
        );

        assert!(external_integration_audit_item_matches(
            &waiting,
            &waiting_filter
        ));
        assert!(external_integration_audit_item_matches(
            &pending,
            &pending_filter
        ));
        assert!(external_integration_audit_item_matches(
            &blocked,
            &blocked_filter
        ));
        assert!(!external_integration_audit_item_matches(
            &waiting,
            &pending_filter
        ));
    }

    #[test]
    fn audit_item_matches_action_id_and_item_type() {
        let filter = external_integration_audit_filter(ExternalIntegrationAuditQuery {
            item_type: Some("action".to_string()),
            action_state: None,
            action_id: Some(" act-001 ".to_string()),
            limit: Some(0),
        })
        .expect("action_id filter should parse");
        let mut matching = audit_item("action", json!({}));
        matching.action_id = Some("act-001".to_string());
        let mut other = audit_item("action", json!({}));
        other.action_id = Some("act-002".to_string());
        let message = audit_item("message", json!({}));

        assert_eq!(filter.limit, 1);
        assert!(external_integration_audit_item_matches(&matching, &filter));
        assert!(!external_integration_audit_item_matches(&other, &filter));
        assert!(!external_integration_audit_item_matches(&message, &filter));
    }
}
