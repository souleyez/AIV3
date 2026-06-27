use contracts::{
    CodexHostFixedTaskHumanReviewPolicyView, CodexHostFixedTaskTemplateContextView,
    CodexHostFixedTaskTemplateIdView, CodexHostFixedTaskWriteScopeView,
};
use serde_json::Value;

use crate::json_value_support::value_array;

pub(crate) fn assistant_run_answer_quality_autofix_fixed_task_from_case(
    case_package: &Value,
) -> Option<CodexHostFixedTaskTemplateContextView> {
    if case_package.get("template_id").and_then(Value::as_str) != Some("answer_quality_autofix") {
        return None;
    }
    let case_id = case_package
        .get("case_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            case_package
                .get("assistant_run_id")
                .and_then(Value::as_str)
                .map(|run_id| format!("assistant-run-{run_id}"))
        });
    Some(CodexHostFixedTaskTemplateContextView {
        template_id: CodexHostFixedTaskTemplateIdView::AnswerQualityAutofix,
        version: 1,
        assistant_run_id: case_package
            .get("assistant_run_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        draft_id: None,
        case_id,
        dataset_scope: Value::Null,
        requirements: Value::Null,
        image2: Value::Null,
        policies: Value::Null,
        low_quality_signals: value_array(
            case_package
                .get("low_quality_signals")
                .cloned()
                .unwrap_or(Value::Null),
        )
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect(),
        user_question: case_package
            .get("user_question")
            .and_then(Value::as_str)
            .map(str::to_string),
        customer_answer: case_package
            .get("customer_answer_excerpt")
            .and_then(Value::as_str)
            .map(str::to_string),
        evidence_summary: case_package
            .get("evidence_summary")
            .cloned()
            .unwrap_or(Value::Null),
        trace_summary: case_package
            .get("trace_summary")
            .cloned()
            .unwrap_or(Value::Null),
        allowed_write_scope: Some(assistant_run_answer_quality_autofix_allowed_write_scope()),
        human_review_policy:
            CodexHostFixedTaskHumanReviewPolicyView::AutoForDiagnosisAndPatchProposal,
    })
}

pub(crate) fn assistant_run_answer_quality_autofix_allowed_write_scope(
) -> CodexHostFixedTaskWriteScopeView {
    CodexHostFixedTaskWriteScopeView {
        files: vec![
            "crates/platform-api/src/lib.rs".to_string(),
            "fixtures/document-quality/**".to_string(),
            "scripts/run-document-quality-smoke.ps1".to_string(),
            "scripts/run-v3-quality-gate-smoke.ps1".to_string(),
            "docs/validation/**".to_string(),
        ],
        symbols: vec![
            "assistant_run_answer_quality_*".to_string(),
            "assistant_run_react_*".to_string(),
            "assistant_run_supply_quality_*".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn answer_quality_autofix_fixed_task_builds_allowlisted_package() {
        let fixed_task = assistant_run_answer_quality_autofix_fixed_task_from_case(&json!({
            "template_id": "answer_quality_autofix",
            "assistant_run_id": "run-123",
            "case_id": "case-123",
            "low_quality_signals": ["weak_insufficient_evidence_answer", 123],
            "user_question": "客户问了什么",
            "customer_answer_excerpt": "当前资料不足。",
            "evidence_summary": {"answer_supply_sources": ["dataset_fact_snapshot"]},
            "trace_summary": {"events": ["assistant_run.answer_quality_autofix.case_collected"]}
        }))
        .expect("fixed task");

        assert_eq!(
            fixed_task.template_id,
            CodexHostFixedTaskTemplateIdView::AnswerQualityAutofix
        );
        assert_eq!(fixed_task.version, 1);
        assert_eq!(fixed_task.assistant_run_id.as_deref(), Some("run-123"));
        assert_eq!(fixed_task.case_id.as_deref(), Some("case-123"));
        assert_eq!(
            fixed_task.low_quality_signals,
            vec!["weak_insufficient_evidence_answer".to_string()]
        );
        assert_eq!(fixed_task.user_question.as_deref(), Some("客户问了什么"));
        assert_eq!(
            fixed_task.customer_answer.as_deref(),
            Some("当前资料不足。")
        );
        assert!(fixed_task.allowed_write_scope.is_some());
        assert_eq!(
            fixed_task.human_review_policy,
            CodexHostFixedTaskHumanReviewPolicyView::AutoForDiagnosisAndPatchProposal
        );
    }

    #[test]
    fn answer_quality_autofix_fixed_task_rejects_other_templates() {
        assert!(
            assistant_run_answer_quality_autofix_fixed_task_from_case(&json!({
                "template_id": "static_page_artifact"
            }))
            .is_none()
        );
    }

    #[test]
    fn answer_quality_autofix_fixed_task_falls_back_case_id_from_run_id() {
        let fixed_task = assistant_run_answer_quality_autofix_fixed_task_from_case(&json!({
            "template_id": "answer_quality_autofix",
            "assistant_run_id": "run-456"
        }))
        .expect("fixed task");

        assert_eq!(fixed_task.case_id.as_deref(), Some("assistant-run-run-456"));
        assert_eq!(fixed_task.evidence_summary, Value::Null);
        assert_eq!(fixed_task.trace_summary, Value::Null);
    }

    #[test]
    fn answer_quality_autofix_allowed_write_scope_matches_contract() {
        let scope = assistant_run_answer_quality_autofix_allowed_write_scope();

        assert!(scope
            .files
            .iter()
            .any(|file| file == "crates/platform-api/src/lib.rs"));
        assert!(scope
            .symbols
            .iter()
            .any(|symbol| symbol == "assistant_run_answer_quality_*"));
        assert!(scope
            .symbols
            .iter()
            .any(|symbol| symbol == "assistant_run_supply_quality_*"));
    }
}
