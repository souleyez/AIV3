use contracts::ExternalBotMessageView;

use crate::{sha256_hex, text_normalization::non_empty_trimmed_string};

pub(crate) fn external_channel_temporary_dataset_key(
    connection_id: &str,
    message: &ExternalBotMessageView,
    source_id: Option<&str>,
) -> String {
    let mut parts = vec![
        "external".to_string(),
        "session".to_string(),
        external_channel_temporary_dataset_key_component(connection_id),
    ];
    parts.push(external_channel_temporary_dataset_key_component(
        &message.conversation_external_id,
    ));
    if let Some(source_id) = source_id.and_then(non_empty_trimmed_string) {
        parts.push(external_channel_temporary_dataset_key_component(&source_id));
    }
    compact_external_channel_temporary_dataset_key(parts.join("-"))
}

fn external_channel_temporary_dataset_key_component(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !slug.is_empty() && !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        return format!("ref-{}", &sha256_hex([value.as_bytes()])[..12]);
    }
    if slug.len() > 64 {
        let hash = sha256_hex([value.as_bytes()]);
        slug.truncate(48);
        while slug.ends_with('-') {
            slug.pop();
        }
        slug.push('-');
        slug.push_str(&hash[..12]);
    }
    slug
}

fn compact_external_channel_temporary_dataset_key(mut key: String) -> String {
    if key.len() <= 128 {
        return key;
    }
    let hash = sha256_hex([key.as_bytes()]);
    key.truncate(112);
    while key.ends_with('-') {
        key.pop();
    }
    key.push('-');
    key.push_str(&hash[..12]);
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_message(conversation_external_id: &str) -> ExternalBotMessageView {
        serde_json::from_value(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": conversation_external_id,
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "临时文档问答",
            "idempotency_key": "generic:tenant-ext-001:msg-001",
            "received_at": "2026-06-14T00:00:00Z"
        }))
        .expect("sample message")
    }

    #[test]
    fn temporary_dataset_key_uses_connection_conversation_and_source_slugs() {
        let message = sample_message(" Conv 51B7324D ");

        assert_eq!(
            external_channel_temporary_dataset_key(
                " Generic.Chat_Main ",
                &message,
                Some(" Source Main "),
            ),
            "external-session-generic-chat-main-conv-51b7324d-source-main"
        );
    }

    #[test]
    fn temporary_dataset_key_omits_empty_source_and_hashes_empty_slug_parts() {
        let message = sample_message("   ");
        let key = external_channel_temporary_dataset_key("系统", &message, Some("   "));

        assert!(key.starts_with("external-session-ref-"));
        assert_eq!(key.matches("-ref-").count(), 2);
        assert!(!key.ends_with('-'));
    }

    #[test]
    fn temporary_dataset_key_compacts_long_values_to_stable_limit() {
        let long = "a".repeat(220);
        let message = sample_message(&long);
        let key = external_channel_temporary_dataset_key(&long, &message, Some(&long));

        assert!(key.len() <= 128);
        assert!(key.starts_with("external-session-"));
        assert!(!key.ends_with('-'));
    }
}
