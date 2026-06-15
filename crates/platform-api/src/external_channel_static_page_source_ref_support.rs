use serde_json::Value;

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
}
