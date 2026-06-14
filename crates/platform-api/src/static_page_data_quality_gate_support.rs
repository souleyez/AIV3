use std::collections::BTreeMap;

use domain_model::StaticPageDraft;
use serde_json::{json, Value};

use crate::{
    assistant_run_static_page_binding_quality_module_brief,
    refresh_static_page_payload_design_contract, static_page_artifact_string,
    static_page_payload_support::ensure_json_object, static_page_visualization_needs_sample_rows,
};

pub(crate) fn static_page_preview_data_quality_gate_for_draft(
    draft: &StaticPageDraft,
) -> Option<(String, Value)> {
    let mut payload = draft.draft_payload.clone();
    ensure_json_object(&mut payload);
    if let Some(object) = payload.as_object_mut() {
        object
            .entry("selected_scope".to_string())
            .or_insert_with(|| draft.selected_scope.clone());
    }
    refresh_static_page_payload_design_contract(&mut payload);

    static_page_preview_data_quality_gate_for_payload(&payload)
}

pub(crate) fn static_page_preview_data_quality_gate_for_payload(
    payload: &Value,
) -> Option<(String, Value)> {
    let attention_modules = static_page_preview_data_quality_attention_modules(payload);
    if attention_modules.is_empty() {
        return None;
    }
    let message = static_page_preview_data_quality_message(&attention_modules);
    let details = static_page_data_quality_gate_details(
        &attention_modules,
        &message,
        "submit_static_page_image_preview",
        "Return to module editing, add sample rows or rebind weak fields, then submit the effect preview again.",
        &[
            "static_page.update_draft",
            "retrieval.search",
            "static_page.submit_preview_after_repair",
        ],
    );
    Some((message, details))
}

pub(crate) fn static_page_final_render_data_quality_gate_for_draft(
    draft: &StaticPageDraft,
) -> Option<(String, Value)> {
    let mut payload = draft.draft_payload.clone();
    ensure_json_object(&mut payload);
    if let Some(object) = payload.as_object_mut() {
        object
            .entry("selected_scope".to_string())
            .or_insert_with(|| draft.selected_scope.clone());
    }
    refresh_static_page_payload_design_contract(&mut payload);

    let attention_modules = static_page_final_render_data_quality_attention_modules(&payload);
    if attention_modules.is_empty() {
        return None;
    }
    let message = static_page_preview_data_quality_message(&attention_modules);
    let details = static_page_data_quality_gate_details(
        &attention_modules,
        &message,
        "render_static_page",
        "Return to module editing, add sample rows or rebind weak fields, then regenerate and confirm the effect preview before final render.",
        &[
            "static_page.update_draft",
            "retrieval.search",
            "submit_static_page_image_preview",
        ],
    );
    Some((message, details))
}

fn static_page_preview_data_quality_attention_modules(payload: &Value) -> Vec<Value> {
    payload
        .get("dataSnapshot")
        .or_else(|| payload.get("data_snapshot"))
        .and_then(|snapshot| {
            snapshot
                .get("moduleBindings")
                .or_else(|| snapshot.get("module_bindings"))
        })
        .and_then(Value::as_array)
        .map(|bindings| {
            bindings
                .iter()
                .take(24)
                .map(assistant_run_static_page_binding_quality_module_brief)
                .filter(static_page_preview_binding_module_needs_attention)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn static_page_preview_binding_module_needs_attention(module: &Value) -> bool {
    let status = static_page_artifact_string(
        module,
        &["bindingQualityStatus", "binding_quality_status", "status"],
    )
    .unwrap_or_default();
    if !status.is_empty()
        && !matches!(
            status.as_str(),
            "confirmed" | "ready" | "non_chart" | "partial"
        )
    {
        return true;
    }

    let chart_data_fit = static_page_artifact_string(module, &["chartDataFit", "chart_data_fit"])
        .unwrap_or_default();
    if !chart_data_fit.is_empty()
        && !matches!(
            chart_data_fit.as_str(),
            "ready" | "not_required" | "non_chart_ready" | "needs_sample_rows" | "inferred_signal"
        )
    {
        return true;
    }

    let visualization_type =
        static_page_artifact_string(module, &["visualizationType", "visualization_type"])
            .unwrap_or_default();
    let has_binding = static_page_preview_module_has_binding_source(module)
        || module
            .get("binding")
            .or_else(|| module.get("dataBinding"))
            .or_else(|| module.get("data_binding"))
            .is_some_and(static_page_preview_module_has_binding_source);
    static_page_visualization_needs_sample_rows(&visualization_type)
        && static_page_preview_quality_u64(module, &["sampleRows", "sample_rows"]) == 0
        && !has_binding
}

fn static_page_preview_module_has_binding_source(binding: &Value) -> bool {
    static_page_artifact_string(
        binding,
        &["sourceId", "source_id", "fieldPath", "field_path", "label"],
    )
    .is_some()
}

fn static_page_final_render_data_quality_attention_modules(payload: &Value) -> Vec<Value> {
    payload
        .get("dataSnapshot")
        .or_else(|| payload.get("data_snapshot"))
        .and_then(|snapshot| {
            snapshot
                .get("moduleBindings")
                .or_else(|| snapshot.get("module_bindings"))
        })
        .and_then(Value::as_array)
        .map(|bindings| {
            bindings
                .iter()
                .take(24)
                .map(assistant_run_static_page_binding_quality_module_brief)
                .filter(static_page_final_render_binding_module_needs_attention)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn static_page_final_render_binding_module_needs_attention(module: &Value) -> bool {
    let status = static_page_artifact_string(
        module,
        &["bindingQualityStatus", "binding_quality_status", "status"],
    )
    .unwrap_or_default();
    if !status.is_empty()
        && !matches!(
            status.as_str(),
            "confirmed" | "ready" | "non_chart" | "partial"
        )
    {
        return true;
    }

    let chart_data_fit = static_page_artifact_string(module, &["chartDataFit", "chart_data_fit"])
        .unwrap_or_default();
    if !chart_data_fit.is_empty()
        && !matches!(
            chart_data_fit.as_str(),
            "ready" | "not_required" | "non_chart_ready" | "needs_sample_rows" | "inferred_signal"
        )
    {
        return true;
    }

    let visualization_type =
        static_page_artifact_string(module, &["visualizationType", "visualization_type"])
            .unwrap_or_default();
    let has_binding = static_page_preview_module_has_binding_source(module)
        || module
            .get("binding")
            .or_else(|| module.get("dataBinding"))
            .or_else(|| module.get("data_binding"))
            .is_some_and(static_page_preview_module_has_binding_source);
    static_page_visualization_needs_sample_rows(&visualization_type)
        && static_page_preview_quality_u64(module, &["sampleRows", "sample_rows"]) == 0
        && !has_binding
}

fn static_page_preview_quality_u64(value: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(|item| item.as_u64().or_else(|| item.as_str()?.parse::<u64>().ok()))
        })
        .unwrap_or(0)
}

fn static_page_preview_data_quality_message(modules: &[Value]) -> String {
    let labels = modules
        .iter()
        .take(3)
        .map(static_page_preview_data_quality_module_label)
        .collect::<Vec<_>>()
        .join("、");
    let suffix = format!(" {} 个模块", modules.len());
    format!(
        "当前静态页还有{suffix}的数据绑定未达到可视化生成要求：{labels}。请先让 DataMax 补充样本行、重新匹配字段，或检索/修复模块数据。"
    )
}

fn static_page_data_quality_gate_details(
    modules: &[Value],
    message: &str,
    blocked_action: &str,
    next_step: &str,
    recommended_actions: &[&str],
) -> Value {
    let modules = modules
        .iter()
        .take(24)
        .map(static_page_data_quality_gate_module)
        .collect::<Vec<_>>();
    let module_count = modules.len();
    let gate_reason_counts = static_page_data_quality_gate_reason_counts(&modules);
    json!({
        "gate": "static_page_preview_data_quality",
        "blockedAction": blocked_action,
        "blocked_action": blocked_action,
        "reason": message,
        "attentionModuleCount": module_count,
        "attention_module_count": module_count,
        "attentionModules": modules.clone(),
        "attention_modules": modules,
        "gateReasonCounts": gate_reason_counts.clone(),
        "gate_reason_counts": gate_reason_counts,
        "recommendedActions": recommended_actions,
        "recommended_actions": recommended_actions,
        "nextStep": next_step,
        "next_step": next_step
    })
}

fn static_page_data_quality_gate_module(module: &Value) -> Value {
    let mut module = module.clone();
    let gate_reasons = static_page_preview_binding_module_gate_reasons(&module);
    if let Some(object) = module.as_object_mut() {
        let gate_reasons = json!(gate_reasons);
        object.insert("gateReasons".to_string(), gate_reasons.clone());
        object.insert("gate_reasons".to_string(), gate_reasons);
    }
    module
}

fn static_page_preview_binding_module_gate_reasons(module: &Value) -> Vec<String> {
    let mut reasons = Vec::new();
    let status = static_page_artifact_string(
        module,
        &["bindingQualityStatus", "binding_quality_status", "status"],
    )
    .unwrap_or_default();
    if !status.is_empty() && !matches!(status.as_str(), "confirmed" | "ready" | "non_chart") {
        reasons.push(format!("binding_quality_status:{status}"));
    }

    let chart_data_fit = static_page_artifact_string(module, &["chartDataFit", "chart_data_fit"])
        .unwrap_or_default();
    if !chart_data_fit.is_empty()
        && !matches!(
            chart_data_fit.as_str(),
            "ready" | "not_required" | "non_chart_ready"
        )
    {
        reasons.push(format!("chart_data_fit:{chart_data_fit}"));
    }

    let visualization_type =
        static_page_artifact_string(module, &["visualizationType", "visualization_type"])
            .unwrap_or_default();
    if static_page_visualization_needs_sample_rows(&visualization_type)
        && static_page_preview_quality_u64(module, &["sampleRows", "sample_rows"]) == 0
    {
        reasons.push("chart_sample_rows_missing".to_string());
    }

    if reasons.is_empty() {
        reasons.push("data_quality_attention_required".to_string());
    }
    reasons
}

fn static_page_data_quality_gate_reason_counts(modules: &[Value]) -> Value {
    let mut counts = BTreeMap::<String, u64>::new();
    for reason in modules
        .iter()
        .filter_map(|module| module.get("gateReasons").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str)
    {
        *counts.entry(reason.to_string()).or_insert(0) += 1;
    }
    json!(counts)
}

fn static_page_preview_data_quality_module_label(module: &Value) -> String {
    let title = static_page_artifact_string(module, &["title"])
        .or_else(|| static_page_artifact_string(module, &["moduleId", "module_id", "id"]))
        .unwrap_or_else(|| "未命名模块".to_string());
    let chart_data_fit = static_page_artifact_string(module, &["chartDataFit", "chart_data_fit"])
        .unwrap_or_default();
    let status = static_page_artifact_string(
        module,
        &["bindingQualityStatus", "binding_quality_status", "status"],
    )
    .unwrap_or_default();
    let marker = if !chart_data_fit.is_empty()
        && !matches!(
            chart_data_fit.as_str(),
            "ready" | "not_required" | "non_chart_ready"
        ) {
        chart_data_fit
    } else {
        status
    };
    if marker.is_empty() {
        title
    } else {
        format!("{title}（{marker}）")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_gate_blocks_unbound_chart_module() {
        let payload = json!({
            "dataSnapshot": {
                "moduleBindings": [{
                    "moduleId": "trend",
                    "title": "订单趋势",
                    "visualizationType": "line-chart",
                    "bindingQuality": {
                        "status": "missing",
                        "chartDataFit": "missing_binding",
                        "sampleRows": 0
                    }
                }]
            }
        });

        let (message, details) = static_page_preview_data_quality_gate_for_payload(&payload)
            .expect("weak chart binding should require attention");

        assert!(message.contains("订单趋势"));
        assert_eq!(
            details["blockedAction"],
            json!("submit_static_page_image_preview")
        );
        assert_eq!(details["attentionModuleCount"], json!(1));
        assert_eq!(
            details["gateReasonCounts"]["binding_quality_status:missing"],
            json!(1)
        );
        assert_eq!(
            details["gateReasonCounts"]["chart_data_fit:missing_binding"],
            json!(1)
        );
        assert_eq!(
            details["gateReasonCounts"]["chart_sample_rows_missing"],
            json!(1)
        );
    }

    #[test]
    fn preview_gate_allows_partial_chart_with_binding_source() {
        let payload = json!({
            "data_snapshot": {
                "module_bindings": [{
                    "module_id": "trend",
                    "title": "订单趋势",
                    "visualization_type": "line-chart",
                    "binding": {"source_id": "database", "field_path": "sales.amount"},
                    "binding_quality": {
                        "status": "partial",
                        "chart_data_fit": "needs_sample_rows",
                        "sample_rows": 0
                    }
                }]
            }
        });

        assert!(static_page_preview_data_quality_gate_for_payload(&payload).is_none());
    }

    #[test]
    fn final_gate_details_use_render_action_and_repair_steps() {
        let payload = json!({
            "dataSnapshot": {
                "moduleBindings": [{
                    "moduleId": "risk",
                    "title": "风险矩阵",
                    "visualizationType": "risk-matrix",
                    "bindingQuality": {
                        "status": "missing",
                        "chartDataFit": "missing_binding",
                        "sampleRows": 0
                    }
                }]
            }
        });

        let modules = static_page_final_render_data_quality_attention_modules(&payload);
        let message = static_page_preview_data_quality_message(&modules);
        let details = static_page_data_quality_gate_details(
            &modules,
            &message,
            "render_static_page",
            "next",
            &[
                "static_page.update_draft",
                "retrieval.search",
                "submit_static_page_image_preview",
            ],
        );

        assert_eq!(details["blockedAction"], json!("render_static_page"));
        assert!(details["recommendedActions"]
            .as_array()
            .is_some_and(|actions| actions.contains(&json!("submit_static_page_image_preview"))));
    }
}
