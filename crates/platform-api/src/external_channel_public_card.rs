use contracts::ExternalBotReplyView;
use serde_json::Value;

use crate::{
    external_channel_public_status, external_channel_public_task_status,
    external_channel_public_text, external_channel_static_page_accepted_template_baseline,
    external_channel_static_page_cancelled_should_continue,
    external_channel_static_page_preview_public_url,
    external_channel_static_page_provisional_existing_artifact,
};

pub(crate) fn external_channel_public_reply_task_status(
    task_status: &str,
    card: Option<&Value>,
) -> String {
    if external_channel_static_page_cancelled_should_continue(task_status, card) {
        return "processing".to_string();
    }
    external_channel_public_task_status(task_status).to_string()
}

pub(crate) fn external_channel_reply_static_page_card_status(
    reply: &ExternalBotReplyView,
) -> Option<&str> {
    reply
        .card
        .as_ref()
        .and_then(|card| card.get("status"))
        .and_then(Value::as_str)
}

pub(crate) fn external_channel_public_status_allows_artifact_link(status: &str) -> bool {
    matches!(
        external_channel_public_status(status).as_str(),
        "static_page_published" | "static_page_stable_artifact_reused"
    )
}

pub(crate) fn external_channel_public_status_allows_artifact_link_for_card(
    status: &str,
    card: Option<&Value>,
) -> bool {
    if external_channel_static_page_accepted_template_baseline(card) {
        return true;
    }
    if external_channel_static_page_provisional_existing_artifact(card)
        && !external_channel_static_page_accepted_template_baseline(card)
        && external_channel_public_status(status) != "static_page_stable_artifact_reused"
    {
        return false;
    }
    external_channel_public_status_allows_artifact_link(status)
}

pub(crate) fn external_channel_public_status_allows_preview_link(status: &str) -> bool {
    external_channel_public_status(status) == "static_page_preview_ready"
}

pub(crate) fn external_channel_reply_public_status(reply: &ExternalBotReplyView) -> Option<String> {
    external_channel_reply_static_page_card_status(reply)
        .or(reply.task_status.as_deref())
        .map(|status| external_channel_public_reply_task_status(status, reply.card.as_ref()))
}

pub(crate) fn external_channel_reply_is_static_page_like(reply: &ExternalBotReplyView) -> bool {
    reply
        .card
        .as_ref()
        .and_then(|card| card.get("type"))
        .and_then(Value::as_str)
        .map(|card_type| card_type.contains("static_page"))
        .unwrap_or(false)
        || external_channel_reply_static_page_card_status(reply)
            .or(reply.task_status.as_deref())
            .map(|status| status.starts_with("static_page_"))
            .unwrap_or(false)
}

pub(crate) fn prune_external_channel_public_card_links(
    value: &mut Value,
    include_artifact_link: bool,
    include_preview_link: bool,
) {
    match value {
        Value::Object(map) => {
            let remove_keys = [
                "artifact_links",
                "artifact_public_url",
                "data_url",
                "download_url",
                "generated_artifact_url",
                "html_download_url",
                "html_preview_url",
                "render_asset_url",
            ];
            for key in remove_keys {
                map.remove(key);
            }
            if !include_artifact_link {
                map.remove("public_url");
            }
            if !include_preview_link {
                map.remove("preview_url");
            }
            for value in map.values_mut() {
                prune_external_channel_public_card_links(
                    value,
                    include_artifact_link,
                    include_preview_link,
                );
            }
        }
        Value::Array(items) => {
            for value in items {
                prune_external_channel_public_card_links(
                    value,
                    include_artifact_link,
                    include_preview_link,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn external_channel_public_card_value(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            if !map.contains_key("preview_url") {
                if let Some(preview_url) = map
                    .get("preview_asset_key")
                    .and_then(Value::as_str)
                    .and_then(external_channel_static_page_preview_public_url)
                {
                    map.insert("preview_url".to_string(), Value::String(preview_url));
                }
            }
            let mut output = serde_json::Map::new();
            for (key, value) in map {
                let key_lc = key.to_ascii_lowercase();
                if key_lc.contains("codex")
                    || key_lc.contains("image_job")
                    || key_lc.contains("effect_image")
                    || key_lc.contains("fixed_task")
                    || key_lc.contains("preview_asset")
                    || key_lc.contains("runtime")
                    || key_lc.contains("manifest")
                    || key_lc.contains("evidence")
                    || key_lc.contains("prompt")
                    || key_lc.contains("snapshot")
                    || key_lc.contains("module")
                    || key_lc == "data"
                    || key_lc == "debug"
                    || key_lc == "output"
                    || key_lc == "validation"
                    || key_lc == "source_refs"
                    || key_lc == "auto_publish_after_preview"
                    || key_lc == "asset_provenance"
                {
                    continue;
                }
                let sanitized = if key == "status" {
                    value
                        .as_str()
                        .map(|status| Value::String(external_channel_public_status(status)))
                        .unwrap_or_else(|| external_channel_public_card_value(value))
                } else if key == "type" {
                    value
                        .as_str()
                        .map(|kind| {
                            if kind.contains("static_page") {
                                Value::String("v3_static_page_pipeline".to_string())
                            } else {
                                Value::String(external_channel_public_text(kind))
                            }
                        })
                        .unwrap_or_else(|| external_channel_public_card_value(value))
                } else {
                    external_channel_public_card_value(value)
                };
                output.insert(key, sanitized);
            }
            Value::Object(output)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(external_channel_public_card_value)
                .collect(),
        ),
        Value::String(value) => Value::String(external_channel_public_text(&value)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ExternalBotReplyTypeView;
    use serde_json::json;

    fn reply_with_card_and_status(
        card: Option<Value>,
        task_status: Option<&str>,
    ) -> ExternalBotReplyView {
        ExternalBotReplyView {
            target_conversation_external_id: "room-1".to_string(),
            reply_type: ExternalBotReplyTypeView::TaskStatus,
            text: None,
            card,
            artifact_links: Vec::new(),
            task_status: task_status.map(ToOwned::to_owned),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        }
    }

    #[test]
    fn public_card_value_strips_internal_fields_and_maps_static_page_type() {
        let value = external_channel_public_card_value(json!({
            "type": "v3_static_page_image2_pipeline",
            "status": "static_page_effect_image_ready",
            "title": "测试报表",
            "codex_internal": "hidden",
            "source_refs": {"must": "hide"},
            "nested": {
                "prompt": "hide",
                "label": "Cloudflare Codex 已完成"
            }
        }));

        assert_eq!(value["type"], json!("v3_static_page_pipeline"));
        assert_eq!(value["status"], json!("static_page_preview_ready"));
        assert_eq!(value["title"], json!("测试报表"));
        assert!(value.get("codex_internal").is_none());
        assert!(value.get("source_refs").is_none());
        assert!(value["nested"].get("prompt").is_none());
        assert_eq!(value["nested"]["label"], json!("DataMax 后台 已完成"));
    }

    #[test]
    fn prune_public_card_links_keeps_only_allowed_link_classes() {
        let mut value = json!({
            "public_url": "https://v3.elepcloud.com/generated-artifacts/a/index.html",
            "preview_url": "https://v3.elepcloud.com/generated-artifacts/a/preview.png",
            "generated_artifact_url": "https://v3.elepcloud.com/generated-artifacts/a/index.html",
            "nested": {
                "public_url": "https://v3.elepcloud.com/generated-artifacts/b/index.html",
                "preview_url": "https://v3.elepcloud.com/generated-artifacts/b/preview.png",
                "download_url": "https://v3.elepcloud.com/generated-artifacts/b/index.html"
            }
        });

        prune_external_channel_public_card_links(&mut value, true, false);

        assert!(value.get("public_url").is_some());
        assert!(value.get("preview_url").is_none());
        assert!(value.get("generated_artifact_url").is_none());
        assert!(value["nested"].get("public_url").is_some());
        assert!(value["nested"].get("preview_url").is_none());
        assert!(value["nested"].get("download_url").is_none());
    }

    #[test]
    fn accepted_template_baseline_allows_artifact_link() {
        let card = json!({
            "provisional_existing_artifact": true,
            "provisional_existing_artifact_reason": "accepted_dataset_overlap_template_baseline",
            "public_url": "https://v3.elepcloud.com/generated-artifacts/reports/current/index.html"
        });

        assert!(
            external_channel_public_status_allows_artifact_link_for_card(
                "static_page_publish_running",
                Some(&card),
            )
        );
    }

    #[test]
    fn reply_is_static_page_like_reads_card_type_and_status() {
        assert!(external_channel_reply_is_static_page_like(
            &reply_with_card_and_status(
                Some(json!({"type": "v3_static_page_image2_pipeline"})),
                None,
            )
        ));
        assert!(external_channel_reply_is_static_page_like(
            &reply_with_card_and_status(None, Some("static_page_published"),)
        ));
        assert!(external_channel_reply_is_static_page_like(
            &reply_with_card_and_status(Some(json!({"status": "static_page_preview_ready"})), None,)
        ));
        assert!(!external_channel_reply_is_static_page_like(
            &reply_with_card_and_status(
                Some(json!({"type": "normal_task", "status": "processing"})),
                Some("processing"),
            )
        ));
    }
}
