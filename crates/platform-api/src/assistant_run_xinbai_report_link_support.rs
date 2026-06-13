use serde_json::{json, Value};

use crate::{
    ascii_prompt_contains_any, external_channel_static_page_download_exports, prompt_contains_any,
    static_page_artifact_sibling_url, static_page_prompt_generated_artifact_urls,
    static_page_prompt_requests_existing_artifact_revision,
};

pub(crate) const XINBAI_PUBLISHED_REPORT_TITLE: &str = "新世界百货经营管理月报表";
pub(crate) const XINBAI_PUBLISHED_REPORT_DEFAULT_PUBLIC_URL: &str =
    "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html";

pub(crate) fn assistant_run_xinbai_published_report_url() -> String {
    std::env::var("XINBAI_PUBLISHED_REPORT_PUBLIC_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| XINBAI_PUBLISHED_REPORT_DEFAULT_PUBLIC_URL.to_string())
}

pub(crate) fn assistant_run_xinbai_published_report_sibling_url(file_name: &str) -> String {
    static_page_artifact_sibling_url(&assistant_run_xinbai_published_report_url(), file_name)
        .unwrap_or_else(assistant_run_xinbai_published_report_url)
}

pub(crate) fn assistant_run_xinbai_published_report_link_answer(prompt: &str) -> Option<String> {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return None;
    }
    let strong_revision_signal = prompt_contains_any(
        &compact,
        &[
            "修复",
            "修正",
            "更正",
            "联动",
            "筛选",
            "不会变",
            "不变",
            "数据绑定",
            "绑定错误",
            "口径错",
            "口径不对",
            "单位错",
            "小数点",
            "重新生成",
            "生成新的",
            "新产物",
            "不覆盖旧页面",
            "bug",
        ],
    ) || ascii_prompt_contains_any(
        &compact.to_ascii_lowercase(),
        &["fix", "revise", "update", "correct", "bug"],
    );
    if static_page_prompt_requests_existing_artifact_revision(prompt)
        && (strong_revision_signal
            || !static_page_prompt_generated_artifact_urls(prompt).is_empty())
    {
        return None;
    }
    let compact_without_soft_punctuation =
        compact.replace(['/', '\\', '"', '\'', '“', '”', '‘', '’'], "");
    let lower_prompt = compact.to_ascii_lowercase();
    let lower_without_soft_punctuation = compact_without_soft_punctuation.to_ascii_lowercase();
    let exact_request = compact.contains("昨天/之前生成的新百报表链接")
        || compact_without_soft_punctuation.contains("昨天之前生成的新百报表链接");
    let has_xinbai_signal = compact.contains("新百")
        || lower_prompt.contains("xinbai")
        || lower_prompt.contains("xin bai");
    let has_report_signal = prompt_contains_any(
        &compact,
        &["报表", "报告", "看板", "页面", "链接", "可视化", "经营分析"],
    ) || ascii_prompt_contains_any(
        &lower_without_soft_punctuation,
        &["report", "dashboard", "page", "link", "url"],
    );
    let has_previous_signal = prompt_contains_any(
        &compact,
        &[
            "昨天",
            "之前",
            "前面",
            "上次",
            "刚才",
            "已经",
            "已生成",
            "生成过",
            "做过",
            "已有",
        ],
    ) || ascii_prompt_contains_any(
        &lower_without_soft_punctuation,
        &["previous", "last", "existing"],
    );
    let has_link_request = prompt_contains_any(
        &compact,
        &[
            "链接",
            "地址",
            "发",
            "给我",
            "给客户",
            "查看",
            "打开",
            "看看",
            "哪里",
        ],
    ) || ascii_prompt_contains_any(
        &lower_without_soft_punctuation,
        &["link", "url", "open", "send"],
    );

    if !(exact_request
        || (has_xinbai_signal && has_report_signal && has_previous_signal && has_link_request))
    {
        return None;
    }

    let public_url = assistant_run_xinbai_published_report_url();
    Some(format!(
        "{XINBAI_PUBLISHED_REPORT_TITLE}已生成，可点击查看：[{XINBAI_PUBLISHED_REPORT_TITLE}]({public_url})\n\n后续如果需要调整指标、门店权限、时间口径或版式，可以在这个页面基础上继续修改。"
    ))
}

pub(crate) fn assistant_run_xinbai_report_link_runtime_manifest(lane: &str) -> Value {
    json!({
        "mode": "direct_answer",
        "provider": "platform_direct_answer",
        "model": "xinbai-published-report-link-v1",
        "lane": lane,
        "public_url": assistant_run_xinbai_published_report_url(),
    })
}

pub(crate) fn assistant_run_xinbai_report_link_output_artifacts(
    answer: &str,
    include_external_channel_artifact: bool,
) -> Vec<Value> {
    let public_url = assistant_run_xinbai_published_report_url();
    let mut artifacts = vec![
        json!({
            "type": "assistant_message",
            "role": "assistant",
            "content": answer,
            "source": "xinbai_published_report_link",
        }),
        json!({
            "type": "generated_artifact",
            "artifact_kind": "static_page",
            "title": XINBAI_PUBLISHED_REPORT_TITLE,
            "report_title": XINBAI_PUBLISHED_REPORT_TITLE,
            "display_title": XINBAI_PUBLISHED_REPORT_TITLE,
            "public_url": public_url,
            "generated_artifact_url": public_url,
            "download_url": public_url,
            "html_download_url": public_url,
            "data_url": assistant_run_xinbai_published_report_sibling_url("data.json"),
            "table_data_url": assistant_run_xinbai_published_report_sibling_url("table-data.csv"),
            "ppt_download_url": assistant_run_xinbai_published_report_sibling_url("report.ppt"),
            "markdown_download_url": assistant_run_xinbai_published_report_sibling_url("report.md"),
            "text_download_url": assistant_run_xinbai_published_report_sibling_url("report.md"),
            "download_exports": external_channel_static_page_download_exports(
                &public_url,
                json!(assistant_run_xinbai_published_report_sibling_url("data.json")),
                Some(XINBAI_PUBLISHED_REPORT_TITLE),
            ),
            "source": "xinbai_published_report_link",
            "editable_after_publish": true,
        }),
    ];
    if include_external_channel_artifact {
        artifacts.push(json!({
            "type": "external_channel_static_page_artifact",
            "title": XINBAI_PUBLISHED_REPORT_TITLE,
            "report_title": XINBAI_PUBLISHED_REPORT_TITLE,
            "display_title": XINBAI_PUBLISHED_REPORT_TITLE,
            "public_url": public_url,
            "generated_artifact_url": public_url,
            "download_url": public_url,
            "html_download_url": public_url,
            "artifact_links": [public_url],
            "data_url": assistant_run_xinbai_published_report_sibling_url("data.json"),
            "table_data_url": assistant_run_xinbai_published_report_sibling_url("table-data.csv"),
            "ppt_download_url": assistant_run_xinbai_published_report_sibling_url("report.ppt"),
            "markdown_download_url": assistant_run_xinbai_published_report_sibling_url("report.md"),
            "text_download_url": assistant_run_xinbai_published_report_sibling_url("report.md"),
            "download_exports": external_channel_static_page_download_exports(
                &public_url,
                json!(assistant_run_xinbai_published_report_sibling_url("data.json")),
                Some(XINBAI_PUBLISHED_REPORT_TITLE),
            ),
            "source": "xinbai_published_report_link",
            "editable_after_publish": true,
            "validation_summary": {
                "status": "published_link_reused",
                "warnings": [],
            },
        }));
    }
    artifacts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xinbai_report_link_artifacts_include_export_urls_and_external_card() {
        let answer =
            assistant_run_xinbai_published_report_link_answer("昨天/之前生成的新百报表链接")
                .expect("known customer phrase should return the published report link");
        let artifacts = assistant_run_xinbai_report_link_output_artifacts(&answer, true);

        assert_eq!(artifacts.len(), 3);
        assert_eq!(artifacts[1]["title"], json!(XINBAI_PUBLISHED_REPORT_TITLE));
        assert_eq!(
            artifacts[1]["table_data_url"],
            json!(assistant_run_xinbai_published_report_sibling_url(
                "table-data.csv"
            ))
        );
        assert_eq!(
            artifacts[1]["download_exports"][0]["kind"],
            json!("table_data")
        );
        assert_eq!(
            artifacts[2]["type"],
            json!("external_channel_static_page_artifact")
        );
        assert_eq!(
            artifacts[2]["artifact_links"][0],
            artifacts[1]["public_url"]
        );
        assert_eq!(
            artifacts[2]["validation_summary"]["status"],
            json!("published_link_reused")
        );
    }

    #[test]
    fn xinbai_report_link_runtime_manifest_keeps_direct_answer_lane() {
        let manifest = assistant_run_xinbai_report_link_runtime_manifest("external_channel");

        assert_eq!(manifest["mode"], json!("direct_answer"));
        assert_eq!(manifest["provider"], json!("platform_direct_answer"));
        assert_eq!(manifest["model"], json!("xinbai-published-report-link-v1"));
        assert_eq!(manifest["lane"], json!("external_channel"));
        assert_eq!(
            manifest["public_url"],
            json!(assistant_run_xinbai_published_report_url())
        );
    }
}
