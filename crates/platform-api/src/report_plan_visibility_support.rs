use domain_model::{ReportPlan, ReportPlanId, SecretBindingId, UserId};

use crate::{
    load_visible_dataset_for_user, not_found_errors::report_plan_not_found_error,
    resource_access::report_owner_is_visible, ApiError, AppState,
};

pub(crate) async fn load_visible_report_plan(
    state: &AppState,
    plan_id: ReportPlanId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<ReportPlan, ApiError> {
    load_report_plan_with_visible_dataset_for_user(state, plan_id, active_secret_binding_ids, None)
        .await
}

pub(crate) async fn load_visible_report_plan_for_user(
    state: &AppState,
    plan_id: ReportPlanId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<ReportPlan, ApiError> {
    let plan = load_report_plan_with_visible_dataset_for_user(
        state,
        plan_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    if report_plan_owner_is_visible_for_user(&plan, current_user_id) {
        return Ok(plan);
    }
    Err(report_plan_not_found_error(plan_id))
}

async fn load_report_plan_with_visible_dataset_for_user(
    state: &AppState,
    plan_id: ReportPlanId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<ReportPlan, ApiError> {
    let plan = state
        .storage
        .report_plans()
        .get_by_id(state.tenant_id, plan_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| report_plan_not_found_error(plan_id))?;
    load_visible_dataset_for_user(
        state,
        plan.dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    Ok(plan)
}

fn report_plan_owner_is_visible_for_user(
    plan: &ReportPlan,
    current_user_id: Option<UserId>,
) -> bool {
    report_owner_is_visible(plan.owner_user_id, current_user_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, ReportPlanStatus, TenantId};

    fn report_plan(owner_user_id: Option<UserId>) -> ReportPlan {
        ReportPlan {
            id: ReportPlanId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id,
            title: "Quarterly Report".to_string(),
            objective: "Summarize product and market signals".to_string(),
            status: ReportPlanStatus::Draft,
            theme_key: "executive-default".to_string(),
            current_ast_version_id: None,
            modules: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn report_plan_owner_visibility_preserves_public_owner_and_hidden_rules() {
        let owner = UserId::new();

        assert!(report_plan_owner_is_visible_for_user(
            &report_plan(None),
            None
        ));
        assert!(report_plan_owner_is_visible_for_user(
            &report_plan(None),
            Some(owner)
        ));
        assert!(report_plan_owner_is_visible_for_user(
            &report_plan(Some(owner)),
            Some(owner)
        ));
        assert!(!report_plan_owner_is_visible_for_user(
            &report_plan(Some(owner)),
            None
        ));
        assert!(!report_plan_owner_is_visible_for_user(
            &report_plan(Some(owner)),
            Some(UserId::new())
        ));
    }
}
