use contracts::{ExternalBotReplyTypeView, ExternalBotReplyView};
use serde_json::{json, Value};

use crate::{
    external_channel_public_artifact_url_from_reply,
    external_channel_public_status_allows_artifact_link,
    external_channel_reply_is_static_page_like, external_channel_reply_static_page_card_status,
    external_channel_static_page_accepted_template_baseline,
    external_channel_static_page_customer_ready_text,
    external_channel_static_page_customer_ready_text_for_payload,
    external_channel_static_page_provisional_existing_artifact,
    external_channel_static_page_public_url_with_payload_focus,
    external_channel_text_with_public_artifact_link,
};

pub(crate) fn external_channel_static_page_reply_with_public_artifact_terminal(
    mut reply: ExternalBotReplyView,
) -> ExternalBotReplyView {
    if !external_channel_reply_is_static_page_like(&reply) {
        return reply;
    }
    let Some(raw_public_url) = external_channel_public_artifact_url_from_reply(&reply) else {
        return reply;
    };
    let public_url = reply
        .card
        .as_ref()
        .map(|card| {
            external_channel_static_page_public_url_with_payload_focus(&raw_public_url, card)
        })
        .unwrap_or(raw_public_url);
    let raw_status = external_channel_reply_static_page_card_status(&reply)
        .or(reply.task_status.as_deref())
        .unwrap_or_default()
        .to_string();
    let provisional_existing_artifact =
        external_channel_static_page_provisional_existing_artifact(reply.card.as_ref());
    let accepted_template_baseline =
        external_channel_static_page_accepted_template_baseline(reply.card.as_ref());
    if provisional_existing_artifact
        && !accepted_template_baseline
        && raw_status != "static_page_stable_artifact_reused"
    {
        return reply;
    }
    let terminal_artifact_status = external_channel_public_status_allows_artifact_link(&raw_status);
    let final_publish_with_artifact_url =
        raw_status == "static_page_publish_running" || accepted_template_baseline;
    if !terminal_artifact_status && !final_publish_with_artifact_url {
        return reply;
    }
    if provisional_existing_artifact
        && !accepted_template_baseline
        && !terminal_artifact_status
        && reply.reply_type != ExternalBotReplyTypeView::ArtifactLink
    {
        return reply;
    }
    let preserve_direct_answer_text = reply.reply_type == ExternalBotReplyTypeView::Text
        && reply.task_status.as_deref() == Some("answered")
        && reply
            .text
            .as_deref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false);
    let ready_text = external_channel_static_page_customer_ready_text_for_payload(
        external_channel_static_page_customer_ready_text(),
        reply.card.as_ref(),
        &public_url,
    );
    if preserve_direct_answer_text {
        if let Some(text) = reply.text.take() {
            reply.text = Some(external_channel_text_with_public_artifact_link(
                text,
                &public_url,
            ));
        }
    } else if raw_status != "static_page_stable_artifact_reused" && !provisional_existing_artifact {
        reply.task_status = Some("static_page_published".to_string());
        reply.reply_type = ExternalBotReplyTypeView::ArtifactLink;
        reply.text = Some(external_channel_text_with_public_artifact_link(
            ready_text,
            &public_url,
        ));
    } else if raw_status == "static_page_stable_artifact_reused" {
        reply.text = Some(external_channel_text_with_public_artifact_link(
            ready_text,
            &public_url,
        ));
        reply.task_status = Some("static_page_published".to_string());
        reply.reply_type = ExternalBotReplyTypeView::ArtifactLink;
    } else if accepted_template_baseline {
        let pending_text = external_channel_static_page_customer_ready_text_for_payload(
            "已依据客户需求生成可查看的报表页面，DataMax 会继续刷新并同步最新结果。",
            reply.card.as_ref(),
            &public_url,
        );
        reply.text = Some(external_channel_text_with_public_artifact_link(
            pending_text,
            &public_url,
        ));
    } else {
        let pending_text = external_channel_static_page_customer_ready_text_for_payload(
            "已依据客户需求准备好可查看的报表页面，页面更新完成后会继续同步最新结果。",
            reply.card.as_ref(),
            &public_url,
        );
        reply.text = Some(external_channel_text_with_public_artifact_link(
            pending_text,
            &public_url,
        ));
    }
    if !reply
        .artifact_links
        .iter()
        .any(|link| link.trim() == public_url)
    {
        reply.artifact_links.insert(0, public_url.clone());
    }
    if let Some(Value::Object(card)) = reply.card.as_mut() {
        if raw_status == "static_page_stable_artifact_reused" || !provisional_existing_artifact {
            card.insert(
                "status".to_string(),
                Value::String("static_page_published".to_string()),
            );
        }
        for key in [
            "public_url",
            "generated_artifact_url",
            "download_url",
            "html_download_url",
        ] {
            card.entry(key.to_string())
                .or_insert_with(|| Value::String(public_url.clone()));
        }
        card.entry("artifact_links".to_string())
            .or_insert_with(|| json!([public_url]));
    }
    reply
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(
        reply_type: ExternalBotReplyTypeView,
        text: Option<&str>,
        task_status: Option<&str>,
        card: Value,
        links: Vec<&str>,
    ) -> ExternalBotReplyView {
        ExternalBotReplyView {
            target_conversation_external_id: "room-1".to_string(),
            reply_type,
            text: text.map(ToOwned::to_owned),
            card: Some(card),
            artifact_links: links.into_iter().map(ToOwned::to_owned).collect(),
            task_status: task_status.map(ToOwned::to_owned),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        }
    }

    #[test]
    fn terminal_static_page_reply_promotes_artifact_link_and_card_urls() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html";
        let output = external_channel_static_page_reply_with_public_artifact_terminal(reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some("页面生成完成"),
            Some("static_page_published"),
            json!({"type": "v3_static_page_image2_pipeline"}),
            vec![public_url],
        ));

        assert_eq!(output.reply_type, ExternalBotReplyTypeView::ArtifactLink);
        assert_eq!(output.task_status.as_deref(), Some("static_page_published"));
        assert_eq!(output.artifact_links, vec![public_url.to_string()]);
        assert!(output
            .text
            .as_deref()
            .unwrap_or_default()
            .contains(public_url));
        let card = output.card.as_ref().expect("card");
        assert_eq!(card["status"], json!("static_page_published"));
        assert_eq!(card["public_url"], json!(public_url));
        assert_eq!(card["generated_artifact_url"], json!(public_url));
    }

    #[test]
    fn terminal_static_page_reply_preserves_direct_answer_text() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html";
        let output = external_channel_static_page_reply_with_public_artifact_terminal(reply(
            ExternalBotReplyTypeView::Text,
            Some("这是模型正常回答。"),
            Some("answered"),
            json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_published",
                "public_url": public_url
            }),
            Vec::new(),
        ));

        assert_eq!(output.reply_type, ExternalBotReplyTypeView::Text);
        assert_eq!(output.task_status.as_deref(), Some("answered"));
        let text = output.text.as_deref().unwrap_or_default();
        assert!(text.starts_with("这是模型正常回答。"));
        assert!(text.contains(public_url));
        assert_eq!(output.artifact_links, vec![public_url.to_string()]);
    }

    #[test]
    fn terminal_static_page_reply_keeps_nonaccepted_provisional_reply_processing() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/current/index.html";
        let output = external_channel_static_page_reply_with_public_artifact_terminal(reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some("已先返回已有静态页链接，后台继续刷新。"),
            Some("processing"),
            json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_image2_auto_publish_pending",
                "public_url": public_url,
                "provisional_existing_artifact": true,
                "provisional_existing_artifact_reason": "waiting_for_image2"
            }),
            vec![public_url],
        ));

        assert_eq!(output.reply_type, ExternalBotReplyTypeView::TaskStatus);
        assert_eq!(output.task_status.as_deref(), Some("processing"));
        assert_eq!(
            output.text.as_deref(),
            Some("已先返回已有静态页链接，后台继续刷新。")
        );
        assert_eq!(output.artifact_links, vec![public_url.to_string()]);
        assert_eq!(
            output.card.as_ref().expect("card")["status"],
            json!("static_page_image2_auto_publish_pending")
        );
    }

    #[test]
    fn terminal_static_page_reply_allows_accepted_template_baseline_link() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/current/index.html";
        let output = external_channel_static_page_reply_with_public_artifact_terminal(reply(
            ExternalBotReplyTypeView::TaskStatus,
            Some("已匹配模板。"),
            Some("processing"),
            json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_image2_auto_publish_pending",
                "public_url": public_url,
                "provisional_existing_artifact": true,
                "provisional_existing_artifact_reason": "accepted_dataset_overlap_template_baseline"
            }),
            Vec::new(),
        ));

        assert_eq!(output.reply_type, ExternalBotReplyTypeView::TaskStatus);
        assert_eq!(output.task_status.as_deref(), Some("processing"));
        assert_eq!(output.artifact_links, vec![public_url.to_string()]);
        assert!(output
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("已依据客户需求生成可查看的报表页面"));
        assert_eq!(
            output.card.as_ref().expect("card")["status"],
            json!("static_page_image2_auto_publish_pending")
        );
    }
}
