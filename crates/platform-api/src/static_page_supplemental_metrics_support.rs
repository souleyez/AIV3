use crate::assistant_run_evidence_state_support::assistant_run_evidence_supplied_count;
use crate::assistant_run_text_support::push_string_hint;
use serde_json::{json, Value};

pub(crate) fn static_page_supplemental_metrics_policy() -> Value {
    json!({
        "schema": "v3.static_page.supplemental_metrics.v1",
        "source_scope": "selected_scope_and_temporary_uploaded_documents_only",
        "contract_area": {
            "enabled": true,
            "use_case": "per_square_meter_efficiency",
            "accepted_fields": [
                "storecode",
                "store_code",
                "store_name",
                "area",
                "store_area",
                "contract_area",
                "leased_area",
                "business_area",
                "经营面积",
                "租赁面积",
                "合同面积",
                "门店面积",
                "铺位面积",
                "面积"
            ],
            "output_targets": [
                "data.storeList[].area",
                "data.supplementalMetrics.storeAreas",
                "data.supplementalMetrics.storeAreaRows"
            ],
            "merge_policy": "temporary_uploaded_contract_area_overrides_missing_or_zero_store_area_only"
        },
        "traffic": {
            "enabled": true,
            "use_case": "traffic_comparison_and_drop_warning",
            "accepted_fields": [
                "storecode",
                "store_code",
                "store_name",
                "txdate",
                "date",
                "traffic",
                "traffic_count",
                "visitor_count",
                "customer_flow",
                "客流",
                "客流量",
                "人流",
                "人流量",
                "客数",
                "进店人数",
                "到店人数"
            ],
            "output_targets": [
                "data.trafficRows",
                "data.supplementalMetrics.trafficRows"
            ],
            "comparison_policy": "compute_only_when_current_and_comparison_ranges_have_rows"
        },
        "no_invention": true,
    })
}

pub(crate) fn build_static_page_supplemental_metrics_summary_from_candidates(
    field_candidates: &Value,
    evidence_state: Option<&Value>,
) -> Value {
    let store_area =
        static_page_supplemental_metric_candidate_summary(field_candidates, "store.area");
    let traffic =
        static_page_supplemental_metric_candidate_summary(field_candidates, "traffic.count");
    let has_store_area = store_area["status"] == json!("candidate_available");
    let has_traffic = traffic["status"] == json!("candidate_available");

    json!({
        "schema": "v3.static_page.supplemental_metrics.v1",
        "status": if has_store_area || has_traffic {
            "candidate_available"
        } else {
            "not_detected"
        },
        "store_area": store_area,
        "traffic": traffic,
        "supplied_count": evidence_state
            .map(assistant_run_evidence_supplied_count)
            .unwrap_or(0),
        "temporary_upload_policy": "Selected temporary documents can supply these metrics for the current report run only; do not persist them as production master data without confirmation.",
        "output_contract": {
            "store_area": [
                "data.storeList[].area",
                "data.supplementalMetrics.storeAreas",
                "data.supplementalMetrics.storeAreaRows"
            ],
            "traffic": [
                "data.trafficRows",
                "data.supplementalMetrics.trafficRows"
            ]
        },
        "no_invention": true,
    })
}

fn static_page_supplemental_metric_candidate_summary(
    field_candidates: &Value,
    field_path: &str,
) -> Value {
    let mut labels = Vec::<String>::new();
    let mut evidence_refs = Vec::<Value>::new();
    let mut confidence = 0.0f64;

    if let Some(candidates) = field_candidates.as_array() {
        for candidate in candidates {
            if candidate.get("fieldPath").and_then(Value::as_str) != Some(field_path) {
                continue;
            }
            if let Some(label) = candidate.get("label").and_then(Value::as_str) {
                push_string_hint(&mut labels, label);
            }
            if let Some(value) = candidate.get("confidence").and_then(Value::as_f64) {
                confidence = confidence.max(value);
            }
            let evidence_ref = candidate
                .get("evidenceRef")
                .or_else(|| candidate.get("evidence_ref"))
                .cloned()
                .unwrap_or(Value::Null);
            if !evidence_ref.is_null() && evidence_refs.len() < 4 {
                evidence_refs.push(evidence_ref);
            }
        }
    }

    labels.truncate(4);
    json!({
        "status": if labels.is_empty() {
            "not_detected"
        } else {
            "candidate_available"
        },
        "field_path": field_path,
        "labels": labels,
        "confidence": if confidence > 0.0 {
            Value::from(confidence)
        } else {
            Value::Null
        },
        "evidence_refs": evidence_refs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplemental_metrics_policy_declares_area_and_traffic_contracts() {
        let policy = static_page_supplemental_metrics_policy();

        assert_eq!(policy["schema"], "v3.static_page.supplemental_metrics.v1");
        assert_eq!(
            policy["source_scope"],
            "selected_scope_and_temporary_uploaded_documents_only"
        );
        assert_eq!(policy["no_invention"], true);
        assert_eq!(policy["contract_area"]["enabled"], true);
        assert_eq!(
            policy["contract_area"]["use_case"],
            "per_square_meter_efficiency"
        );
        assert!(policy["contract_area"]["accepted_fields"]
            .as_array()
            .unwrap()
            .contains(&json!("合同面积")));
        assert!(policy["contract_area"]["output_targets"]
            .as_array()
            .unwrap()
            .contains(&json!("data.storeList[].area")));
        assert_eq!(
            policy["contract_area"]["merge_policy"],
            "temporary_uploaded_contract_area_overrides_missing_or_zero_store_area_only"
        );
        assert_eq!(policy["traffic"]["enabled"], true);
        assert!(policy["traffic"]["accepted_fields"]
            .as_array()
            .unwrap()
            .contains(&json!("客流")));
        assert!(policy["traffic"]["output_targets"]
            .as_array()
            .unwrap()
            .contains(&json!("data.trafficRows")));
        assert_eq!(
            policy["traffic"]["comparison_policy"],
            "compute_only_when_current_and_comparison_ranges_have_rows"
        );
    }

    #[test]
    fn supplemental_metrics_summary_detects_store_area_and_traffic_candidates() {
        let summary = build_static_page_supplemental_metrics_summary_from_candidates(
            &json!([
                {
                    "fieldPath": "store.area",
                    "label": "合同面积",
                    "confidence": 0.83,
                    "evidenceRef": {"item": "contract-area"}
                },
                {
                    "fieldPath": "traffic.count",
                    "label": "客流人数",
                    "confidence": 0.77,
                    "evidence_ref": {"item": "traffic-row"}
                }
            ]),
            Some(&json!({
                "supplied_items": [
                    {"id": "one"},
                    {"id": "two"}
                ]
            })),
        );

        assert_eq!(summary["schema"], "v3.static_page.supplemental_metrics.v1");
        assert_eq!(summary["status"], "candidate_available");
        assert_eq!(summary["supplied_count"], 2);
        assert_eq!(summary["no_invention"], true);
        assert_eq!(summary["store_area"]["status"], "candidate_available");
        assert_eq!(summary["store_area"]["labels"], json!(["合同面积"]));
        assert_eq!(summary["store_area"]["confidence"], 0.83);
        assert_eq!(summary["traffic"]["status"], "candidate_available");
        assert_eq!(summary["traffic"]["labels"], json!(["客流人数"]));
        assert_eq!(
            summary["output_contract"]["store_area"][0],
            "data.storeList[].area"
        );
        assert_eq!(summary["output_contract"]["traffic"][0], "data.trafficRows");
    }

    #[test]
    fn supplemental_metric_candidate_summary_dedupes_labels_caps_refs_and_uses_max_confidence() {
        let summary = static_page_supplemental_metric_candidate_summary(
            &json!([
                {"fieldPath": "store.area", "label": " 面积 ", "confidence": 0.5, "evidenceRef": 1},
                {"fieldPath": "store.area", "label": "面积", "confidence": 0.9, "evidenceRef": 2},
                {"fieldPath": "store.area", "label": "合同面积", "confidence": 0.4, "evidenceRef": 3},
                {"fieldPath": "store.area", "label": "建筑面积", "confidence": 0.6, "evidence_ref": 4},
                {"fieldPath": "store.area", "label": "经营面积", "confidence": 0.7, "evidenceRef": 5},
                {"fieldPath": "store.area", "label": "备用面积", "confidence": 0.8, "evidenceRef": 6},
                {"fieldPath": "traffic.count", "label": "客流", "confidence": 1.0, "evidenceRef": 7}
            ]),
            "store.area",
        );

        assert_eq!(summary["status"], "candidate_available");
        assert_eq!(
            summary["labels"],
            json!(["面积", "合同面积", "建筑面积", "经营面积"])
        );
        assert_eq!(summary["confidence"], 0.9);
        assert_eq!(summary["evidence_refs"], json!([1, 2, 3, 4]));
    }

    #[test]
    fn supplemental_metrics_summary_reports_not_detected_without_matching_candidates() {
        let summary = build_static_page_supplemental_metrics_summary_from_candidates(
            &json!([
                {
                    "fieldPath": "sales.amount",
                    "label": "销售额",
                    "confidence": 0.99,
                    "evidenceRef": "sales"
                }
            ]),
            None,
        );

        assert_eq!(summary["status"], "not_detected");
        assert_eq!(summary["supplied_count"], 0);
        assert_eq!(summary["store_area"]["status"], "not_detected");
        assert_eq!(summary["store_area"]["confidence"], Value::Null);
        assert_eq!(summary["store_area"]["evidence_refs"], json!([]));
        assert_eq!(summary["traffic"]["status"], "not_detected");
        assert_eq!(summary["traffic"]["labels"], json!([]));
    }
}
