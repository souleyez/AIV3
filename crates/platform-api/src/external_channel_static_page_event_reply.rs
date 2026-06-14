use contracts::ExternalBotReplyView;
use domain_model::AssistantRunEvent;

use crate::{
    external_channel_public_artifact_url_from_value,
    external_channel_static_page_accepted_template_baseline,
    external_channel_static_page_payload_with_event_intent,
    external_channel_static_page_provisional_existing_artifact,
    external_channel_static_page_published_reply,
};

pub(crate) fn external_channel_static_page_event_artifact_link_reply_from_events(
    events: &[AssistantRunEvent],
    conversation_external_id: &str,
) -> Option<ExternalBotReplyView> {
    for event in events.iter().rev() {
        let event_name = event.event_name.as_str();
        let payload = &event.payload;
        let template_baseline_link = event_name
            == "assistant_run.external_channel_static_page_pipeline_queued"
            && external_channel_static_page_accepted_template_baseline(Some(payload));
        let terminal_or_stable_link = matches!(
            event_name,
            "assistant_run.external_channel_static_page_stable_artifact_reused"
                | "assistant_run.external_channel_static_page_publish_completed"
                | "assistant_run.external_channel_static_page_pipeline_queued"
        )
            && !external_channel_static_page_provisional_existing_artifact(Some(payload));
        if !template_baseline_link && !terminal_or_stable_link {
            continue;
        }
        let payload = external_channel_static_page_payload_with_event_intent(payload, events);
        let Some(public_url) = external_channel_public_artifact_url_from_value(&payload) else {
            continue;
        };
        return Some(external_channel_static_page_published_reply(
            conversation_external_id,
            &public_url,
            &payload,
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};
    use serde_json::{json, Value};

    fn assistant_event(sequence_no: i32, event_name: &str, payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no,
            event_name: event_name.to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn publish_completed_event_becomes_artifact_reply() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html";
        let events = vec![assistant_event(
            1,
            "assistant_run.external_channel_static_page_publish_completed",
            json!({ "public_url": public_url }),
        )];

        let reply =
            external_channel_static_page_event_artifact_link_reply_from_events(&events, "room-1")
                .expect("publish event should become artifact reply");

        assert_eq!(reply.artifact_links, vec![public_url.to_string()]);
        assert_eq!(
            reply.card.as_ref().expect("card")["public_url"],
            json!(public_url)
        );
    }

    #[test]
    fn provisional_pipeline_event_does_not_become_artifact_reply() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html";
        let events = vec![assistant_event(
            1,
            "assistant_run.external_channel_static_page_pipeline_queued",
            json!({
                "public_url": public_url,
                "provisional_existing_artifact": true,
                "provisional_existing_artifact_reason": "waiting_for_image2"
            }),
        )];

        assert!(
            external_channel_static_page_event_artifact_link_reply_from_events(&events, "room-1")
                .is_none()
        );
    }
}
