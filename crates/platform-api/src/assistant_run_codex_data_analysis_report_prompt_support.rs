use contracts::CreateAssistantRunRequest;
use serde_json::Value;

use crate::{
    assistant_run_codex_artifact_context_support::{
        assistant_run_customer_codex_context_present,
        assistant_run_prompt_or_context_has_report_artifact,
    },
    external_channel_prompt_is_report_explanation_question,
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
};

pub(crate) fn assistant_run_prompt_requests_data_analysis_report_sidecar(
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
    let has_customer_context =
        assistant_run_customer_codex_context_present(request, selected_scope, &compact, &lower);
    if !has_customer_context {
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
            "趋势分析",
            "统计分析",
            "综合分析",
            "整体分析",
            "全面分析",
            "多维分析",
            "分析数据",
            "分析一下数据",
            "新百经营",
            "新世界经营",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "analysis",
            "analytics",
            "business",
            "operating",
            "operation",
            "dataanalysis",
            "businessanalysis",
        ],
    );
    if !has_data_analysis_intent {
        return false;
    }
    let has_execution_signal = prompt_contains_any(
        &compact,
        &[
            "做一下",
            "做个",
            "做一个",
            "生成",
            "输出",
            "创建",
            "制作",
            "整理",
            "给出",
            "出一版",
            "复盘",
            "分析",
            "梳理",
            "帮我",
            "处理",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "create",
            "generate",
            "build",
            "make",
            "produce",
            "analyze",
            "analyse",
            "review",
            "summarize",
            "summarise",
        ],
    );
    if !has_execution_signal {
        return false;
    }

    let explicit_report_output = prompt_contains_any(
        &compact,
        &[
            "输出报告",
            "输出报表",
            "生成报告",
            "生成报表",
            "生成看板",
            "生成图表",
            "生成可视化",
            "做成报告",
            "做成报表",
            "做成看板",
            "整理成报告",
            "整理成报表",
            "给一份报告",
            "给一份报表",
            "出一份报告",
            "出一份报表",
        ],
    ) || lower.contains("analysisreport")
        || lower.contains("analyticsreport")
        || lower.contains("generatereport")
        || lower.contains("createreport")
        || lower.contains("buildreport");
    let report_context =
        assistant_run_prompt_or_context_has_report_artifact(prompt, request, selected_scope);
    let strong_large_output_signal = prompt_contains_any(
        &compact,
        &[
            "全面",
            "完整",
            "详细",
            "系统",
            "整体",
            "多维",
            "多角度",
            "大篇幅",
            "长篇",
            "大体量",
            "大量",
            "内容量大",
            "内容比较多",
            "长输出",
            "全部",
            "所有",
            "各",
            "逐",
            "分维度",
            "分门店",
            "分品牌",
            "明细",
            "清单",
            "汇总",
            "表格",
            "图表",
            "可视化",
            "报告",
            "报表",
            "看板",
            "页面",
            "输出",
            "整理",
            "生成",
            "做成",
            "给一份",
            "出一份",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "detailed",
            "complete",
            "comprehensive",
            "long",
            "large",
            "full",
            "report",
            "dashboard",
            "visualization",
            "visualisation",
            "table",
            "chart",
        ],
    );

    if external_channel_prompt_is_report_explanation_question(&lower, prompt)
        && !explicit_report_output
    {
        return false;
    }

    explicit_report_output || report_context || strong_large_output_signal
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
    fn data_analysis_report_sidecar_requires_customer_context() {
        let request = sample_request("cc generate analysis report.");
        assert!(!assistant_run_prompt_requests_data_analysis_report_sidecar(
            &request.prompt,
            &request,
            &json!({})
        ));
    }

    #[test]
    fn data_analysis_report_sidecar_accepts_explicit_report_output() {
        let request = sample_request("cc 基于新百经营数据做经营分析，生成报表。");
        assert!(assistant_run_prompt_requests_data_analysis_report_sidecar(
            &request.prompt,
            &request,
            &json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})
        ));
    }

    #[test]
    fn data_analysis_report_sidecar_accepts_large_output_request() {
        let request = sample_request("cc 请基于新百数据做一版全面多维经营分析，输出内容比较完整。");
        assert!(assistant_run_prompt_requests_data_analysis_report_sidecar(
            &request.prompt,
            &request,
            &json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})
        ));
    }

    #[test]
    fn data_analysis_report_sidecar_accepts_report_context_request() {
        let mut request = sample_request("cc 基于当前报表做一下新百经营分析，给管理层建议。");
        request.current_artifact = Some(json!({"type": "dashboard"}));
        assert!(assistant_run_prompt_requests_data_analysis_report_sidecar(
            &request.prompt,
            &request,
            &json!({})
        ));
    }

    #[test]
    fn data_analysis_report_sidecar_keeps_explanation_questions_readonly() {
        let request = sample_request("cc 解释这份经营分析报表口径问题。");
        assert!(!assistant_run_prompt_requests_data_analysis_report_sidecar(
            &request.prompt,
            &request,
            &json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})
        ));
    }

    #[test]
    fn data_analysis_report_sidecar_rejects_non_analysis_artifacts() {
        let request = sample_request("cc 生成一份客户沟通方案文档。");
        assert!(!assistant_run_prompt_requests_data_analysis_report_sidecar(
            &request.prompt,
            &request,
            &json!({})
        ));
    }
}
