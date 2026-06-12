use contracts::{ExternalArtifactTemplateView, ExternalBotMessageView, ExternalRequestedSkillView};
use serde_json::{json, Value};

use crate::{
    external_bot_message_requested_dataset_external_ids,
    external_channel_support::{
        external_channel_platform_wire_value, external_message_type_wire_value,
    },
    external_requested_skill_mode, sha256_hex,
};

pub(crate) fn requested_skills_summary(skills: &[ExternalRequestedSkillView]) -> Vec<Value> {
    skills
        .iter()
        .map(|skill| {
            let argument_keys = skill
                .arguments
                .as_ref()
                .and_then(Value::as_object)
                .map(|object| object.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            json!({
                "skill_id": skill.skill_id.as_str(),
                "version": skill.version.as_deref(),
                "mode": external_requested_skill_mode(skill),
                "argument_keys": argument_keys,
            })
        })
        .collect()
}

pub(crate) fn bot_message_payload_summary(message: &ExternalBotMessageView) -> Value {
    json!({
        "platform": external_channel_platform_wire_value(&message.platform),
        "tenant_external_id": message.tenant_external_id,
        "bot_external_id": message.bot_external_id,
        "conversation_external_id": message.conversation_external_id,
        "thread_external_id": message.thread_external_id,
        "sender_external_id": message.sender_external_id,
        "message_external_id": message.message_external_id,
        "message_type": external_message_type_wire_value(&message.message_type),
        "text_chars": message.text.as_ref().map(|text| text.chars().count()).unwrap_or(0),
        "text_fingerprint": message.text.as_deref().map(message_text_fingerprint),
        "default_prompt_chars": message.default_prompt.as_ref().map(|text| text.chars().count()).unwrap_or(0),
        "default_prompt_fingerprint": message.default_prompt.as_deref().map(message_text_fingerprint),
        "output_format": message.output_format,
        "render_mode": message.render_mode,
        "artifact_type": message.artifact_type,
        "template": message.template.as_ref().map(artifact_template_summary),
        "mention_count": message.mention_external_user_ids.len(),
        "attachment_count": message.attachment_refs.len(),
        "available_document_count": message.available_document_external_ids.len(),
        "available_document_source_id": message.available_document_source_id,
        "dataset_external_id": message.dataset_external_id,
        "dataset_external_ids": message.dataset_external_ids,
        "dataset_external_count": external_bot_message_requested_dataset_external_ids(message).len(),
        "requested_skill_count": message.requested_skills.len(),
        "requested_skills": requested_skills_summary(&message.requested_skills),
        "attachments": message.attachment_refs.iter().map(|attachment| json!({
            "attachment_external_id": attachment.attachment_external_id,
            "filename": attachment.filename,
            "content_type": attachment.content_type,
            "size_bytes": attachment.size_bytes,
            "download_url_redacted": attachment.download_url_redacted.as_ref().map(|_| "[redacted]"),
            "download_url_fingerprint": attachment
                .download_url_redacted
                .as_deref()
                .map(attachment_download_url_fingerprint),
        })).collect::<Vec<_>>(),
        "received_at": message.received_at,
    })
}

pub(crate) fn artifact_template_summary(template: &ExternalArtifactTemplateView) -> Value {
    json!({
        "source_id": template.source_id,
        "document_external_id": template.document_external_id,
        "document_id": template.document_id,
        "revision_external_id": template.revision_external_id,
        "output_type": template.output_type,
        "mode": template.mode,
        "template_reference_id": template.template_reference_id,
        "title": template.title,
    })
}

fn message_text_fingerprint(value: &str) -> String {
    let hash = sha256_hex([value.trim().as_bytes()]);
    format!("sha256:{}", &hash[..16])
}

fn attachment_download_url_fingerprint(value: &str) -> String {
    let hash = sha256_hex([value.trim().as_bytes()]);
    format!("sha256:{}", &hash[..16])
}
