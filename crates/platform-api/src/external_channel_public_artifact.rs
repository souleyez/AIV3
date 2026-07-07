use std::collections::HashSet;

use contracts::ExternalBotReplyView;
use serde_json::{json, Map, Value};

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
use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, normalize_static_page_dynamic_page_contract,
    static_page_artifact_sibling_url,
};

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

pub(crate) fn external_channel_static_page_dynamic_page_contract_from_payload(
    payload: &Value,
) -> Value {
    normalize_static_page_dynamic_page_contract(
        external_channel_static_page_artifact_payload_value(payload, "dynamic_page_contract"),
    )
}

pub(crate) fn external_channel_static_page_artifact_stability_payload_value(
    payload: &Value,
    key: &str,
) -> Value {
    payload
        .get(key)
        .cloned()
        .or_else(|| {
            payload
                .pointer(&format!("/artifact_stability/{key}"))
                .cloned()
        })
        .or_else(|| {
            payload
                .pointer(&format!("/source_refs/artifact_stability/{key}"))
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
    if let Some(title) = external_channel_static_page_report_title_candidate(payload) {
        if !external_channel_static_page_report_title_is_generic(&title) {
            return title;
        }
    }
    external_channel_static_page_report_title_fallback(payload, public_url)
}

pub(crate) fn external_channel_static_page_report_title_fallback(
    payload: &Value,
    public_url: &str,
) -> String {
    if external_channel_static_page_is_xinbai_primary_report(payload, public_url) {
        return XINBAI_PUBLISHED_REPORT_TITLE.to_string();
    }
    "DataMax 经营分析报表".to_string()
}

pub(crate) fn external_channel_static_page_report_title_candidate(
    payload: &Value,
) -> Option<String> {
    external_channel_static_page_artifact_payload_string(
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
    )
}

pub(crate) fn external_channel_static_page_is_xinbai_primary_report(
    payload: &Value,
    public_url: &str,
) -> bool {
    if external_channel_static_page_public_url_is_xinbai_primary(public_url) {
        return true;
    }
    if external_channel_static_page_template_reference_id_is_xinbai_primary(payload) {
        return true;
    }
    external_channel_static_page_template_reference_text_is_xinbai_primary(payload)
}

pub(crate) fn external_channel_static_page_public_url_is_xinbai_primary(public_url: &str) -> bool {
    let lower_url = public_url.to_ascii_lowercase();
    lower_url.contains("xinbai-functional-modular-template-20260604")
        || lower_url.contains("xinbai-functional-modular-report")
        || lower_url.contains("/xinbai/")
}

pub(crate) fn external_channel_static_page_template_reference_id_is_xinbai_primary(
    payload: &Value,
) -> bool {
    let template_reference_id =
        external_channel_static_page_template_reference_id_from_payload(payload);
    let template_reference_id = template_reference_id
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    template_reference_id.contains("xinbai-functional-modular-template-20260604")
        || template_reference_id.contains("xinbai_business_report")
}

pub(crate) fn external_channel_static_page_template_reference_text_is_xinbai_primary(
    payload: &Value,
) -> bool {
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
    if let Some(url) = external_channel_static_page_explicit_export_url(payload, keys) {
        return json!(url);
    }
    if external_channel_static_page_is_xinbai_primary_report(payload, public_url) {
        return external_channel_static_page_sibling_file_url_value(public_url, file_name);
    }
    Value::Null
}

pub(crate) fn external_channel_static_page_sibling_file_url_value(
    public_url: &str,
    file_name: &str,
) -> Value {
    static_page_artifact_sibling_url(public_url, file_name)
        .map(Value::String)
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_static_page_explicit_export_url(
    payload: &Value,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        external_channel_static_page_artifact_payload_value(payload, key)
            .as_str()
            .map(str::trim)
            .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
            .map(ToOwned::to_owned)
    })
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
    if let Some(exports) = external_channel_static_page_explicit_download_exports(payload) {
        return exports;
    }
    external_channel_static_page_default_download_exports_for_payload(
        payload,
        public_url,
        data_url,
        report_title,
    )
}

pub(crate) fn external_channel_static_page_default_download_exports_for_payload(
    payload: &Value,
    public_url: &str,
    data_url: Value,
    report_title: &str,
) -> Value {
    if external_channel_static_page_is_xinbai_primary_report(payload, public_url) {
        return external_channel_static_page_download_exports(
            public_url,
            data_url,
            Some(report_title),
        );
    }
    json!([])
}

pub(crate) fn external_channel_static_page_explicit_download_exports(
    payload: &Value,
) -> Option<Value> {
    external_channel_static_page_explicit_download_exports_value(payload)
        .filter(external_channel_static_page_download_exports_is_non_empty_array)
}

pub(crate) fn external_channel_static_page_explicit_download_exports_value(
    payload: &Value,
) -> Option<Value> {
    payload
        .get("download_exports")
        .or_else(|| payload.get("downloadExports"))
        .cloned()
}

pub(crate) fn external_channel_static_page_download_exports_is_non_empty_array(
    value: &Value,
) -> bool {
    value
        .as_array()
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

pub(crate) fn external_channel_static_page_download_exports(
    public_url: &str,
    data_url: Value,
    report_title: Option<&str>,
) -> Value {
    let title = external_channel_static_page_download_export_title(report_title);
    let (table_url, ppt_url, md_url) =
        external_channel_static_page_download_export_file_urls(public_url);
    json!([
        external_channel_static_page_download_export_entry(
            "table_data",
            "表格数据",
            "csv",
            title,
            table_url,
            Some(data_url),
        ),
        external_channel_static_page_download_export_entry(
            "ppt",
            "导出PPT",
            "ppt",
            title,
            ppt_url,
            None,
        ),
        external_channel_static_page_download_export_entry(
            "markdown",
            "文本下载（MD）",
            "md",
            title,
            md_url,
            None,
        ),
    ])
}

pub(crate) fn external_channel_static_page_download_export_title(
    report_title: Option<&str>,
) -> &str {
    report_title.unwrap_or("DataMax 经营分析报表")
}

pub(crate) fn external_channel_static_page_download_export_file_urls(
    public_url: &str,
) -> (Value, Value, Value) {
    (
        external_channel_static_page_sibling_file_url_value(
            public_url,
            external_channel_static_page_table_data_export_file_name(),
        ),
        external_channel_static_page_sibling_file_url_value(
            public_url,
            external_channel_static_page_ppt_download_export_file_name(),
        ),
        external_channel_static_page_sibling_file_url_value(
            public_url,
            external_channel_static_page_markdown_download_export_file_name(),
        ),
    )
}

pub(crate) fn external_channel_static_page_download_export_entry(
    kind: &str,
    label: &str,
    format: &str,
    title: &str,
    url: Value,
    data_url: Option<Value>,
) -> Value {
    let mut entry = json!({
        "kind": kind,
        "label": label,
        "format": format,
        "title": title,
        "url": url,
    });
    if let Some(data_url) = data_url {
        if let Some(object) = entry.as_object_mut() {
            object.insert("data_url".to_string(), data_url);
        }
    }
    entry
}

pub(crate) fn external_channel_static_page_apply_focused_public_url(
    object: &mut Map<String, Value>,
    raw_public_url: &str,
    public_url: &str,
) {
    if external_channel_static_page_focused_public_url_is_unchanged(raw_public_url, public_url) {
        return;
    }
    external_channel_static_page_apply_focused_public_url_aliases(
        object,
        raw_public_url,
        public_url,
    );
    object.insert(
        "artifact_links".to_string(),
        external_channel_static_page_focused_public_url_artifact_links(public_url),
    );
}

pub(crate) fn external_channel_static_page_focused_public_url_is_unchanged(
    raw_public_url: &str,
    public_url: &str,
) -> bool {
    public_url == raw_public_url
}

pub(crate) fn external_channel_static_page_focused_public_url_aliases() -> &'static [&'static str] {
    &[
        "public_url",
        "generated_artifact_url",
        "download_url",
        "html_download_url",
    ]
}

pub(crate) fn external_channel_static_page_focused_public_url_artifact_links(
    public_url: &str,
) -> Value {
    json!([public_url])
}

pub(crate) fn external_channel_static_page_should_apply_focused_public_url_alias(
    object: &Map<String, Value>,
    key: &str,
    raw_public_url: &str,
) -> bool {
    key == "public_url"
        || object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim() == raw_public_url)
}

pub(crate) fn external_channel_static_page_apply_focused_public_url_alias(
    object: &mut Map<String, Value>,
    key: &str,
    raw_public_url: &str,
    public_url: &str,
) -> bool {
    if !external_channel_static_page_should_apply_focused_public_url_alias(
        object,
        key,
        raw_public_url,
    ) {
        return false;
    }
    object.insert(key.to_string(), Value::String(public_url.to_string()));
    true
}

pub(crate) fn external_channel_static_page_apply_focused_public_url_aliases(
    object: &mut Map<String, Value>,
    raw_public_url: &str,
    public_url: &str,
) -> usize {
    external_channel_static_page_focused_public_url_aliases()
        .iter()
        .copied()
        .filter(|key| {
            external_channel_static_page_apply_focused_public_url_alias(
                object,
                key,
                raw_public_url,
                public_url,
            )
        })
        .count()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExternalChannelStaticPageReportCardDownloads {
    pub(crate) data_url: Value,
    pub(crate) table_data_url: Value,
    pub(crate) ppt_download_url: Value,
    pub(crate) markdown_download_url: Value,
    pub(crate) download_exports: Value,
}

pub(crate) fn external_channel_static_page_insert_report_card_downloads(
    object: &mut Map<String, Value>,
    downloads: ExternalChannelStaticPageReportCardDownloads,
) {
    let ExternalChannelStaticPageReportCardDownloads {
        data_url,
        table_data_url,
        ppt_download_url,
        markdown_download_url,
        download_exports,
    } = downloads;

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

pub(crate) fn external_channel_static_page_insert_report_card_titles(
    object: &mut Map<String, Value>,
    report_title: &str,
) -> usize {
    external_channel_static_page_report_card_title_keys()
        .iter()
        .copied()
        .filter(|key| {
            if !external_channel_static_page_card_title_missing_or_generic(object.get(*key)) {
                return false;
            }
            object.insert((*key).to_string(), Value::String(report_title.to_string()));
            true
        })
        .count()
}

pub(crate) fn external_channel_static_page_report_card_title_keys() -> &'static [&'static str] {
    &["title", "report_title", "display_title"]
}

pub(crate) fn external_channel_static_page_table_data_export_keys() -> &'static [&'static str] {
    &["table_data_url", "tableDataUrl", "csv_url", "csvUrl"]
}

pub(crate) fn external_channel_static_page_ppt_download_export_keys() -> &'static [&'static str] {
    &["ppt_download_url", "pptDownloadUrl", "ppt_url", "pptUrl"]
}

pub(crate) fn external_channel_static_page_markdown_download_export_keys() -> &'static [&'static str]
{
    &[
        "markdown_download_url",
        "markdownDownloadUrl",
        "text_download_url",
        "textDownloadUrl",
        "md_url",
        "mdUrl",
    ]
}

pub(crate) fn external_channel_static_page_table_data_export_file_name() -> &'static str {
    "table-data.csv"
}

pub(crate) fn external_channel_static_page_ppt_download_export_file_name() -> &'static str {
    "report.ppt"
}

pub(crate) fn external_channel_static_page_markdown_download_export_file_name() -> &'static str {
    "report.md"
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExternalChannelStaticPageReportCardIdentity {
    pub(crate) raw_public_url: String,
    pub(crate) public_url: String,
    pub(crate) report_title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExternalChannelStaticPageReportCardEnrichment {
    pub(crate) identity: ExternalChannelStaticPageReportCardIdentity,
    pub(crate) downloads: ExternalChannelStaticPageReportCardDownloads,
}

pub(crate) fn external_channel_static_page_report_card_export_urls(
    card: &Value,
    public_url: &str,
) -> (Value, Value, Value) {
    (
        external_channel_static_page_export_url(
            card,
            public_url,
            external_channel_static_page_table_data_export_keys(),
            external_channel_static_page_table_data_export_file_name(),
        ),
        external_channel_static_page_export_url(
            card,
            public_url,
            external_channel_static_page_ppt_download_export_keys(),
            external_channel_static_page_ppt_download_export_file_name(),
        ),
        external_channel_static_page_export_url(
            card,
            public_url,
            external_channel_static_page_markdown_download_export_keys(),
            external_channel_static_page_markdown_download_export_file_name(),
        ),
    )
}

pub(crate) fn external_channel_static_page_report_card_public_urls(
    card: &Value,
) -> Option<(String, String)> {
    let raw_public_url = external_channel_public_artifact_url_from_value(card)?;
    let public_url =
        external_channel_static_page_public_url_with_payload_focus(&raw_public_url, card);
    Some((raw_public_url, public_url))
}

pub(crate) fn external_channel_static_page_report_card_identity(
    card: &Value,
) -> Option<ExternalChannelStaticPageReportCardIdentity> {
    let (raw_public_url, public_url) = external_channel_static_page_report_card_public_urls(card)?;
    let report_title = external_channel_static_page_report_title(card, &public_url);
    Some(ExternalChannelStaticPageReportCardIdentity {
        raw_public_url,
        public_url,
        report_title,
    })
}

pub(crate) fn external_channel_static_page_report_card_downloads(
    card: &Value,
    public_url: &str,
    report_title: &str,
) -> ExternalChannelStaticPageReportCardDownloads {
    let data_url = external_channel_static_page_data_url(card, public_url);
    let (table_data_url, ppt_download_url, markdown_download_url) =
        external_channel_static_page_report_card_export_urls(card, public_url);
    let download_exports = external_channel_static_page_download_exports_from_payload(
        card,
        public_url,
        data_url.clone(),
        report_title,
    );

    ExternalChannelStaticPageReportCardDownloads {
        data_url,
        table_data_url,
        ppt_download_url,
        markdown_download_url,
        download_exports,
    }
}

pub(crate) fn external_channel_static_page_apply_report_card_identity(
    object: &mut Map<String, Value>,
    identity: &ExternalChannelStaticPageReportCardIdentity,
) {
    external_channel_static_page_insert_report_card_titles(object, &identity.report_title);
    external_channel_static_page_apply_focused_public_url(
        object,
        &identity.raw_public_url,
        &identity.public_url,
    );
}

pub(crate) fn external_channel_static_page_report_card_enrichment(
    card: &Value,
) -> Option<ExternalChannelStaticPageReportCardEnrichment> {
    let identity = external_channel_static_page_report_card_identity(card)?;
    let downloads = external_channel_static_page_report_card_downloads(
        card,
        &identity.public_url,
        &identity.report_title,
    );

    Some(ExternalChannelStaticPageReportCardEnrichment {
        identity,
        downloads,
    })
}

pub(crate) fn external_channel_static_page_apply_report_card_enrichment(
    object: &mut Map<String, Value>,
    enrichment: ExternalChannelStaticPageReportCardEnrichment,
) {
    let ExternalChannelStaticPageReportCardEnrichment {
        identity,
        downloads,
    } = enrichment;

    external_channel_static_page_apply_report_card_identity(object, &identity);
    external_channel_static_page_insert_report_card_downloads(object, downloads);
}

pub(crate) fn external_channel_static_page_try_enrich_reply_card(
    reply: &mut ExternalBotReplyView,
) -> bool {
    let Some(card) = reply.card.as_mut() else {
        return false;
    };

    external_channel_static_page_try_enrich_report_card(card)
}

pub(crate) fn external_channel_static_page_try_enrich_report_card(card: &mut Value) -> bool {
    let Some(enrichment) = external_channel_static_page_report_card_enrichment(card) else {
        return false;
    };

    let Some(object) = card.as_object_mut() else {
        return false;
    };
    external_channel_static_page_apply_report_card_enrichment(object, enrichment);
    true
}

pub(crate) fn external_channel_static_page_enrich_report_card(card: &mut Value) {
    let _ = external_channel_static_page_try_enrich_report_card(card);
}

pub(crate) fn external_channel_static_page_enrich_reply_card(reply: &mut ExternalBotReplyView) {
    let _ = external_channel_static_page_try_enrich_reply_card(reply);
}

pub(crate) fn external_channel_static_page_public_artifact_url_after_enrichment(
    reply: &mut ExternalBotReplyView,
) -> Option<String> {
    external_channel_static_page_enrich_reply_card(reply);
    external_channel_public_artifact_url_from_reply(reply)
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
    fn static_page_dynamic_contract_from_payload_reads_nested_artifact_and_normalizes() {
        let payload = json!({
            "output": {
                "artifact": {
                    "dynamic_page_contract": {
                        "data_file": "custom-data.json",
                        "custom_flag": true,
                        "report_time_range": {"default_preset": "ignored"}
                    }
                }
            }
        });

        let contract = external_channel_static_page_dynamic_page_contract_from_payload(&payload);

        assert_eq!(contract["data_file"], json!("custom-data.json"));
        assert_eq!(contract["custom_flag"], json!(true));
        assert_eq!(
            contract["source_snapshot_file"],
            json!("data-snapshot.json")
        );
        assert!(contract.get("report_time_range").is_none());
    }

    #[test]
    fn static_page_dynamic_contract_from_payload_uses_default_when_missing() {
        let contract = external_channel_static_page_dynamic_page_contract_from_payload(&json!({}));

        assert_eq!(contract["data_file"], json!("data.json"));
        assert_eq!(contract["refresh_policy"]["interval_seconds"], json!(60));
    }

    #[test]
    fn static_page_artifact_stability_payload_value_uses_root_then_nested_fallbacks() {
        let payload = json!({
            "dataset_artifact_key": "root-key",
            "artifact_stability": {
                "dataset_artifact_key": "artifact-key",
                "baseline_status": "accepted"
            },
            "source_refs": {
                "artifact_stability": {
                    "baseline_status": "source-ref-status",
                    "default_template_scope": "dataset_combination"
                }
            }
        });

        assert_eq!(
            external_channel_static_page_artifact_stability_payload_value(
                &payload,
                "dataset_artifact_key"
            ),
            json!("root-key")
        );
        assert_eq!(
            external_channel_static_page_artifact_stability_payload_value(
                &payload,
                "baseline_status"
            ),
            json!("accepted")
        );
        assert_eq!(
            external_channel_static_page_artifact_stability_payload_value(
                &payload,
                "default_template_scope"
            ),
            json!("dataset_combination")
        );
        assert_eq!(
            external_channel_static_page_artifact_stability_payload_value(&payload, "missing"),
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
    fn static_page_report_title_candidate_uses_configured_title_priority() {
        assert_eq!(
            external_channel_static_page_report_title_candidate(&json!({
                "report_title": "  门店风险报表  ",
                "title": "低优先级标题",
            })),
            Some("门店风险报表".to_string())
        );
        assert_eq!(
            external_channel_static_page_report_title_candidate(&json!({
                "artifact": {
                    "artifactTitle": "  资产画像报表  "
                }
            })),
            Some("资产画像报表".to_string())
        );
        assert_eq!(
            external_channel_static_page_report_title_candidate(&json!({})),
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
    fn static_page_report_title_fallback_uses_xinbai_detection_or_default() {
        assert_eq!(
            external_channel_static_page_report_title_fallback(
                &json!({
                    "template_reference": {
                        "name": "新世界百货经营分析模板"
                    }
                }),
                &artifact_url("reports/current/index.html")
            ),
            XINBAI_PUBLISHED_REPORT_TITLE
        );
        assert_eq!(
            external_channel_static_page_report_title_fallback(
                &json!({}),
                &artifact_url("reports/current/index.html")
            ),
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
    fn static_page_try_enrich_reply_card_reports_success_and_no_card_noop() {
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

        assert!(external_channel_static_page_try_enrich_reply_card(
            &mut reply
        ));
        let card = reply.card.as_ref().expect("card is preserved");
        assert_eq!(card["title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert_eq!(
            card["data_url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/data.json"
            ))
        );

        let mut no_card_reply = ExternalBotReplyView {
            target_conversation_external_id: "conv".to_string(),
            reply_type: contracts::ExternalBotReplyTypeView::Text,
            text: Some("done".to_string()),
            card: None,
            artifact_links: Vec::new(),
            task_status: None,
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        };

        assert!(!external_channel_static_page_try_enrich_reply_card(
            &mut no_card_reply
        ));
        assert!(no_card_reply.card.is_none());
    }

    #[test]
    fn static_page_public_artifact_url_after_enrichment_returns_focused_or_existing_link() {
        let raw_public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let mut reply = ExternalBotReplyView {
            target_conversation_external_id: "conv".to_string(),
            reply_type: contracts::ExternalBotReplyTypeView::ArtifactLink,
            text: Some("done".to_string()),
            card: Some(json!({
                "public_url": raw_public_url,
                "title": "DataMax 经营分析报表",
                "template_adaptation": {
                    "userIntent": "请优先看门店取高机会"
                }
            })),
            artifact_links: Vec::new(),
            task_status: Some("static_page_published".to_string()),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        };

        let public_url =
            external_channel_static_page_public_artifact_url_after_enrichment(&mut reply)
                .expect("public url");

        assert!(public_url.contains("focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"));
        let card = reply.card.as_ref().expect("card is preserved");
        assert_eq!(card["title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert_eq!(card["public_url"], json!(public_url));

        let existing_link = artifact_url("reports/existing/index.html");
        let mut link_only_reply = ExternalBotReplyView {
            target_conversation_external_id: "conv".to_string(),
            reply_type: contracts::ExternalBotReplyTypeView::ArtifactLink,
            text: Some("done".to_string()),
            card: None,
            artifact_links: vec![existing_link.clone()],
            task_status: Some("static_page_published".to_string()),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        };

        assert_eq!(
            external_channel_static_page_public_artifact_url_after_enrichment(&mut link_only_reply),
            Some(existing_link)
        );
        assert!(link_only_reply.card.is_none());
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
    fn static_page_public_url_is_xinbai_primary_matches_known_paths() {
        assert!(external_channel_static_page_public_url_is_xinbai_primary(
            &artifact_url("xinbai-functional-modular-template-20260604/index.html")
        ));
        assert!(external_channel_static_page_public_url_is_xinbai_primary(
            &artifact_url("reports/xinbai-functional-modular-report/index.html")
        ));
        assert!(external_channel_static_page_public_url_is_xinbai_primary(
            &artifact_url("dashboards/xinbai/current/index.html")
        ));
        assert!(!external_channel_static_page_public_url_is_xinbai_primary(
            &artifact_url("reports/current/index.html")
        ));
    }

    #[test]
    fn static_page_template_reference_id_is_xinbai_primary_matches_known_ids() {
        assert!(
            external_channel_static_page_template_reference_id_is_xinbai_primary(&json!({
                "templateReferenceId": "xinbai_business_report_monthly"
            }))
        );
        assert!(
            external_channel_static_page_template_reference_id_is_xinbai_primary(&json!({
                "template_reference_id": "xinbai-functional-modular-template-20260604"
            }))
        );
        assert!(
            !external_channel_static_page_template_reference_id_is_xinbai_primary(&json!({
                "templateReferenceId": "generic-report"
            }))
        );
        assert!(!external_channel_static_page_template_reference_id_is_xinbai_primary(&json!({})));
    }

    #[test]
    fn static_page_template_reference_text_is_xinbai_primary_matches_known_text() {
        assert!(
            external_channel_static_page_template_reference_text_is_xinbai_primary(&json!({
                "template_reference": {
                    "name": "新世界百货月报模板"
                }
            }))
        );
        assert!(
            external_channel_static_page_template_reference_text_is_xinbai_primary(&json!({
                "template_reference": {
                    "description": "xinbai dashboard baseline"
                }
            }))
        );
        assert!(
            !external_channel_static_page_template_reference_text_is_xinbai_primary(&json!({
                "template_reference": {
                    "name": "通用经营报表模板"
                }
            }))
        );
        assert!(
            !external_channel_static_page_template_reference_text_is_xinbai_primary(&json!({}))
        );
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
    fn static_page_explicit_export_url_skips_disallowed_and_reads_nested_payload() {
        let explicit_url = artifact_url("reports/custom/table-data.csv");
        let payload = json!({
            "table_data_url": "https://example.com/not-allowed.csv",
            "artifact": {
                "tableDataUrl": explicit_url
            }
        });

        assert_eq!(
            external_channel_static_page_explicit_export_url(
                &payload,
                &["table_data_url", "tableDataUrl"]
            ),
            Some(explicit_url)
        );
        assert_eq!(
            external_channel_static_page_explicit_export_url(&json!({}), &["missing"]),
            None
        );
    }

    #[test]
    fn static_page_sibling_file_url_value_returns_artifact_sibling_or_null() {
        assert_eq!(
            external_channel_static_page_sibling_file_url_value(
                &format!("{}?focus=risk", artifact_url("reports/current/index.html")),
                "table-data.csv"
            ),
            json!(artifact_url("reports/current/table-data.csv"))
        );
        assert_eq!(
            external_channel_static_page_sibling_file_url_value("not a url", "table-data.csv"),
            Value::Null
        );
    }

    #[test]
    fn static_page_download_export_entry_adds_data_url_only_when_present() {
        let entry = external_channel_static_page_download_export_entry(
            "table_data",
            "表格数据",
            "csv",
            "经营报表",
            json!(artifact_url("reports/current/table-data.csv")),
            Some(json!(artifact_url("reports/current/data.json"))),
        );

        assert_eq!(entry["kind"], json!("table_data"));
        assert_eq!(entry["label"], json!("表格数据"));
        assert_eq!(entry["format"], json!("csv"));
        assert_eq!(entry["title"], json!("经营报表"));
        assert_eq!(
            entry["url"],
            json!(artifact_url("reports/current/table-data.csv"))
        );
        assert_eq!(
            entry["data_url"],
            json!(artifact_url("reports/current/data.json"))
        );

        let no_data_url = external_channel_static_page_download_export_entry(
            "ppt",
            "导出PPT",
            "ppt",
            "经营报表",
            json!(artifact_url("reports/current/report.ppt")),
            None,
        );
        assert!(no_data_url.get("data_url").is_none());
    }

    #[test]
    fn static_page_download_export_file_urls_preserve_default_siblings() {
        let (table_url, ppt_url, md_url) = external_channel_static_page_download_export_file_urls(
            &format!("{}?focus=risk", artifact_url("reports/current/index.html")),
        );

        assert_eq!(
            table_url,
            json!(artifact_url("reports/current/table-data.csv"))
        );
        assert_eq!(ppt_url, json!(artifact_url("reports/current/report.ppt")));
        assert_eq!(md_url, json!(artifact_url("reports/current/report.md")));
    }

    #[test]
    fn static_page_download_export_title_preserves_explicit_or_default_title() {
        assert_eq!(
            external_channel_static_page_download_export_title(Some("专项经营看板")),
            "专项经营看板"
        );
        assert_eq!(
            external_channel_static_page_download_export_title(None),
            "DataMax 经营分析报表"
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

    #[test]
    fn static_page_default_download_exports_for_payload_builds_xinbai_or_empty() {
        let public_url = artifact_url("reports/current/index.html");
        let data_url = json!(artifact_url("reports/current/data.json"));
        let generated = external_channel_static_page_default_download_exports_for_payload(
            &json!({
                "template_reference": {
                    "name": "新世界百货月报模板"
                }
            }),
            &public_url,
            data_url.clone(),
            "新世界百货经营管理月报表",
        );

        assert_eq!(generated[0]["kind"], json!("table_data"));
        assert_eq!(generated[0]["title"], json!("新世界百货经营管理月报表"));
        assert_eq!(generated[0]["data_url"], data_url);
        assert_eq!(
            external_channel_static_page_default_download_exports_for_payload(
                &json!({}),
                &artifact_url("reports/generic/index.html"),
                Value::Null,
                "DataMax 经营分析报表",
            ),
            json!([])
        );
    }

    #[test]
    fn static_page_explicit_download_exports_requires_non_empty_array() {
        let snake_exports = json!([{"kind": "snake"}]);
        let camel_exports = json!([{"kind": "camel"}]);

        assert_eq!(
            external_channel_static_page_explicit_download_exports(&json!({
                "download_exports": snake_exports,
                "downloadExports": camel_exports,
            })),
            Some(json!([{"kind": "snake"}]))
        );
        assert_eq!(
            external_channel_static_page_explicit_download_exports(&json!({
                "downloadExports": camel_exports,
            })),
            Some(json!([{"kind": "camel"}]))
        );
        assert_eq!(
            external_channel_static_page_explicit_download_exports(&json!({
                "download_exports": [],
                "downloadExports": [{"kind": "ignored"}],
            })),
            None
        );
        assert_eq!(
            external_channel_static_page_explicit_download_exports(&json!({
                "download_exports": {"kind": "not-array"},
            })),
            None
        );
    }

    #[test]
    fn static_page_explicit_download_exports_value_prefers_snake_then_camel() {
        assert_eq!(
            external_channel_static_page_explicit_download_exports_value(&json!({
                "download_exports": [],
                "downloadExports": [{"kind": "camel"}],
            })),
            Some(json!([]))
        );
        assert_eq!(
            external_channel_static_page_explicit_download_exports_value(&json!({
                "downloadExports": [{"kind": "camel"}],
            })),
            Some(json!([{"kind": "camel"}]))
        );
        assert_eq!(
            external_channel_static_page_explicit_download_exports_value(&json!({})),
            None
        );
    }

    #[test]
    fn static_page_download_exports_is_non_empty_array_requires_items() {
        assert!(
            external_channel_static_page_download_exports_is_non_empty_array(
                &json!([{"kind": "custom"}])
            )
        );
        assert!(!external_channel_static_page_download_exports_is_non_empty_array(&json!([])));
        assert!(
            !external_channel_static_page_download_exports_is_non_empty_array(
                &json!({"kind": "not-array"})
            )
        );
        assert!(!external_channel_static_page_download_exports_is_non_empty_array(&Value::Null));
    }

    #[test]
    fn static_page_focused_public_url_is_unchanged_detects_exact_match_only() {
        let public_url = artifact_url("reports/current/index.html");

        assert!(
            external_channel_static_page_focused_public_url_is_unchanged(&public_url, &public_url,)
        );
        assert!(
            !external_channel_static_page_focused_public_url_is_unchanged(
                &public_url,
                &format!("{public_url}?focus=overview"),
            )
        );
        assert!(
            !external_channel_static_page_focused_public_url_is_unchanged(
                &format!(" {public_url} "),
                &public_url,
            )
        );
    }

    #[test]
    fn static_page_apply_focused_public_url_aliases_updates_expected_alias_count() {
        let raw_public_url = artifact_url("reports/current/index.html");
        let focused_public_url = format!("{raw_public_url}?focus=overview");
        let custom_url = artifact_url("reports/custom/index.html");
        let mut object = json!({
            "public_url": custom_url,
            "generated_artifact_url": raw_public_url,
            "download_url": custom_url,
            "html_download_url": raw_public_url,
        })
        .as_object()
        .cloned()
        .expect("object payload");

        assert_eq!(
            external_channel_static_page_apply_focused_public_url_aliases(
                &mut object,
                &raw_public_url,
                &focused_public_url,
            ),
            3
        );

        assert_eq!(object["public_url"], json!(focused_public_url));
        assert_eq!(object["generated_artifact_url"], json!(focused_public_url));
        assert_eq!(object["download_url"], json!(custom_url));
        assert_eq!(object["html_download_url"], json!(focused_public_url));
    }

    #[test]
    fn static_page_apply_focused_public_url_alias_updates_only_applicable_aliases() {
        let raw_public_url = artifact_url("reports/current/index.html");
        let focused_public_url = format!("{raw_public_url}?focus=overview");
        let custom_url = artifact_url("reports/custom/index.html");
        let mut object = json!({
            "public_url": custom_url,
            "generated_artifact_url": format!("  {raw_public_url}  "),
            "download_url": custom_url,
        })
        .as_object()
        .cloned()
        .expect("object payload");

        assert!(external_channel_static_page_apply_focused_public_url_alias(
            &mut object,
            "public_url",
            &raw_public_url,
            &focused_public_url,
        ));
        assert!(external_channel_static_page_apply_focused_public_url_alias(
            &mut object,
            "generated_artifact_url",
            &raw_public_url,
            &focused_public_url,
        ));
        assert!(
            !external_channel_static_page_apply_focused_public_url_alias(
                &mut object,
                "download_url",
                &raw_public_url,
                &focused_public_url,
            )
        );

        assert_eq!(object["public_url"], json!(focused_public_url));
        assert_eq!(object["generated_artifact_url"], json!(focused_public_url));
        assert_eq!(object["download_url"], json!(custom_url));
        assert!(object.get("html_download_url").is_none());
    }

    #[test]
    fn static_page_focused_public_url_artifact_links_wrap_url_in_single_item_array() {
        let public_url = artifact_url("reports/current/index.html?focus=overview");

        assert_eq!(
            external_channel_static_page_focused_public_url_artifact_links(&public_url),
            json!([public_url])
        );
    }

    #[test]
    fn static_page_should_apply_focused_public_url_alias_forces_public_url_and_matches_raw_url() {
        let raw_public_url = artifact_url("reports/current/index.html");
        let custom_url = artifact_url("reports/custom/index.html");
        let object = json!({
            "public_url": custom_url,
            "generated_artifact_url": format!("  {raw_public_url}  "),
            "download_url": custom_url,
        })
        .as_object()
        .cloned()
        .expect("object payload");

        assert!(
            external_channel_static_page_should_apply_focused_public_url_alias(
                &object,
                "public_url",
                &raw_public_url,
            )
        );
        assert!(
            external_channel_static_page_should_apply_focused_public_url_alias(
                &object,
                "generated_artifact_url",
                &raw_public_url,
            )
        );
        assert!(
            !external_channel_static_page_should_apply_focused_public_url_alias(
                &object,
                "download_url",
                &raw_public_url,
            )
        );
        assert!(
            !external_channel_static_page_should_apply_focused_public_url_alias(
                &object,
                "html_download_url",
                &raw_public_url,
            )
        );
    }

    #[test]
    fn static_page_focused_public_url_aliases_preserve_order() {
        assert_eq!(
            external_channel_static_page_focused_public_url_aliases(),
            &[
                "public_url",
                "generated_artifact_url",
                "download_url",
                "html_download_url",
            ]
        );
    }

    #[test]
    fn static_page_apply_focused_public_url_updates_matching_aliases_only() {
        let raw_public_url = artifact_url("reports/current/index.html");
        let focused_public_url =
            format!("{raw_public_url}?focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A");
        let custom_url = artifact_url("reports/custom/index.html");
        let mut object = json!({
            "public_url": raw_public_url,
            "generated_artifact_url": format!("  {raw_public_url}  "),
            "download_url": custom_url,
            "html_download_url": raw_public_url,
            "artifact_links": [raw_public_url],
        })
        .as_object()
        .cloned()
        .expect("object payload");

        external_channel_static_page_apply_focused_public_url(
            &mut object,
            &raw_public_url,
            &focused_public_url,
        );

        assert_eq!(object["public_url"], json!(focused_public_url));
        assert_eq!(object["generated_artifact_url"], json!(focused_public_url));
        assert_eq!(object["download_url"], json!(custom_url));
        assert_eq!(object["html_download_url"], json!(focused_public_url));
        assert_eq!(object["artifact_links"], json!([focused_public_url]));
    }

    #[test]
    fn static_page_apply_focused_public_url_noops_when_url_is_unchanged() {
        let public_url = artifact_url("reports/current/index.html");
        let mut object = json!({
            "public_url": public_url,
            "artifact_links": ["keep-existing"],
        })
        .as_object()
        .cloned()
        .expect("object payload");

        external_channel_static_page_apply_focused_public_url(
            &mut object,
            &public_url,
            &public_url,
        );

        assert_eq!(object["public_url"], json!(public_url));
        assert_eq!(object["artifact_links"], json!(["keep-existing"]));
    }

    #[test]
    fn static_page_insert_report_card_downloads_fills_missing_and_preserves_existing_values() {
        let existing_data_url = artifact_url("reports/custom/data.json");
        let existing_text_url = artifact_url("reports/custom/text.md");
        let table_url = artifact_url("reports/current/table-data.csv");
        let ppt_url = artifact_url("reports/current/report.ppt");
        let md_url = artifact_url("reports/current/report.md");
        let exports = json!([{"kind": "table_data", "url": table_url}]);
        let mut object = json!({
            "data_url": existing_data_url,
            "table_data_url": Value::Null,
            "text_download_url": existing_text_url,
            "download_exports": [],
        })
        .as_object()
        .cloned()
        .expect("object payload");

        external_channel_static_page_insert_report_card_downloads(
            &mut object,
            ExternalChannelStaticPageReportCardDownloads {
                data_url: json!(artifact_url("reports/current/data.json")),
                table_data_url: json!(table_url),
                ppt_download_url: json!(ppt_url),
                markdown_download_url: json!(md_url),
                download_exports: exports.clone(),
            },
        );

        assert_eq!(object["data_url"], json!(existing_data_url));
        assert_eq!(object["table_data_url"], json!(table_url));
        assert_eq!(object["ppt_download_url"], json!(ppt_url));
        assert_eq!(object["markdown_download_url"], json!(md_url));
        assert_eq!(object["text_download_url"], json!(existing_text_url));
        assert_eq!(object["download_exports"], exports);
    }

    #[test]
    fn static_page_insert_report_card_titles_fills_missing_or_generic_titles_only() {
        let mut object = json!({
            "title": "DataMax 经营分析报表",
            "report_title": "专项经营看板",
            "display_title": "",
        })
        .as_object()
        .cloned()
        .expect("object payload");

        assert_eq!(
            external_channel_static_page_insert_report_card_titles(
                &mut object,
                "新世界百货经营管理月报表",
            ),
            2
        );

        assert_eq!(object["title"], json!("新世界百货经营管理月报表"));
        assert_eq!(object["report_title"], json!("专项经营看板"));
        assert_eq!(object["display_title"], json!("新世界百货经营管理月报表"));
    }

    #[test]
    fn static_page_report_card_title_keys_preserve_existing_order() {
        assert_eq!(
            external_channel_static_page_report_card_title_keys(),
            &["title", "report_title", "display_title"]
        );
    }

    #[test]
    fn static_page_export_key_helpers_preserve_existing_order() {
        assert_eq!(
            external_channel_static_page_table_data_export_keys(),
            &["table_data_url", "tableDataUrl", "csv_url", "csvUrl"]
        );
        assert_eq!(
            external_channel_static_page_ppt_download_export_keys(),
            &["ppt_download_url", "pptDownloadUrl", "ppt_url", "pptUrl"]
        );
        assert_eq!(
            external_channel_static_page_markdown_download_export_keys(),
            &[
                "markdown_download_url",
                "markdownDownloadUrl",
                "text_download_url",
                "textDownloadUrl",
                "md_url",
                "mdUrl",
            ]
        );
    }

    #[test]
    fn static_page_export_file_name_helpers_preserve_existing_names() {
        assert_eq!(
            external_channel_static_page_table_data_export_file_name(),
            "table-data.csv"
        );
        assert_eq!(
            external_channel_static_page_ppt_download_export_file_name(),
            "report.ppt"
        );
        assert_eq!(
            external_channel_static_page_markdown_download_export_file_name(),
            "report.md"
        );
    }

    #[test]
    fn static_page_report_card_export_urls_preserve_explicit_priority_and_fallbacks() {
        let public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let explicit_table_url = artifact_url("reports/custom/table-data.csv");
        let explicit_ppt_url = artifact_url("reports/custom/report.ppt");
        let explicit_md_url = artifact_url("reports/custom/report.md");
        let card = json!({
            "csvUrl": explicit_table_url,
            "pptUrl": explicit_ppt_url,
            "mdUrl": explicit_md_url,
        });

        let (table_data_url, ppt_download_url, markdown_download_url) =
            external_channel_static_page_report_card_export_urls(&card, &public_url);

        assert_eq!(table_data_url, json!(explicit_table_url));
        assert_eq!(ppt_download_url, json!(explicit_ppt_url));
        assert_eq!(markdown_download_url, json!(explicit_md_url));

        let (fallback_table_url, fallback_ppt_url, fallback_md_url) =
            external_channel_static_page_report_card_export_urls(&json!({}), &public_url);
        assert_eq!(
            fallback_table_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/table-data.csv"
            ))
        );
        assert_eq!(
            fallback_ppt_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/report.ppt"
            ))
        );
        assert_eq!(
            fallback_md_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/report.md"
            ))
        );
    }

    #[test]
    fn static_page_report_card_public_urls_reads_public_url_and_applies_focus() {
        let raw_public_url = artifact_url("reports/current/index.html");
        let card = json!({
            "public_url": raw_public_url,
            "template_adaptation": {
                "userIntent": "生成取高机会"
            }
        });

        let (raw, focused) =
            external_channel_static_page_report_card_public_urls(&card).expect("public urls");

        assert_eq!(raw, raw_public_url);
        assert!(focused.starts_with(&raw_public_url));
        assert!(focused.contains("focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"));
        assert_eq!(
            external_channel_static_page_report_card_public_urls(&json!({})),
            None
        );
    }

    #[test]
    fn static_page_report_card_identity_collects_public_url_focus_and_title() {
        let raw_public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let card = json!({
            "public_url": raw_public_url,
            "reportTitle": "DataMax 经营分析报表",
            "template_adaptation": {
                "userIntent": "请优先看门店取高机会"
            }
        });

        let identity = external_channel_static_page_report_card_identity(&card).expect("identity");

        assert_eq!(identity.raw_public_url, raw_public_url);
        assert!(identity.public_url.starts_with(&raw_public_url));
        assert!(identity
            .public_url
            .contains("focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"));
        assert_eq!(identity.report_title, XINBAI_PUBLISHED_REPORT_TITLE);
        assert_eq!(
            external_channel_static_page_report_card_identity(&json!({})),
            None
        );
    }

    #[test]
    fn static_page_report_card_downloads_collects_urls_and_exports() {
        let public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let explicit_table_url = artifact_url("reports/custom/table.csv");
        let explicit_ppt_url = artifact_url("reports/custom/slides.ppt");
        let explicit_md_url = artifact_url("reports/custom/report.md");
        let existing_exports = json!([{"kind": "custom", "url": "custom"}]);
        let card = json!({
            "csvUrl": explicit_table_url,
            "pptUrl": explicit_ppt_url,
            "mdUrl": explicit_md_url,
            "downloadExports": existing_exports,
        });

        let downloads = external_channel_static_page_report_card_downloads(
            &card,
            &public_url,
            "新世界百货经营管理月报表",
        );

        assert_eq!(
            downloads.data_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/data.json"
            ))
        );
        assert_eq!(downloads.table_data_url, json!(explicit_table_url));
        assert_eq!(downloads.ppt_download_url, json!(explicit_ppt_url));
        assert_eq!(downloads.markdown_download_url, json!(explicit_md_url));
        assert_eq!(
            downloads.download_exports,
            json!([{"kind": "custom", "url": "custom"}])
        );
    }

    #[test]
    fn static_page_apply_report_card_identity_updates_title_and_focus_aliases() {
        let raw_public_url = artifact_url("reports/current/index.html");
        let focused_public_url = format!("{raw_public_url}?focus=overview");
        let mut object = json!({
            "public_url": raw_public_url,
            "generated_artifact_url": raw_public_url,
            "download_url": artifact_url("reports/custom/index.html"),
            "title": "DataMax 经营分析报表",
            "report_title": "专项经营看板",
            "display_title": "",
        })
        .as_object()
        .cloned()
        .expect("object payload");

        external_channel_static_page_apply_report_card_identity(
            &mut object,
            &ExternalChannelStaticPageReportCardIdentity {
                raw_public_url: raw_public_url.clone(),
                public_url: focused_public_url.clone(),
                report_title: "新世界百货经营管理月报表".to_string(),
            },
        );

        assert_eq!(object["public_url"], json!(focused_public_url));
        assert_eq!(object["generated_artifact_url"], json!(focused_public_url));
        assert_eq!(
            object["download_url"],
            json!(artifact_url("reports/custom/index.html"))
        );
        assert_eq!(object["artifact_links"], json!([focused_public_url]));
        assert_eq!(object["title"], json!("新世界百货经营管理月报表"));
        assert_eq!(object["report_title"], json!("专项经营看板"));
        assert_eq!(object["display_title"], json!("新世界百货经营管理月报表"));
    }

    #[test]
    fn static_page_report_card_enrichment_collects_derived_values() {
        let raw_public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let card = json!({
            "public_url": raw_public_url,
            "reportTitle": "DataMax 经营分析报表",
            "template_adaptation": {
                "userIntent": "请优先看门店取高机会"
            }
        });

        let enrichment =
            external_channel_static_page_report_card_enrichment(&card).expect("enrichment");

        assert_eq!(enrichment.identity.raw_public_url, raw_public_url);
        assert!(enrichment.identity.public_url.starts_with(&raw_public_url));
        assert!(enrichment
            .identity
            .public_url
            .contains("focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"));
        assert_eq!(
            enrichment.identity.report_title,
            XINBAI_PUBLISHED_REPORT_TITLE
        );
        assert_eq!(
            enrichment.downloads.data_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/data.json"
            ))
        );
        assert_eq!(
            enrichment.downloads.table_data_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/table-data.csv"
            ))
        );
        assert_eq!(
            enrichment.downloads.ppt_download_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/report.ppt"
            ))
        );
        assert_eq!(
            enrichment.downloads.markdown_download_url,
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/report.md"
            ))
        );
        assert_eq!(
            enrichment.downloads.download_exports[0]["kind"],
            json!("table_data")
        );
        assert_eq!(
            external_channel_static_page_report_card_enrichment(&json!({})),
            None
        );
    }

    #[test]
    fn static_page_apply_report_card_enrichment_preserves_existing_ordered_mutations() {
        let raw_public_url = artifact_url("reports/current/index.html");
        let focused_public_url = format!("{raw_public_url}?focus=overview");
        let table_url = artifact_url("reports/current/table-data.csv");
        let ppt_url = artifact_url("reports/current/report.ppt");
        let md_url = artifact_url("reports/current/report.md");
        let exports = json!([{"kind": "table_data", "url": table_url}]);
        let mut object = json!({
            "public_url": raw_public_url,
            "generated_artifact_url": raw_public_url,
            "report_title": "专项经营看板",
            "data_url": Value::Null,
            "download_exports": [],
        })
        .as_object()
        .cloned()
        .expect("object payload");

        external_channel_static_page_apply_report_card_enrichment(
            &mut object,
            ExternalChannelStaticPageReportCardEnrichment {
                identity: ExternalChannelStaticPageReportCardIdentity {
                    raw_public_url: raw_public_url.clone(),
                    public_url: focused_public_url.clone(),
                    report_title: "新世界百货经营管理月报表".to_string(),
                },
                downloads: ExternalChannelStaticPageReportCardDownloads {
                    data_url: json!(artifact_url("reports/current/data.json")),
                    table_data_url: json!(table_url),
                    ppt_download_url: json!(ppt_url),
                    markdown_download_url: json!(md_url),
                    download_exports: exports.clone(),
                },
            },
        );

        assert_eq!(object["public_url"], json!(focused_public_url));
        assert_eq!(object["generated_artifact_url"], json!(focused_public_url));
        assert_eq!(object["artifact_links"], json!([focused_public_url]));
        assert_eq!(object["title"], json!("新世界百货经营管理月报表"));
        assert_eq!(object["report_title"], json!("专项经营看板"));
        assert_eq!(object["display_title"], json!("新世界百货经营管理月报表"));
        assert_eq!(
            object["data_url"],
            json!(artifact_url("reports/current/data.json"))
        );
        assert_eq!(object["table_data_url"], json!(table_url));
        assert_eq!(object["ppt_download_url"], json!(ppt_url));
        assert_eq!(object["markdown_download_url"], json!(md_url));
        assert_eq!(object["text_download_url"], json!(md_url));
        assert_eq!(object["download_exports"], exports);
    }

    #[test]
    fn static_page_try_enrich_report_card_reports_success_and_missing_public_url_noop() {
        let raw_public_url = artifact_url("xinbai-functional-modular-template-20260604/index.html");
        let mut card = json!({
            "public_url": raw_public_url,
            "reportTitle": "DataMax 经营分析报表",
            "template_adaptation": {
                "userIntent": "请优先看门店取高机会"
            }
        });

        assert!(external_channel_static_page_try_enrich_report_card(
            &mut card
        ));
        assert_eq!(card["title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert!(card["public_url"]
            .as_str()
            .expect("focused public url")
            .contains("focus=%E5%8F%96%E9%AB%98%E6%9C%BA%E4%BC%9A"));
        assert_eq!(
            card["data_url"],
            json!(artifact_url(
                "xinbai-functional-modular-template-20260604/data.json"
            ))
        );

        let mut unchanged = json!({"status": "queued"});
        assert!(!external_channel_static_page_try_enrich_report_card(
            &mut unchanged
        ));
        assert_eq!(unchanged, json!({"status": "queued"}));
    }
}
