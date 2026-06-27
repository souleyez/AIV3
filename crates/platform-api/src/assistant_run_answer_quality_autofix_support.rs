use contracts::{
    CodexHostFixedTaskHumanReviewPolicyView, CodexHostFixedTaskTemplateContextView,
    CodexHostFixedTaskTemplateIdView, CodexHostFixedTaskWriteScopeView,
};
use serde_json::{json, Value};

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

pub(crate) fn assistant_run_answer_quality_autofix_output_validation(output: &Value) -> Value {
    if output.get("template_id").and_then(Value::as_str) != Some("answer_quality_autofix") {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "template_id_mismatch"
        });
    }
    let status = output
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed");
    if !matches!(
        status,
        "patch_ready" | "needs_human" | "not_system_defect" | "failed"
    ) {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "unknown_status"
        });
    }
    let failure_type = output
        .get("failure_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !assistant_run_answer_quality_autofix_failure_type_allowed(failure_type) {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "reason": "invalid_failure_type"
        });
    }
    if status == "not_system_defect" {
        return json!({
            "accepted": true,
            "status": status,
            "auto_apply_allowed": false,
            "patch_review_required": false,
            "failure_type": failure_type,
            "reason": failure_type
        });
    }
    if status != "patch_ready" {
        return json!({
            "accepted": true,
            "status": status,
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "reason": output.get("human_review_reason").and_then(Value::as_str).unwrap_or(failure_type)
        });
    }
    let changed_files = value_array(output.get("changed_files").cloned().unwrap_or(Value::Null))
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    if changed_files.is_empty() {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "reason": "changed_files_required"
        });
    }
    if changed_files
        .iter()
        .any(|file| !assistant_run_answer_quality_autofix_file_allowed(file))
    {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "reason": "changed_file_outside_allowlist"
        });
    }
    let tests_added = value_array(output.get("tests_added").cloned().unwrap_or(Value::Null));
    let test_commands = value_array(output.get("test_commands").cloned().unwrap_or(Value::Null));
    if tests_added.is_empty() || test_commands.is_empty() {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "reason": "tests_required"
        });
    }
    let rollback_notes_present = output
        .get("rollback_notes")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|notes| !notes.is_empty());
    if !rollback_notes_present {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "reason": "rollback_notes_required"
        });
    }
    let Some(risk_level) = output.get("risk_level").and_then(Value::as_str) else {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "reason": "risk_level_required"
        });
    };
    if !matches!(risk_level, "low" | "medium" | "high") {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "reason": "invalid_risk_level"
        });
    }
    if risk_level == "high" {
        return json!({
            "accepted": true,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "patch_review_required": true,
            "failure_type": failure_type,
            "risk_level": risk_level,
            "reason": "high_risk_requires_human_review"
        });
    }
    json!({
        "accepted": true,
        "status": "patch_ready",
        "auto_apply_allowed": false,
        "patch_review_required": true,
        "failure_type": failure_type,
        "risk_level": risk_level,
        "reason": "patch_ready_requires_human_review"
    })
}

fn assistant_run_answer_quality_autofix_failure_type_allowed(failure_type: &str) -> bool {
    matches!(
        failure_type,
        "missing_source"
            | "parse_quality"
            | "retrieval_supply"
            | "answer_policy"
            | "not_reproducible"
            | "unsafe_or_out_of_scope"
    )
}

fn assistant_run_answer_quality_autofix_file_allowed(file: &str) -> bool {
    matches!(
        file,
        "crates/platform-api/src/lib.rs"
            | "fixtures/document-quality/**"
            | "scripts/run-document-quality-smoke.ps1"
            | "scripts/run-v3-quality-gate-smoke.ps1"
            | "docs/validation/**"
    ) || file.starts_with("fixtures/document-quality/")
        || file.starts_with("docs/validation/")
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

    #[test]
    fn answer_quality_autofix_output_marks_missing_source_as_not_system_defect() {
        let decision = assistant_run_answer_quality_autofix_output_validation(&json!({
            "template_id": "answer_quality_autofix",
            "status": "not_system_defect",
            "failure_type": "missing_source",
            "root_cause": "source table was not selected"
        }));

        assert_eq!(decision["accepted"], json!(true));
        assert_eq!(decision["status"], json!("not_system_defect"));
        assert_eq!(decision["auto_apply_allowed"], json!(false));
        assert_eq!(decision["patch_review_required"], json!(false));
    }

    #[test]
    fn answer_quality_autofix_output_requires_allowlisted_files_and_tests() {
        let outside = assistant_run_answer_quality_autofix_output_validation(&json!({
            "template_id": "answer_quality_autofix",
            "status": "patch_ready",
            "failure_type": "retrieval_supply",
            "changed_files": ["apps/web/app/globals.css"],
            "tests_added": ["assistant_run_answer_quality_case"],
            "test_commands": ["cargo test -p platform-api assistant_run_answer_quality --lib"],
            "risk_level": "low",
            "rollback_notes": "revert the candidate answer quality patch"
        }));
        assert_eq!(outside["reason"], json!("changed_file_outside_allowlist"));

        let missing_tests = assistant_run_answer_quality_autofix_output_validation(&json!({
            "template_id": "answer_quality_autofix",
            "status": "patch_ready",
            "failure_type": "retrieval_supply",
            "changed_files": ["crates/platform-api/src/lib.rs"],
            "tests_added": [],
            "test_commands": ["cargo test -p platform-api assistant_run_answer_quality --lib"],
            "risk_level": "low",
            "rollback_notes": "revert the candidate answer quality patch"
        }));
        assert_eq!(missing_tests["reason"], json!("tests_required"));
    }

    #[test]
    fn answer_quality_autofix_output_keeps_patch_ready_review_gated() {
        let decision = assistant_run_answer_quality_autofix_output_validation(&json!({
            "template_id": "answer_quality_autofix",
            "status": "patch_ready",
            "failure_type": "retrieval_supply",
            "changed_files": [
                "crates/platform-api/src/lib.rs",
                "fixtures/document-quality/smoke-cases.json"
            ],
            "tests_added": ["assistant_run_answer_quality_autofix_collects_weak_insufficient_case"],
            "test_commands": ["cargo test -p platform-api assistant_run_answer_quality --lib"],
            "risk_level": "medium",
            "rollback_notes": "revert the candidate answer quality patch"
        }));

        assert_eq!(decision["accepted"], json!(true));
        assert_eq!(decision["status"], json!("patch_ready"));
        assert_eq!(decision["auto_apply_allowed"], json!(false));
        assert_eq!(decision["patch_review_required"], json!(true));
    }

    #[test]
    fn answer_quality_autofix_output_requires_failure_type_and_rollback_notes() {
        let invalid_failure_type = assistant_run_answer_quality_autofix_output_validation(&json!({
            "template_id": "answer_quality_autofix",
            "status": "needs_human",
            "failure_type": "unknown",
            "human_review_reason": "cannot classify"
        }));
        assert_eq!(invalid_failure_type["accepted"], json!(false));
        assert_eq!(
            invalid_failure_type["reason"],
            json!("invalid_failure_type")
        );

        let missing_rollback = assistant_run_answer_quality_autofix_output_validation(&json!({
            "template_id": "answer_quality_autofix",
            "status": "patch_ready",
            "failure_type": "parse_quality",
            "changed_files": ["crates/platform-api/src/lib.rs"],
            "tests_added": ["assistant_run_answer_quality_parse_quality_regression"],
            "test_commands": ["cargo test -p platform-api assistant_run_answer_quality --lib"],
            "risk_level": "low"
        }));
        assert_eq!(missing_rollback["reason"], json!("rollback_notes_required"));

        let high_risk = assistant_run_answer_quality_autofix_output_validation(&json!({
            "template_id": "answer_quality_autofix",
            "status": "patch_ready",
            "failure_type": "parse_quality",
            "changed_files": ["crates/platform-api/src/lib.rs"],
            "tests_added": ["assistant_run_answer_quality_parse_quality_regression"],
            "test_commands": ["cargo test -p platform-api assistant_run_answer_quality --lib"],
            "risk_level": "high",
            "rollback_notes": "revert the candidate answer quality patch"
        }));
        assert_eq!(high_risk["status"], json!("needs_human"));
        assert_eq!(
            high_risk["reason"],
            json!("high_risk_requires_human_review")
        );
    }
}
