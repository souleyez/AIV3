use contracts::{ExternalBotReplyTypeView, ExternalBotReplyView};
use serde_json::{json, Value};

use crate::{
    codex_host_fixed_task_public_artifact_url_allowed,
    external_channel_public_artifact_url_from_reply,
    external_channel_public_status_allows_artifact_link_for_card,
    external_channel_text_with_public_artifact_link, static_page_public_url_matches_ignoring_focus,
};

pub(crate) fn external_channel_reply_with_static_page_artifact_links(
    mut reply: ExternalBotReplyView,
    static_page_reply: Option<&ExternalBotReplyView>,
) -> ExternalBotReplyView {
    let Some(static_page_reply) = static_page_reply else {
        return reply;
    };
    let static_page_public_url = external_channel_public_artifact_url_from_reply(static_page_reply);
    let reply_card_allows_artifact = reply
        .card
        .as_ref()
        .map(|card| {
            let status = card
                .get("status")
                .and_then(Value::as_str)
                .or(reply.task_status.as_deref())
                .unwrap_or_default();
            external_channel_public_status_allows_artifact_link_for_card(status, Some(card))
        })
        .unwrap_or(false);
    if reply.card.is_none() || (static_page_public_url.is_some() && !reply_card_allows_artifact) {
        reply.card = static_page_reply.card.clone();
        if reply.reply_type != ExternalBotReplyTypeView::Text {
            reply.reply_type = static_page_reply.reply_type.clone();
            reply.task_status = static_page_reply.task_status.clone();
        }
    }
    let mut first_public_link: Option<String> = None;
    for link in &static_page_reply.artifact_links {
        let link = link.trim();
        if link.is_empty() || !codex_host_fixed_task_public_artifact_url_allowed(link) {
            continue;
        }
        if first_public_link.is_none() {
            first_public_link = Some(link.to_string());
        }
    }
    if let Some(public_url) = first_public_link.as_deref() {
        reply.artifact_links.retain(|existing| {
            !static_page_public_url_matches_ignoring_focus(existing, public_url)
        });
        reply.artifact_links.insert(0, public_url.to_string());
        if let Some(Value::Object(card)) = reply.card.as_mut() {
            for key in [
                "public_url",
                "generated_artifact_url",
                "download_url",
                "html_download_url",
            ] {
                card.insert(key.to_string(), Value::String(public_url.to_string()));
            }
            card.insert("artifact_links".to_string(), json!([public_url]));
        }
        if let Some(text) = reply.text.take() {
            reply.text = Some(external_channel_text_with_public_artifact_link(
                text, public_url,
            ));
        } else {
            reply.text = static_page_reply.text.clone();
        }
    }
    reply
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reply_with_text(text: Option<&str>) -> ExternalBotReplyView {
        ExternalBotReplyView {
            target_conversation_external_id: "room-1".to_string(),
            reply_type: ExternalBotReplyTypeView::Text,
            text: text.map(ToOwned::to_owned),
            card: None,
            artifact_links: Vec::new(),
            task_status: Some("answered".to_string()),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        }
    }

    fn static_page_reply(public_url: &str) -> ExternalBotReplyView {
        ExternalBotReplyView {
            target_conversation_external_id: "room-1".to_string(),
            reply_type: ExternalBotReplyTypeView::ArtifactLink,
            text: Some("已依据客户需求生成可访问的报表页面。".to_string()),
            card: Some(json!({
                "type": "v3_static_page_image2_publish_completed",
                "status": "static_page_published",
                "public_url": public_url,
                "generated_artifact_url": public_url,
                "artifact_links": [public_url]
            })),
            artifact_links: vec![public_url.to_string()],
            task_status: Some("static_page_published".to_string()),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        }
    }

    #[test]
    fn merge_appends_static_page_link_to_text_reply_once() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html";
        let merged = external_channel_reply_with_static_page_artifact_links(
            reply_with_text(Some("这是正常业务回答。")),
            Some(&static_page_reply(public_url)),
        );

        assert_eq!(merged.reply_type, ExternalBotReplyTypeView::Text);
        assert_eq!(merged.task_status.as_deref(), Some("answered"));
        assert_eq!(merged.artifact_links, vec![public_url.to_string()]);
        assert_eq!(
            merged.card.as_ref().expect("merged card")["artifact_links"],
            json!([public_url])
        );
        let text = merged.text.as_deref().expect("merged text");
        assert!(text.contains("这是正常业务回答。"));
        assert_eq!(text.matches(public_url).count(), 1);
    }

    #[test]
    fn merge_uses_static_page_text_when_base_has_no_text() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html";
        let merged = external_channel_reply_with_static_page_artifact_links(
            reply_with_text(None),
            Some(&static_page_reply(public_url)),
        );

        assert_eq!(
            merged.text.as_deref(),
            Some("已依据客户需求生成可访问的报表页面。")
        );
        assert_eq!(merged.artifact_links, vec![public_url.to_string()]);
    }
}
