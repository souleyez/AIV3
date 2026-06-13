use contracts::{ExternalArtifactTemplateView, ExternalBotMessageView, ExternalRequestedSkillView};
use domain_model::DocumentId;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::{
    external_document_template_skill_document_id, external_document_template_skill_external_id,
    external_requested_skill_is_document_template, external_requested_skill_mode, push_string_hint,
    ApiError,
};

fn normalize_external_optional_plain_field(
    value: &mut Option<String>,
    field_name: &str,
    limit: usize,
) -> std::result::Result<(), ApiError> {
    if let Some(raw) = value.take() {
        let normalized = raw.trim().to_string();
        if normalized.is_empty() {
            *value = None;
        } else if normalized.chars().count() > limit || normalized.chars().any(char::is_control) {
            return Err(ApiError::bad_request_with_details(
                "external_channel_artifact_request_invalid",
                format!("{field_name} must be printable text within {limit} characters"),
                json!({ "reason": "invalid_artifact_template_field", "field": field_name }),
            ));
        } else {
            *value = Some(normalized);
        }
    }
    Ok(())
}

fn normalize_external_artifact_type_value(value: &str) -> Option<&'static str> {
    let compact = value
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' ' | '\t' | '\n' | '\r'))
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    match compact.as_str() {
        "" => None,
        "staticpage" | "webpage" | "page" | "htmlpage" => Some("static_page"),
        "html" => Some("html"),
        "report" => Some("report"),
        "document" | "doc" => Some("document"),
        "table" | "spreadsheet" => Some("table"),
        "image" | "picture" => Some("image"),
        "any" => Some("any"),
        _ => None,
    }
}

fn validate_and_normalize_external_artifact_template(
    template: &mut ExternalArtifactTemplateView,
) -> std::result::Result<(), ApiError> {
    normalize_external_optional_plain_field(&mut template.source_id, "template.source_id", 128)?;
    normalize_external_optional_plain_field(
        &mut template.document_external_id,
        "template.document_external_id",
        256,
    )?;
    normalize_external_optional_plain_field(&mut template.document_id, "template.document_id", 64)?;
    normalize_external_optional_plain_field(
        &mut template.revision_external_id,
        "template.revision_external_id",
        128,
    )?;
    normalize_external_optional_plain_field(
        &mut template.template_reference_id,
        "template.template_reference_id",
        128,
    )?;
    normalize_external_optional_plain_field(&mut template.mode, "template.mode", 32)?;
    normalize_external_optional_plain_field(&mut template.title, "template.title", 160)?;
    if let Some(output_type) = template.output_type.take() {
        let normalized = output_type.trim();
        if normalized.is_empty() {
            template.output_type = None;
        } else if let Some(value) = normalize_external_artifact_type_value(normalized) {
            template.output_type = Some(value.to_string());
        } else {
            return Err(ApiError::bad_request_with_details(
                "external_channel_artifact_request_invalid",
                "template.output_type must be one of static_page, html, report, document, table, image, or any".to_string(),
                json!({ "reason": "invalid_template_output_type" }),
            ));
        }
    }
    Ok(())
}

fn external_artifact_template_requested_skill_exists(
    requested_skills: &[ExternalRequestedSkillView],
    template: &ExternalArtifactTemplateView,
) -> bool {
    requested_skills
        .iter()
        .filter(|skill| external_requested_skill_mode(skill) != "disabled")
        .filter(|skill| external_requested_skill_is_document_template(skill))
        .any(|skill| {
            let same_document_id = template
                .document_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value.trim()).ok())
                .map(DocumentId)
                .is_some_and(|document_id| {
                    external_document_template_skill_document_id(skill) == Some(document_id)
                });
            let same_external_id =
                template
                    .document_external_id
                    .as_deref()
                    .is_some_and(|external_id| {
                        external_document_template_skill_external_id(skill).as_deref()
                            == Some(external_id)
                    });
            same_document_id || same_external_id
        })
}

fn external_artifact_template_to_requested_skill(
    template: &ExternalArtifactTemplateView,
    artifact_type: Option<&str>,
) -> Option<ExternalRequestedSkillView> {
    if template.document_id.is_none() && template.document_external_id.is_none() {
        return None;
    }
    let mut arguments = Map::new();
    if let Some(document_id) = template.document_id.as_deref() {
        arguments.insert("template_document_id".to_string(), json!(document_id));
    }
    if let Some(document_external_id) = template.document_external_id.as_deref() {
        arguments.insert(
            "template_document_external_id".to_string(),
            json!(document_external_id),
        );
    }
    if let Some(source_id) = template.source_id.as_deref() {
        arguments.insert("source_id".to_string(), json!(source_id));
    }
    if let Some(revision_external_id) = template.revision_external_id.as_deref() {
        arguments.insert(
            "revision_external_id".to_string(),
            json!(revision_external_id),
        );
    }
    if let Some(title) = template.title.as_deref() {
        arguments.insert("template_title".to_string(), json!(title));
    }
    if let Some(mode) = template.mode.as_deref() {
        arguments.insert("template_mode".to_string(), json!(mode));
    }
    arguments.insert(
        "output_type".to_string(),
        json!(template
            .output_type
            .as_deref()
            .or(artifact_type)
            .unwrap_or("any")),
    );
    arguments.insert(
        "reference_source".to_string(),
        json!("external_channel_template_field"),
    );
    Some(ExternalRequestedSkillView {
        skill_id: "document_template_skill".to_string(),
        version: template.revision_external_id.clone(),
        mode: Some("required".to_string()),
        arguments: Some(Value::Object(arguments)),
    })
}

pub(crate) fn validate_and_normalize_external_artifact_request(
    message: &mut ExternalBotMessageView,
) -> std::result::Result<(), ApiError> {
    if let Some(artifact_type) = message.artifact_type.take() {
        let normalized = artifact_type.trim();
        if normalized.is_empty() {
            message.artifact_type = None;
        } else if let Some(value) = normalize_external_artifact_type_value(normalized) {
            message.artifact_type = Some(value.to_string());
        } else {
            return Err(ApiError::bad_request_with_details(
                "external_channel_artifact_request_invalid",
                "artifact_type must be one of static_page, html, report, document, table, image, or any".to_string(),
                json!({ "reason": "invalid_artifact_type" }),
            ));
        }
    }

    let mut target_artifact_type = message.artifact_type.clone();
    if let Some(template) = message.template.as_mut() {
        validate_and_normalize_external_artifact_template(template)?;
        if target_artifact_type.is_none() {
            target_artifact_type = template.output_type.clone();
            message.artifact_type = target_artifact_type.clone();
        }
        if let Some(document_external_id) = template.document_external_id.as_deref() {
            push_string_hint(
                &mut message.available_document_external_ids,
                document_external_id,
            );
        }
        if message.available_document_source_id.is_none() {
            message.available_document_source_id = template.source_id.clone();
        }
        if !external_artifact_template_requested_skill_exists(&message.requested_skills, template) {
            if let Some(skill) = external_artifact_template_to_requested_skill(
                template,
                target_artifact_type.as_deref(),
            ) {
                message.requested_skills.push(skill);
            }
        }
    }

    if target_artifact_type
        .as_deref()
        .is_some_and(|value| value != "any")
        && message.render_mode.is_none()
    {
        message.render_mode = Some("artifact".to_string());
    }

    if target_artifact_type.as_deref() == Some("static_page") {
        if message.render_mode.is_none() {
            message.render_mode = Some("artifact".to_string());
        }
        if message.output_format.is_none() {
            message.output_format = Some("image_text".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message() -> ExternalBotMessageView {
        serde_json::from_value(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "conv-001",
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "生成经营分析静态页",
            "idempotency_key": "generic:tenant-ext-001:msg-001",
            "received_at": "2026-06-14T00:00:00Z"
        }))
        .expect("sample message")
    }

    #[test]
    fn normalizes_static_page_artifact_request() {
        let mut message = sample_message();
        message.artifact_type = Some(" Static Page ".to_string());

        validate_and_normalize_external_artifact_request(&mut message).expect("valid artifact");

        assert_eq!(message.artifact_type.as_deref(), Some("static_page"));
        assert_eq!(message.render_mode.as_deref(), Some("artifact"));
        assert_eq!(message.output_format.as_deref(), Some("image_text"));
    }

    #[test]
    fn injects_document_template_requested_skill() {
        let mut message = sample_message();
        message.template = Some(ExternalArtifactTemplateView {
            source_id: Some(" third-party-source-main ".to_string()),
            document_external_id: Some(" template-doc-001 ".to_string()),
            document_id: None,
            revision_external_id: Some(" v1 ".to_string()),
            output_type: Some(" report ".to_string()),
            mode: Some(" reference ".to_string()),
            template_reference_id: None,
            title: Some(" 周报模板 ".to_string()),
        });

        validate_and_normalize_external_artifact_request(&mut message).expect("valid template");

        assert_eq!(message.artifact_type.as_deref(), Some("report"));
        assert_eq!(
            message.available_document_source_id.as_deref(),
            Some("third-party-source-main")
        );
        assert_eq!(
            message.available_document_external_ids,
            vec!["template-doc-001".to_string()]
        );
        assert_eq!(message.requested_skills.len(), 1);
        let skill = &message.requested_skills[0];
        assert_eq!(skill.skill_id, "document_template_skill");
        assert_eq!(skill.version.as_deref(), Some("v1"));
        assert_eq!(skill.mode.as_deref(), Some("required"));
        let arguments = skill
            .arguments
            .as_ref()
            .and_then(Value::as_object)
            .expect("template skill arguments");
        assert_eq!(
            arguments
                .get("template_document_external_id")
                .and_then(Value::as_str),
            Some("template-doc-001")
        );
        assert_eq!(
            arguments.get("output_type").and_then(Value::as_str),
            Some("report")
        );
    }

    #[test]
    fn rejects_invalid_artifact_type() {
        let mut message = sample_message();
        message.artifact_type = Some("slideshare".to_string());

        assert!(validate_and_normalize_external_artifact_request(&mut message).is_err());
    }
}
