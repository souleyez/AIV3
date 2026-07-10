#[cfg(test)]
use chrono::Utc;
#[cfg(test)]
use contracts::{ExternalAttachmentRefView, ExternalChannelPlatformView, ExternalMessageTypeView};
use contracts::{
    ExternalBotMessageView, ExternalBotReplyTypeView, ExternalBotReplyView,
    ExternalRequestedSkillView,
};
use domain_model::AssistantRunEvent;
use serde_json::{json, Map, Value};

use crate::{
    external_channel_message_has_image_attachment, external_requested_skill_argument_object,
    external_requested_skill_mode, external_requested_skills_bad_request, object_string, ApiError,
};

pub(crate) const AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA: &str = "aigolf.course_map_segmentation.v1";
pub(crate) const EXTERNAL_AIGOLF_COURSE_MAP_SEGMENTATION_EVENT_NAME: &str =
    "assistant_run.aigolf_course_map_segmentation_needs_review";
pub(crate) const EXTERNAL_AIGOLF_COURSE_MAP_SEGMENTATION_ARTIFACT_TYPE: &str =
    "aigolf_course_map_segmentation";

fn external_requested_skill_compact_id(skill: &ExternalRequestedSkillView) -> String {
    skill
        .skill_id
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' ' | '.'))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn external_requested_skill_is_disabled(skill: &ExternalRequestedSkillView) -> bool {
    external_requested_skill_mode(skill) == "disabled"
}

fn external_aigolf_skill_kind(skill: &ExternalRequestedSkillView) -> Option<&'static str> {
    match external_requested_skill_compact_id(skill).as_str() {
        "aigolfcoursemapsegmentation" | "aigolfmapsegmentation" => {
            Some("aigolf_course_map_segmentation")
        }
        "aigolfholegeofencerefine" => Some("aigolf_hole_geofence_refine"),
        "aigolfdispatchriskanalysis" => Some("aigolf_dispatch_risk_analysis"),
        "aigolfscoreanomalyanalysis" => Some("aigolf_score_anomaly_analysis"),
        "aigolfeventworkorderdraft" => Some("aigolf_event_work_order_draft"),
        "aigolfsportcardgeneration" => Some("aigolf_sport_card_generation"),
        _ => None,
    }
}

fn object_nested_string(
    object: &Map<String, Value>,
    object_keys: &[&str],
    string_keys: &[&str],
) -> Option<String> {
    object_keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(Value::as_object)
            .and_then(|nested| object_string(nested, string_keys))
    })
}

fn external_aigolf_course_map_schema(skill: &ExternalRequestedSkillView) -> Option<String> {
    let object = external_requested_skill_argument_object(skill)?;
    object_string(
        object,
        &[
            "schema",
            "output_schema",
            "outputSchema",
            "expected_schema",
            "expectedSchema",
            "required_schema",
            "requiredSchema",
        ],
    )
    .or_else(|| {
        object_nested_string(
            object,
            &["output", "result", "contract"],
            &["schema", "outputSchema", "requiredSchema"],
        )
    })
}

pub(crate) fn external_aigolf_course_map_argument_image_ref(
    skill: &ExternalRequestedSkillView,
) -> Option<String> {
    let object = external_requested_skill_argument_object(skill)?;
    object_string(
        object,
        &[
            "image_url",
            "imageUrl",
            "map_image_url",
            "mapImageUrl",
            "course_map_image_url",
            "courseMapImageUrl",
            "attachment_external_id",
            "attachmentExternalId",
            "map_attachment_external_id",
            "mapAttachmentExternalId",
        ],
    )
    .or_else(|| {
        object_nested_string(
            object,
            &["map", "course_map", "courseMap", "image"],
            &[
                "url",
                "image_url",
                "imageUrl",
                "map_image_url",
                "mapImageUrl",
                "attachment_external_id",
                "attachmentExternalId",
            ],
        )
    })
}

fn external_aigolf_course_map_skill_has_image_input(
    skill: &ExternalRequestedSkillView,
    message: &ExternalBotMessageView,
) -> bool {
    external_aigolf_course_map_argument_image_ref(skill).is_some()
        || external_channel_message_has_image_attachment(message)
}

pub(crate) fn external_aigolf_course_map_segmentation_skill(
    message: &ExternalBotMessageView,
) -> Option<&ExternalRequestedSkillView> {
    message
        .requested_skills
        .iter()
        .filter(|skill| !external_requested_skill_is_disabled(skill))
        .find(|skill| external_aigolf_skill_kind(skill) == Some("aigolf_course_map_segmentation"))
}

pub(crate) fn validate_external_aigolf_requested_skills(
    message: &ExternalBotMessageView,
) -> std::result::Result<(), ApiError> {
    for (index, skill) in message.requested_skills.iter().enumerate() {
        if external_requested_skill_is_disabled(skill) {
            continue;
        }
        let Some(skill_kind) = external_aigolf_skill_kind(skill) else {
            continue;
        };
        if skill_kind != "aigolf_course_map_segmentation" {
            continue;
        }

        if let Some(schema) = external_aigolf_course_map_schema(skill) {
            if schema != AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA {
                return Err(external_requested_skills_bad_request(
                    "aigolf_course_map_segmentation_unsupported_schema",
                    format!(
                        "requested_skills[{index}] aigolf_course_map_segmentation only accepts schema {AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA}"
                    ),
                ));
            }
        }

        if !external_aigolf_course_map_skill_has_image_input(skill, message) {
            return Err(external_requested_skills_bad_request(
                "aigolf_course_map_segmentation_missing_image",
                format!(
                    "requested_skills[{index}] aigolf_course_map_segmentation requires an image attachment or map.imageUrl/mapImageUrl argument"
                ),
            ));
        }
    }

    Ok(())
}

pub(crate) fn external_aigolf_requested_skill_policy(
    skills: &[ExternalRequestedSkillView],
) -> Option<Value> {
    let skill_policies = skills
        .iter()
        .filter(|skill| !external_requested_skill_is_disabled(skill))
        .filter_map(|skill| {
            let skill_kind = external_aigolf_skill_kind(skill)?;
            let arguments = skill.arguments.clone().unwrap_or_else(|| json!({}));
            let policy = if skill_kind == "aigolf_course_map_segmentation" {
                json!({
                    "skill_id": skill.skill_id.as_str(),
                    "normalized_skill_id": skill_kind,
                    "status": "enabled",
                    "mode": external_requested_skill_mode(skill),
                    "version": skill.version.as_deref(),
                    "model_profile": "datamax_high_quality_paid_route",
                    "provider_detail_policy": "do_not_expose_provider_or_model_name_to_integrator",
                    "card_type": "aigolf_course_map_segmentation",
                    "reply_card_type": "aigolf_course_map_segmentation",
                    "required_schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA,
                    "input_contract": {
                        "requires_image": true,
                        "accepted_inputs": [
                            "attachment_refs image/*",
                            "requested_skills.arguments.map.imageUrl",
                            "requested_skills.arguments.mapImageUrl"
                        ],
                        "operator_review_required": true
                    },
                    "output_contract": {
                        "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA,
                        "structured_json_required": true,
                        "card_type": "aigolf_course_map_segmentation",
                        "business_rule": "Return a reviewable draft only. Do not apply production hole geofences directly.",
                        "fields": {
                            "holes": [{
                                "holeNo": "number",
                                "name": "optional string",
                                "polygonPixels": [[0, 0]],
                                "centerPixel": [0, 0],
                                "confidence": "0..1",
                                "warnings": ["optional string"]
                            }],
                            "image": {
                                "width": "number when known",
                                "height": "number when known"
                            },
                            "review": {
                                "status": "ready_for_operator_review | needs_input | partial",
                                "notes": ["string"]
                            }
                        }
                    },
                    "failure_contract": {
                        "status": "needs_input",
                        "message": "Ask for a clearer course map image or calibration points when segmentation is not possible.",
                        "card_type": "aigolf_course_map_segmentation"
                    },
                    "arguments": arguments
                })
            } else {
                json!({
                    "skill_id": skill.skill_id.as_str(),
                    "normalized_skill_id": skill_kind,
                    "status": "unsupported_skill_not_enabled",
                    "mode": external_requested_skill_mode(skill),
                    "version": skill.version.as_deref(),
                    "model_profile": "datamax_high_quality_paid_route_when_enabled",
                    "provider_detail_policy": "do_not_expose_provider_or_model_name_to_integrator",
                    "failure_contract": {
                        "status": "unsupported_skill_not_enabled",
                        "message": "This AI Golf skill is declared but not enabled in the current V3 routing release."
                    },
                    "arguments": arguments
                })
            };
            Some(policy)
        })
        .collect::<Vec<_>>();

    if skill_policies.is_empty() {
        return None;
    }

    Some(json!({
        "source": "external_channel_requested_skills",
        "domain": "aigolf",
        "runtime_policy": "use_datamax_high_quality_paid_route_for_low_frequency_high_value_tasks",
        "provider_detail_policy": "do_not_expose_provider_or_model_name_to_integrator",
        "skills": skill_policies
    }))
}

pub(crate) fn external_aigolf_course_map_segmentation_reply_for_conversation(
    conversation_external_id: &str,
    payload: &Value,
) -> ExternalBotReplyView {
    ExternalBotReplyView {
        target_conversation_external_id: conversation_external_id.to_string(),
        reply_type: ExternalBotReplyTypeView::Card,
        text: Some(
            "DataMax 已接收 AI Golf 球场地图分割任务，并返回可复核的结构化初稿。".to_string(),
        ),
        card: Some(payload.clone()),
        artifact_links: Vec::new(),
        task_status: Some("aigolf_course_map_segmentation_needs_review".to_string()),
        requires_confirmation: false,
        action_id: None,
        confirmation_id: None,
    }
}

pub(crate) fn external_aigolf_course_map_segmentation_output_artifacts(
    payload: &Value,
    reply: &ExternalBotReplyView,
) -> Value {
    json!([{
        "type": EXTERNAL_AIGOLF_COURSE_MAP_SEGMENTATION_ARTIFACT_TYPE,
        "source": "external_channel_requested_skill",
        "content": reply.text.clone().unwrap_or_default(),
        "payload": payload.clone(),
    }])
}

pub(crate) fn external_channel_aigolf_skill_reply_from_events(
    events: &[AssistantRunEvent],
    conversation_external_id: &str,
) -> Option<ExternalBotReplyView> {
    let event = events
        .iter()
        .rev()
        .find(|event| event.event_name == EXTERNAL_AIGOLF_COURSE_MAP_SEGMENTATION_EVENT_NAME)?;
    event
        .payload
        .get("reply")
        .cloned()
        .and_then(|reply| serde_json::from_value::<ExternalBotReplyView>(reply).ok())
        .or_else(|| {
            event.payload.get("payload").map(|payload| {
                external_aigolf_course_map_segmentation_reply_for_conversation(
                    conversation_external_id,
                    payload,
                )
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    fn event(event_name: &str, sequence_no: i64, payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no: sequence_no as i32,
            event_name: event_name.to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    fn sample_message() -> ExternalBotMessageView {
        ExternalBotMessageView {
            platform: ExternalChannelPlatformView::GenericChat,
            tenant_external_id: "tenant-ext-001".to_string(),
            bot_external_id: "bot-v3".to_string(),
            conversation_external_id: "conv-aigolf".to_string(),
            thread_external_id: None,
            sender_external_id: "user-ext-001".to_string(),
            message_external_id: "msg-aigolf-001".to_string(),
            message_type: ExternalMessageTypeView::Text,
            text: Some("识别球场地图".to_string()),
            default_prompt: None,
            output_format: None,
            render_mode: None,
            artifact_type: None,
            template: None,
            mention_external_user_ids: Vec::new(),
            attachment_refs: Vec::new(),
            business_datasource_ids: Vec::new(),
            available_document_external_ids: Vec::new(),
            available_document_source_id: None,
            dataset_external_id: None,
            dataset_external_ids: Vec::new(),
            requested_skills: Vec::new(),
            idempotency_key: "generic_chat:tenant-ext-001:msg-aigolf-001".to_string(),
            received_at: Utc::now(),
        }
    }

    #[test]
    fn validates_course_map_skill_with_nested_image_url_and_policy() {
        let mut message = sample_message();
        message.requested_skills = vec![ExternalRequestedSkillView {
            skill_id: "AI Golf Course Map Segmentation".to_string(),
            version: Some("2026-06-11".to_string()),
            mode: Some("REQUIRED".to_string()),
            arguments: Some(json!({
                "output": {
                    "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA
                },
                "map": {
                    "imageUrl": "https://assets.example.com/course-map.png"
                }
            })),
        }];

        validate_external_aigolf_requested_skills(&message).expect("valid course map skill");
        let skill = external_aigolf_course_map_segmentation_skill(&message).expect("skill");
        assert_eq!(
            external_aigolf_course_map_argument_image_ref(skill).as_deref(),
            Some("https://assets.example.com/course-map.png")
        );

        let policy = external_aigolf_requested_skill_policy(&message.requested_skills)
            .expect("AI Golf policy");
        assert_eq!(policy["domain"], json!("aigolf"));
        assert_eq!(
            policy["skills"][0]["required_schema"],
            json!(AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA)
        );
        assert_eq!(
            policy["skills"][0]["provider_detail_policy"],
            json!("do_not_expose_provider_or_model_name_to_integrator")
        );
    }

    #[test]
    fn validates_course_map_skill_with_image_attachment() {
        let mut message = sample_message();
        message.attachment_refs = vec![ExternalAttachmentRefView {
            attachment_external_id: "course-map-image-001".to_string(),
            filename: Some("course-map.png".to_string()),
            content_type: Some("image/png".to_string()),
            size_bytes: Some(8192),
            download_url_redacted: Some("https://assets.example.com/course-map.png".to_string()),
        }];
        message.requested_skills = vec![ExternalRequestedSkillView {
            skill_id: "aigolf_course_map_segmentation".to_string(),
            version: None,
            mode: Some("required".to_string()),
            arguments: Some(json!({
                "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA
            })),
        }];

        validate_external_aigolf_requested_skills(&message)
            .expect("image attachment should satisfy map input requirement");
    }

    #[test]
    fn rejects_course_map_skill_with_missing_image_input() {
        let mut message = sample_message();
        message.requested_skills = vec![ExternalRequestedSkillView {
            skill_id: "aigolf_course_map_segmentation".to_string(),
            version: None,
            mode: Some("required".to_string()),
            arguments: Some(json!({
                "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA
            })),
        }];

        let error = validate_external_aigolf_requested_skills(&message)
            .expect_err("missing image should fail");
        assert_eq!(
            error
                .payload
                .details
                .as_ref()
                .and_then(|value| value.get("reason"))
                .and_then(Value::as_str),
            Some("aigolf_course_map_segmentation_missing_image")
        );
    }

    #[test]
    fn rejects_course_map_skill_with_unsupported_schema() {
        let mut message = sample_message();
        message.requested_skills = vec![ExternalRequestedSkillView {
            skill_id: "aigolf_course_map_segmentation".to_string(),
            version: None,
            mode: Some("required".to_string()),
            arguments: Some(json!({
                "schema": "aigolf.course_map_segmentation.v0",
                "mapImageUrl": "https://assets.example.com/course-map.png"
            })),
        }];

        let error = validate_external_aigolf_requested_skills(&message)
            .expect_err("unsupported schema should fail");
        assert_eq!(
            error
                .payload
                .details
                .as_ref()
                .and_then(|value| value.get("reason"))
                .and_then(Value::as_str),
            Some("aigolf_course_map_segmentation_unsupported_schema")
        );
    }

    #[test]
    fn ignores_disabled_course_map_skill_for_validation_and_selection() {
        let mut message = sample_message();
        message.requested_skills = vec![ExternalRequestedSkillView {
            skill_id: "aigolf_course_map_segmentation".to_string(),
            version: None,
            mode: Some("disabled".to_string()),
            arguments: Some(json!({
                "schema": "aigolf.course_map_segmentation.v0"
            })),
        }];

        validate_external_aigolf_requested_skills(&message)
            .expect("disabled skill should not be validated");
        assert!(external_aigolf_course_map_segmentation_skill(&message).is_none());
        assert!(external_aigolf_requested_skill_policy(&message.requested_skills).is_none());
    }

    #[test]
    fn course_map_segmentation_reply_uses_card_payload_and_review_status() {
        let payload = json!({
            "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA,
            "status": "needs_review",
            "holes": [],
            "warnings": ["operator_review_required_before_publish"]
        });

        let reply =
            external_aigolf_course_map_segmentation_reply_for_conversation("conv-aigolf", &payload);

        assert_eq!(reply.target_conversation_external_id, "conv-aigolf");
        assert_eq!(reply.reply_type, ExternalBotReplyTypeView::Card);
        assert_eq!(
            reply.text.as_deref(),
            Some("DataMax 已接收 AI Golf 球场地图分割任务，并返回可复核的结构化初稿。")
        );
        assert_eq!(reply.card.as_ref(), Some(&payload));
        assert_eq!(
            reply.task_status.as_deref(),
            Some("aigolf_course_map_segmentation_needs_review")
        );
        assert!(!reply.requires_confirmation);
    }

    #[test]
    fn course_map_segmentation_output_artifacts_keep_type_source_content_and_payload() {
        let payload = json!({
            "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA,
            "status": "needs_review",
            "holes": [{ "hole": 1 }]
        });
        let reply =
            external_aigolf_course_map_segmentation_reply_for_conversation("conv-aigolf", &payload);

        let artifacts = external_aigolf_course_map_segmentation_output_artifacts(&payload, &reply);

        assert_eq!(
            artifacts[0]["type"],
            json!("aigolf_course_map_segmentation")
        );
        assert_eq!(
            artifacts[0]["source"],
            json!("external_channel_requested_skill")
        );
        assert_eq!(
            artifacts[0]["content"],
            json!("DataMax 已接收 AI Golf 球场地图分割任务，并返回可复核的结构化初稿。")
        );
        assert_eq!(artifacts[0]["payload"], payload);
    }

    #[test]
    fn course_map_segmentation_reply_from_events_prefers_latest_persisted_reply() {
        let stale_payload = json!({
            "payload": {
                "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA,
                "status": "stale"
            }
        });
        let latest_payload = json!({
            "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA,
            "status": "latest"
        });
        let latest_reply = external_aigolf_course_map_segmentation_reply_for_conversation(
            "conv-latest",
            &latest_payload,
        );
        let events = vec![
            event(
                EXTERNAL_AIGOLF_COURSE_MAP_SEGMENTATION_EVENT_NAME,
                1,
                stale_payload,
            ),
            event(
                "assistant_run.other_event",
                2,
                json!({ "payload": { "status": "ignored" } }),
            ),
            event(
                EXTERNAL_AIGOLF_COURSE_MAP_SEGMENTATION_EVENT_NAME,
                3,
                json!({
                    "payload": { "status": "fallback_should_not_win" },
                    "reply": latest_reply
                }),
            ),
        ];

        let reply =
            external_channel_aigolf_skill_reply_from_events(&events, "conv-fallback").unwrap();

        assert_eq!(reply.target_conversation_external_id, "conv-latest");
        assert_eq!(reply.card.as_ref(), Some(&latest_payload));
    }

    #[test]
    fn course_map_segmentation_reply_from_events_falls_back_to_payload() {
        let payload = json!({
            "schema": AIGOLF_COURSE_MAP_SEGMENTATION_SCHEMA,
            "status": "needs_review",
            "warnings": ["operator_review_required_before_publish"]
        });
        let events = vec![event(
            EXTERNAL_AIGOLF_COURSE_MAP_SEGMENTATION_EVENT_NAME,
            1,
            json!({ "payload": payload }),
        )];

        let reply =
            external_channel_aigolf_skill_reply_from_events(&events, "conv-fallback").unwrap();

        assert_eq!(reply.target_conversation_external_id, "conv-fallback");
        assert_eq!(
            reply.task_status.as_deref(),
            Some("aigolf_course_map_segmentation_needs_review")
        );
        assert_eq!(reply.card, Some(payload));
    }
}
