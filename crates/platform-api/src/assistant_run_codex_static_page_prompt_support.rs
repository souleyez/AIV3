use contracts::CreateAssistantRunRequest;
use serde_json::Value;

use crate::{
    assistant_run_codex_artifact_context_support::assistant_run_customer_codex_context_present,
    assistant_run_codex_forward_prompt_support::assistant_run_prompt_explicit_customer_codex_signal,
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
    static_page_revision_artifact_support::{
        static_page_public_url_from_current_artifact, static_page_revision_explicit_intent_present,
    },
};

pub(crate) fn assistant_run_prompt_requests_generated_static_page_edit(
    prompt: &str,
    current_artifact: Option<&Value>,
) -> bool {
    let Some(current_artifact) = current_artifact else {
        return false;
    };
    if static_page_public_url_from_current_artifact(current_artifact).is_none() {
        return false;
    }
    static_page_revision_explicit_intent_present(prompt)
}

pub(crate) fn assistant_run_prompt_requests_generated_static_page_publish(
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
    let has_static_page_surface = prompt_contains_any(
        &compact,
        &[
            "静态页",
            "报表页面",
            "报表页",
            "交互页面",
            "动态页面",
            "页面",
            "网页",
            "网站",
            "看板",
            "仪表盘",
            "大屏",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &["dashboard", "webpage", "htmlpage", "landingpage", "website"],
    );
    if !has_static_page_surface {
        return false;
    }
    let has_publish_action = prompt_contains_any(
        &compact,
        &[
            "生成",
            "输出",
            "创建",
            "制作",
            "做成",
            "做个",
            "做一个",
            "出页面",
            "发布",
            "渲染",
            "重新生成",
            "重新做",
            "重新设计",
            "全新页面",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &["create", "generate", "build", "make", "publish", "render"],
    );
    if !has_publish_action {
        return false;
    }
    let explicit_codex_signal =
        assistant_run_prompt_explicit_customer_codex_signal(&compact, &lower);
    assistant_run_customer_codex_context_present(request, selected_scope, &compact, &lower)
        || explicit_codex_signal
        || has_static_page_surface
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
    fn generated_static_page_edit_requires_current_artifact_url_and_revision_intent() {
        let artifact = json!({
            "finalPage": {
                "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/index.html"
            }
        });
        assert!(assistant_run_prompt_requests_generated_static_page_edit(
            "cc 修改当前报表页面：把取高风险模块提到最前面。",
            Some(&artifact)
        ));
        assert!(!assistant_run_prompt_requests_generated_static_page_edit(
            "cc 看一下当前报表页面。",
            Some(&artifact)
        ));
        assert!(!assistant_run_prompt_requests_generated_static_page_edit(
            "cc 修改当前报表页面。",
            None
        ));
        assert!(!assistant_run_prompt_requests_generated_static_page_edit(
            "cc 修改当前报表页面。",
            Some(&json!({"type": "static_page"}))
        ));
    }

    #[test]
    fn generated_static_page_publish_requires_surface_and_action() {
        let request = sample_request("cc 生成一个经营看板静态页。");
        assert!(assistant_run_prompt_requests_generated_static_page_publish(
            &request.prompt,
            &request,
            &json!({})
        ));

        let request = sample_request("cc 看一下经营数据。");
        assert!(
            !assistant_run_prompt_requests_generated_static_page_publish(
                &request.prompt,
                &request,
                &json!({})
            )
        );

        let request = sample_request("cc 这个经营看板怎么样。");
        assert!(
            !assistant_run_prompt_requests_generated_static_page_publish(
                &request.prompt,
                &request,
                &json!({})
            )
        );
    }

    #[test]
    fn generated_static_page_publish_accepts_ascii_surface_and_action() {
        let request = sample_request("cc-create-dashboard-page for store operations.");
        assert!(assistant_run_prompt_requests_generated_static_page_publish(
            &request.prompt,
            &request,
            &json!({})
        ));
    }
}
