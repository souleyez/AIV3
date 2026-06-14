use domain_model::{StaticPageDraft, StaticPageImageJob, StaticPageImageJobStatus};
use serde_json::{json, Value};

use crate::{
    build_static_page_render_spec,
    static_page_payload_support::{
        build_static_page_preview_contract, static_page_design_fingerprint,
        static_page_payload_mobile_order, static_page_payload_modules, static_page_payload_string,
        static_page_payload_value, static_page_preview_contract_status,
    },
    ApiError,
};

pub(crate) fn static_page_draft_allows_preview_ready_render(draft: &StaticPageDraft) -> bool {
    draft
        .source_refs
        .get("effect_image_confirmation_required")
        .and_then(Value::as_bool)
        != Some(true)
}

pub(crate) fn static_page_draft_requires_preview_data_quality_gate(
    draft: &StaticPageDraft,
) -> bool {
    if draft
        .source_refs
        .get("preview_data_quality_gate_required")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return true;
    }
    if draft
        .source_refs
        .get("continue_to_publish_after_effect_image")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return false;
    }
    draft
        .source_refs
        .get("effect_image_confirmation_required")
        .and_then(Value::as_bool)
        != Some(false)
}

pub(crate) fn static_page_draft_requires_final_render_data_quality_gate(
    draft: &StaticPageDraft,
) -> bool {
    if draft
        .source_refs
        .get("final_render_data_quality_gate_required")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return true;
    }
    if draft
        .source_refs
        .get("continue_to_publish_after_effect_image")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return false;
    }
    draft
        .source_refs
        .get("effect_image_confirmation_required")
        .and_then(Value::as_bool)
        != Some(false)
}

pub(crate) fn static_page_image_job_has_preview_asset(job: &StaticPageImageJob) -> bool {
    job.preview_asset_key
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

pub(crate) fn static_page_image_job_ready_for_render(
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
) -> bool {
    matches!(job.status, StaticPageImageJobStatus::Confirmed)
        || (static_page_draft_allows_preview_ready_render(draft)
            && matches!(job.status, StaticPageImageJobStatus::PreviewReady)
            && static_page_image_job_has_preview_asset(job))
}

pub(crate) fn static_page_current_design_fingerprint_from_payload(payload: &Value) -> String {
    let style_direction =
        static_page_payload_string(payload, &["styleDirection", "style_direction"])
            .unwrap_or_else(|| "client-delivery".to_string());
    let modules = static_page_payload_modules(payload);
    let render_spec = static_page_payload_value(payload, &["renderSpec", "render_spec"])
        .unwrap_or_else(build_static_page_render_spec);
    let mobile_order = static_page_payload_mobile_order(payload, &modules);
    static_page_design_fingerprint(&json!({
        "style_direction": style_direction,
        "modules": modules,
        "render_spec": render_spec,
        "mobile_order": mobile_order,
    }))
}

pub(crate) fn static_page_preview_contract_fingerprint(contract: &Value) -> Option<String> {
    contract
        .get("draftFingerprint")
        .or_else(|| contract.get("draft_fingerprint"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn static_page_image_job_prompt_fingerprint(job: &StaticPageImageJob) -> Option<String> {
    job.image_prompt_payload
        .get("preview_contract")
        .or_else(|| job.image_prompt_payload.get("previewContract"))
        .and_then(static_page_preview_contract_fingerprint)
}

pub(crate) fn ensure_static_page_preview_contract_current(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
) -> std::result::Result<(), ApiError> {
    let current_fingerprint =
        static_page_current_design_fingerprint_from_payload(&draft.draft_payload);
    let preview_contract = static_page_payload_value(
        &draft.draft_payload,
        &["previewContract", "preview_contract"],
    )
    .unwrap_or_else(|| {
        let style_direction = static_page_payload_string(
            &draft.draft_payload,
            &["styleDirection", "style_direction"],
        )
        .unwrap_or_else(|| "client-delivery".to_string());
        let modules = static_page_payload_modules(&draft.draft_payload);
        let render_spec =
            static_page_payload_value(&draft.draft_payload, &["renderSpec", "render_spec"])
                .unwrap_or_else(build_static_page_render_spec);
        let mobile_order = static_page_payload_mobile_order(&draft.draft_payload, &modules);
        build_static_page_preview_contract(
            &style_direction,
            &modules,
            &render_spec,
            &mobile_order,
            None,
        )
    });
    let allow_preview_ready = static_page_draft_allows_preview_ready_render(draft)
        && image_job.is_some_and(|job| {
            matches!(job.status, StaticPageImageJobStatus::PreviewReady)
                && static_page_image_job_has_preview_asset(job)
        });
    let contract_status = static_page_preview_contract_status(&preview_contract);
    let status_current = contract_status == Some("confirmed")
        || (allow_preview_ready && contract_status == Some("preview_ready"));
    if !status_current {
        return Err(ApiError::bad_request(
            "static_page_preview_stale",
            "current static page draft needs a fresh effect preview before rendering".to_string(),
        ));
    }
    let contract_fingerprint = static_page_preview_contract_fingerprint(&preview_contract);
    let job_fingerprint = image_job.and_then(static_page_image_job_prompt_fingerprint);
    let has_any_fingerprint = contract_fingerprint.is_some() || job_fingerprint.is_some();
    let has_matching_fingerprint = contract_fingerprint.as_deref()
        == Some(current_fingerprint.as_str())
        || job_fingerprint.as_deref() == Some(current_fingerprint.as_str());
    if has_any_fingerprint && !has_matching_fingerprint {
        return Err(ApiError::bad_request(
            "static_page_preview_stale",
            "effect preview does not match the current static page draft".to_string(),
        ));
    }
    if let Some(job_fingerprint) = job_fingerprint {
        if job_fingerprint != current_fingerprint {
            return Err(ApiError::bad_request(
                "static_page_preview_stale",
                "effect preview was generated for an older static page draft".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId, TenantId,
    };

    fn test_draft(source_refs: Value, draft_payload: Value) -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: None,
            assistant_run_id: AssistantRunId::new(),
            title: "static page".to_string(),
            status: StaticPageDraftStatus::Previewed,
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            source_refs,
            draft_payload,
            created_at: now,
            updated_at: now,
        }
    }

    fn test_job(
        draft: &StaticPageDraft,
        status: StaticPageImageJobStatus,
        preview_asset_key: Option<&str>,
    ) -> StaticPageImageJob {
        let now = Utc::now();
        StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: draft.tenant_id,
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            status,
            queue_position: None,
            image_prompt_payload: json!({
                "preview_contract": draft.draft_payload["previewContract"].clone()
            }),
            preview_asset_key: preview_asset_key.map(ToOwned::to_owned),
            failure_reason: None,
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn payload_with_preview(status: &str) -> Value {
        let modules = json!([{
            "id": "risk",
            "title": "风险提示",
            "content": "库存压力可控。"
        }]);
        let render_spec = build_static_page_render_spec();
        let mobile_order = json!(["risk"]);
        let preview_contract = build_static_page_preview_contract(
            "client-delivery",
            &modules,
            &render_spec,
            &mobile_order,
            Some(json!({ "status": status })),
        );
        json!({
            "version": 1,
            "styleDirection": "client-delivery",
            "modules": modules,
            "renderSpec": render_spec,
            "mobileOrder": mobile_order,
            "previewContract": preview_contract
        })
    }

    #[test]
    fn preview_and_final_data_quality_gates_keep_existing_precedence() {
        let forced_preview = test_draft(
            json!({
                "preview_data_quality_gate_required": true,
                "continue_to_publish_after_effect_image": true
            }),
            json!({}),
        );
        let auto_continue = test_draft(
            json!({
                "continue_to_publish_after_effect_image": true,
                "effect_image_confirmation_required": true
            }),
            json!({}),
        );
        let no_confirmation = test_draft(
            json!({
                "effect_image_confirmation_required": false
            }),
            json!({}),
        );

        assert!(static_page_draft_requires_preview_data_quality_gate(
            &forced_preview
        ));
        assert!(!static_page_draft_requires_preview_data_quality_gate(
            &auto_continue
        ));
        assert!(!static_page_draft_requires_final_render_data_quality_gate(
            &auto_continue
        ));
        assert!(!static_page_draft_requires_preview_data_quality_gate(
            &no_confirmation
        ));
    }

    #[test]
    fn preview_ready_render_requires_allowed_draft_status_and_asset() {
        let allowed_draft = test_draft(Value::Null, payload_with_preview("preview_ready"));
        let blocked_draft = test_draft(
            json!({ "effect_image_confirmation_required": true }),
            payload_with_preview("preview_ready"),
        );
        let ready_job = test_job(
            &allowed_draft,
            StaticPageImageJobStatus::PreviewReady,
            Some("asset.png"),
        );
        let no_asset_job = test_job(&allowed_draft, StaticPageImageJobStatus::PreviewReady, None);
        let confirmed_job = test_job(&blocked_draft, StaticPageImageJobStatus::Confirmed, None);

        assert!(static_page_image_job_ready_for_render(
            &allowed_draft,
            &ready_job
        ));
        assert!(!static_page_image_job_ready_for_render(
            &allowed_draft,
            &no_asset_job
        ));
        assert!(!static_page_image_job_ready_for_render(
            &blocked_draft,
            &ready_job
        ));
        assert!(static_page_image_job_ready_for_render(
            &blocked_draft,
            &confirmed_job
        ));
    }

    #[test]
    fn preview_contract_current_accepts_matching_preview_ready_auto_render() {
        let draft = test_draft(
            json!({
                "effect_image_confirmation_required": false,
                "continue_to_publish_after_effect_image": true
            }),
            payload_with_preview("preview_ready"),
        );
        let job = test_job(
            &draft,
            StaticPageImageJobStatus::PreviewReady,
            Some("asset.png"),
        );

        ensure_static_page_preview_contract_current(&draft, Some(&job))
            .expect("matching preview-ready job should be renderable");
    }

    #[test]
    fn preview_contract_current_rejects_stale_fingerprint() {
        let mut draft = test_draft(Value::Null, payload_with_preview("confirmed"));
        let job = test_job(
            &draft,
            StaticPageImageJobStatus::Confirmed,
            Some("asset.png"),
        );
        draft.draft_payload["modules"] = json!([{
            "id": "risk",
            "title": "风险提示",
            "content": "库存压力已变化。"
        }]);

        let error = ensure_static_page_preview_contract_current(&draft, Some(&job))
            .expect_err("stale preview should be rejected");

        assert_eq!(error.payload.code, "static_page_preview_stale");
    }
}
