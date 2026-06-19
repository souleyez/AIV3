use chrono::{DateTime, Utc};
use domain_model::{ReportPlanId, WorkflowExecution};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    format_chat_session_report_entry_resolution, format_model_facing_report_entry_state, ApiError,
};

pub(crate) struct ChatSessionReportEntryManifestUpdate<'a> {
    pub(crate) state: &'a contracts::ModelFacingReportEntryStateView,
    pub(crate) requested_at: DateTime<Utc>,
    pub(crate) resolved_at: Option<DateTime<Utc>>,
    pub(crate) resolved_action: Option<&'a contracts::ChatSessionReportEntryResolutionView>,
    pub(crate) suggested_title: &'a str,
    pub(crate) suggested_objective: &'a str,
    pub(crate) confirmed_report_plan_id: Option<ReportPlanId>,
}

pub(crate) fn parse_model_facing_report_entry_state(
    value: &str,
) -> Option<contracts::ModelFacingReportEntryStateView> {
    match value {
        "not_applicable" => Some(contracts::ModelFacingReportEntryStateView::NotApplicable),
        "confirmation_required" => {
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        }
        "confirmed" => Some(contracts::ModelFacingReportEntryStateView::Confirmed),
        _ => None,
    }
}

fn parse_model_facing_service_lane(value: &str) -> Option<contracts::ModelFacingServiceLaneView> {
    match value {
        "material_service" => Some(contracts::ModelFacingServiceLaneView::MaterialService),
        "report_service" => Some(contracts::ModelFacingServiceLaneView::ReportService),
        "controlled_platform_action" => {
            Some(contracts::ModelFacingServiceLaneView::ControlledPlatformAction)
        }
        _ => None,
    }
}

pub(crate) fn parse_chat_session_report_entry_resolution(
    value: &str,
) -> Option<contracts::ChatSessionReportEntryResolutionView> {
    match value {
        "stay_material_service" => {
            Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService)
        }
        "enter_report_service" => {
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        }
        _ => None,
    }
}

fn parse_manifest_service_handoff_source(
    value: &str,
) -> Option<contracts::ManifestServiceHandoffSourceView> {
    match value {
        "chat_session_report_entry" => {
            Some(contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry)
        }
        _ => None,
    }
}

pub(crate) fn parse_manifest_service_handoff(
    value: &Value,
) -> Option<contracts::ManifestServiceHandoffView> {
    let handoff = value.as_object()?;
    Some(contracts::ManifestServiceHandoffView {
        source: parse_manifest_service_handoff_source(handoff.get("source")?.as_str()?)?,
        service_lane: parse_model_facing_service_lane(handoff.get("service_lane")?.as_str()?)?,
        report_entry_state: parse_model_facing_report_entry_state(
            handoff.get("report_entry_state")?.as_str()?,
        )?,
        requested_at: handoff
            .get("requested_at")
            .and_then(parse_manifest_timestamp),
        resolved_at: handoff
            .get("resolved_at")
            .and_then(parse_manifest_timestamp),
        resolved_action: handoff
            .get("resolved_action")
            .and_then(Value::as_str)
            .and_then(parse_chat_session_report_entry_resolution),
        suggested_title: handoff
            .get("suggested_title")
            .and_then(Value::as_str)
            .map(str::to_string),
        suggested_objective: handoff
            .get("suggested_objective")
            .and_then(Value::as_str)
            .map(str::to_string),
        confirmed_report_plan_id: handoff
            .get("confirmed_report_plan_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(ReportPlanId::from),
    })
}

pub(crate) fn workflow_execution_context_service_handoff(
    execution: &WorkflowExecution,
) -> Option<contracts::ManifestServiceHandoffView> {
    execution
        .context
        .as_object()
        .and_then(|context| context.get("service_handoff"))
        .and_then(parse_manifest_service_handoff)
}

pub(crate) fn confirmed_report_entry_service_handoff(
    requested_at: DateTime<Utc>,
    resolved_at: DateTime<Utc>,
    suggested_title: &str,
    suggested_objective: &str,
) -> contracts::ManifestServiceHandoffView {
    contracts::ManifestServiceHandoffView {
        source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
        service_lane: contracts::ModelFacingServiceLaneView::ReportService,
        report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
        requested_at: Some(requested_at),
        resolved_at: Some(resolved_at),
        resolved_action: Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService),
        suggested_title: Some(suggested_title.to_string()),
        suggested_objective: Some(suggested_objective.to_string()),
        confirmed_report_plan_id: None,
    }
}

pub(crate) fn finalize_report_service_handoff(
    service_handoff: Option<contracts::ManifestServiceHandoffView>,
    report_plan_id: ReportPlanId,
) -> Option<contracts::ManifestServiceHandoffView> {
    service_handoff.map(|mut handoff| {
        if handoff.confirmed_report_plan_id.is_none()
            && handoff.report_entry_state == contracts::ModelFacingReportEntryStateView::Confirmed
        {
            handoff.confirmed_report_plan_id = Some(report_plan_id);
        }
        handoff
    })
}

pub(crate) fn write_chat_session_report_entry_manifest(
    session_manifest: &mut Value,
    update: ChatSessionReportEntryManifestUpdate<'_>,
) -> std::result::Result<(), ApiError> {
    let manifest = session_manifest.as_object_mut().ok_or_else(|| {
        ApiError::internal(
            "chat_session_manifest_invalid",
            "chat session manifest must be a JSON object".to_string(),
        )
    })?;
    manifest.insert(
        "report_entry".to_string(),
        json!({
            "state": format_model_facing_report_entry_state(update.state),
            "requested_at": update.requested_at,
            "resolved_at": update.resolved_at,
            "resolved_action": update
                .resolved_action
                .map(format_chat_session_report_entry_resolution),
            "suggested_title": update.suggested_title,
            "suggested_objective": update.suggested_objective,
            "confirmed_report_plan_id": update.confirmed_report_plan_id,
        }),
    );
    Ok(())
}

pub(crate) fn parse_manifest_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let timestamp = value.as_str()?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use domain_model::{TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus};
    use serde_json::json;

    use super::*;

    #[test]
    fn service_handoff_preserves_required_fields_optional_fields_and_timestamps() {
        let report_plan_id = ReportPlanId::new();
        let handoff = parse_manifest_service_handoff(&json!({
            "source": "chat_session_report_entry",
            "service_lane": "report_service",
            "report_entry_state": "confirmed",
            "requested_at": "2026-06-14T00:00:00Z",
            "resolved_at": "2026-06-14T00:01:02+00:00",
            "resolved_action": "enter_report_service",
            "suggested_title": "Monthly Report",
            "suggested_objective": "Summarize operating signals",
            "confirmed_report_plan_id": report_plan_id
        }))
        .expect("valid handoff should parse");

        assert_eq!(
            handoff.source,
            contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry
        );
        assert_eq!(
            handoff.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            handoff.report_entry_state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(
            handoff
                .requested_at
                .as_ref()
                .map(DateTime::<Utc>::to_rfc3339),
            Some("2026-06-14T00:00:00+00:00".to_string())
        );
        assert_eq!(
            handoff.resolved_action,
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        );
        assert_eq!(handoff.suggested_title.as_deref(), Some("Monthly Report"));
        assert_eq!(handoff.confirmed_report_plan_id, Some(report_plan_id));
    }

    #[test]
    fn service_handoff_rejects_unknown_required_enums_but_allows_missing_optional_dates() {
        assert!(parse_manifest_service_handoff(&json!({
            "source": "chat_session_report_entry",
            "service_lane": "unknown_service",
            "report_entry_state": "confirmed"
        }))
        .is_none());

        let handoff = parse_manifest_service_handoff(&json!({
            "source": "chat_session_report_entry",
            "service_lane": "material_service",
            "report_entry_state": "confirmation_required",
            "requested_at": "not-a-date",
            "resolved_action": "stay_material_service"
        }))
        .expect("invalid optional timestamp should be ignored");

        assert_eq!(handoff.requested_at, None);
        assert_eq!(
            handoff.report_entry_state,
            contracts::ModelFacingReportEntryStateView::ConfirmationRequired
        );
        assert_eq!(
            handoff.resolved_action,
            Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService)
        );
    }

    #[test]
    fn entry_state_resolution_and_timestamp_keep_known_values_only() {
        assert_eq!(
            parse_model_facing_report_entry_state("not_applicable"),
            Some(contracts::ModelFacingReportEntryStateView::NotApplicable)
        );
        assert_eq!(parse_model_facing_report_entry_state("other"), None);
        assert_eq!(
            parse_chat_session_report_entry_resolution("enter_report_service"),
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        );
        assert_eq!(parse_chat_session_report_entry_resolution("other"), None);
        assert!(parse_manifest_timestamp(&json!("2026-06-14T00:00:00Z")).is_some());
        assert!(parse_manifest_timestamp(&json!("invalid")).is_none());
    }

    #[test]
    fn workflow_execution_context_service_handoff_parses_valid_context() {
        let report_plan_id = ReportPlanId::new();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: Some(report_plan_id),
            kind: WorkflowKind::ReportPlan,
            version: "report_plan/v1".to_string(),
            stage: "created".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({
                "service_handoff": {
                    "source": "chat_session_report_entry",
                    "service_lane": "report_service",
                    "report_entry_state": "confirmed",
                    "confirmed_report_plan_id": report_plan_id
                }
            }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let handoff = workflow_execution_context_service_handoff(&execution)
            .expect("valid service handoff context should parse");

        assert_eq!(handoff.confirmed_report_plan_id, Some(report_plan_id));
        assert_eq!(
            handoff.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
    }

    #[test]
    fn workflow_execution_context_service_handoff_ignores_missing_or_invalid_context() {
        let mut execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::ReportPlan,
            version: "report_plan/v1".to_string(),
            stage: "created".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(workflow_execution_context_service_handoff(&execution).is_none());

        execution.context = json!({
            "service_handoff": {
                "source": "unknown_source",
                "service_lane": "report_service",
                "report_entry_state": "confirmed"
            }
        });

        assert!(workflow_execution_context_service_handoff(&execution).is_none());
    }

    #[test]
    fn confirmed_report_entry_service_handoff_builds_report_service_handoff() {
        let requested_at = Utc::now();
        let resolved_at = requested_at + chrono::Duration::seconds(30);

        let handoff = confirmed_report_entry_service_handoff(
            requested_at,
            resolved_at,
            "Monthly Report",
            "Summarize operating signals",
        );

        assert_eq!(
            handoff.source,
            contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry
        );
        assert_eq!(
            handoff.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            handoff.report_entry_state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(handoff.requested_at, Some(requested_at));
        assert_eq!(handoff.resolved_at, Some(resolved_at));
        assert_eq!(
            handoff.resolved_action,
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        );
        assert_eq!(handoff.suggested_title.as_deref(), Some("Monthly Report"));
        assert_eq!(
            handoff.suggested_objective.as_deref(),
            Some("Summarize operating signals")
        );
        assert_eq!(handoff.confirmed_report_plan_id, None);
    }

    #[test]
    fn finalize_report_service_handoff_sets_confirmed_plan_id_only_when_missing_and_confirmed() {
        let report_plan_id = ReportPlanId::new();
        let mut handoff = confirmed_report_entry_service_handoff(
            Utc::now(),
            Utc::now(),
            "Monthly Report",
            "Summarize operating signals",
        );

        let finalized = finalize_report_service_handoff(Some(handoff.clone()), report_plan_id)
            .expect("handoff should remain present");
        assert_eq!(finalized.confirmed_report_plan_id, Some(report_plan_id));

        let existing_plan_id = ReportPlanId::new();
        handoff.confirmed_report_plan_id = Some(existing_plan_id);
        let finalized_existing =
            finalize_report_service_handoff(Some(handoff.clone()), report_plan_id)
                .expect("handoff should remain present");
        assert_eq!(
            finalized_existing.confirmed_report_plan_id,
            Some(existing_plan_id)
        );

        handoff.confirmed_report_plan_id = None;
        handoff.report_entry_state =
            contracts::ModelFacingReportEntryStateView::ConfirmationRequired;
        let finalized_unconfirmed = finalize_report_service_handoff(Some(handoff), report_plan_id)
            .expect("handoff should remain present");
        assert_eq!(finalized_unconfirmed.confirmed_report_plan_id, None);

        assert!(finalize_report_service_handoff(None, report_plan_id).is_none());
    }

    #[test]
    fn write_chat_session_report_entry_manifest_persists_report_entry_fields() {
        let requested_at = Utc::now();
        let resolved_at = requested_at + chrono::Duration::minutes(2);
        let report_plan_id = ReportPlanId::new();
        let mut manifest = json!({});

        write_chat_session_report_entry_manifest(
            &mut manifest,
            ChatSessionReportEntryManifestUpdate {
                state: &contracts::ModelFacingReportEntryStateView::Confirmed,
                requested_at,
                resolved_at: Some(resolved_at),
                resolved_action: Some(
                    &contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                ),
                suggested_title: "Monthly Report",
                suggested_objective: "Summarize operating signals",
                confirmed_report_plan_id: Some(report_plan_id),
            },
        )
        .expect("object manifest should be writable");

        let report_entry = &manifest["report_entry"];
        assert_eq!(report_entry["state"], json!("confirmed"));
        assert_eq!(report_entry["requested_at"], json!(requested_at));
        assert_eq!(report_entry["resolved_at"], json!(resolved_at));
        assert_eq!(
            report_entry["resolved_action"],
            json!("enter_report_service")
        );
        assert_eq!(report_entry["suggested_title"], json!("Monthly Report"));
        assert_eq!(
            report_entry["suggested_objective"],
            json!("Summarize operating signals")
        );
        assert_eq!(
            report_entry["confirmed_report_plan_id"],
            json!(report_plan_id)
        );
    }

    #[test]
    fn write_chat_session_report_entry_manifest_rejects_non_object_manifest() {
        let mut manifest = json!(null);

        let error = write_chat_session_report_entry_manifest(
            &mut manifest,
            ChatSessionReportEntryManifestUpdate {
                state: &contracts::ModelFacingReportEntryStateView::ConfirmationRequired,
                requested_at: Utc::now(),
                resolved_at: None,
                resolved_action: None,
                suggested_title: "Monthly Report",
                suggested_objective: "Summarize operating signals",
                confirmed_report_plan_id: None,
            },
        )
        .expect_err("non-object manifest should be rejected");

        assert_eq!(error.payload.code, "chat_session_manifest_invalid");
    }
}
