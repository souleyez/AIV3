use contracts::{ExternalBotReplyTypeView, ExternalBotReplyView, ExternalChannelEventResponse};
use serde_json::{json, Value};

use crate::external_channel_public_artifact::{
    dedupe_external_channel_public_artifact_links, external_channel_public_artifact_url_from_reply,
    external_channel_static_page_public_artifact_url_after_enrichment,
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
    response: ExternalChannelEventResponse,
) -> ExternalChannelEventResponse {
    external_channel_public_response_with_result(response).0
}

pub(crate) fn external_channel_public_response_with_result(
    mut response: ExternalChannelEventResponse,
) -> (
    ExternalChannelEventResponse,
    ExternalChannelPublicReplyCleanupResult,
) {
    let (reply, cleanup_result) = external_channel_public_reply_with_result(response.reply);
    response.reply = reply;
    (response, cleanup_result)
}

pub(crate) fn external_channel_public_reply_should_include_artifact_links(
    reply: &ExternalBotReplyView,
    static_page_like: bool,
) -> bool {
    if !static_page_like {
        return true;
    }

    let status_allows_artifact_link = external_channel_reply_public_status(reply)
        .as_deref()
        .map(|status| {
            external_channel_public_status_allows_artifact_link_for_card(
                status,
                reply.card.as_ref(),
            )
        })
        .unwrap_or(false);
    let artifact_link_reply_allows_link = reply.reply_type
        == ExternalBotReplyTypeView::ArtifactLink
        && !external_channel_static_page_provisional_existing_artifact(reply.card.as_ref());

    status_allows_artifact_link || artifact_link_reply_allows_link
}

pub(crate) fn external_channel_public_reply_should_include_preview_link(
    reply: &ExternalBotReplyView,
    static_page_like: bool,
) -> bool {
    if !static_page_like {
        return true;
    }

    external_channel_reply_static_page_card_status(reply)
        .or(reply.task_status.as_deref())
        .map(external_channel_public_status_allows_preview_link)
        .unwrap_or(false)
}

pub(crate) fn external_channel_public_reply_cancelled_should_continue(
    reply: &ExternalBotReplyView,
) -> bool {
    external_channel_reply_static_page_card_status(reply)
        .or(reply.task_status.as_deref())
        .map(|status| {
            external_channel_static_page_cancelled_should_continue(status, reply.card.as_ref())
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExternalChannelPublicReplyPolicy {
    pub(crate) include_artifact_links: bool,
    pub(crate) include_preview_link: bool,
    pub(crate) cancelled_should_continue: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalChannelPublicReplyPrepared {
    pub(crate) static_page_like: bool,
    pub(crate) public_artifact_url: Option<String>,
    pub(crate) policy: ExternalChannelPublicReplyPolicy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ExternalChannelPublicReplyCleanupResult {
    pub(crate) pre_card_marked_continue_polling: bool,
    pub(crate) post_card_attached_artifact_link: bool,
}

pub(crate) fn external_channel_public_reply_policy(
    reply: &ExternalBotReplyView,
    static_page_like: bool,
) -> ExternalChannelPublicReplyPolicy {
    ExternalChannelPublicReplyPolicy {
        include_artifact_links: external_channel_public_reply_should_include_artifact_links(
            reply,
            static_page_like,
        ),
        include_preview_link: external_channel_public_reply_should_include_preview_link(
            reply,
            static_page_like,
        ),
        cancelled_should_continue: external_channel_public_reply_cancelled_should_continue(reply),
    }
}

pub(crate) fn external_channel_public_reply_apply_continue_polling_policy(
    reply: &mut ExternalBotReplyView,
    policy: ExternalChannelPublicReplyPolicy,
) -> bool {
    if !policy.cancelled_should_continue {
        return false;
    }

    let Some(card) = reply.card.as_mut() else {
        return false;
    };

    external_channel_public_card_mark_continue_polling(card)
}

pub(crate) fn external_channel_public_reply_attach_artifact_link(
    reply: &mut ExternalBotReplyView,
    public_url: &str,
) {
    if !reply.artifact_links.iter().any(|link| link == public_url) {
        reply.artifact_links.insert(0, public_url.to_string());
    }
    let text = reply.text.take().unwrap_or_default();
    reply.text = Some(external_channel_text_with_public_artifact_link(
        text, public_url,
    ));
}

pub(crate) fn external_channel_public_card_attach_artifact_url(
    card: &mut Value,
    public_url: &str,
) -> bool {
    let Some(object) = card.as_object_mut() else {
        return false;
    };

    object.insert("public_url".to_string(), json!(public_url));
    object.insert("generated_artifact_url".to_string(), json!(public_url));
    object.insert("artifact_links".to_string(), json!([public_url]));
    true
}

pub(crate) fn external_channel_public_card_mark_continue_polling(card: &mut Value) -> bool {
    let Some(object) = card.as_object_mut() else {
        return false;
    };

    object.insert(
        "status".to_string(),
        Value::String("static_page_continue_polling".to_string()),
    );
    object
        .entry("poll_after_seconds".to_string())
        .or_insert_with(|| json!(30));
    true
}

pub(crate) fn external_channel_public_reply_normalize_artifact_links(
    artifact_links: Vec<String>,
    static_page_like: bool,
    include_artifact_links: bool,
) -> Vec<String> {
    if static_page_like && !include_artifact_links {
        Vec::new()
    } else {
        dedupe_external_channel_public_artifact_links(artifact_links)
    }
}

pub(crate) fn external_channel_public_reply_apply_artifact_links_policy(
    reply: &mut ExternalBotReplyView,
    static_page_like: bool,
    policy: ExternalChannelPublicReplyPolicy,
) {
    reply.artifact_links = external_channel_public_reply_normalize_artifact_links(
        std::mem::take(&mut reply.artifact_links),
        static_page_like,
        policy.include_artifact_links,
    );
}

pub(crate) fn external_channel_public_reply_card(
    card: Value,
    static_page_like: bool,
    include_artifact_links: bool,
    include_preview_link: bool,
    public_artifact_url: Option<&str>,
) -> Value {
    let mut card = external_channel_public_card_value(card);
    if static_page_like {
        prune_external_channel_public_card_links(
            &mut card,
            include_artifact_links,
            include_preview_link,
        );
        if include_artifact_links {
            if let Some(public_url) = public_artifact_url {
                external_channel_public_card_attach_artifact_url(&mut card, public_url);
            }
        }
    }
    card
}

pub(crate) fn external_channel_public_reply_apply_card_policy(
    reply: &mut ExternalBotReplyView,
    static_page_like: bool,
    policy: ExternalChannelPublicReplyPolicy,
    public_artifact_url: Option<&str>,
) {
    reply.card = reply.card.take().map(|card| {
        external_channel_public_reply_card(
            card,
            static_page_like,
            policy.include_artifact_links,
            policy.include_preview_link,
            public_artifact_url,
        )
    });
}

pub(crate) fn external_channel_public_reply_attach_resolved_artifact_link(
    reply: &mut ExternalBotReplyView,
) -> bool {
    if reply.reply_type != ExternalBotReplyTypeView::ArtifactLink {
        return false;
    }

    let Some(public_url) = external_channel_public_artifact_url_from_reply(reply) else {
        return false;
    };

    external_channel_public_reply_attach_artifact_link(reply, &public_url);
    true
}

pub(crate) fn external_channel_public_reply_clean_text(text: Option<String>) -> Option<String> {
    text.map(|text| external_channel_public_reply_text(&text))
}

pub(crate) fn external_channel_public_reply_clean_task_status(
    task_status: Option<String>,
    card: Option<&Value>,
) -> Option<String> {
    task_status.map(|status| external_channel_public_reply_task_status(&status, card))
}

pub(crate) fn external_channel_public_reply_clean_text_and_task_status(
    reply: &mut ExternalBotReplyView,
) {
    reply.text = external_channel_public_reply_clean_text(reply.text.take());
    reply.task_status = external_channel_public_reply_clean_task_status(
        reply.task_status.take(),
        reply.card.as_ref(),
    );
}

pub(crate) fn external_channel_public_reply_apply_pre_card_cleanup(
    reply: &mut ExternalBotReplyView,
    policy: ExternalChannelPublicReplyPolicy,
) -> bool {
    let marked_continue_polling =
        external_channel_public_reply_apply_continue_polling_policy(reply, policy);
    external_channel_public_reply_clean_text_and_task_status(reply);
    marked_continue_polling
}

pub(crate) fn external_channel_public_reply_apply_post_card_cleanup(
    reply: &mut ExternalBotReplyView,
    static_page_like: bool,
    policy: ExternalChannelPublicReplyPolicy,
) -> bool {
    external_channel_public_reply_apply_artifact_links_policy(reply, static_page_like, policy);
    external_channel_public_reply_attach_resolved_artifact_link(reply)
}

pub(crate) fn external_channel_public_reply_static_page_context(
    reply: &mut ExternalBotReplyView,
) -> (bool, Option<String>) {
    let static_page_like = external_channel_reply_is_static_page_like(reply);
    let public_artifact_url = if static_page_like {
        external_channel_static_page_public_artifact_url_after_enrichment(reply)
    } else {
        None
    };
    (static_page_like, public_artifact_url)
}

pub(crate) fn external_channel_public_reply_prepare(
    reply: &mut ExternalBotReplyView,
) -> ExternalChannelPublicReplyPrepared {
    let (static_page_like, public_artifact_url) =
        external_channel_public_reply_static_page_context(reply);
    let policy = external_channel_public_reply_policy(reply, static_page_like);
    ExternalChannelPublicReplyPrepared {
        static_page_like,
        public_artifact_url,
        policy,
    }
}

pub(crate) fn external_channel_public_reply_apply_policy(
    reply: &mut ExternalBotReplyView,
    static_page_like: bool,
    policy: ExternalChannelPublicReplyPolicy,
    public_artifact_url: Option<&str>,
) -> ExternalChannelPublicReplyCleanupResult {
    let pre_card_marked_continue_polling =
        external_channel_public_reply_apply_pre_card_cleanup(reply, policy);
    external_channel_public_reply_apply_card_policy(
        reply,
        static_page_like,
        policy,
        public_artifact_url,
    );
    let post_card_attached_artifact_link =
        external_channel_public_reply_apply_post_card_cleanup(reply, static_page_like, policy);
    ExternalChannelPublicReplyCleanupResult {
        pre_card_marked_continue_polling,
        post_card_attached_artifact_link,
    }
}

pub(crate) fn external_channel_public_reply_apply_prepared(
    reply: &mut ExternalBotReplyView,
    prepared: ExternalChannelPublicReplyPrepared,
) -> ExternalChannelPublicReplyCleanupResult {
    external_channel_public_reply_apply_policy(
        reply,
        prepared.static_page_like,
        prepared.policy,
        prepared.public_artifact_url.as_deref(),
    )
}

#[allow(dead_code)]
pub(crate) fn external_channel_public_reply_finalize(
    reply: ExternalBotReplyView,
) -> ExternalBotReplyView {
    external_channel_public_reply_finalize_with_result(reply).0
}

pub(crate) fn external_channel_public_reply_finalize_with_result(
    mut reply: ExternalBotReplyView,
) -> (
    ExternalBotReplyView,
    ExternalChannelPublicReplyCleanupResult,
) {
    let prepared = external_channel_public_reply_prepare(&mut reply);
    let cleanup_result = external_channel_public_reply_apply_prepared(&mut reply, prepared);
    (reply, cleanup_result)
}

pub(crate) fn external_channel_public_reply_terminal(
    reply: ExternalBotReplyView,
) -> ExternalBotReplyView {
    external_channel_static_page_reply_with_public_artifact_terminal(reply)
}

pub(crate) fn external_channel_public_reply_with_result(
    reply: ExternalBotReplyView,
) -> (
    ExternalBotReplyView,
    ExternalChannelPublicReplyCleanupResult,
) {
    let reply = external_channel_public_reply_terminal(reply);
    external_channel_public_reply_finalize_with_result(reply)
}

#[allow(dead_code)]
pub(crate) fn external_channel_public_reply(reply: ExternalBotReplyView) -> ExternalBotReplyView {
    external_channel_public_reply_with_result(reply).0
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
    fn public_reply_should_include_artifact_links_matches_static_page_rules() {
        let normal_reply = reply(ExternalBotReplyTypeView::Text, None, None);
        assert!(external_channel_public_reply_should_include_artifact_links(
            &normal_reply,
            false
        ));

        let published_reply = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
            })),
            None,
        );
        assert!(external_channel_public_reply_should_include_artifact_links(
            &published_reply,
            true
        ));

        let provisional_reply = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_queued",
                "provisional_existing_artifact": true,
            })),
            None,
        );
        assert!(
            !external_channel_public_reply_should_include_artifact_links(&provisional_reply, true)
        );
    }

    #[test]
    fn public_reply_should_include_preview_link_matches_static_page_rules() {
        let normal_reply = reply(ExternalBotReplyTypeView::Text, None, None);
        assert!(external_channel_public_reply_should_include_preview_link(
            &normal_reply,
            false
        ));

        let preview_ready_reply = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_preview_ready",
            })),
            None,
        );
        assert!(external_channel_public_reply_should_include_preview_link(
            &preview_ready_reply,
            true
        ));

        let published_reply = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
            })),
            None,
        );
        assert!(!external_channel_public_reply_should_include_preview_link(
            &published_reply,
            true
        ));
    }

    #[test]
    fn public_reply_cancelled_should_continue_matches_static_page_rules() {
        let retryable_cancelled = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "runtime_event": {
                    "background_continuation": "retryable"
                },
            })),
            None,
        );
        assert!(external_channel_public_reply_cancelled_should_continue(
            &retryable_cancelled
        ));

        let plain_cancelled = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
            })),
            None,
        );
        assert!(!external_channel_public_reply_cancelled_should_continue(
            &plain_cancelled
        ));

        let published = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "runtime_event": {
                    "background_continuation": "retryable"
                },
            })),
            None,
        );
        assert!(!external_channel_public_reply_cancelled_should_continue(
            &published
        ));
    }

    #[test]
    fn public_reply_policy_combines_link_preview_and_cancelled_rules() {
        let normal_reply = reply(ExternalBotReplyTypeView::Text, None, None);
        assert_eq!(
            external_channel_public_reply_policy(&normal_reply, false),
            ExternalChannelPublicReplyPolicy {
                include_artifact_links: true,
                include_preview_link: true,
                cancelled_should_continue: false,
            }
        );

        let preview_ready = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_preview_ready",
            })),
            None,
        );
        assert_eq!(
            external_channel_public_reply_policy(&preview_ready, true),
            ExternalChannelPublicReplyPolicy {
                include_artifact_links: false,
                include_preview_link: true,
                cancelled_should_continue: false,
            }
        );

        let retryable_cancelled = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "runtime_event": {
                    "background_continuation": "retryable"
                },
            })),
            None,
        );
        assert_eq!(
            external_channel_public_reply_policy(&retryable_cancelled, true),
            ExternalChannelPublicReplyPolicy {
                include_artifact_links: false,
                include_preview_link: false,
                cancelled_should_continue: true,
            }
        );

        let published = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
            })),
            None,
        );
        assert_eq!(
            external_channel_public_reply_policy(&published, true),
            ExternalChannelPublicReplyPolicy {
                include_artifact_links: true,
                include_preview_link: false,
                cancelled_should_continue: false,
            }
        );
    }

    #[test]
    fn public_reply_apply_continue_polling_policy_updates_card_when_enabled() {
        let disabled_policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: false,
            include_preview_link: false,
            cancelled_should_continue: false,
        };
        let enabled_policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: false,
            include_preview_link: false,
            cancelled_should_continue: true,
        };

        let mut disabled = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "status": "static_page_publish_cancelled",
            })),
            Some("static_page_publish_cancelled"),
        );
        assert!(
            !external_channel_public_reply_apply_continue_polling_policy(
                &mut disabled,
                disabled_policy
            )
        );
        assert_eq!(
            disabled.card.as_ref().and_then(|card| card.get("status")),
            Some(&json!("static_page_publish_cancelled"))
        );

        let mut no_card = reply(ExternalBotReplyTypeView::TaskStatus, None, None);
        assert!(
            !external_channel_public_reply_apply_continue_polling_policy(
                &mut no_card,
                enabled_policy
            )
        );
        assert!(no_card.card.is_none());

        let mut enabled = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "status": "static_page_publish_cancelled",
            })),
            Some("static_page_publish_cancelled"),
        );
        assert!(external_channel_public_reply_apply_continue_polling_policy(
            &mut enabled,
            enabled_policy
        ));
        assert_eq!(
            enabled.card.as_ref().and_then(|card| card.get("status")),
            Some(&json!("static_page_continue_polling"))
        );
        assert_eq!(
            enabled
                .card
                .as_ref()
                .and_then(|card| card.get("poll_after_seconds")),
            Some(&json!(30))
        );
    }

    #[test]
    fn public_reply_attach_artifact_link_dedupes_and_appends_text() {
        let public_url = artifact_url("reports/current/index.html");
        let mut input = reply(ExternalBotReplyTypeView::ArtifactLink, None, None);
        input.text = Some("已生成。".to_string());
        input.artifact_links = vec![public_url.clone()];

        external_channel_public_reply_attach_artifact_link(&mut input, &public_url);
        let text_after_first_attach = input.text.clone();
        external_channel_public_reply_attach_artifact_link(&mut input, &public_url);

        assert_eq!(input.artifact_links, vec![public_url.clone()]);
        assert_eq!(input.text, text_after_first_attach);
        assert!(input
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
    }

    #[test]
    fn public_card_attach_artifact_url_writes_public_fields_for_objects_only() {
        let public_url = artifact_url("reports/current/index.html");
        let mut object_card = json!({"status": "static_page_published"});

        assert!(external_channel_public_card_attach_artifact_url(
            &mut object_card,
            &public_url
        ));
        assert_eq!(object_card.get("public_url"), Some(&json!(public_url)));
        assert_eq!(
            object_card.get("generated_artifact_url"),
            Some(&json!(public_url))
        );
        assert_eq!(
            object_card.get("artifact_links"),
            Some(&json!([public_url]))
        );

        let mut non_object_card = json!(["static_page_published"]);
        assert!(!external_channel_public_card_attach_artifact_url(
            &mut non_object_card,
            &public_url
        ));
        assert_eq!(non_object_card, json!(["static_page_published"]));
    }

    #[test]
    fn public_card_mark_continue_polling_sets_status_and_preserves_polling_hint() {
        let mut existing_poll_card = json!({
            "status": "static_page_publish_cancelled",
            "poll_after_seconds": 15,
        });
        assert!(external_channel_public_card_mark_continue_polling(
            &mut existing_poll_card
        ));
        assert_eq!(
            existing_poll_card.get("status"),
            Some(&json!("static_page_continue_polling"))
        );
        assert_eq!(
            existing_poll_card.get("poll_after_seconds"),
            Some(&json!(15))
        );

        let mut missing_poll_card = json!({"status": "static_page_publish_cancelled"});
        assert!(external_channel_public_card_mark_continue_polling(
            &mut missing_poll_card
        ));
        assert_eq!(
            missing_poll_card.get("poll_after_seconds"),
            Some(&json!(30))
        );

        let mut non_object_card = json!(["static_page_publish_cancelled"]);
        assert!(!external_channel_public_card_mark_continue_polling(
            &mut non_object_card
        ));
        assert_eq!(non_object_card, json!(["static_page_publish_cancelled"]));
    }

    #[test]
    fn public_reply_normalize_artifact_links_clears_static_page_suppressed_links_and_keeps_first_allowed_link(
    ) {
        let public_url = artifact_url("reports/current/index.html");
        let secondary_url = artifact_url("reports/other/index.html");

        assert_eq!(
            external_channel_public_reply_normalize_artifact_links(
                vec![
                    public_url.clone(),
                    public_url.clone(),
                    secondary_url.clone()
                ],
                true,
                false,
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            external_channel_public_reply_normalize_artifact_links(
                vec![
                    public_url.clone(),
                    public_url.clone(),
                    secondary_url.clone()
                ],
                true,
                true,
            ),
            vec![public_url.clone()]
        );
        assert_eq!(
            external_channel_public_reply_normalize_artifact_links(
                vec![public_url.clone(), public_url.clone()],
                false,
                false,
            ),
            vec![public_url]
        );
    }

    #[test]
    fn public_reply_apply_artifact_links_policy_normalizes_links_with_policy() {
        let public_url = artifact_url("reports/current/index.html");
        let secondary_url = artifact_url("reports/other/index.html");
        let suppressed_policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: false,
            include_preview_link: false,
            cancelled_should_continue: false,
        };
        let allowed_policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: true,
            include_preview_link: false,
            cancelled_should_continue: false,
        };

        let mut suppressed_static = reply(ExternalBotReplyTypeView::TaskStatus, None, None);
        suppressed_static.artifact_links = vec![public_url.clone(), secondary_url.clone()];
        external_channel_public_reply_apply_artifact_links_policy(
            &mut suppressed_static,
            true,
            suppressed_policy,
        );
        assert!(suppressed_static.artifact_links.is_empty());

        let mut allowed_static = reply(ExternalBotReplyTypeView::TaskStatus, None, None);
        allowed_static.artifact_links = vec![
            public_url.clone(),
            public_url.clone(),
            secondary_url.clone(),
        ];
        external_channel_public_reply_apply_artifact_links_policy(
            &mut allowed_static,
            true,
            allowed_policy,
        );
        assert_eq!(allowed_static.artifact_links, vec![public_url.clone()]);

        let mut normal_reply = reply(ExternalBotReplyTypeView::Text, None, None);
        normal_reply.artifact_links = vec![public_url.clone(), public_url.clone()];
        external_channel_public_reply_apply_artifact_links_policy(
            &mut normal_reply,
            false,
            suppressed_policy,
        );
        assert_eq!(normal_reply.artifact_links, vec![public_url]);
    }

    #[test]
    fn public_reply_card_sanitizes_and_applies_static_page_link_rules() {
        let public_url = artifact_url("reports/current/index.html");
        let preview_url = artifact_url("reports/current/preview.png");

        let normal_card = external_channel_public_reply_card(
            json!({
                "type": "normal_card",
                "title": "完成",
                "runtime_manifest": {"internal": true},
            }),
            false,
            false,
            false,
            Some(&public_url),
        );
        assert_eq!(normal_card.get("title"), Some(&json!("完成")));
        assert!(normal_card.get("runtime_manifest").is_none());
        assert!(normal_card.get("public_url").is_none());

        let suppressed_static_card = external_channel_public_reply_card(
            json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_preview_ready",
                "public_url": public_url,
                "preview_url": preview_url,
                "generated_artifact_url": "https://v3.elepcloud.com/generated-artifacts/stale/index.html",
                "artifact_links": ["https://v3.elepcloud.com/generated-artifacts/stale/index.html"],
            }),
            true,
            false,
            true,
            None,
        );
        assert_eq!(
            suppressed_static_card.get("preview_url"),
            Some(&json!(preview_url))
        );
        assert!(suppressed_static_card.get("public_url").is_none());
        assert!(suppressed_static_card
            .get("generated_artifact_url")
            .is_none());
        assert!(suppressed_static_card.get("artifact_links").is_none());

        let replacement_url = artifact_url("reports/current/replacement.html");
        let allowed_static_card = external_channel_public_reply_card(
            json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "preview_url": preview_url,
                "public_url": "https://v3.elepcloud.com/generated-artifacts/stale/index.html",
            }),
            true,
            true,
            false,
            Some(&replacement_url),
        );
        assert_eq!(
            allowed_static_card.get("public_url"),
            Some(&json!(replacement_url))
        );
        assert_eq!(
            allowed_static_card.get("generated_artifact_url"),
            Some(&json!(replacement_url))
        );
        assert_eq!(
            allowed_static_card.get("artifact_links"),
            Some(&json!([replacement_url]))
        );
        assert!(allowed_static_card.get("preview_url").is_none());
    }

    #[test]
    fn public_reply_apply_card_policy_sanitizes_card_with_policy() {
        let public_url = artifact_url("reports/current/index.html");
        let preview_url = artifact_url("reports/current/preview.png");
        let policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: true,
            include_preview_link: false,
            cancelled_should_continue: false,
        };
        let mut input = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "runtime_manifest": {"internal": true},
                "preview_url": preview_url,
            })),
            Some("static_page_published"),
        );

        external_channel_public_reply_apply_card_policy(
            &mut input,
            true,
            policy,
            Some(&public_url),
        );

        let card = input.card.as_ref().expect("card");
        assert_eq!(card.get("type"), Some(&json!("v3_static_page_pipeline")));
        assert!(card.get("runtime_manifest").is_none());
        assert!(card.get("preview_url").is_none());
        assert_eq!(card.get("public_url"), Some(&json!(public_url.clone())));
        assert_eq!(
            card.get("generated_artifact_url"),
            Some(&json!(public_url.clone()))
        );
        assert_eq!(card.get("artifact_links"), Some(&json!([public_url])));

        let mut empty = reply(ExternalBotReplyTypeView::Text, None, None);
        external_channel_public_reply_apply_card_policy(&mut empty, false, policy, None);
        assert!(empty.card.is_none());
    }

    #[test]
    fn public_reply_attach_resolved_artifact_link_matches_artifact_link_rules() {
        let public_url = artifact_url("reports/current/index.html");

        let mut non_artifact = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({"public_url": public_url.clone()})),
            None,
        );
        assert!(!external_channel_public_reply_attach_resolved_artifact_link(&mut non_artifact));
        assert!(non_artifact.artifact_links.is_empty());

        let mut artifact_without_url = reply(ExternalBotReplyTypeView::ArtifactLink, None, None);
        assert!(
            !external_channel_public_reply_attach_resolved_artifact_link(&mut artifact_without_url)
        );
        assert!(artifact_without_url.artifact_links.is_empty());

        let mut artifact_with_url = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({"public_url": public_url.clone()})),
            None,
        );
        artifact_with_url.artifact_links = vec![public_url.clone()];
        assert!(external_channel_public_reply_attach_resolved_artifact_link(
            &mut artifact_with_url
        ));
        let text_after_first_attach = artifact_with_url.text.clone();
        assert!(external_channel_public_reply_attach_resolved_artifact_link(
            &mut artifact_with_url
        ));
        assert_eq!(artifact_with_url.artifact_links, vec![public_url]);
        assert_eq!(artifact_with_url.text, text_after_first_attach);
        assert!(artifact_with_url
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
    }

    #[test]
    fn public_reply_clean_text_helper_applies_public_text_rules() {
        let output = external_channel_public_reply_clean_text(Some(
            "已创建静态页草稿并提交 Image2 效果图队列；效果图无需客户确认，生成后会继续进入固定 Cloudflare Codex 发布链路。"
                .to_string(),
        ))
        .expect("cleaned text");

        assert!(output.contains("已创建报表页面草稿并进入生成队列"));
        assert!(!output.contains("Image2"));
        assert!(!output.contains("Cloudflare"));
        assert!(!output.contains("Codex"));
        assert!(external_channel_public_reply_clean_text(None).is_none());
    }

    #[test]
    fn public_reply_clean_task_status_helper_applies_card_aware_rules() {
        let card = json!({
            "type": "v3_static_page_image2_pipeline",
            "status": "static_page_publish_cancelled",
            "runtime_event": {
                "background_continuation": "retryable"
            }
        });

        assert_eq!(
            external_channel_public_reply_clean_task_status(
                Some("static_page_publish_cancelled".to_string()),
                Some(&card),
            )
            .as_deref(),
            Some("processing")
        );
        assert_eq!(
            external_channel_public_reply_clean_task_status(Some("completed".to_string()), None)
                .as_deref(),
            Some("completed")
        );
        assert!(external_channel_public_reply_clean_task_status(None, Some(&card)).is_none());
    }

    #[test]
    fn public_reply_clean_text_and_task_status_applies_public_rules() {
        let mut input = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "runtime_event": {
                    "background_continuation": "retryable"
                }
            })),
            Some("static_page_publish_cancelled"),
        );
        input.text = Some(
            "已创建静态页草稿并提交 Image2 效果图队列；效果图无需客户确认，生成后会继续进入固定 Cloudflare Codex 发布链路。"
                .to_string(),
        );

        external_channel_public_reply_clean_text_and_task_status(&mut input);

        let text = input.text.as_deref().unwrap_or_default();
        assert!(text.contains("已创建报表页面草稿并进入生成队列"));
        assert!(!text.contains("Image2"));
        assert!(!text.contains("Cloudflare"));
        assert!(!text.contains("Codex"));
        assert_eq!(input.task_status.as_deref(), Some("processing"));

        let mut empty_input = reply(ExternalBotReplyTypeView::TaskStatus, None, None);
        empty_input.text = None;
        external_channel_public_reply_clean_text_and_task_status(&mut empty_input);
        assert!(empty_input.text.is_none());
        assert!(empty_input.task_status.is_none());
    }

    #[test]
    fn public_reply_apply_pre_card_cleanup_marks_polling_before_status_cleanup() {
        let public_url = artifact_url("reports/current/index.html");
        let mut input = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "public_url": public_url,
                "runtime_event": {
                    "background_continuation": "retryable"
                }
            })),
            Some("static_page_publish_cancelled"),
        );
        input.text = Some(
            "已创建静态页草稿并提交 Image2 效果图队列；效果图无需客户确认，生成后会继续进入固定 Cloudflare Codex 发布链路。"
                .to_string(),
        );
        let policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: false,
            include_preview_link: false,
            cancelled_should_continue: true,
        };

        assert!(external_channel_public_reply_apply_pre_card_cleanup(
            &mut input, policy
        ));

        assert_eq!(input.task_status.as_deref(), Some("processing"));
        assert_eq!(
            input.card.as_ref().and_then(|card| card.get("status")),
            Some(&json!("static_page_continue_polling"))
        );
        let text = input.text.as_deref().unwrap_or_default();
        assert!(text.contains("已创建报表页面草稿并进入生成队列"));
        assert!(!text.contains("Image2"));
        assert!(!text.contains("Cloudflare"));
        assert!(!text.contains("Codex"));
    }

    #[test]
    fn public_reply_apply_post_card_cleanup_normalizes_and_attaches_artifact_link() {
        let public_url = artifact_url("reports/current/index.html");
        let policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: true,
            include_preview_link: true,
            cancelled_should_continue: false,
        };
        let mut input = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({
                "type": "normal_card",
                "public_url": public_url.clone(),
            })),
            Some("completed"),
        );
        input.artifact_links = vec![public_url.clone(), public_url.clone()];
        input.text = Some("已生成。".to_string());

        assert!(external_channel_public_reply_apply_post_card_cleanup(
            &mut input, false, policy
        ));

        assert_eq!(input.artifact_links, vec![public_url]);
        assert!(input
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
    }

    #[test]
    fn public_reply_static_page_context_detects_and_enriches_only_static_pages() {
        let public_url = artifact_url("reports/current/index.html");
        let mut non_static = reply(
            ExternalBotReplyTypeView::Text,
            Some(json!({
                "type": "normal_card",
                "public_url": public_url.clone()
            })),
            None,
        );

        assert_eq!(
            external_channel_public_reply_static_page_context(&mut non_static),
            (false, None)
        );

        let mut static_reply = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "public_url": public_url.clone()
            })),
            Some("static_page_published"),
        );

        let (static_page_like, resolved_url) =
            external_channel_public_reply_static_page_context(&mut static_reply);
        assert!(static_page_like);
        assert_eq!(resolved_url.as_deref(), Some(public_url.as_str()));
        assert_eq!(
            static_reply
                .card
                .as_ref()
                .and_then(|card| card.get("public_url")),
            Some(&json!(public_url))
        );
    }

    #[test]
    fn public_reply_prepare_resolves_static_page_context_and_policy() {
        let public_url = artifact_url("reports/current/index.html");
        let mut input = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "public_url": public_url.clone()
            })),
            Some("static_page_published"),
        );

        let prepared = external_channel_public_reply_prepare(&mut input);

        assert!(prepared.static_page_like);
        assert_eq!(
            prepared.public_artifact_url.as_deref(),
            Some(public_url.as_str())
        );
        assert_eq!(
            prepared.policy,
            ExternalChannelPublicReplyPolicy {
                include_artifact_links: true,
                include_preview_link: false,
                cancelled_should_continue: false,
            }
        );
        assert_eq!(
            input.card.as_ref().and_then(|card| card.get("public_url")),
            Some(&json!(public_url))
        );
    }

    #[test]
    fn public_reply_apply_policy_runs_public_cleanup_sequence() {
        let public_url = artifact_url("reports/current/index.html");
        let policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: true,
            include_preview_link: true,
            cancelled_should_continue: false,
        };
        let mut input = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({
                "type": "normal_card",
                "public_url": public_url.clone(),
                "runtime_manifest": {"internal": true},
            })),
            Some("completed"),
        );
        input.artifact_links = vec![public_url.clone(), public_url.clone()];
        input.text = Some("已生成。".to_string());

        external_channel_public_reply_apply_policy(&mut input, false, policy, None);

        assert_eq!(input.artifact_links, vec![public_url]);
        assert!(input
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
        assert_eq!(input.task_status.as_deref(), Some("completed"));
        let card = input.card.as_ref().expect("card");
        assert_eq!(card.get("type"), Some(&json!("normal_card")));
        assert!(card.get("runtime_manifest").is_none());
    }

    #[test]
    fn public_reply_apply_policy_reports_cleanup_result() {
        let public_url = artifact_url("reports/current/index.html");
        let policy = ExternalChannelPublicReplyPolicy {
            include_artifact_links: true,
            include_preview_link: false,
            cancelled_should_continue: true,
        };
        let mut input = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "public_url": public_url.clone(),
                "runtime_event": {
                    "background_continuation": "retryable"
                },
                "runtime_manifest": {"internal": true},
            })),
            Some("static_page_publish_cancelled"),
        );
        input.artifact_links = vec![public_url.clone(), public_url.clone()];

        let result =
            external_channel_public_reply_apply_policy(&mut input, true, policy, Some(&public_url));

        assert_eq!(
            result,
            ExternalChannelPublicReplyCleanupResult {
                pre_card_marked_continue_polling: true,
                post_card_attached_artifact_link: true,
            }
        );
        assert_eq!(input.task_status.as_deref(), Some("processing"));
        assert_eq!(input.artifact_links, vec![public_url.clone()]);
        assert!(input
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
        let card = input.card.as_ref().expect("card");
        assert_eq!(
            card.get("status"),
            Some(&json!("static_page_continue_polling"))
        );
        assert!(card.get("runtime_manifest").is_none());
    }

    #[test]
    fn public_reply_apply_prepared_runs_prepared_cleanup_sequence() {
        let public_url = artifact_url("reports/current/index.html");
        let preview_url = artifact_url("reports/current/preview.png");
        let mut input = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "public_url": public_url.clone(),
                "preview_url": preview_url,
                "runtime_manifest": {"internal": true},
            })),
            Some("static_page_published"),
        );

        let prepared = external_channel_public_reply_prepare(&mut input);
        let result = external_channel_public_reply_apply_prepared(&mut input, prepared);

        assert_eq!(result, ExternalChannelPublicReplyCleanupResult::default());
        assert_eq!(input.task_status.as_deref(), Some("static_page_published"));
        let card = input.card.as_ref().expect("card");
        assert_eq!(card.get("type"), Some(&json!("v3_static_page_pipeline")));
        assert_eq!(card.get("public_url"), Some(&json!(public_url.clone())));
        assert_eq!(
            card.get("generated_artifact_url"),
            Some(&json!(public_url.clone()))
        );
        assert_eq!(card.get("artifact_links"), Some(&json!([public_url])));
        assert!(card.get("preview_url").is_none());
        assert!(card.get("runtime_manifest").is_none());
    }

    #[test]
    fn public_reply_finalize_prepares_and_applies_public_cleanup() {
        let public_url = artifact_url("reports/current/index.html");
        let preview_url = artifact_url("reports/current/preview.png");
        let input = reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "public_url": public_url.clone(),
                "preview_url": preview_url,
                "runtime_manifest": {"internal": true},
            })),
            Some("static_page_published"),
        );

        let output = external_channel_public_reply_finalize(input);

        assert_eq!(output.task_status.as_deref(), Some("static_page_published"));
        let card = output.card.as_ref().expect("card");
        assert_eq!(card.get("type"), Some(&json!("v3_static_page_pipeline")));
        assert_eq!(card.get("public_url"), Some(&json!(public_url.clone())));
        assert_eq!(
            card.get("generated_artifact_url"),
            Some(&json!(public_url.clone()))
        );
        assert_eq!(card.get("artifact_links"), Some(&json!([public_url])));
        assert!(card.get("preview_url").is_none());
        assert!(card.get("runtime_manifest").is_none());
    }

    #[test]
    fn public_reply_finalize_with_result_reports_cleanup_result() {
        let public_url = artifact_url("reports/current/index.html");
        let mut input = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "public_url": public_url.clone(),
                "runtime_event": {
                    "background_continuation": "retryable"
                },
                "runtime_manifest": {"internal": true},
            })),
            Some("static_page_publish_cancelled"),
        );
        input.artifact_links = vec![public_url.clone(), public_url.clone()];

        let (output, result) = external_channel_public_reply_finalize_with_result(input);

        assert_eq!(
            result,
            ExternalChannelPublicReplyCleanupResult {
                pre_card_marked_continue_polling: true,
                post_card_attached_artifact_link: true,
            }
        );
        assert_eq!(output.task_status.as_deref(), Some("processing"));
        assert_eq!(output.artifact_links, vec![public_url.clone()]);
        assert!(output
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
        let card = output.card.as_ref().expect("card");
        assert_eq!(
            card.get("status"),
            Some(&json!("static_page_continue_polling"))
        );
        assert!(card.get("runtime_manifest").is_none());
    }

    #[test]
    fn public_reply_with_result_runs_terminal_and_finalize_cleanup() {
        let public_url = artifact_url("reports/current/index.html");
        let mut input = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "public_url": public_url.clone(),
                "runtime_event": {
                    "background_continuation": "retryable"
                },
                "runtime_manifest": {"internal": true},
            })),
            Some("static_page_publish_cancelled"),
        );
        input.artifact_links = vec![public_url.clone(), public_url.clone()];

        let (output, result) = external_channel_public_reply_with_result(input);

        assert_eq!(
            result,
            ExternalChannelPublicReplyCleanupResult {
                pre_card_marked_continue_polling: true,
                post_card_attached_artifact_link: true,
            }
        );
        assert_eq!(output.task_status.as_deref(), Some("processing"));
        assert_eq!(output.artifact_links, vec![public_url.clone()]);
        assert!(output
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
        let card = output.card.as_ref().expect("card");
        assert_eq!(
            card.get("status"),
            Some(&json!("static_page_continue_polling"))
        );
        assert!(card.get("runtime_manifest").is_none());
    }

    #[test]
    fn public_reply_terminal_leaves_non_static_replies_unchanged() {
        let public_url = artifact_url("reports/current/index.html");
        let mut input = reply(
            ExternalBotReplyTypeView::Text,
            Some(json!({
                "type": "normal_card",
                "title": "完成",
                "public_url": public_url.clone(),
            })),
            Some("completed"),
        );
        input.text = Some("已完成。".to_string());
        input.artifact_links = vec![public_url];

        let output = external_channel_public_reply_terminal(input.clone());

        assert_eq!(output.reply_type, input.reply_type);
        assert_eq!(output.text, input.text);
        assert_eq!(output.card, input.card);
        assert_eq!(output.task_status, input.task_status);
        assert_eq!(output.artifact_links, input.artifact_links);
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

    #[test]
    fn public_response_with_result_reports_nested_reply_cleanup_result() {
        let public_url = artifact_url("reports/current/index.html");
        let mut reply = reply(
            ExternalBotReplyTypeView::ArtifactLink,
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_publish_cancelled",
                "public_url": public_url.clone(),
                "runtime_event": {
                    "background_continuation": "retryable"
                },
                "runtime_manifest": {"internal": true},
            })),
            Some("static_page_publish_cancelled"),
        );
        reply.artifact_links = vec![public_url.clone(), public_url.clone()];
        let response = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-1".to_string(),
            reply,
        };

        let (output, result) = external_channel_public_response_with_result(response);

        assert_eq!(output.idempotency_key, "idem-1");
        assert_eq!(
            result,
            ExternalChannelPublicReplyCleanupResult {
                pre_card_marked_continue_polling: true,
                post_card_attached_artifact_link: true,
            }
        );
        assert_eq!(output.reply.task_status.as_deref(), Some("processing"));
        assert_eq!(output.reply.artifact_links, vec![public_url.clone()]);
        assert!(output
            .reply
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("页面链接：[点击查看报表]("));
        let card = output.reply.card.as_ref().expect("card");
        assert_eq!(
            card.get("status"),
            Some(&json!("static_page_continue_polling"))
        );
        assert!(card.get("runtime_manifest").is_none());
    }
}
