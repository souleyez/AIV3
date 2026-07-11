use serde_json::{json, Value};
use std::fmt::Write as _;

use crate::sha256_hex;

pub(crate) fn external_message_payload_summaries_match_except_attachments(
    left: &Value,
    right: &Value,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    if let Some(object) = left.as_object_mut() {
        object.remove("attachments");
        object.remove("received_at");
    }
    if let Some(object) = right.as_object_mut() {
        object.remove("attachments");
        object.remove("received_at");
    }
    left == right
}

pub(crate) fn external_message_payload_summaries_match_for_idempotency(
    left: &Value,
    right: &Value,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    if let Some(object) = left.as_object_mut() {
        object.remove("received_at");
    }
    if let Some(object) = right.as_object_mut() {
        object.remove("received_at");
    }
    left == right
}

pub(crate) fn external_channel_image_variant_idempotency_key(
    original_idempotency_key: &str,
    payload_summary: &Value,
) -> Option<String> {
    let fingerprint = external_channel_image_variant_fingerprint(payload_summary)?;
    let hash = sha256_hex([
        original_idempotency_key.trim().as_bytes(),
        b":image:",
        fingerprint.as_bytes(),
    ]);
    Some(format!(
        "{}:image:{}",
        original_idempotency_key.trim(),
        &hash[..16]
    ))
}

pub(crate) fn external_channel_image_variant_fingerprint(
    payload_summary: &Value,
) -> Option<String> {
    let attachments = payload_summary.get("attachments")?.as_array()?;
    if attachments.is_empty() {
        return None;
    }
    let parts = attachments
        .iter()
        .map(|attachment| {
            json!({
                "attachment_external_id": attachment.get("attachment_external_id").cloned().unwrap_or(Value::Null),
                "filename": attachment.get("filename").cloned().unwrap_or(Value::Null),
                "content_type": attachment.get("content_type").cloned().unwrap_or(Value::Null),
                "size_bytes": attachment.get("size_bytes").cloned().unwrap_or(Value::Null),
                "download_url_fingerprint": attachment.get("download_url_fingerprint").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let serialized = serde_json::to_string(&parts).ok()?;
    Some(sha256_hex([serialized.as_bytes()]))
}

pub(crate) fn external_asset_import_request_fingerprint(payload: &Value) -> String {
    let mut canonical = String::new();
    write_canonical_json(payload, &mut canonical);
    sha256_hex([canonical.as_bytes()])
}

pub(crate) fn external_asset_import_source_id(
    connection_id: &str,
    source_id: &str,
    external_id: Option<&str>,
    object_key: Option<&str>,
    image_url: Option<&str>,
) -> Option<String> {
    let identity = [external_id, object_key, image_url]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())?;
    let digest = sha256_hex([
        connection_id.trim().as_bytes(),
        b"\0",
        source_id.trim().as_bytes(),
        b"\0",
        identity.as_bytes(),
    ]);
    Some(format!("external-asset:{digest}"))
}

fn write_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Object(object) => {
            output.push('{');
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                let _ = write!(
                    output,
                    "{}:",
                    serde_json::to_string(key).unwrap_or_default()
                );
                write_canonical_json(&object[key], output);
            }
            output.push('}');
        }
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical_json(item, output);
            }
            output.push(']');
        }
        _ => output.push_str(&serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn payload_summary_match_for_idempotency_ignores_received_at_only() {
        let left = json!({
            "message_external_id": "msg-1",
            "text_chars": 12,
            "received_at": "2026-07-08T00:00:00Z",
            "attachments": [{"filename": "a.png"}]
        });
        let mut right = left.clone();
        right["received_at"] = json!("2026-07-08T00:01:00Z");
        assert!(external_message_payload_summaries_match_for_idempotency(
            &left, &right
        ));

        right["attachments"] = json!([{"filename": "b.png"}]);
        assert!(!external_message_payload_summaries_match_for_idempotency(
            &left, &right
        ));
    }

    #[test]
    fn payload_summary_match_except_attachments_ignores_attachment_changes() {
        let left = json!({
            "message_external_id": "msg-1",
            "text_chars": 12,
            "received_at": "2026-07-08T00:00:00Z",
            "attachments": [{"filename": "a.png"}]
        });
        let mut right = left.clone();
        right["received_at"] = json!("2026-07-08T00:01:00Z");
        right["attachments"] = json!([{"filename": "b.png"}]);
        assert!(external_message_payload_summaries_match_except_attachments(
            &left, &right
        ));

        right["text_chars"] = json!(13);
        assert!(!external_message_payload_summaries_match_except_attachments(&left, &right));
    }

    #[test]
    fn image_variant_fingerprint_uses_stable_attachment_fields() {
        let payload = json!({
            "attachments": [{
                "attachment_external_id": "img-1",
                "filename": "order.png",
                "content_type": "image/png",
                "size_bytes": 2048,
                "download_url_fingerprint": "sha256:abc",
                "download_url_redacted": "[redacted]"
            }]
        });
        let mut changed_redaction = payload.clone();
        changed_redaction["attachments"][0]["download_url_redacted"] =
            json!("https://example.com/private.png");

        assert_eq!(
            external_channel_image_variant_fingerprint(&payload),
            external_channel_image_variant_fingerprint(&changed_redaction)
        );
        assert!(external_channel_image_variant_fingerprint(&json!({
            "attachments": []
        }))
        .is_none());
    }

    #[test]
    fn image_variant_idempotency_key_is_derived_from_trimmed_key_and_fingerprint() {
        let payload = json!({
            "attachments": [{
                "attachment_external_id": "img-1",
                "filename": "order.png",
                "content_type": "image/png",
                "size_bytes": 2048,
                "download_url_fingerprint": "sha256:abc"
            }]
        });

        let key = external_channel_image_variant_idempotency_key(" idem-1 ", &payload)
            .expect("attachment fingerprint should produce key");

        assert!(key.starts_with("idem-1:image:"));
        assert_eq!(key.len(), "idem-1:image:".len() + 16);
        assert!(external_channel_image_variant_idempotency_key("idem-1", &json!({})).is_none());
    }

    #[test]
    fn external_asset_import_fingerprint_is_object_order_independent() {
        let left = json!({"request_id": "req-1", "metadata": {"b": 2, "a": 1}});
        let right: Value =
            serde_json::from_str(r#"{"metadata":{"a":1,"b":2},"request_id":"req-1"}"#).unwrap();
        assert_eq!(
            external_asset_import_request_fingerprint(&left),
            external_asset_import_request_fingerprint(&right)
        );
    }

    #[test]
    fn external_asset_source_id_is_stable_and_connection_isolated() {
        let first = external_asset_import_source_id(
            "connection-a",
            "source-main",
            Some("asset-1"),
            Some("objects/first.png"),
            None,
        )
        .unwrap();
        let replacement = external_asset_import_source_id(
            "connection-a",
            "source-main",
            Some("asset-1"),
            Some("objects/replacement.png"),
            None,
        )
        .unwrap();
        let other_connection = external_asset_import_source_id(
            "connection-b",
            "source-main",
            Some("asset-1"),
            Some("objects/first.png"),
            None,
        )
        .unwrap();

        assert_eq!(first, replacement);
        assert_ne!(first, other_connection);
        assert!(first.starts_with("external-asset:"));
    }
}
