use contracts::ExternalBotReplyView;
use serde_json::Value;

use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, external_channel_static_page_published_reply,
};

pub(crate) fn external_channel_static_page_artifact_reply_from_output_artifacts(
    output_artifacts: &Value,
    conversation_external_id: &str,
) -> Option<ExternalBotReplyView> {
    let artifact = output_artifacts.as_array()?.iter().rev().find(|artifact| {
        artifact.get("type").and_then(Value::as_str)
            == Some("external_channel_static_page_artifact")
            && artifact
                .get("public_url")
                .and_then(Value::as_str)
                .map(codex_host_fixed_task_public_artifact_url_allowed)
                .unwrap_or(false)
    })?;
    let public_url = artifact.get("public_url").and_then(Value::as_str)?;
    Some(external_channel_static_page_published_reply(
        conversation_external_id,
        public_url,
        artifact,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_artifact_reply_uses_latest_allowed_static_page_artifact() {
        let older_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/older/index.html";
        let latest_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/latest/index.html";
        let output_artifacts = json!([
            {
                "type": "external_channel_static_page_artifact",
                "public_url": older_url
            },
            {
                "type": "other_artifact",
                "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/ignored/index.html"
            },
            {
                "type": "external_channel_static_page_artifact",
                "public_url": latest_url
            }
        ]);

        let reply = external_channel_static_page_artifact_reply_from_output_artifacts(
            &output_artifacts,
            "room-1",
        )
        .expect("latest valid static page artifact should become a reply");

        assert_eq!(reply.target_conversation_external_id, "room-1");
        assert_eq!(reply.artifact_links, vec![latest_url.to_string()]);
        assert_eq!(
            reply.card.as_ref().expect("card")["public_url"],
            json!(latest_url)
        );
    }

    #[test]
    fn output_artifact_reply_rejects_invalid_or_wrong_type_artifacts() {
        let output_artifacts = json!([
            {
                "type": "external_channel_static_page_artifact",
                "public_url": "https://example.com/not-allowed/index.html"
            },
            {
                "type": "other_artifact",
                "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/wrong-type/index.html"
            }
        ]);

        assert!(
            external_channel_static_page_artifact_reply_from_output_artifacts(
                &output_artifacts,
                "room-1"
            )
            .is_none()
        );
    }
}
