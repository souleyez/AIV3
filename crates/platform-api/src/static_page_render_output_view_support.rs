use domain_model::{
    StaticPageRenderOutput, StaticPageRenderOutputId, StaticPageRenderOutputStatus,
};
use serde_json::Value;

pub(crate) fn static_page_html_download_url(
    selected_scope: Option<&Value>,
    output: &StaticPageRenderOutput,
) -> Option<String> {
    if !matches!(output.status, StaticPageRenderOutputStatus::Rendered)
        || output.html.trim().is_empty()
    {
        return None;
    }
    selected_scope
        .and_then(|scope| static_page_external_html_download_url(scope, output.id))
        .or_else(|| {
            Some(format!(
                "/v1/static-page-render-outputs/{}/download",
                output.id
            ))
        })
}

pub(crate) fn static_page_html_preview_url(
    selected_scope: Option<&Value>,
    output: &StaticPageRenderOutput,
) -> Option<String> {
    if !matches!(output.status, StaticPageRenderOutputStatus::Rendered)
        || output.html.trim().is_empty()
    {
        return None;
    }
    selected_scope
        .and_then(|scope| static_page_external_html_preview_url(scope, output.id))
        .or_else(|| {
            Some(format!(
                "/v1/static-page-render-outputs/{}/preview",
                output.id
            ))
        })
}

pub(crate) fn static_page_external_html_download_url(
    selected_scope: &Value,
    render_output_id: StaticPageRenderOutputId,
) -> Option<String> {
    if value_at_any_key(selected_scope, &["type", "scope_type", "scopeType"])
        .and_then(Value::as_str)
        != Some("external_channel")
    {
        return None;
    }
    let channel_connection_id = value_at_any_key(
        selected_scope,
        &["channel_connection_id", "channelConnectionId"],
    )
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|value| !value.is_empty())?;
    Some(format!(
        "/v1/external/channels/{}/static-page-renders/{}/download",
        encode_url_path_segment(channel_connection_id),
        render_output_id
    ))
}

pub(crate) fn static_page_external_html_preview_url(
    selected_scope: &Value,
    render_output_id: StaticPageRenderOutputId,
) -> Option<String> {
    if value_at_any_key(selected_scope, &["type", "scope_type", "scopeType"])
        .and_then(Value::as_str)
        != Some("external_channel")
    {
        return None;
    }
    let channel_connection_id = value_at_any_key(
        selected_scope,
        &["channel_connection_id", "channelConnectionId"],
    )
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|value| !value.is_empty())?;
    Some(format!(
        "/v1/external/channels/{}/static-page-renders/{}/preview",
        encode_url_path_segment(channel_connection_id),
        render_output_id
    ))
}

pub(crate) fn static_page_render_output_retryable_error_reason(
    output: &StaticPageRenderOutput,
) -> Option<String> {
    if !matches!(output.status, StaticPageRenderOutputStatus::Failed) {
        return None;
    }
    [
        "/retryable_error_reason",
        "/retryableErrorReason",
        "/failure_reason",
        "/failureReason",
        "/last_error",
        "/lastError",
        "/error/reason",
        "/error/message",
        "/workflow/error/reason",
        "/workflow/error/message",
    ]
    .iter()
    .find_map(|pointer| {
        output
            .asset_manifest
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

pub(crate) fn encode_url_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '~') {
            encoded.push(ch);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn value_at_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftId, TenantId};
    use serde_json::json;

    use super::*;

    fn render_output(
        status: StaticPageRenderOutputStatus,
        html: &str,
        asset_manifest: Value,
    ) -> StaticPageRenderOutput {
        StaticPageRenderOutput {
            id: StaticPageRenderOutputId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            image_job_id: None,
            status,
            html: html.to_string(),
            asset_manifest,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn rendered_output_uses_external_scope_or_internal_fallback_urls() {
        let output = render_output(
            StaticPageRenderOutputStatus::Rendered,
            "<html></html>",
            json!({}),
        );
        let external_scope = json!({
            "type": "external_channel",
            "channelConnectionId": "generic chat/主通道"
        });

        assert_eq!(
            static_page_html_download_url(Some(&external_scope), &output),
            Some(format!(
                "/v1/external/channels/generic%20chat%2F%E4%B8%BB%E9%80%9A%E9%81%93/static-page-renders/{}/download",
                output.id
            ))
        );
        assert_eq!(
            static_page_html_preview_url(Some(&external_scope), &output),
            Some(format!(
                "/v1/external/channels/generic%20chat%2F%E4%B8%BB%E9%80%9A%E9%81%93/static-page-renders/{}/preview",
                output.id
            ))
        );
        assert_eq!(
            static_page_html_download_url(None, &output),
            Some(format!(
                "/v1/static-page-render-outputs/{}/download",
                output.id
            ))
        );
    }

    #[test]
    fn non_rendered_or_empty_output_has_no_html_urls() {
        let failed = render_output(
            StaticPageRenderOutputStatus::Failed,
            "<html></html>",
            json!({}),
        );
        let empty = render_output(StaticPageRenderOutputStatus::Rendered, "   ", json!({}));

        assert_eq!(static_page_html_download_url(None, &failed), None);
        assert_eq!(static_page_html_preview_url(None, &failed), None);
        assert_eq!(static_page_html_download_url(None, &empty), None);
        assert_eq!(static_page_html_preview_url(None, &empty), None);
    }

    #[test]
    fn retryable_failure_reason_uses_first_non_empty_manifest_field() {
        let output = render_output(
            StaticPageRenderOutputStatus::Failed,
            "",
            json!({
                "failure_reason": " ",
                "workflow": {
                    "error": {
                        "reason": "renderer timeout"
                    }
                }
            }),
        );

        assert_eq!(
            static_page_render_output_retryable_error_reason(&output).as_deref(),
            Some("renderer timeout")
        );
    }

    #[test]
    fn url_path_segment_encoding_preserves_unreserved_ascii() {
        assert_eq!(
            encode_url_path_segment("abc-._~ /主"),
            "abc-._~%20%2F%E4%B8%BB"
        );
    }
}
