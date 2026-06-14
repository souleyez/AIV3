use domain_model::AssistantRunEvent;
use serde_json::Value;

use crate::static_page_template_prewarm_support::STATIC_PAGE_TEMPLATE_PREWARM_SOURCE;

pub(crate) fn external_channel_static_page_publish_completed_event_is_final(
    event: &AssistantRunEvent,
) -> bool {
    if event.event_name != "assistant_run.external_channel_static_page_publish_completed" {
        return false;
    }
    let publish_mode = event
        .payload
        .get("publish_mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let provisional_direct_html = event
        .payload
        .get("provisional_direct_html")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    publish_mode != "demo_direct_generated_artifact" && !provisional_direct_html
}

pub(crate) fn external_channel_static_page_source_allows_auto_publish(
    source_kind: Option<&str>,
) -> bool {
    matches!(
        source_kind,
        Some("external_channel_static_page_artifact_request")
            | Some("local_chat_static_page_image2_pipeline")
            | Some(STATIC_PAGE_TEMPLATE_PREWARM_SOURCE)
    )
}

pub(crate) fn external_channel_static_page_publish_customer_visible(source_refs: &Value) -> bool {
    source_refs
        .get("prewarm")
        .and_then(|value| value.get("customer_visible"))
        .and_then(Value::as_bool)
        != Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};
    use serde_json::{json, Value};

    fn assistant_event(event_name: &str, payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no: 1,
            event_name: event_name.to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn publish_completed_event_finality_rejects_non_final_variants() {
        assert!(
            external_channel_static_page_publish_completed_event_is_final(&assistant_event(
                "assistant_run.external_channel_static_page_publish_completed",
                json!({"publish_mode": "new_generated_artifact_only"})
            ))
        );
        assert!(
            !external_channel_static_page_publish_completed_event_is_final(&assistant_event(
                "assistant_run.external_channel_static_page_publish_completed",
                json!({"publish_mode": "demo_direct_generated_artifact"})
            ))
        );
        assert!(
            !external_channel_static_page_publish_completed_event_is_final(&assistant_event(
                "assistant_run.external_channel_static_page_publish_completed",
                json!({"provisional_direct_html": true})
            ))
        );
        assert!(
            !external_channel_static_page_publish_completed_event_is_final(&assistant_event(
                "assistant_run.other_event",
                json!({})
            ))
        );
    }

    #[test]
    fn source_allowlist_accepts_only_static_page_publish_sources() {
        assert!(external_channel_static_page_source_allows_auto_publish(
            Some("external_channel_static_page_artifact_request")
        ));
        assert!(external_channel_static_page_source_allows_auto_publish(
            Some("local_chat_static_page_image2_pipeline")
        ));
        assert!(external_channel_static_page_source_allows_auto_publish(
            Some(STATIC_PAGE_TEMPLATE_PREWARM_SOURCE)
        ));
        assert!(!external_channel_static_page_source_allows_auto_publish(
            Some("external_channel_chat_answer")
        ));
        assert!(!external_channel_static_page_source_allows_auto_publish(
            None
        ));
    }

    #[test]
    fn prewarm_customer_visibility_can_be_suppressed() {
        assert!(!external_channel_static_page_publish_customer_visible(
            &json!({
                "source": STATIC_PAGE_TEMPLATE_PREWARM_SOURCE,
                "prewarm": {
                    "mode": "silent_low_load_template_prewarm",
                    "customer_visible": false
                }
            })
        ));
        assert!(external_channel_static_page_publish_customer_visible(
            &json!({
                "source": STATIC_PAGE_TEMPLATE_PREWARM_SOURCE,
                "prewarm": {
                    "customer_visible": true
                }
            })
        ));
        assert!(external_channel_static_page_publish_customer_visible(
            &json!({
                "source": "external_channel_static_page_artifact_request"
            })
        ));
    }
}
