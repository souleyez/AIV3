use chrono::{DateTime, Utc};
use domain_model::ReportPlanId;
use serde_json::Value;
use uuid::Uuid;

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

pub(crate) fn parse_manifest_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let timestamp = value.as_str()?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
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
}
