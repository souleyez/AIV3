use contracts::CreateAssistantRunRequest;
use serde_json::Value;

use crate::{
    assistant_run_codex_artifact_context_support::assistant_run_customer_codex_context_present,
    assistant_run_codex_forward_prompt_support::{
        assistant_run_prompt_explicit_customer_codex_signal,
        assistant_run_prompt_requests_codex_forward,
    },
    external_channel_prompt_is_report_explanation_question,
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
};

pub(crate) fn assistant_run_prompt_requests_customer_codex_report_artifact_package(
    prompt: &str,
    request: &CreateAssistantRunRequest,
    selected_scope: &Value,
) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return false;
    }
    let lower = compact.to_ascii_lowercase();
    if !assistant_run_prompt_requests_codex_forward(prompt)
        && !assistant_run_prompt_explicit_customer_codex_signal(&compact, &lower)
    {
        return false;
    }
    if !assistant_run_customer_codex_context_present(request, selected_scope, &compact, &lower) {
        return false;
    }
    let has_data_analysis_intent = prompt_contains_any(
        &compact,
        &[
            "经营分析",
            "数据分析",
            "经营工作分析",
            "经营复盘",
            "管理层复盘",
            "业务分析",
            "综合分析",
            "多维分析",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "analysis",
            "analytics",
            "businessanalysis",
            "operatinganalysis",
        ],
    );
    if !has_data_analysis_intent {
        return false;
    }
    let has_explicit_deliverable_signal = prompt_contains_any(
        &compact,
        &[
            "可下载",
            "说明文件",
            "文件产物",
            "页面产物",
            "报告产物",
            "报表产物",
            "产物包",
            "交付包",
            "下载包",
            "文档包",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &["downloadable", "artifact", "package", "deliverable"],
    );
    if external_channel_prompt_is_report_explanation_question(&lower, prompt)
        && !has_explicit_deliverable_signal
    {
        return false;
    }
    let has_artifact_package_signal = prompt_contains_any(
        &compact,
        &[
            "可下载",
            "报告",
            "报表",
            "说明文件",
            "文件产物",
            "页面产物",
            "报告产物",
            "报表产物",
            "产物包",
            "交付包",
            "下载包",
            "文档包",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "downloadable",
            "artifact",
            "package",
            "report",
            "deliverable",
        ],
    );
    if !has_artifact_package_signal {
        return false;
    }
    let strong_static_page_surface = prompt_contains_any(
        &compact,
        &[
            "静态页",
            "静态页面",
            "看板",
            "仪表盘",
            "大屏",
            "网页",
            "网站",
            "交互页面",
            "动态页面",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "dashboard",
            "webpage",
            "website",
            "htmlpage",
            "staticpage",
            "static_page",
        ],
    );
    !strong_static_page_surface
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_request(prompt: &str) -> CreateAssistantRunRequest {
        CreateAssistantRunRequest {
            prompt: prompt.to_string(),
            local_thread_id: None,
            startup_briefing: None,
            selected_scope: None,
            scope_candidates: Vec::new(),
            context_policy_hint: None,
            current_artifact: None,
            messages: Vec::new(),
        }
    }

    #[test]
    fn report_artifact_package_requires_codex_signal() {
        let request = sample_request("请基于新百经营数据做经营分析，输出可下载报表产物包。");
        assert!(
            !assistant_run_prompt_requests_customer_codex_report_artifact_package(
                &request.prompt,
                &request,
                &json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})
            )
        );
    }

    #[test]
    fn report_artifact_package_accepts_cc_data_analysis_deliverable() {
        let request = sample_request("cc 基于新百经营数据做经营分析，输出可下载报表产物包。");
        assert!(
            assistant_run_prompt_requests_customer_codex_report_artifact_package(
                &request.prompt,
                &request,
                &json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})
            )
        );
    }

    #[test]
    fn report_artifact_package_accepts_explicit_codex_without_cc() {
        let request = sample_request("让 Codex 基于经营数据做多维分析，输出交付包。");
        assert!(
            assistant_run_prompt_requests_customer_codex_report_artifact_package(
                &request.prompt,
                &request,
                &json!({})
            )
        );
    }

    #[test]
    fn report_artifact_package_keeps_report_explanation_readonly_without_deliverable() {
        let request = sample_request("cc 解释这份经营分析报表口径问题。");
        assert!(
            !assistant_run_prompt_requests_customer_codex_report_artifact_package(
                &request.prompt,
                &request,
                &json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})
            )
        );
    }

    #[test]
    fn report_artifact_package_defers_static_page_surfaces() {
        let request = sample_request("cc 基于经营数据做经营分析，生成报表静态页。");
        assert!(
            !assistant_run_prompt_requests_customer_codex_report_artifact_package(
                &request.prompt,
                &request,
                &json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})
            )
        );
    }
}
