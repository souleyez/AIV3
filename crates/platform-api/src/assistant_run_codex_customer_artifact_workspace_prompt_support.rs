use contracts::CreateAssistantRunRequest;
use serde_json::Value;

use crate::{
    assistant_run_codex_artifact_context_support::assistant_run_customer_codex_context_present,
    assistant_run_codex_forward_prompt_support::assistant_run_prompt_explicit_customer_codex_signal,
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
};

pub(crate) fn assistant_run_prompt_requests_customer_artifact_workspace(
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
    let explicit_codex_signal =
        assistant_run_prompt_explicit_customer_codex_signal(&compact, &lower);
    let artifact_signal = prompt_contains_any(
        &compact,
        &[
            "生成页面",
            "生成网页",
            "生成网站",
            "生成报表",
            "生成报告",
            "生成看板",
            "生成图表",
            "生成文档",
            "生成方案",
            "生成脚本",
            "生成产物",
            "生成计划",
            "输出页面",
            "输出网页",
            "输出报表",
            "输出报告",
            "输出看板",
            "输出文档",
            "输出方案",
            "输出脚本",
            "输出产物",
            "输出计划",
            "创建文档",
            "创建方案",
            "创建计划",
            "制作页面",
            "制作报表",
            "制作报告",
            "制作方案",
            "制作脚本",
            "制作计划",
            "做成页面",
            "做成报表",
            "做成报告",
            "做一份文档",
            "做一份方案",
            "做一份计划",
            "做个方案",
            "做个计划",
            "说明文件",
            "方案文档",
            "执行方案",
            "分析方案",
            "运营方案",
            "客户沟通方案",
            "客户产物",
            "客户制品",
            "产物包",
            "文档包",
            "脚本包",
            "文档",
            "文件",
            "制品包",
            "写一个页面",
            "写个页面",
            "写一份文档",
            "写一份方案",
            "写一份计划",
            "写个文档",
            "写个方案",
            "写个计划",
            "写一个脚本",
            "写个脚本",
            "出一份报告",
            "出一份报表",
            "出一份文档",
            "出一份方案",
            "出一份计划",
            "出一个文档包",
            "出一个脚本包",
            "脚本",
            "改这个页面",
            "改当前页面",
            "修改这个页面",
            "修改当前页面",
            "调整这个页面",
            "调整当前页面",
            "静态页",
            "可视化报表",
            "dashboard",
            "html",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "buildpage",
            "buildreport",
            "createpage",
            "createreport",
            "dashboard",
            "artifact",
            "package",
            "webpage",
            "html",
            "script",
        ],
    );
    let execution_signal = prompt_contains_any(
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
            "写",
            "改",
            "修改",
            "调整",
            "优化",
            "执行",
            "跑一下",
            "帮我",
            "处理",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "create", "generate", "build", "make", "write", "revise", "edit", "run", "handle",
        ],
    );
    let has_customer_context =
        assistant_run_customer_codex_context_present(request, selected_scope, &compact, &lower)
            || artifact_signal
            || explicit_codex_signal;

    has_customer_context && execution_signal && artifact_signal
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
    fn customer_artifact_workspace_accepts_document_artifact_without_dataset_context() {
        let request = sample_request("cc 生成一份客户沟通方案文档。");
        assert!(assistant_run_prompt_requests_customer_artifact_workspace(
            &request.prompt,
            &request,
            &json!({})
        ));
    }

    #[test]
    fn customer_artifact_workspace_accepts_script_package_request() {
        let request = sample_request("cc 出一个脚本包，处理这些客户数据。");
        assert!(assistant_run_prompt_requests_customer_artifact_workspace(
            &request.prompt,
            &request,
            &json!({})
        ));
    }

    #[test]
    fn customer_artifact_workspace_accepts_current_artifact_edit() {
        let mut request = sample_request("cc 修改当前页面，调整模块顺序。");
        request.current_artifact = Some(json!({"type": "static_page_draft"}));
        assert!(assistant_run_prompt_requests_customer_artifact_workspace(
            &request.prompt,
            &request,
            &json!({})
        ));
    }

    #[test]
    fn customer_artifact_workspace_rejects_plain_codex_analysis_without_artifact_signal() {
        let request = sample_request("用 Codex 帮我分析这条客户经营需求，给出处理思路。");
        assert!(!assistant_run_prompt_requests_customer_artifact_workspace(
            &request.prompt,
            &request,
            &json!({})
        ));
    }

    #[test]
    fn customer_artifact_workspace_rejects_general_chat() {
        let request = sample_request("cc 帮我看一下这个客户问题，给一段处理建议。");
        assert!(!assistant_run_prompt_requests_customer_artifact_workspace(
            &request.prompt,
            &request,
            &json!({})
        ));
    }
}
