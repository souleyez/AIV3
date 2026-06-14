use domain_model::StaticPageDraft;
use serde_json::{json, Value};

use crate::static_page_template_match_support::static_page_default_prompt_from_scope_or_refs;

pub(crate) struct StaticPageTemplateOverlapSearchOutcome {
    pub(crate) draft: Option<StaticPageDraft>,
    pub(crate) selected_score: Option<i32>,
    pub(crate) selected_features: Vec<String>,
    pub(crate) current_token_count: usize,
    pub(crate) accepted_baseline_count: usize,
    pub(crate) visible_published_baseline_count: usize,
    pub(crate) scope_intersection_count: usize,
    pub(crate) template_intent_match_count: usize,
    pub(crate) template_intent_mismatch_count: usize,
    pub(crate) default_prompt_match_count: usize,
    pub(crate) default_prompt_mismatch_count: usize,
}

impl StaticPageTemplateOverlapSearchOutcome {
    fn not_matched_reason(&self) -> &'static str {
        if self.current_token_count == 0 {
            "missing_candidate_scope_tokens"
        } else if self.accepted_baseline_count == 0 {
            "no_accepted_template_baseline"
        } else if self.visible_published_baseline_count == 0 {
            "no_visible_published_template_baseline"
        } else if self.scope_intersection_count == 0 {
            "no_dataset_scope_intersection"
        } else if self.template_intent_match_count == 0 && self.template_intent_mismatch_count > 0 {
            "template_intent_mismatch"
        } else if self.default_prompt_match_count == 0 && self.default_prompt_mismatch_count > 0 {
            "default_prompt_mismatch"
        } else {
            "no_matching_template_baseline"
        }
    }

    pub(crate) fn summary(&self, selected_scope: &Value, source_refs: &Value) -> Value {
        json!({
            "policy": "dataset_overlap",
            "status": if self.draft.is_some() { "matched" } else { "not_matched" },
            "reason": if self.draft.is_some() {
                "dataset_scope_intersects_existing_template_baseline_and_default_prompt_matches"
            } else {
                self.not_matched_reason()
            },
            "default_prompt_match_policy": "same_or_compatible_business_default_prompt",
            "candidate_default_prompt_present": static_page_default_prompt_from_scope_or_refs(
                selected_scope,
                source_refs,
            ).is_some(),
            "current_token_count": self.current_token_count,
            "selected_score": self.selected_score,
            "selected_features": self.selected_features,
            "accepted_baseline_count": self.accepted_baseline_count,
            "visible_published_baseline_count": self.visible_published_baseline_count,
            "scope_intersection_count": self.scope_intersection_count,
            "template_intent_match_count": self.template_intent_match_count,
            "template_intent_mismatch_count": self.template_intent_mismatch_count,
            "default_prompt_match_count": self.default_prompt_match_count,
            "default_prompt_mismatch_count": self.default_prompt_mismatch_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unmatched_outcome() -> StaticPageTemplateOverlapSearchOutcome {
        StaticPageTemplateOverlapSearchOutcome {
            draft: None,
            selected_score: None,
            selected_features: Vec::new(),
            current_token_count: 1,
            accepted_baseline_count: 1,
            visible_published_baseline_count: 1,
            scope_intersection_count: 1,
            template_intent_match_count: 1,
            template_intent_mismatch_count: 0,
            default_prompt_match_count: 1,
            default_prompt_mismatch_count: 0,
        }
    }

    #[test]
    fn summary_reports_default_prompt_mismatch() {
        let selected_scope = json!({
            "requested_dataset_external_ids": ["dataset-b"]
        });
        let source_refs = json!({
            "answer_policy": {
                "default_prompt": "请面向业务用户，按经营月报口径输出。"
            }
        });
        let outcome = StaticPageTemplateOverlapSearchOutcome {
            draft: None,
            selected_score: None,
            selected_features: Vec::new(),
            current_token_count: 1,
            accepted_baseline_count: 2,
            visible_published_baseline_count: 2,
            scope_intersection_count: 2,
            template_intent_match_count: 2,
            template_intent_mismatch_count: 0,
            default_prompt_match_count: 0,
            default_prompt_mismatch_count: 2,
        };

        let summary = outcome.summary(&selected_scope, &source_refs);

        assert_eq!(summary["status"], json!("not_matched"));
        assert_eq!(summary["reason"], json!("default_prompt_mismatch"));
        assert_eq!(
            summary["default_prompt_match_policy"],
            json!("same_or_compatible_business_default_prompt")
        );
        assert_eq!(summary["candidate_default_prompt_present"], json!(true));
        assert_eq!(summary["default_prompt_mismatch_count"], json!(2));
    }

    #[test]
    fn not_matched_reason_prioritizes_observable_overlap_state() {
        let mut outcome = unmatched_outcome();
        outcome.current_token_count = 0;
        assert_eq!(
            outcome.not_matched_reason(),
            "missing_candidate_scope_tokens"
        );

        let mut outcome = unmatched_outcome();
        outcome.accepted_baseline_count = 0;
        assert_eq!(
            outcome.not_matched_reason(),
            "no_accepted_template_baseline"
        );

        let mut outcome = unmatched_outcome();
        outcome.visible_published_baseline_count = 0;
        assert_eq!(
            outcome.not_matched_reason(),
            "no_visible_published_template_baseline"
        );

        let mut outcome = unmatched_outcome();
        outcome.scope_intersection_count = 0;
        assert_eq!(
            outcome.not_matched_reason(),
            "no_dataset_scope_intersection"
        );

        let mut outcome = unmatched_outcome();
        outcome.template_intent_match_count = 0;
        outcome.template_intent_mismatch_count = 1;
        assert_eq!(outcome.not_matched_reason(), "template_intent_mismatch");
    }
}
