use chrono::{DateTime, Utc};
use domain_model::StaticPageDraft;
use serde_json::Value;
use storage::NewAssistantRunEvent;

use crate::{ApiError, AppState};

pub(crate) async fn append_static_page_draft_run_event(
    state: &AppState,
    draft: &StaticPageDraft,
    event_name: &str,
    payload: Value,
) -> std::result::Result<(), ApiError> {
    state
        .storage
        .assistant_runs()
        .append_event(
            state.tenant_id,
            draft.assistant_run_id,
            &static_page_draft_run_event_record(event_name, payload, Utc::now()),
        )
        .await
        .map_err(ApiError::from_storage)?;
    Ok(())
}

fn static_page_draft_run_event_record(
    event_name: &str,
    payload: Value,
    created_at: DateTime<Utc>,
) -> NewAssistantRunEvent {
    NewAssistantRunEvent {
        event_name: event_name.to_string(),
        payload,
        created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::*;

    #[test]
    fn draft_run_event_record_preserves_event_name_payload_and_time() {
        let created_at = Utc
            .with_ymd_and_hms(2026, 6, 20, 8, 0, 0)
            .single()
            .expect("valid timestamp");
        let payload = json!({
            "draft_id": "draft-1",
            "status": "queued"
        });

        let event = static_page_draft_run_event_record(
            "static_page.image_queued",
            payload.clone(),
            created_at,
        );

        assert_eq!(event.event_name, "static_page.image_queued");
        assert_eq!(event.payload, payload);
        assert_eq!(event.created_at, created_at);
    }

    #[test]
    fn draft_run_event_record_accepts_empty_payload() {
        let created_at = Utc
            .with_ymd_and_hms(2026, 6, 20, 8, 5, 0)
            .single()
            .expect("valid timestamp");

        let event = static_page_draft_run_event_record("static_page.noop", Value::Null, created_at);

        assert_eq!(event.event_name, "static_page.noop");
        assert_eq!(event.payload, Value::Null);
        assert_eq!(event.created_at, created_at);
    }
}
