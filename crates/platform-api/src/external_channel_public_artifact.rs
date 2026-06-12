use std::collections::HashSet;

use contracts::ExternalBotReplyView;
use serde_json::Value;

use crate::codex_host_fixed_task_public_artifact_url_allowed;

pub(crate) fn external_channel_public_artifact_url_from_links_value(
    value: Option<&Value>,
) -> Option<String> {
    value.and_then(Value::as_array).and_then(|links| {
        links.iter().find_map(|link| {
            link.as_str()
                .map(str::trim)
                .filter(|url| codex_host_fixed_task_public_artifact_url_allowed(url))
                .map(ToOwned::to_owned)
        })
    })
}

pub(crate) fn external_channel_public_artifact_url_from_value(value: &Value) -> Option<String> {
    let direct = [
        "public_url",
        "generated_artifact_url",
        "artifact_public_url",
        "html_download_url",
        "download_url",
    ];
    for key in direct {
        if let Some(url) = value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| codex_host_fixed_task_public_artifact_url_allowed(url))
        {
            return Some(url.to_string());
        }
    }
    for pointer in [
        "/finalPage/public_url",
        "/finalPage/generated_artifact_url",
        "/final_page/public_url",
        "/final_page/generated_artifact_url",
        "/artifact/public_url",
        "/output/artifact_public_url",
    ] {
        if let Some(url) = value
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| codex_host_fixed_task_public_artifact_url_allowed(url))
        {
            return Some(url.to_string());
        }
    }
    value
        .get("artifact_links")
        .and_then(Value::as_array)
        .and_then(|links| {
            links.iter().find_map(|link| {
                link.as_str()
                    .map(str::trim)
                    .filter(|url| codex_host_fixed_task_public_artifact_url_allowed(url))
                    .map(ToOwned::to_owned)
            })
        })
}

pub(crate) fn external_channel_public_artifact_url_from_reply(
    reply: &ExternalBotReplyView,
) -> Option<String> {
    reply
        .artifact_links
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .find(|url| codex_host_fixed_task_public_artifact_url_allowed(url))
        .map(ToOwned::to_owned)
        .or_else(|| {
            reply
                .card
                .as_ref()
                .and_then(external_channel_public_artifact_url_from_value)
        })
}

pub(crate) fn external_channel_text_with_public_artifact_link(
    text: impl Into<String>,
    public_url: &str,
) -> String {
    let mut text = text.into();
    let public_url = public_url.trim();
    if public_url.is_empty() || text.contains(public_url) {
        return text;
    }
    if !text.trim().is_empty() {
        text.push_str("\n\n");
    }
    text.push_str("页面链接：[点击查看报表](");
    text.push_str(public_url);
    text.push(')');
    text
}

pub(crate) fn dedupe_external_channel_public_artifact_links(links: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for link in links {
        let trimmed = link.trim();
        if trimmed.is_empty() || !codex_host_fixed_task_public_artifact_url_allowed(trimmed) {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            output.push(trimmed.to_string());
        }
        if output.len() >= 1 {
            break;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn artifact_url(path: &str) -> String {
        format!("https://v3.elepcloud.com/generated-artifacts/{path}")
    }

    #[test]
    fn external_channel_public_artifact_url_from_value_prefers_direct_public_url() {
        let direct = artifact_url("reports/current/index.html");
        let fallback = artifact_url("reports/fallback/index.html");
        let value = json!({
            "public_url": direct,
            "artifact_links": [fallback],
        });

        assert_eq!(
            external_channel_public_artifact_url_from_value(&value),
            Some(direct)
        );
    }

    #[test]
    fn external_channel_public_artifact_url_from_value_reads_nested_public_url() {
        let public_url = artifact_url("reports/nested/index.html");
        let value = json!({
            "final_page": {
                "generated_artifact_url": public_url,
            },
        });

        assert_eq!(
            external_channel_public_artifact_url_from_value(&value),
            Some(public_url)
        );
    }

    #[test]
    fn dedupe_external_channel_public_artifact_links_keeps_first_allowed_link_only() {
        let first = artifact_url("reports/a/index.html");
        let second = artifact_url("reports/b/index.html");

        assert_eq!(
            dedupe_external_channel_public_artifact_links(vec![
                "https://example.com/not-allowed".to_string(),
                format!(" {first} "),
                first.clone(),
                second,
            ]),
            vec![first]
        );
    }

    #[test]
    fn external_channel_text_with_public_artifact_link_appends_once() {
        let public_url = artifact_url("reports/current/index.html");
        let text = external_channel_text_with_public_artifact_link("已生成。", &public_url);
        let repeated = external_channel_text_with_public_artifact_link(text.clone(), &public_url);

        assert!(text.contains("页面链接：[点击查看报表]("));
        assert_eq!(repeated, text);
    }
}
