use serde_json::Value;

use crate::external_bot_message_payload_support::external_string_ids_from_payload_value;

pub(crate) fn external_channel_static_page_source_ref_string(
    source_refs: &Value,
    key: &str,
) -> Option<String> {
    source_refs
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn external_channel_static_page_source_refs_string_array(
    source_refs: &Value,
    key: &str,
) -> Vec<String> {
    source_refs
        .get(key)
        .map(|value| external_string_ids_from_payload_value(value.clone()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_ref_string_trims_non_empty_strings() {
        let source_refs = json!({
            "channel_connection_id": "  generic-chat-main  "
        });

        assert_eq!(
            external_channel_static_page_source_ref_string(&source_refs, "channel_connection_id")
                .as_deref(),
            Some("generic-chat-main")
        );
    }

    #[test]
    fn source_ref_string_rejects_blank_missing_and_non_string_values() {
        let source_refs = json!({
            "blank": "   ",
            "number": 42,
            "array": ["x"]
        });

        assert!(external_channel_static_page_source_ref_string(&source_refs, "blank").is_none());
        assert!(external_channel_static_page_source_ref_string(&source_refs, "missing").is_none());
        assert!(external_channel_static_page_source_ref_string(&source_refs, "number").is_none());
        assert!(external_channel_static_page_source_ref_string(&source_refs, "array").is_none());
    }

    #[test]
    fn source_refs_string_array_normalizes_string_and_array_values() {
        let source_refs = json!({
            "single": " doc-one ",
            "many": ["doc-two", "  ", 9, "doc-three"]
        });

        assert_eq!(
            external_channel_static_page_source_refs_string_array(&source_refs, "single"),
            vec!["doc-one"]
        );
        assert_eq!(
            external_channel_static_page_source_refs_string_array(&source_refs, "many"),
            vec!["doc-two", "doc-three"]
        );
    }

    #[test]
    fn source_refs_string_array_returns_empty_for_missing_or_blank_values() {
        let source_refs = json!({
            "blank": "   ",
            "nullish": null
        });

        assert!(
            external_channel_static_page_source_refs_string_array(&source_refs, "blank").is_empty()
        );
        assert!(
            external_channel_static_page_source_refs_string_array(&source_refs, "nullish")
                .is_empty()
        );
        assert!(
            external_channel_static_page_source_refs_string_array(&source_refs, "missing")
                .is_empty()
        );
    }
}
