use std::collections::HashSet;

use contracts::ExternalBotReplyView;
use serde_json::{json, Value};

use crate::{codex_host_fixed_task_public_artifact_url_allowed, static_page_artifact_sibling_url};

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

pub(crate) fn external_channel_static_page_artifact_payload_value(
    payload: &Value,
    key: &str,
) -> Value {
    payload
        .get(key)
        .cloned()
        .or_else(|| {
            payload
                .get("artifact")
                .and_then(|artifact| artifact.get(key))
                .cloned()
        })
        .or_else(|| {
            payload
                .get("output")
                .and_then(|output| output.get(key))
                .cloned()
        })
        .or_else(|| {
            payload
                .get("output")
                .and_then(|output| output.get("artifact"))
                .and_then(|artifact| artifact.get(key))
                .cloned()
        })
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_static_page_download_exports(
    public_url: &str,
    data_url: Value,
    report_title: Option<&str>,
) -> Value {
    let title = report_title.unwrap_or("DataMax 经营分析报表");
    let table_url = static_page_artifact_sibling_url(public_url, "table-data.csv")
        .map(Value::String)
        .unwrap_or(Value::Null);
    let ppt_url = static_page_artifact_sibling_url(public_url, "report.ppt")
        .map(Value::String)
        .unwrap_or(Value::Null);
    let md_url = static_page_artifact_sibling_url(public_url, "report.md")
        .map(Value::String)
        .unwrap_or(Value::Null);
    json!([
        {
            "kind": "table_data",
            "label": "表格数据",
            "format": "csv",
            "title": title,
            "url": table_url,
            "data_url": data_url,
        },
        {
            "kind": "ppt",
            "label": "导出PPT",
            "format": "ppt",
            "title": title,
            "url": ppt_url,
        },
        {
            "kind": "markdown",
            "label": "文本下载（MD）",
            "format": "md",
            "title": title,
            "url": md_url,
        },
    ])
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

    #[test]
    fn static_page_download_exports_builds_standard_report_files() {
        let public_url = artifact_url("reports/current/index.html");
        let data_url = json!(artifact_url("reports/current/data.json"));

        let exports = external_channel_static_page_download_exports(
            &public_url,
            data_url.clone(),
            Some("新世界百货经营管理月报表"),
        );

        assert_eq!(exports[0]["kind"], json!("table_data"));
        assert_eq!(exports[0]["label"], json!("表格数据"));
        assert_eq!(exports[0]["format"], json!("csv"));
        assert_eq!(exports[0]["title"], json!("新世界百货经营管理月报表"));
        assert_eq!(
            exports[0]["url"],
            json!(artifact_url("reports/current/table-data.csv"))
        );
        assert_eq!(exports[0]["data_url"], data_url);
        assert_eq!(exports[1]["kind"], json!("ppt"));
        assert_eq!(exports[1]["label"], json!("导出PPT"));
        assert_eq!(
            exports[1]["url"],
            json!(artifact_url("reports/current/report.ppt"))
        );
        assert_eq!(exports[2]["kind"], json!("markdown"));
        assert_eq!(exports[2]["label"], json!("文本下载（MD）"));
        assert_eq!(
            exports[2]["url"],
            json!(artifact_url("reports/current/report.md"))
        );
    }

    #[test]
    fn static_page_artifact_payload_value_uses_existing_precedence() {
        let payload = json!({
            "public_url": "root",
            "artifact": {
                "public_url": "artifact",
                "data_url": "artifact-data"
            },
            "output": {
                "data_url": "output-data",
                "artifact": {
                    "data_url": "output-artifact-data",
                    "report_url": "output-artifact-report"
                }
            }
        });

        assert_eq!(
            external_channel_static_page_artifact_payload_value(&payload, "public_url"),
            json!("root")
        );
        assert_eq!(
            external_channel_static_page_artifact_payload_value(&payload, "data_url"),
            json!("artifact-data")
        );
        assert_eq!(
            external_channel_static_page_artifact_payload_value(&payload, "report_url"),
            json!("output-artifact-report")
        );
        assert_eq!(
            external_channel_static_page_artifact_payload_value(&payload, "missing"),
            Value::Null
        );
    }
}
