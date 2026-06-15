use contracts::{ExternalBotReplyTypeView, ExternalBotReplyView, ExternalChannelEventResponse};
use serde_json::{json, Value};

use crate::external_channel_public_artifact::{
    dedupe_external_channel_public_artifact_links, external_channel_public_artifact_url_from_reply,
    external_channel_static_page_enrich_reply_card,
    external_channel_text_with_public_artifact_link,
};
use crate::external_channel_public_card::{
    external_channel_public_card_value, external_channel_public_reply_task_status,
    external_channel_public_status_allows_artifact_link_for_card,
    external_channel_public_status_allows_preview_link, external_channel_reply_is_static_page_like,
    external_channel_reply_public_status, external_channel_reply_static_page_card_status,
    prune_external_channel_public_card_links,
};
use crate::external_channel_public_text::external_channel_public_reply_text;
use crate::external_channel_sse_support::{
    external_channel_static_page_cancelled_should_continue,
    external_channel_static_page_provisional_existing_artifact,
};
use crate::external_channel_static_page_terminal_reply::external_channel_static_page_reply_with_public_artifact_terminal;

pub(crate) fn external_channel_public_response(
    mut response: ExternalChannelEventResponse,
) -> ExternalChannelEventResponse {
    response.reply = external_channel_public_reply(response.reply);
    response
}

pub(crate) fn external_channel_public_reply(
    mut reply: ExternalBotReplyView,
) -> ExternalBotReplyView {
    reply = external_channel_static_page_reply_with_public_artifact_terminal(reply);
    let static_page_like = external_channel_reply_is_static_page_like(&reply);
    if static_page_like {
        external_channel_static_page_enrich_reply_card(&mut reply);
    }
    let static_page_public_artifact_url = if static_page_like {
        external_channel_public_artifact_url_from_reply(&reply)
    } else {
        None
    };
    let public_status = external_channel_reply_public_status(&reply);
    let provisional_existing_artifact =
        external_channel_static_page_provisional_existing_artifact(reply.card.as_ref());
    let include_artifact_links = if static_page_like {
        public_status
            .as_deref()
            .map(|status| {
                external_channel_public_status_allows_artifact_link_for_card(
                    status,
                    reply.card.as_ref(),
                )
            })
            .unwrap_or(false)
            || (reply.reply_type == ExternalBotReplyTypeView::ArtifactLink
                && !provisional_existing_artifact)
    } else {
        true
    };
    let include_preview_link = if static_page_like {
        external_channel_reply_static_page_card_status(&reply)
            .or(reply.task_status.as_deref())
            .map(external_channel_public_status_allows_preview_link)
            .unwrap_or(false)
    } else {
        true
    };
    let cancelled_should_continue = external_channel_reply_static_page_card_status(&reply)
        .or(reply.task_status.as_deref())
        .map(|status| {
            external_channel_static_page_cancelled_should_continue(status, reply.card.as_ref())
        })
        .unwrap_or(false);
    if cancelled_should_continue {
        if let Some(Value::Object(card)) = reply.card.as_mut() {
            card.insert(
                "status".to_string(),
                Value::String("static_page_continue_polling".to_string()),
            );
            card.entry("poll_after_seconds".to_string())
                .or_insert_with(|| json!(30));
        }
    }
    reply.text = reply
        .text
        .take()
        .map(|text| external_channel_public_reply_text(&text));
    reply.task_status = reply
        .task_status
        .take()
        .map(|status| external_channel_public_reply_task_status(&status, reply.card.as_ref()));
    reply.card = reply.card.take().map(|card| {
        let mut card = external_channel_public_card_value(card);
        if static_page_like {
            prune_external_channel_public_card_links(
                &mut card,
                include_artifact_links,
                include_preview_link,
            );
            if include_artifact_links {
                if let Some(public_url) = static_page_public_artifact_url.as_deref() {
                    if let Some(object) = card.as_object_mut() {
                        object.insert("public_url".to_string(), json!(public_url));
                        object.insert("generated_artifact_url".to_string(), json!(public_url));
                        object.insert("artifact_links".to_string(), json!([public_url]));
                    }
                }
            }
        }
        card
    });
    if static_page_like && !include_artifact_links {
        reply.artifact_links.clear();
    } else {
        reply.artifact_links = dedupe_external_channel_public_artifact_links(reply.artifact_links);
    }
    if reply.reply_type == ExternalBotReplyTypeView::ArtifactLink {
        if let Some(public_url) = external_channel_public_artifact_url_from_reply(&reply) {
            if !reply.artifact_links.iter().any(|link| link == &public_url) {
                reply.artifact_links.insert(0, public_url.clone());
            }
            let text = reply.text.take().unwrap_or_default();
            reply.text = Some(external_channel_text_with_public_artifact_link(
                text,
                &public_url,
            ));
        }
    }
    reply
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn artifact_url(path: &str) -> String {
        format!("https://v3.elepcloud.com/generated-artifacts/{path}")
    }

    fn reply(
        reply_type: ExternalBotReplyTypeView,
        card: Option<Value>,
        task_status: Option<&str>,
    ) -> ExternalBotReplyView {
        ExternalBotReplyView {
            target_conversation_external_id: "conv-1".to_string(),
            reply_type,
            text: Some("已生成。".to_string()),
            card,
            artifact_links: Vec::new(),
            task_status: task_status.map(ToOwned::to_owned),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        }
    }

    #[test]
    fn public_reply_dedupes_artifact_link_and_appends_markdown_once() {
        let public_url = artifact_url("reports/current/index.html");
        let mut input = reply(ExternalBotReplyTypeView::ArtifactLink, None, None);
        input.artifact_links = vec![public_url.clone(), public_url.clone()];

        let output = external_channel_public_reply(input);
        let repeated = external_channel_public_reply(output.clone());

        assert_eq!(output.artifact_links, vec![public_url.clone()]);
        assert_eq!(repeated.artifact_links, vec![public_url.clone()]);
        assert_eq!(repeated.text, output.text);
        assert!(output
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
    }

    #[test]
    fn public_reply_keeps_background_cancelled_static_page_pollable() {
        let public_url = artifact_url("reports/current/index.html");
        let input = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "public_url": public_url,
                "poll_after_seconds": 15
            })),
            Some("static_page_publish_cancelled"),
        );

        let output = external_channel_public_reply(input);

        assert_eq!(output.task_status.as_deref(), Some("processing"));
        assert_eq!(
            output.card.as_ref().and_then(|card| card.get("status")),
            Some(&json!("static_page_continue_polling"))
        );
        assert_eq!(
            output
                .card
                .as_ref()
                .and_then(|card| card.get("poll_after_seconds")),
            Some(&json!(15))
        );
    }

    #[test]
    fn public_response_sanitizes_nested_reply() {
        let public_url = artifact_url("reports/current/index.html");
        let response = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-1".to_string(),
            reply: reply(
                ExternalBotReplyTypeView::ArtifactLink,
                Some(json!({
                    "type": "normal_card",
                    "title": "完成",
                    "runtime_manifest": {"internal": true}
                })),
                None,
            ),
        };
        let mut response = response;
        response.reply.artifact_links = vec![public_url.clone()];

        let output = external_channel_public_response(response);

        assert_eq!(output.idempotency_key, "idem-1");
        assert_eq!(output.reply.artifact_links, vec![public_url]);
        assert!(output
            .reply
            .card
            .as_ref()
            .is_some_and(|card| card.get("runtime_manifest").is_none()));
    }
}
