use serde_json::Value;

use crate::assistant_run_prompt_dimension_support::prompt_has_resume_signal;
use crate::assistant_run_scope_policy_support::assistant_run_scope_prefers_detail;
use crate::assistant_run_scope_selection_support::{
    selected_dataset_ids_from_scope, selected_document_ids_for_evidence_from_scope,
};
use crate::{
    prompt_requests_spreadsheet_row_level_analysis, ASSISTANT_RUN_EVIDENCE_DEFAULT_LIMIT,
    ASSISTANT_RUN_EVIDENCE_MAX_LIMIT, ASSISTANT_RUN_SELECTED_DOCUMENT_ROW_EVIDENCE_LIMIT,
};

#[cfg(test)]
pub(crate) fn assistant_run_evidence_limit_for_scope(selected_scope: &Value) -> usize {
    assistant_run_evidence_limit_for_scope_and_prompt(selected_scope, "")
}

pub(crate) fn assistant_run_evidence_limit_for_scope_and_prompt(
    selected_scope: &Value,
    prompt: &str,
) -> usize {
    if !selected_document_ids_for_evidence_from_scope(selected_scope).is_empty()
        && prompt_requests_spreadsheet_row_level_analysis(prompt)
        && !prompt_has_resume_signal(prompt)
    {
        return ASSISTANT_RUN_SELECTED_DOCUMENT_ROW_EVIDENCE_LIMIT;
    }
    if assistant_run_scope_prefers_detail(selected_scope)
        && !selected_dataset_ids_from_scope(selected_scope).is_empty()
    {
        ASSISTANT_RUN_EVIDENCE_MAX_LIMIT
    } else {
        assistant_run_evidence_limit()
    }
}

fn assistant_run_evidence_limit() -> usize {
    std::env::var("ASSISTANT_RUN_EVIDENCE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(ASSISTANT_RUN_EVIDENCE_DEFAULT_LIMIT)
        .clamp(1, ASSISTANT_RUN_EVIDENCE_MAX_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selected_document_spreadsheet_prompt_uses_row_limit() {
        let scope = json!({
            "documents": ["00000000-0000-0000-0000-000000000001"]
        });

        assert_eq!(
            assistant_run_evidence_limit_for_scope_and_prompt(&scope, "列出谁有缺勤考勤明细"),
            ASSISTANT_RUN_SELECTED_DOCUMENT_ROW_EVIDENCE_LIMIT
        );
    }

    #[test]
    fn resume_prompt_keeps_default_limit_for_selected_document() {
        let scope = json!({
            "documents": ["00000000-0000-0000-0000-000000000001"]
        });

        assert_eq!(
            assistant_run_evidence_limit_for_scope_and_prompt(&scope, "统计简历项目经历"),
            assistant_run_evidence_limit()
        );
    }

    #[test]
    fn detail_dataset_scope_uses_max_limit() {
        let scope = json!({
            "datasets": ["00000000-0000-0000-0000-000000000002"],
            "supply_policy": {"preferDetail": true}
        });

        assert_eq!(
            assistant_run_evidence_limit_for_scope(&scope),
            ASSISTANT_RUN_EVIDENCE_MAX_LIMIT
        );
    }

    #[test]
    fn ordinary_scope_uses_configured_default_limit() {
        assert_eq!(
            assistant_run_evidence_limit_for_scope(&json!({"mode": "all_visible"})),
            assistant_run_evidence_limit()
        );
    }
}
