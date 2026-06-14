use std::path::PathBuf;

use contracts::{ExternalBotMessageView, HtmlArtifactDataRefView};

use crate::{
    encode_url_path_segment, external_document_template_skill_document_id,
    external_document_template_skill_external_id, external_requested_skill_is_document_template,
    external_requested_skill_mode, ApiError,
};

pub(crate) fn external_template_html_artifact_data_refs(
    message: &ExternalBotMessageView,
) -> Vec<HtmlArtifactDataRefView> {
    message
        .requested_skills
        .iter()
        .filter(|skill| external_requested_skill_mode(skill) != "disabled")
        .filter(|skill| external_requested_skill_is_document_template(skill))
        .filter_map(|skill| {
            external_document_template_skill_external_id(skill)
                .map(|id| HtmlArtifactDataRefView {
                    kind: "template_document_external_id".to_string(),
                    label: "第三方模板文档".to_string(),
                    id,
                })
                .or_else(|| {
                    external_document_template_skill_document_id(skill).map(|id| {
                        HtmlArtifactDataRefView {
                            kind: "template_document_id".to_string(),
                            label: "第三方模板文档".to_string(),
                            id: id.to_string(),
                        }
                    })
                })
        })
        .collect()
}

pub(crate) fn external_channel_template_html_artifact_root() -> PathBuf {
    std::env::var("EXTERNAL_CHANNEL_HTML_ARTIFACT_DIR")
        .ok()
        .map(|value| PathBuf::from(value.trim()))
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| std::env::temp_dir().join("aidp-v3-external-template-html"))
}

pub(crate) fn external_channel_html_artifact_download_url(
    connection_id: &str,
    artifact_id: &str,
    file_index: usize,
) -> String {
    format!(
        "/v1/external/channels/{}/html-artifacts/{}/files/{}",
        encode_url_path_segment(connection_id),
        encode_url_path_segment(artifact_id),
        file_index
    )
}

pub(crate) fn extract_external_template_html_from_model_output(
    output_text: &str,
) -> Option<String> {
    extract_fenced_external_template_html(output_text)
        .or_else(|| {
            let trimmed = output_text.trim();
            external_template_html_looks_like_html(trimmed).then(|| trimmed.to_string())
        })
        .map(|html| html.trim().to_string())
        .filter(|html| !html.is_empty())
}

fn extract_fenced_external_template_html(output_text: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(fence_start) = output_text[search_from..].find("```") {
        let fence_start = search_from + fence_start;
        let info_start = fence_start + 3;
        let line_end = output_text[info_start..]
            .find('\n')
            .map(|offset| info_start + offset)?;
        let info = output_text[info_start..line_end]
            .trim()
            .to_ascii_lowercase();
        let body_start = line_end + 1;
        let close = output_text[body_start..].find("```")?;
        if info
            .split_whitespace()
            .next()
            .is_some_and(|value| value == "html" || value == "htm" || value == "xhtml")
        {
            let body = &output_text[body_start..body_start + close];
            return external_template_html_looks_like_html(body.trim()).then(|| body.to_string());
        }
        search_from = body_start + close + 3;
    }
    None
}

fn external_template_html_looks_like_html(value: &str) -> bool {
    let trimmed = value.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("<!doctype")
        || lower.starts_with("<html")
        || (trimmed.starts_with('<') && trimmed.contains('>') && trimmed.contains("</"))
}

pub(crate) fn validate_external_template_html_artifact_content(
    html: &str,
) -> std::result::Result<(), ApiError> {
    const MAX_EXTERNAL_TEMPLATE_HTML_CHARS: usize = 1_000_000;
    if html.chars().count() > MAX_EXTERNAL_TEMPLATE_HTML_CHARS {
        return Err(ApiError::bad_request(
            "external_template_html_artifact_too_large",
            "template HTML artifact is too large to persist".to_string(),
        ));
    }
    let lower = html.to_ascii_lowercase();
    let denied_fragments = [
        "<script",
        "</script",
        "<iframe",
        "<object",
        "<embed",
        "<form",
        "<base",
        "<link",
        "javascript:",
        "data:",
        "http://",
        "https://",
        "srcdoc",
        "@import",
        "url(",
    ];
    if let Some(fragment) = denied_fragments
        .iter()
        .find(|fragment| lower.contains(**fragment))
    {
        return Err(ApiError::bad_request(
            "external_template_html_artifact_unsafe",
            format!("template HTML artifact contains unsupported content: {fragment}"),
        ));
    }
    let denied_event_attributes = [
        "onload",
        "onclick",
        "onerror",
        "onmouseover",
        "onfocus",
        "onchange",
        "onsubmit",
        "onanimation",
        "ontransition",
    ];
    if let Some(attribute) = denied_event_attributes.iter().find(|attribute| {
        lower.contains(&format!("{attribute}=")) || lower.contains(&format!("{attribute} ="))
    }) {
        return Err(ApiError::bad_request(
            "external_template_html_artifact_unsafe",
            format!("template HTML artifact contains unsupported event attribute: {attribute}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::ExternalRequestedSkillView;
    use serde_json::json;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn clear_template_html_env() {
        std::env::remove_var("EXTERNAL_CHANNEL_HTML_ARTIFACT_DIR");
    }

    fn template_skill(arguments: serde_json::Value) -> ExternalRequestedSkillView {
        ExternalRequestedSkillView {
            skill_id: "document_template".to_string(),
            version: None,
            mode: Some("preferred".to_string()),
            arguments: Some(arguments),
        }
    }

    fn message_with_skills(skills: Vec<ExternalRequestedSkillView>) -> ExternalBotMessageView {
        ExternalBotMessageView {
            platform: contracts::ExternalChannelPlatformView::GenericChat,
            tenant_external_id: "tenant-ext-001".to_string(),
            bot_external_id: "bot-v3".to_string(),
            conversation_external_id: "conv-001".to_string(),
            thread_external_id: None,
            sender_external_id: "user-default".to_string(),
            message_external_id: "msg-001".to_string(),
            message_type: contracts::ExternalMessageTypeView::Text,
            text: Some("template".to_string()),
            default_prompt: None,
            output_format: None,
            render_mode: None,
            artifact_type: None,
            template: None,
            mention_external_user_ids: Vec::new(),
            attachment_refs: Vec::new(),
            business_datasource_ids: Vec::new(),
            available_document_source_id: None,
            available_document_external_ids: Vec::new(),
            dataset_external_id: None,
            dataset_external_ids: Vec::new(),
            requested_skills: skills,
            idempotency_key: "idem-001".to_string(),
            received_at: Utc::now(),
        }
    }

    #[test]
    fn extracts_fenced_and_raw_template_html_only_when_it_looks_like_html() {
        let fenced = extract_external_template_html_from_model_output(
            "说明如下：\n```html\n<section><h1>人员说明报告</h1></section>\n```",
        )
        .expect("fenced HTML should be extracted");
        assert_eq!(fenced, "<section><h1>人员说明报告</h1></section>");

        let raw = extract_external_template_html_from_model_output(
            "  <!doctype html><html><body>OK</body></html>  ",
        )
        .expect("raw HTML should be extracted");
        assert!(raw.starts_with("<!doctype html>"));

        assert!(extract_external_template_html_from_model_output("普通文字").is_none());
        assert!(extract_external_template_html_from_model_output("```json\n{}\n```").is_none());
    }

    #[test]
    fn validates_template_html_safety_and_size() {
        validate_external_template_html_artifact_content(
            "<section><h1>人员说明报告</h1></section>",
        )
        .expect("simple HTML should be accepted");

        let unsafe_error = validate_external_template_html_artifact_content(
            "<section onclick=\"alert(1)\"></section>",
        )
        .expect_err("event handlers should be rejected");
        assert_eq!(
            unsafe_error.payload.code,
            "external_template_html_artifact_unsafe"
        );

        let script_error =
            validate_external_template_html_artifact_content("<script>alert(1)</script>")
                .expect_err("script tags should be rejected");
        assert_eq!(
            script_error.payload.code,
            "external_template_html_artifact_unsafe"
        );

        let huge = "x".repeat(1_000_001);
        let large_error = validate_external_template_html_artifact_content(&huge)
            .expect_err("oversized HTML should be rejected");
        assert_eq!(
            large_error.payload.code,
            "external_template_html_artifact_too_large"
        );
    }

    #[test]
    fn template_html_artifact_root_requires_absolute_env_value() {
        let _guard = env_lock();
        clear_template_html_env();

        assert!(external_channel_template_html_artifact_root()
            .ends_with("aidp-v3-external-template-html"));

        let absolute = std::env::temp_dir().join(format!(
            "aidp-v3-external-template-html-root-{}",
            Uuid::new_v4()
        ));
        std::env::set_var("EXTERNAL_CHANNEL_HTML_ARTIFACT_DIR", &absolute);
        assert_eq!(external_channel_template_html_artifact_root(), absolute);

        std::env::set_var("EXTERNAL_CHANNEL_HTML_ARTIFACT_DIR", "relative/path");
        assert!(external_channel_template_html_artifact_root()
            .ends_with("aidp-v3-external-template-html"));

        clear_template_html_env();
    }

    #[test]
    fn template_html_artifact_download_url_encodes_path_segments() {
        let url = external_channel_html_artifact_download_url(
            "generic/chat main",
            "html-artifact:external template/一",
            12,
        );

        assert_eq!(
            url,
            "/v1/external/channels/generic%2Fchat%20main/html-artifacts/html-artifact%3Aexternal%20template%2F%E4%B8%80/files/12"
        );
    }

    #[test]
    fn template_html_artifact_data_refs_skip_disabled_and_prefer_external_id() {
        let document_id = Uuid::new_v4();
        let message = message_with_skills(vec![
            template_skill(json!({
                "templateDocumentExternalId": "doc-ext-001",
                "template_document_id": document_id.to_string()
            })),
            ExternalRequestedSkillView {
                skill_id: "document_template".to_string(),
                version: None,
                mode: Some("disabled".to_string()),
                arguments: Some(json!({"templateDocumentExternalId": "disabled-doc"})),
            },
            template_skill(json!({"template_document_id": document_id.to_string()})),
        ]);

        let refs = external_template_html_artifact_data_refs(&message);

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].kind, "template_document_external_id");
        assert_eq!(refs[0].id, "doc-ext-001");
        assert_eq!(refs[1].kind, "template_document_id");
        assert_eq!(refs[1].id, document_id.to_string());
    }
}
