use serde_json::{Map, Value};

use crate::external_channel_static_page_source_ref_string;

pub(crate) fn external_channel_static_page_status_source_refs(source_refs: &Value) -> Value {
    let mut output = Map::new();
    for key in [
        "source",
        "local_thread_id",
        "local_draft_id",
        "channel_connection_id",
        "platform",
        "tenant_external_id",
        "bot_external_id",
        "conversation_external_id",
        "thread_external_id",
        "sender_external_id",
        "message_external_id",
        "output_format",
        "render_mode",
        "dataset_artifact_key",
    ] {
        if let Some(value) = external_channel_static_page_source_ref_string(source_refs, key) {
            output.insert(key.to_string(), Value::String(value));
        }
    }
    if let Some(value) = source_refs.get("recipient_delivery") {
        output.insert("recipient_delivery".to_string(), value.clone());
    }
    if let Some(value) = source_refs.get("artifact_stability") {
        output.insert("artifact_stability".to_string(), value.clone());
    }
    Value::Object(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_source_refs_keeps_safe_scope_fields_and_trims_strings() {
        let source_refs = json!({
            "source": " external_channel_static_page_artifact_request ",
            "local_thread_id": " thread-1 ",
            "channel_connection_id": " conn-1 ",
            "platform": " generic_chat ",
            "tenant_external_id": " tenant-1 ",
            "bot_external_id": " bot-1 ",
            "conversation_external_id": " conv-1 ",
            "thread_external_id": " ext-thread-1 ",
            "sender_external_id": " sender-1 ",
            "message_external_id": " msg-1 ",
            "output_format": " rich_text ",
            "render_mode": " normal ",
            "dataset_artifact_key": " dataset:key ",
            "recipient_delivery": {
                "mode": "external_channel"
            },
            "artifact_stability": {
                "default_template_scope": "dataset_combination"
            },
            "selected_scope": {"must": "not leak"},
            "available_document_external_ids": ["doc-1"]
        });

        let filtered = external_channel_static_page_status_source_refs(&source_refs);

        assert_eq!(
            filtered["source"],
            json!("external_channel_static_page_artifact_request")
        );
        assert_eq!(filtered["local_thread_id"], json!("thread-1"));
        assert_eq!(filtered["channel_connection_id"], json!("conn-1"));
        assert_eq!(filtered["platform"], json!("generic_chat"));
        assert_eq!(filtered["conversation_external_id"], json!("conv-1"));
        assert_eq!(filtered["message_external_id"], json!("msg-1"));
        assert_eq!(filtered["output_format"], json!("rich_text"));
        assert_eq!(filtered["render_mode"], json!("normal"));
        assert_eq!(filtered["dataset_artifact_key"], json!("dataset:key"));
        assert_eq!(
            filtered["recipient_delivery"]["mode"],
            json!("external_channel")
        );
        assert_eq!(
            filtered["artifact_stability"]["default_template_scope"],
            json!("dataset_combination")
        );
        assert!(filtered.get("selected_scope").is_none());
        assert!(filtered.get("available_document_external_ids").is_none());
    }

    #[test]
    fn status_source_refs_omits_empty_or_non_string_scope_fields() {
        let filtered = external_channel_static_page_status_source_refs(&json!({
            "conversation_external_id": "   ",
            "message_external_id": 123,
            "recipient_delivery": null,
            "artifact_stability": null
        }));

        assert!(filtered.get("conversation_external_id").is_none());
        assert!(filtered.get("message_external_id").is_none());
        assert_eq!(filtered["recipient_delivery"], Value::Null);
        assert_eq!(filtered["artifact_stability"], Value::Null);
    }
}
