use chrono::{DateTime, Utc};
#[cfg(test)]
use contracts::ExternalChannelPlatformView;
use serde_json::{json, Value};

use crate::text_normalization::non_empty_trimmed_string;
use crate::{
    ensure_json_object, external_channel_platform_wire_value, set_payload_string,
    set_payload_value, sha256_hex, ExternalChannelConnectionSummary,
};

pub(crate) fn normalize_external_bot_message_payload(
    mut payload: Value,
    connection: &ExternalChannelConnectionSummary,
) -> Value {
    ensure_json_object(&mut payload);
    external_payload_copy_aliases(
        &mut payload,
        &[
            ("tenantExternalId", "tenant_external_id"),
            ("botExternalId", "bot_external_id"),
            ("conversationExternalId", "conversation_external_id"),
            ("threadExternalId", "thread_external_id"),
            ("senderExternalId", "sender_external_id"),
            ("messageExternalId", "message_external_id"),
            ("messageType", "message_type"),
            ("defaultPrompt", "default_prompt"),
            ("systemPrompt", "default_prompt"),
            ("system_prompt", "default_prompt"),
            ("outputFormat", "output_format"),
            ("answerFormat", "output_format"),
            ("answer_format", "output_format"),
            ("replyFormat", "output_format"),
            ("reply_format", "output_format"),
            ("renderMode", "render_mode"),
            ("responseMode", "render_mode"),
            ("response_mode", "render_mode"),
            ("artifactType", "artifact_type"),
            ("outputType", "artifact_type"),
            ("output_type", "artifact_type"),
            ("artifactTemplate", "template"),
            ("artifact_template", "template"),
            ("templateRef", "template"),
            ("template_ref", "template"),
            ("mentionExternalUserIds", "mention_external_user_ids"),
            ("attachmentRefs", "attachment_refs"),
            ("businessDatasourceIds", "business_datasource_ids"),
            ("businessDataSourceIds", "business_datasource_ids"),
            ("business_data_source_ids", "business_datasource_ids"),
            ("businessDatabaseSourceIds", "business_datasource_ids"),
            ("business_database_source_ids", "business_datasource_ids"),
            ("databaseSourceIds", "business_datasource_ids"),
            ("database_source_ids", "business_datasource_ids"),
            (
                "availableDocumentExternalIds",
                "available_document_external_ids",
            ),
            (
                "availableDocumentExternalId",
                "available_document_external_id",
            ),
            ("documentExternalIds", "document_external_ids"),
            ("documentExternalId", "document_external_id"),
            ("availableDocumentSourceId", "available_document_source_id"),
            ("datasetExternalId", "dataset_external_id"),
            ("availableDatasetExternalId", "dataset_external_id"),
            ("datasetExternalIds", "dataset_external_ids"),
            ("availableDatasetExternalIds", "dataset_external_ids"),
            ("requestedSkills", "requested_skills"),
            ("skillRefs", "requested_skills"),
            ("skill_refs", "requested_skills"),
            ("idempotencyKey", "idempotency_key"),
            ("receivedAt", "received_at"),
        ],
    );
    if payload.get("platform").and_then(Value::as_str).is_none() {
        set_payload_string(
            &mut payload,
            "platform",
            external_channel_platform_wire_value(&connection.platform),
        );
    }
    if payload
        .get("message_type")
        .and_then(Value::as_str)
        .is_none()
    {
        set_payload_string(&mut payload, "message_type", "text");
    }
    normalize_external_payload_string_case(&mut payload, "platform");
    normalize_external_payload_string_case(&mut payload, "message_type");
    if payload
        .get("received_at")
        .and_then(Value::as_str)
        .is_none_or(|value| DateTime::parse_from_rfc3339(value.trim()).is_err())
    {
        set_payload_value(&mut payload, "received_at", json!(Utc::now()));
    }
    if payload
        .get("idempotency_key")
        .and_then(Value::as_str)
        .is_none()
    {
        if let Some(message_id) = payload
            .get("message_external_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        {
            let tenant = payload
                .get("tenant_external_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown")
                .to_string();
            set_payload_string(
                &mut payload,
                "idempotency_key",
                &format!(
                    "{}:{}:{}",
                    external_channel_platform_wire_value(&connection.platform),
                    tenant,
                    message_id
                ),
            );
        }
    }
    normalize_external_document_scope_payload(&mut payload);
    normalize_external_attachment_refs_payload(&mut payload);
    payload
}

fn external_payload_copy_aliases(payload: &mut Value, aliases: &[(&str, &str)]) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    for (alias, canonical) in aliases {
        if object.contains_key(*canonical) {
            object.remove(*alias);
            continue;
        }
        if let Some(value) = object.remove(*alias) {
            object.insert((*canonical).to_string(), value);
        }
    }
}

fn normalize_external_document_scope_payload(payload: &mut Value) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let mut ids = Vec::new();
    for key in [
        "available_document_external_ids",
        "available_document_external_id",
        "document_external_ids",
        "document_external_id",
    ] {
        if let Some(value) = object.remove(key) {
            ids.extend(external_string_ids_from_payload_value(value));
        }
    }
    if !ids.is_empty() {
        object.insert(
            "available_document_external_ids".to_string(),
            Value::Array(ids.into_iter().map(Value::String).collect()),
        );
    }

    let mut dataset_ids = Vec::new();
    for key in [
        "dataset_external_ids",
        "available_dataset_external_ids",
        "dataset_external_id_list",
        "available_dataset_external_id_list",
    ] {
        if let Some(value) = object.remove(key) {
            dataset_ids.extend(external_string_ids_from_payload_value(value));
        }
    }
    if !dataset_ids.is_empty() {
        object.insert(
            "dataset_external_ids".to_string(),
            Value::Array(dataset_ids.into_iter().map(Value::String).collect()),
        );
    }

    let mut business_datasource_ids = Vec::new();
    for key in [
        "business_datasource_ids",
        "business_data_source_ids",
        "business_database_source_ids",
        "database_source_ids",
    ] {
        if let Some(value) = object.remove(key) {
            business_datasource_ids.extend(external_string_ids_from_payload_value(value));
        }
    }
    if !business_datasource_ids.is_empty() {
        object.insert(
            "business_datasource_ids".to_string(),
            Value::Array(
                business_datasource_ids
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
}

fn normalize_external_attachment_refs_payload(payload: &mut Value) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let Some(value) = object.get_mut("attachment_refs") else {
        return;
    };
    let items = match value {
        Value::Array(items) => items,
        Value::String(_) => {
            let raw = std::mem::take(value);
            *value = Value::Array(vec![raw]);
            value
                .as_array_mut()
                .expect("attachment_refs just became array")
        }
        _ => return,
    };
    let mut normalized = Vec::new();
    for item in std::mem::take(items) {
        match item {
            Value::String(text) => {
                let Some(url) = non_empty_trimmed_string(&text) else {
                    continue;
                };
                let filename = external_attachment_filename_from_url(&url);
                normalized.push(json!({
                    "attachment_external_id": external_attachment_id_from_url(&url),
                    "filename": filename,
                    "download_url_redacted": url,
                }));
            }
            Value::Object(mut object) => {
                if !object.contains_key("attachment_external_id") {
                    if let Some(value) = object
                        .get("attachmentExternalId")
                        .and_then(Value::as_str)
                        .and_then(non_empty_trimmed_string)
                    {
                        object.insert("attachment_external_id".to_string(), json!(value));
                    }
                }
                if !object.contains_key("download_url_redacted") {
                    if let Some(value) = object
                        .get("downloadUrl")
                        .or_else(|| object.get("download_url"))
                        .or_else(|| object.get("url"))
                        .and_then(Value::as_str)
                        .and_then(non_empty_trimmed_string)
                    {
                        object.insert("download_url_redacted".to_string(), json!(value));
                    }
                }
                if !object.contains_key("attachment_external_id") {
                    if let Some(value) = object
                        .get("download_url_redacted")
                        .and_then(Value::as_str)
                        .and_then(non_empty_trimmed_string)
                    {
                        object.insert(
                            "attachment_external_id".to_string(),
                            json!(external_attachment_id_from_url(&value)),
                        );
                    }
                }
                normalized.push(Value::Object(object));
            }
            other => normalized.push(other),
        }
    }
    *value = Value::Array(normalized);
}

fn external_attachment_id_from_url(url: &str) -> String {
    let hash = sha256_hex([url.as_bytes()]);
    format!("url-{}", &hash[..16])
}

fn external_attachment_filename_from_url(url: &str) -> Option<String> {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    without_query
        .rsplit('/')
        .next()
        .and_then(non_empty_trimmed_string)
}

pub(crate) fn external_string_ids_from_payload_value(value: Value) -> Vec<String> {
    let raw_values = match value {
        Value::Array(items) => items,
        other => vec![other],
    };
    let mut ids = Vec::new();
    for value in raw_values {
        let Some(id) = value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        else {
            continue;
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

pub(crate) fn normalize_external_payload_string_case(payload: &mut Value, key: &str) {
    let Some(value) = payload.get(key).and_then(Value::as_str) else {
        return;
    };
    let compact = value
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    let normalized = match (key, compact.as_str()) {
        ("platform", "genericchat") => "generic_chat".to_string(),
        ("platform", "aigolf") => "generic_chat".to_string(),
        ("platform", "thirdparty") => "third_party".to_string(),
        ("platform", "wecom") => "we_com".to_string(),
        ("message_type", "unknown") => "unknown".to_string(),
        (_, value) => value.to_string(),
    };
    set_payload_string(payload, key, &normalized);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_scope_aliases_attachment_urls_and_idempotency() {
        let connection = ExternalChannelConnectionSummary {
            platform: ExternalChannelPlatformView::GenericChat,
            status: "enabled".to_string(),
            config_redacted: json!({}),
        };
        let normalized = normalize_external_bot_message_payload(
            json!({
                "tenantExternalId": "tenant-ext-001",
                "botExternalId": "bot-v3",
                "conversationExternalId": "conv-001",
                "senderExternalId": "user-001",
                "messageExternalId": "msg-001",
                "messageType": "Text",
                "documentExternalId": "doc-001",
                "availableDocumentExternalIds": ["doc-002", "doc-001"],
                "availableDatasetExternalIds": ["dataset-a", "dataset-b", "dataset-a"],
                "businessDatabaseSourceIds": ["db-main", "db-main"],
                "attachmentRefs": ["https://third.example.com/files/report.xlsx?token=hidden"],
                "requestedSkills": [{"skillId": "risk_review"}],
            }),
            &connection,
        );

        assert_eq!(normalized["platform"], json!("generic_chat"));
        assert_eq!(normalized["message_type"], json!("text"));
        assert_eq!(
            normalized["idempotency_key"],
            json!("generic_chat:tenant-ext-001:msg-001")
        );
        assert_eq!(
            normalized["available_document_external_ids"],
            json!(["doc-002", "doc-001", "doc-001"])
        );
        assert_eq!(
            normalized["dataset_external_ids"],
            json!(["dataset-a", "dataset-b"])
        );
        assert_eq!(normalized["business_datasource_ids"], json!(["db-main"]));
        assert_eq!(
            normalized["requested_skills"][0]["skillId"],
            json!("risk_review")
        );
        assert!(normalized["attachment_refs"][0]["attachment_external_id"]
            .as_str()
            .expect("attachment id")
            .starts_with("url-"));
        assert_eq!(
            normalized["attachment_refs"][0]["filename"],
            json!("report.xlsx")
        );
        assert_eq!(
            normalized["attachment_refs"][0]["download_url_redacted"],
            json!("https://third.example.com/files/report.xlsx?token=hidden")
        );
    }
}
