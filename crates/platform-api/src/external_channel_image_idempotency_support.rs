use serde_json::{json, Value};

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
}
