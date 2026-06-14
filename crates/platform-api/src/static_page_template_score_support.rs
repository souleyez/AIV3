use domain_model::StaticPageDraft;
use serde_json::Value;
use std::collections::BTreeSet;

use crate::static_page_template_binding_support::{
    static_page_template_request_needs_store_sales_binding, static_page_template_text_contains_any,
};
use crate::static_page_template_match_support::{
    static_page_default_prompt_tokens_match, static_page_template_token_is_material_scope,
};
use crate::static_page_template_profile_support::{
    static_page_template_draft_profile_text, static_page_template_limited_value_text,
    static_page_template_profile_has_xinbai_primary_default_signal,
};

#[derive(Debug, Clone)]
pub(crate) struct StaticPageTemplateBaselineScore {
    pub(crate) score: i32,
    pub(crate) features: Vec<String>,
}

impl StaticPageTemplateBaselineScore {
    pub(crate) fn new() -> Self {
        Self {
            score: 100,
            features: vec!["eligible_dataset_overlap".to_string()],
        }
    }

    pub(crate) fn add(&mut self, points: i32, feature: impl Into<String>) {
        self.score += points;
        self.features.push(feature.into());
    }
}

fn static_page_template_current_scope_mentions_recipient_role(
    selected_scope: &Value,
    source_refs: &Value,
) -> bool {
    let text = format!(
        "{}\n{}",
        static_page_template_limited_value_text(selected_scope),
        static_page_template_limited_value_text(source_refs)
    );
    let lower = text.to_ascii_lowercase();
    static_page_template_text_contains_any(
        &text,
        &lower,
        &[
            "recipient_role",
            "recipientRole",
            "store_manager",
            "店总",
            "角色",
            "权限",
        ],
    )
}

pub(crate) fn static_page_template_baseline_score(
    current_prompt: Option<&str>,
    selected_scope: &Value,
    source_refs: &Value,
    current_tokens: &BTreeSet<String>,
    draft: &StaticPageDraft,
    baseline_tokens: &BTreeSet<String>,
) -> StaticPageTemplateBaselineScore {
    let mut score = StaticPageTemplateBaselineScore::new();
    let material_overlap_count = current_tokens
        .iter()
        .filter(|token| {
            static_page_template_token_is_material_scope(token) && baseline_tokens.contains(*token)
        })
        .count();
    if material_overlap_count > 0 {
        score.add(
            (material_overlap_count.min(4) as i32) * 12,
            format!("material_scope_overlap:{material_overlap_count}"),
        );
    }
    if static_page_default_prompt_tokens_match(
        selected_scope,
        source_refs,
        &draft.selected_scope,
        &draft.source_refs,
    ) {
        score.add(20, "default_prompt_compatible");
    }

    let profile = static_page_template_draft_profile_text(draft);
    let profile_lower = profile.to_ascii_lowercase();
    let has_xinbai_primary_default_signal =
        static_page_template_profile_has_xinbai_primary_default_signal(&profile, &profile_lower);
    if has_xinbai_primary_default_signal {
        score.add(90, "baseline_xinbai_primary_default_template");
    }

    if !static_page_template_request_needs_store_sales_binding(current_prompt) {
        return score;
    }

    score.add(25, "request_store_sales_filter_binding");
    let has_store_or_region = static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "门店", "分店", "店铺", "区域", "分区", "store", "region", "area",
        ],
    );
    let has_sales_or_revenue = static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "近7日",
            "近七日",
            "销售",
            "营业额",
            "营收",
            "租金",
            "高分成",
            "取高",
            "sales",
            "revenue",
            "rent",
        ],
    );
    let has_filter_or_binding = static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "联动",
            "筛选",
            "切换",
            "selector",
            "select",
            "filter",
            "binding",
            "store-select",
            "area-select",
        ],
    );
    let has_real_data_report_signal = static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "数据库真实数据",
            "真实数据版",
            "data-buddy-image2-report",
            "template:data-report",
            "business_report",
            "经营分析总报表",
            "xinbai-functional-modular-template-20260604",
            "xinbai_business_report",
            "primary_default_template",
            "project_unique_default_template",
        ],
    );
    let has_generic_health_signal = static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "经营健康度总览",
            "健康度总览",
            "health overview",
            "health_overview",
        ],
    );
    let has_test_or_warmup_signal = static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "并发编号",
            "smoke",
            "template_prewarm",
            "prewarm",
            "测试页",
            "test page",
        ],
    );
    let has_recipient_role_signal = static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "recipient_role",
            "recipientRole",
            "store_manager",
            "role:store",
        ],
    );

    if has_store_or_region {
        score.add(35, "baseline_has_store_or_region");
    } else {
        score.add(-25, "baseline_missing_store_or_region");
    }
    if has_sales_or_revenue {
        score.add(45, "baseline_has_sales_or_revenue");
    } else {
        score.add(-35, "baseline_missing_sales_or_revenue");
    }
    if has_filter_or_binding {
        score.add(20, "baseline_has_filter_or_binding");
    }
    if has_real_data_report_signal {
        score.add(60, "baseline_real_data_report_signal");
    }
    if has_xinbai_primary_default_signal {
        score.add(90, "baseline_xinbai_primary_default_template");
    }
    if has_generic_health_signal {
        score.add(-70, "baseline_generic_health_overview_penalty");
    }
    if has_test_or_warmup_signal {
        score.add(-80, "baseline_test_or_prewarm_penalty");
    }
    if has_recipient_role_signal
        && !static_page_template_current_scope_mentions_recipient_role(selected_scope, source_refs)
    {
        score.add(-30, "baseline_recipient_role_scope_penalty");
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, TenantId, UserId,
    };
    use serde_json::json;

    use crate::static_page_template_match_support::static_page_template_match_tokens;

    fn rendered_draft(
        title: &str,
        selected_scope: Value,
        source_refs: Value,
        draft_payload: Value,
    ) -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: Some(UserId::new()),
            title: title.to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope,
            visibility_snapshot: json!({}),
            source_refs,
            draft_payload,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn baseline_score_rewards_real_store_sales_report_profile() {
        let selected_scope = json!({
            "dataset_external_ids": ["xinbai-project-dataset"]
        });
        let source_refs = json!({
            "answer_policy": {
                "default_prompt": "请面向业务用户输出经营月报。"
            }
        });
        let draft = rendered_draft(
            "静态页：生成【新百经营分析总报表-数据库真实数据版】",
            selected_scope.clone(),
            source_refs.clone(),
            json!({
                "features": [
                    "primary_default_template",
                    "project_unique_default_template"
                ],
                "modules": ["近7日销售", "区域筛选", "门店筛选"],
                "dataShape": {
                    "storeList": ["A店", "B店"],
                    "areaSelect": true,
                    "storeSelect": true
                }
            }),
        );
        let current_tokens = static_page_template_match_tokens(&selected_scope, &source_refs);
        let baseline_tokens =
            static_page_template_match_tokens(&draft.selected_scope, &draft.source_refs);

        let score = static_page_template_baseline_score(
            Some("近7日销售切换区域和门店必须联动刷新"),
            &selected_scope,
            &source_refs,
            &current_tokens,
            &draft,
            &baseline_tokens,
        );

        assert!(score.score > 300, "unexpected score: {score:?}");
        assert!(score
            .features
            .iter()
            .any(|feature| feature == "material_scope_overlap:1"));
        assert!(score
            .features
            .iter()
            .any(|feature| feature == "baseline_real_data_report_signal"));
        assert!(score
            .features
            .iter()
            .any(|feature| feature == "baseline_xinbai_primary_default_template"));
    }

    #[test]
    fn baseline_score_penalizes_role_specific_template_without_role_scope() {
        let selected_scope = json!({
            "dataset_external_ids": ["xinbai-project-dataset"]
        });
        let source_refs = json!({});
        let draft = rendered_draft(
            "静态页：门店销售筛选报表",
            selected_scope.clone(),
            json!({
                "artifact_stability": {
                    "dataset_artifact_key": "dataset_external_id:xinbai-project-dataset|recipient_role:store_manager"
                }
            }),
            json!({
                "modules": ["近7日销售", "门店筛选", "区域联动"]
            }),
        );
        let current_tokens = static_page_template_match_tokens(&selected_scope, &source_refs);
        let baseline_tokens =
            static_page_template_match_tokens(&draft.selected_scope, &draft.source_refs);

        let score = static_page_template_baseline_score(
            Some("近7日销售按门店筛选"),
            &selected_scope,
            &source_refs,
            &current_tokens,
            &draft,
            &baseline_tokens,
        );

        assert!(score
            .features
            .iter()
            .any(|feature| feature == "baseline_recipient_role_scope_penalty"));
    }
}
