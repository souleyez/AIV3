use contracts::{ExternalBotReplyTypeView, ExternalBotReplyView};
use domain_model::AssistantRunEvent;
use serde_json::{json, Value};

pub(crate) const EXTERNAL_IMAGE_STRUCTURED_EXTRACT_EVENT_NAME: &str =
    "assistant_run.external_channel_image_structured_extract_completed";
const EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ARTIFACT_TYPE: &str =
    "external_channel_image_structured_extract";

pub(crate) fn external_image_structured_extract_reply_for_conversation(
    conversation_external_id: &str,
    payload: &Value,
    output_json: bool,
) -> ExternalBotReplyView {
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("answered");
    ExternalBotReplyView {
        target_conversation_external_id: conversation_external_id.to_string(),
        reply_type: ExternalBotReplyTypeView::Card,
        text: Some(external_image_structured_extract_text(payload, output_json)),
        card: Some(payload.clone()),
        artifact_links: Vec::new(),
        task_status: Some(status.to_string()),
        requires_confirmation: false,
        action_id: None,
        confirmation_id: None,
    }
}

pub(crate) fn external_image_structured_extract_text(payload: &Value, output_json: bool) -> String {
    if output_json {
        return serde_json::to_string_pretty(payload).unwrap_or_else(|_| payload.to_string());
    }
    let record_count = payload
        .get("record_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("answered");
    if status == "answered" {
        format!("已识别并结构化 {record_count} 条订单记录，详情见 card.records。")
    } else {
        let reason = payload
            .get("failure_reason")
            .and_then(Value::as_str)
            .unwrap_or("needs_review");
        format!("图片字段抽取需要复核：{reason}。详情见 card.records。")
    }
}

pub(crate) fn external_image_structured_extract_output_artifacts(
    payload: &Value,
    reply: &ExternalBotReplyView,
) -> Value {
    json!([{
        "type": EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ARTIFACT_TYPE,
        "source": "external_channel_image",
        "content": reply
            .text
            .clone()
            .unwrap_or_else(|| external_image_structured_extract_text(payload, true)),
        "payload": payload.clone(),
    }])
}

pub(crate) fn external_channel_image_structured_extract_reply_from_events(
    events: &[AssistantRunEvent],
    conversation_external_id: &str,
) -> Option<ExternalBotReplyView> {
    let event = events
        .iter()
        .rev()
        .find(|event| event.event_name == EXTERNAL_IMAGE_STRUCTURED_EXTRACT_EVENT_NAME)?;
    event
        .payload
        .get("reply")
        .cloned()
        .and_then(|reply| serde_json::from_value::<ExternalBotReplyView>(reply).ok())
        .or_else(|| {
            event.payload.get("payload").map(|payload| {
                external_image_structured_extract_reply_for_conversation(
                    conversation_external_id,
                    payload,
                    true,
                )
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    fn event(event_name: &str, sequence_no: i64, payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            event_name: event_name.to_string(),
            payload,
            sequence_no: sequence_no as i32,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn reply_uses_payload_status_card_and_plain_summary_text() {
        let payload = json!({
            "status": "answered",
            "record_count": 2,
            "records": [{ "order_no": "A1" }]
        });
        let reply =
            external_image_structured_extract_reply_for_conversation("conv-1", &payload, false);

        assert_eq!(reply.target_conversation_external_id, "conv-1");
        assert_eq!(reply.reply_type, ExternalBotReplyTypeView::Card);
        assert_eq!(reply.task_status.as_deref(), Some("answered"));
        assert_eq!(reply.card.as_ref(), Some(&payload));
        assert_eq!(
            reply.text.as_deref(),
            Some("已识别并结构化 2 条订单记录，详情见 card.records。")
        );
    }

    #[test]
    fn reply_can_return_json_text_or_review_message() {
        let payload = json!({
            "status": "needs_review",
            "failure_reason": "image_not_visible",
            "record_count": 0
        });
        let json_text = external_image_structured_extract_text(&payload, true);
        assert!(json_text.contains("\"status\": \"needs_review\""));

        let review_text = external_image_structured_extract_text(&payload, false);
        assert_eq!(
            review_text,
            "图片字段抽取需要复核：image_not_visible。详情见 card.records。"
        );
    }

    #[test]
    fn output_artifacts_keep_type_source_content_and_payload() {
        let payload = json!({
            "status": "answered",
            "record_count": 1,
            "records": [{ "order_no": "A1" }]
        });
        let reply =
            external_image_structured_extract_reply_for_conversation("conv-1", &payload, false);

        let artifacts = external_image_structured_extract_output_artifacts(&payload, &reply);
        assert_eq!(
            artifacts[0]["type"],
            json!("external_channel_image_structured_extract")
        );
        assert_eq!(artifacts[0]["source"], json!("external_channel_image"));
        assert_eq!(
            artifacts[0]["content"],
            json!("已识别并结构化 1 条订单记录，详情见 card.records。")
        );
        assert_eq!(artifacts[0]["payload"], payload);

        let mut reply_without_text = reply.clone();
        reply_without_text.text = None;
        let fallback_artifacts =
            external_image_structured_extract_output_artifacts(&payload, &reply_without_text);
        assert!(fallback_artifacts[0]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("\"record_count\": 1"));
    }

    #[test]
    fn event_reply_prefers_persisted_reply_and_falls_back_to_payload_card() {
        let reply_payload = json!({
            "target_conversation_external_id": "conv-old",
            "reply_type": "card",
            "text": "persisted reply",
            "card": { "status": "answered" },
            "artifact_links": [],
            "task_status": "answered",
            "requires_confirmation": false,
            "action_id": null,
            "confirmation_id": null
        });
        let events = vec![
            event(
                EXTERNAL_IMAGE_STRUCTURED_EXTRACT_EVENT_NAME,
                1,
                json!({
                    "payload": {
                        "status": "needs_review",
                        "failure_reason": "older"
                    }
                }),
            ),
            event(
                EXTERNAL_IMAGE_STRUCTURED_EXTRACT_EVENT_NAME,
                2,
                json!({
                    "reply": reply_payload
                }),
            ),
        ];

        let reply =
            external_channel_image_structured_extract_reply_from_events(&events, "conv-new")
                .expect("persisted reply should restore");
        assert_eq!(reply.text.as_deref(), Some("persisted reply"));
        assert_eq!(reply.target_conversation_external_id, "conv-old");

        let fallback = external_channel_image_structured_extract_reply_from_events(
            &[event(
                EXTERNAL_IMAGE_STRUCTURED_EXTRACT_EVENT_NAME,
                1,
                json!({
                    "payload": {
                        "status": "answered",
                        "record_count": 1,
                        "records": [{ "order_no": "A1" }]
                    }
                }),
            )],
            "conv-new",
        )
        .expect("payload should rebuild reply");
        assert_eq!(fallback.target_conversation_external_id, "conv-new");
        assert_eq!(fallback.task_status.as_deref(), Some("answered"));
        assert!(fallback
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("\"record_count\": 1"));
    }
}
