use std::collections::HashSet;

use contracts::ExternalBotReplyView;
use serde_json::{json, Value};

use crate::assistant_run_xinbai_report_link_support::XINBAI_PUBLISHED_REPORT_TITLE;
use crate::external_channel_static_page_card_defaults::{
    external_channel_static_page_card_insert_if_missing,
    external_channel_static_page_card_title_missing_or_generic,
    external_channel_static_page_report_title_is_generic,
};
use crate::external_channel_static_page_focus::external_channel_static_page_public_url_with_payload_focus;
use crate::external_channel_static_page_template_reference_support::{
    external_channel_static_page_template_reference_from_payload,
    external_channel_static_page_template_reference_id_from_payload,
};
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

pub(crate) fn external_channel_static_page_baseline_public_url(payload: &Value) -> Option<String> {
    external_channel_public_artifact_url_from_value(payload).or_else(|| {
        [
            "/visual_contract_url",
            "/relaxed_template_match/baseline_public_url",
            "/template_reference/public_url",
            "/template_reference/publicUrl",
        ]
        .into_iter()
        .filter_map(|pointer| payload.pointer(pointer).and_then(Value::as_str))
        .map(str::trim)
        .filter(|url| codex_host_fixed_task_public_artifact_url_allowed(url))
        .map(ToOwned::to_owned)
        .next()
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

pub(crate) fn static_page_public_url_without_focus(public_url: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(public_url.trim()).ok()?;
    if !codex_host_fixed_task_public_artifact_url_allowed(url.as_str()) {
        return None;
    }
    let existing_pairs = url
        .query_pairs()
        .filter(|(key, _)| key != "focus")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    if !existing_pairs.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in existing_pairs {
            pairs.append_pair(&key, &value);
        }
    }
    Some(url.to_string())
}

pub(crate) fn static_page_public_url_matches_ignoring_focus(left: &str, right: &str) -> bool {
    let Some(left) = static_page_public_url_without_focus(left) else {
        return false;
    };
    let Some(right) = static_page_public_url_without_focus(right) else {
        return false;
    };
    left == right
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

pub(crate) fn external_channel_static_page_artifact_payload_string(
    payload: &Value,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        external_channel_static_page_artifact_payload_value(payload, key)
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

pub(crate) fn external_channel_static_page_report_title(
    payload: &Value,
    public_url: &str,
) -> String {
    if let Some(title) = external_channel_static_page_artifact_payload_string(
        payload,
        &[
            "report_title",
            "reportTitle",
            "display_title",
            "displayTitle",
            "artifact_title",
            "artifactTitle",
            "title",
        ],
    ) {
        if !external_channel_static_page_report_title_is_generic(&title) {
            return title;
        }
        if external_channel_static_page_is_xinbai_primary_report(payload, public_url) {
            return XINBAI_PUBLISHED_REPORT_TITLE.to_string();
        }
    }
    if external_channel_static_page_is_xinbai_primary_report(payload, public_url) {
        return XINBAI_PUBLISHED_REPORT_TITLE.to_string();
    }
    "DataMax 经营分析报表".to_string()
}

pub(crate) fn external_channel_static_page_is_xinbai_primary_report(
    payload: &Value,
    public_url: &str,
) -> bool {
    let lower_url = public_url.to_ascii_lowercase();
    if lower_url.contains("xinbai-functional-modular-template-20260604")
        || lower_url.contains("xinbai-functional-modular-report")
        || lower_url.contains("/xinbai/")
    {
        return true;
    }
    let template_reference_id =
        external_channel_static_page_template_reference_id_from_payload(payload);
    let template_reference_id = template_reference_id
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if template_reference_id.contains("xinbai-functional-modular-template-20260604")
        || template_reference_id.contains("xinbai_business_report")
    {
        return true;
    }
    let template_reference = external_channel_static_page_template_reference_from_payload(payload);
    let template_reference_text = template_reference.to_string().to_ascii_lowercase();
    template_reference_text.contains("xinbai")
        || template_reference_text.contains("新百")
        || template_reference_text.contains("新世界百货")
}

pub(crate) fn external_channel_static_page_export_url(
    payload: &Value,
    public_url: &str,
    keys: &[&str],
    file_name: &str,
) -> Value {
    for key in keys {
        if let Some(url) = external_channel_static_page_artifact_payload_value(payload, key)
            .as_str()
            .map(str::trim)
            .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
            .map(ToOwned::to_owned)
        {
            return json!(url);
        }
    }
    if external_channel_static_page_is_xinbai_primary_report(payload, public_url) {
        return static_page_artifact_sibling_url(public_url, file_name)
            .map(Value::String)
            .unwrap_or(Value::Null);
    }
    Value::Null
}

pub(crate) fn external_channel_static_page_data_url(payload: &Value, public_url: &str) -> Value {
    external_channel_static_page_export_url(
        payload,
        public_url,
        &["data_url", "dataUrl"],
        "data.json",
    )
}

pub(crate) fn external_channel_static_page_download_exports_from_payload(
    payload: &Value,
    public_url: &str,
    data_url: Value,
    report_title: &str,
) -> Value {
    if let Some(exports) = payload
        .get("download_exports")
        .or_else(|| payload.get("downloadExports"))
        .cloned()
        .filter(|value| {
            value
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        })
    {
        return exports;
    }
    if external_channel_static_page_is_xinbai_primary_report(payload, public_url) {
        return external_channel_static_page_download_exports(
            public_url,
            data_url,
            Some(report_title),
        );
    }
    json!([])
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

pub(crate) fn external_channel_static_page_enrich_reply_card(reply: &mut ExternalBotReplyView) {
    if let Some(card) = reply.card.as_mut() {
        external_channel_static_page_enrich_report_card(card);
    }
}

pub(crate) fn external_channel_static_page_enrich_report_card(card: &mut Value) {
    let Some(raw_public_url) = external_channel_public_artifact_url_from_value(card) else {
        return;
    };
    let public_url =
        external_channel_static_page_public_url_with_payload_focus(&raw_public_url, card);
    let report_title = external_channel_static_page_report_title(card, &public_url);
    let data_url = external_channel_static_page_data_url(card, &public_url);
    let table_data_url = external_channel_static_page_export_url(
        card,
        &public_url,
        &["table_data_url", "tableDataUrl", "csv_url", "csvUrl"],
        "table-data.csv",
    );
    let ppt_download_url = external_channel_static_page_export_url(
        card,
        &public_url,
        &["ppt_download_url", "pptDownloadUrl", "ppt_url", "pptUrl"],
        "report.ppt",
    );
    let markdown_download_url = external_channel_static_page_export_url(
        card,
        &public_url,
        &[
            "markdown_download_url",
            "markdownDownloadUrl",
            "text_download_url",
            "textDownloadUrl",
            "md_url",
            "mdUrl",
        ],
        "report.md",
    );
    let download_exports = external_channel_static_page_download_exports_from_payload(
        card,
        &public_url,
        data_url.clone(),
        &report_title,
    );

    let Some(object) = card.as_object_mut() else {
        return;
    };
    for key in ["title", "report_title", "display_title"] {
        if external_channel_static_page_card_title_missing_or_generic(object.get(key)) {
            object.insert(key.to_string(), Value::String(report_title.clone()));
        }
    }
    if public_url != raw_public_url {
        for key in [
            "public_url",
            "generated_artifact_url",
            "download_url",
            "html_download_url",
        ] {
            if object
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|value| value.trim() == raw_public_url)
                || key == "public_url"
            {
                object.insert(key.to_string(), Value::String(public_url.clone()));
            }
        }
        object.insert("artifact_links".to_string(), json!([public_url.clone()]));
    }
    external_channel_static_page_card_insert_if_missing(object, "data_url", data_url);
    external_channel_static_page_card_insert_if_missing(object, "table_data_url", table_data_url);
    external_channel_static_page_card_insert_if_missing(
        object,
        "ppt_download_url",
        ppt_download_url,
    );
    external_channel_static_page_card_insert_if_missing(
        object,
        "markdown_download_url",
        markdown_download_url.clone(),
    );
    external_channel_static_page_card_insert_if_missing(
        object,
        "text_download_url",
        markdown_download_url,
    );
    external_channel_static_page_card_insert_if_missing(
        object,
        "download_exports",
        download_exports,
    );
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
    fn static_page_baseline_public_url_reads_template_baseline_pointers() {
        let visual_contract_url = artifact_url("reports/visual-contract/index.html");
        let baseline_url = artifact_url("reports/baseline/index.html");
        let template_url = artifact_url("reports/template/index.html");

        let value = json!({
            "public_url": "https://example.com/not-allowed",
            "visual_contract_url": format!(" {visual_contract_url} "),
            "relaxed_template_match": {
                "baseline_public_url": baseline_url,
            },
            "template_reference": {
                "publicUrl": template_url,
            },
        });

        assert_eq!(
            external_channel_static_page_baseline_public_url(&value),
            Some(visual_contract_url)
        );
    }

    #[test]
    fn static_page_baseline_public_url_prefers_standard_public_artifact_url() {
        let direct_url = artifact_url("reports/direct/index.html");
        let baseline_url = artifact_url("reports/baseline/index.html");
        let value = json!({
            "public_url": direct_url,
            "relaxed_template_match": {
                "baseline_public_url": baseline_url,
            },
        });

        assert_eq!(
            external_channel_static_page_baseline_public_url(&value),
            Some(direct_url)
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
    fn static_page_public_url_without_focus_removes_only_focus_query() {
        let public_url =
            artifact_url("reports/current/index.html?focus=%E5%8F%96%E9%AB%98&v=2&mode=live");

        assert_eq!(
            static_page_public_url_without_focus(&public_url),
            Some(artifact_url("reports/current/index.html?v=2&mode=live"))
        );
    }

    #[test]
    fn static_page_public_url_matches_ignoring_focus_keeps_other_query_significant() {
        let left = artifact_url("reports/current/index.html?focus=a&v=2");
        let same = artifact_url("reports/current/index.html?focus=b&v=2");
        let different = artifact_url("reports/current/index.html?focus=a&v=3");

        assert!(static_page_public_url_matches_ignoring_focus(&left, &same));
        assert!(!static_page_public_url_matches_ignoring_focus(
            &left, &different
        ));
        assert!(!static_page_public_url_matches_ignoring_focus(
            "https://example.com/reports/current/index.html?focus=a",
            &same
        ));
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

    #[test]
    fn static_page_artifact_payload_string_trims_and_skips_empty_values() {
        let payload = json!({
            "title": "   ",
            "artifact": {
                "display_title": "  新世界百货经营管理月报表  "
            },
            "output": {
                "report_title": "ignored"
            }
        });

        assert_eq!(
            external_channel_static_page_artifact_payload_string(
                &payload,
                &["title", "display_title", "report_title"]
            ),
            Some("新世界百货经营管理月报表".to_string())
        );
        assert_eq!(
            external_channel_static_page_artifact_payload_string(&payload, &["missing"]),
            None
        );
    }

    #[test]
    fn static_page_report_title_preserves_first_specific_title() {
        let public_url = artifact_url("reports/current/index.html");
        let payload = json!({
            "report_title": "  门店风险专项报表  ",
            "displayTitle": "后备标题",
            "title": "DataMax 经营分析报表",
        });

        assert_eq!(
            external_channel_static_page_report_title(&payload, &public_url),
            "门店风险专项报表"
        );
    }

    #[test]
    fn static_page_report_title_replaces_generic_title_for_xinbai_url() {
        let public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let payload = json!({
            "reportTitle": "DataMax 经营分析报表",
        });

        assert_eq!(
            external_channel_static_page_report_title(&payload, &public_url),
            XINBAI_PUBLISHED_REPORT_TITLE
        );
    }

    #[test]
    fn static_page_report_title_uses_xinbai_template_reference_without_title() {
        let public_url = artifact_url("reports/current/index.html");
        let payload = json!({
            "template_reference": {
                "id": "xinbai-functional-modular-template-20260604"
            },
        });

        assert_eq!(
            external_channel_static_page_report_title(&payload, &public_url),
            XINBAI_PUBLISHED_REPORT_TITLE
        );
    }

    #[test]
    fn static_page_report_title_falls_back_for_generic_non_xinbai_report() {
        let public_url = artifact_url("reports/current/index.html");
        let payload = json!({
            "displayTitle": "DataMax 静态页",
        });

        assert_eq!(
            external_channel_static_page_report_title(&payload, &public_url),
            "DataMax 经营分析报表"
        );
    }

    #[test]
    fn static_page_enrich_report_card_adds_title_focus_and_export_links() {
        let mut card = json!({
            "public_url": artifact_url("xinbai-functional-modular-template-20260604/index.html"),
            "title": "DataMax 静态页",
            "template_adaptation": {
                "userIntent": "生成取高机会"
            }
        });

        external_channel_static_page_enrich_report_card(&mut card);

        let public_url = card["public_url"].as_str().unwrap_or_default();
        assert_eq!(card["title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert_eq!(card["report_title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert_eq!(card["display_title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert!(public_url.contains("focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"));
        assert_eq!(card["artifact_links"], json!([public_url]));
        assert_eq!(
            card["data_url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/data.json"
            ))
        );
        assert_eq!(
            card["table_data_url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/table-data.csv"
            ))
        );
        assert_eq!(
            card["ppt_download_url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/report.ppt"
            ))
        );
        assert_eq!(
            card["markdown_download_url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/report.md"
            ))
        );
        assert_eq!(card["text_download_url"], card["markdown_download_url"]);
        assert_eq!(card["download_exports"].as_array().map(Vec::len), Some(3));
        assert_eq!(
            card["download_exports"][0]["title"],
            json!(XINBAI_PUBLISHED_REPORT_TITLE)
        );
    }

    #[test]
    fn static_page_enrich_report_card_preserves_existing_specific_values() {
        let existing_data_url = artifact_url("reports/custom/data.json");
        let existing_exports = json!([{"kind": "custom", "url": "custom"}]);
        let mut card = json!({
            "public_url": artifact_url("reports/generic/index.html"),
            "report_title": "专项经营看板",
            "data_url": existing_data_url,
            "downloadExports": existing_exports,
        });

        external_channel_static_page_enrich_report_card(&mut card);

        assert_eq!(card["title"], json!("专项经营看板"));
        assert_eq!(card["report_title"], json!("专项经营看板"));
        assert_eq!(card["display_title"], json!("专项经营看板"));
        assert_eq!(card["data_url"], json!(existing_data_url));
        assert_eq!(card["download_exports"], existing_exports);
        assert_eq!(card["table_data_url"], Value::Null);
    }

    #[test]
    fn static_page_enrich_reply_card_updates_nested_card_only() {
        let mut reply = ExternalBotReplyView {
            target_conversation_external_id: "conv".to_string(),
            reply_type: contracts::ExternalBotReplyTypeView::ArtifactLink,
            text: Some("done".to_string()),
            card: Some(json!({
                "public_url": artifact_url("xinbai-functional-modular-template-20260604/index.html"),
                "title": "DataMax 经营分析报表",
            })),
            artifact_links: Vec::new(),
            task_status: Some("static_page_published".to_string()),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        };

        external_channel_static_page_enrich_reply_card(&mut reply);

        let card = reply.card.as_ref().expect("card is preserved");
        assert_eq!(card["title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert_eq!(
            card["table_data_url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/table-data.csv"
            ))
        );
    }

    #[test]
    fn static_page_xinbai_primary_report_detects_url_template_id_and_reference() {
        assert!(external_channel_static_page_is_xinbai_primary_report(
            &json!({}),
            &artifact_url("xinbai-functional-modular-template-20260604/index.html")
        ));
        assert!(external_channel_static_page_is_xinbai_primary_report(
            &json!({"templateReferenceId": "xinbai_business_report_monthly"}),
            &artifact_url("reports/current/index.html")
        ));
        assert!(external_channel_static_page_is_xinbai_primary_report(
            &json!({"template_reference": {"name": "新世界百货月报模板"}}),
            &artifact_url("reports/current/index.html")
        ));
        assert!(!external_channel_static_page_is_xinbai_primary_report(
            &json!({"templateReferenceId": "generic-report"}),
            &artifact_url("reports/current/index.html")
        ));
    }

    #[test]
    fn static_page_export_url_prefers_explicit_allowed_payload_value() {
        let public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let explicit_url = artifact_url("reports/custom/report.ppt");

        assert_eq!(
            external_channel_static_page_export_url(
                &json!({
                    "ppt_download_url": format!(" {explicit_url} "),
                    "artifact": {
                        "ppt_url": artifact_url("reports/fallback/report.ppt")
                    }
                }),
                &public_url,
                &["ppt_download_url", "pptDownloadUrl", "ppt_url", "pptUrl"],
                "report.ppt"
            ),
            json!(explicit_url)
        );
    }

    #[test]
    fn static_page_export_and_data_urls_use_sibling_files_for_xinbai_only() {
        let xinbai_public_url =
            artifact_url("xinbai-functional-modular-template-20260604/index.html?focus=risk");
        let generic_public_url = artifact_url("reports/generic/index.html");

        assert_eq!(
            external_channel_static_page_data_url(&json!({}), &xinbai_public_url),
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/data.json"
            ))
        );
        assert_eq!(
            external_channel_static_page_export_url(
                &json!({}),
                &xinbai_public_url,
                &["table_data_url", "tableDataUrl"],
                "table-data.csv"
            ),
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/table-data.csv"
            ))
        );
        assert_eq!(
            external_channel_static_page_data_url(&json!({}), &generic_public_url),
            Value::Null
        );
    }

    #[test]
    fn static_page_download_exports_from_payload_preserves_existing_or_builds_xinbai_defaults() {
        let public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let existing_exports = json!([{"kind": "custom", "url": "custom"}]);

        assert_eq!(
            external_channel_static_page_download_exports_from_payload(
                &json!({"downloadExports": existing_exports.clone()}),
                &public_url,
                json!(artifact_url(
                    "xinbai-functional-modular-template-20260604/data.json"
                )),
                "新世界百货经营管理月报表"
            ),
            existing_exports
        );

        let generated = external_channel_static_page_download_exports_from_payload(
            &json!({}),
            &public_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/data.json"
            )),
            "新世界百货经营管理月报表",
        );
        assert_eq!(generated[0]["kind"], json!("table_data"));
        assert_eq!(
            generated[0]["url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/table-data.csv"
            ))
        );
        assert_eq!(
            external_channel_static_page_download_exports_from_payload(
                &json!({}),
                &artifact_url("reports/generic/index.html"),
                Value::Null,
                "DataMax 经营分析报表"
            ),
            json!([])
        );
    }
}
