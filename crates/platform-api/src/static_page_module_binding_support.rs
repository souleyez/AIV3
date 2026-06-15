use serde_json::{json, Value};

pub(crate) fn static_page_module_binding_quality(
    module: &Value,
    binding: &Value,
    field_candidates: &Value,
    sample_data: &Value,
    data_quality: &str,
) -> Value {
    let visualization_type = module
        .get("visualization")
        .and_then(|visualization| visualization.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("text-insight");
    let chart_runtime = static_page_module_chart_runtime(module);
    let source_id =
        static_page_binding_string(binding, &["sourceId", "source_id"]).unwrap_or_default();
    let field_path = static_page_module_field_path(module).map(ToString::to_string);
    let sample_rows = sample_data.as_array().map(Vec::len).unwrap_or(0);
    let chart_needs_rows = static_page_visualization_needs_sample_rows(visualization_type);
    let matched_candidate = field_path.as_deref().and_then(|field| {
        static_page_matching_field_candidate(field_candidates, &source_id, field)
    });
    let has_binding = binding.is_object()
        && (!source_id.is_empty()
            || field_path.is_some()
            || static_page_binding_string(binding, &["label"]).is_some());
    let has_confirmed_rows = (sample_rows > 0
        && matches!(
            data_quality,
            "module_data" | "evidence_value" | "schema_context"
        ))
        || static_page_sample_data_contains_kind(sample_data, "media_window")
        || static_page_sample_data_contains_kind(sample_data, "database_schema");
    let has_inferred_rows = sample_rows > 0 && !has_confirmed_rows;

    let (status, reason, chart_data_fit, recommended_action) = if has_confirmed_rows {
        (
            "confirmed",
            "renderable_data_rows",
            "ready",
            "数据样本可直接驱动该模块；交付前只需确认字段口径。",
        )
    } else if has_inferred_rows {
        (
            "partial",
            "inferred_evidence_signal",
            "inferred_signal",
            "当前只有检索信号或媒体线索，建议补充明确数据行或让模型先抽取结构化数据。",
        )
    } else if chart_needs_rows && field_path.is_none() && !has_binding {
        (
            "missing",
            "chart_without_binding",
            "missing_binding",
            "图表模块缺少字段绑定和样本数据，需要先绑定字段或补充数据行。",
        )
    } else if chart_needs_rows && matched_candidate.is_some() {
        (
            "partial",
            "matched_field_candidate_without_rows",
            "needs_sample_rows",
            "已匹配候选字段，但还缺少可渲染样本行；生成可视化前建议抽取或填写数据。",
        )
    } else if chart_needs_rows {
        (
            "partial",
            "binding_without_sample_rows",
            "needs_sample_rows",
            "已有绑定意图但缺少可渲染数据行，最终页会降级为待确认状态。",
        )
    } else if has_binding
        || matches!(
            source_id.as_str(),
            "model" | "session" | "conversation_memory"
        )
    {
        (
            "confirmed",
            "non_chart_binding_ready",
            "not_required",
            "文本或结论模块不强制要求数值样本，可按当前绑定继续规划。",
        )
    } else {
        (
            "missing",
            "non_chart_without_binding",
            "not_required",
            "该模块缺少内容来源，建议绑定模型总结、会话摘要或检索证据。",
        )
    };

    json!({
        "status": status,
        "reason": reason,
        "chartDataFit": chart_data_fit,
        "recommendedAction": recommended_action,
        "sourceId": if source_id.is_empty() { Value::Null } else { json!(source_id) },
        "fieldPath": field_path,
        "visualizationType": visualization_type,
        "chartRuntime": chart_runtime,
        "sampleRows": sample_rows,
        "dataQuality": data_quality,
        "matchedFieldCandidate": matched_candidate.cloned().unwrap_or(Value::Null),
        "confidence": static_page_binding_confidence(matched_candidate, sample_rows, status),
    })
}

pub(crate) fn static_page_module_field_path(module: &Value) -> Option<&str> {
    module
        .get("dataBinding")
        .or_else(|| module.get("data_binding"))
        .and_then(|binding| {
            binding
                .get("fieldPath")
                .or_else(|| binding.get("field_path"))
                .or_else(|| binding.get("field"))
        })
        .or_else(|| {
            module
                .get("visualization")
                .and_then(|visualization| visualization.get("chartOptions"))
                .and_then(|chart_options| chart_options.get("dataKey"))
        })
        .or_else(|| {
            module
                .get("chartOptions")
                .and_then(|chart_options| chart_options.get("dataKey"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn static_page_module_source_id(module: &Value) -> String {
    module
        .get("dataBinding")
        .or_else(|| module.get("data_binding"))
        .and_then(|binding| {
            binding
                .get("sourceId")
                .or_else(|| binding.get("source_id"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn static_page_module_chart_runtime(module: &Value) -> &'static str {
    let runtime = module
        .get("visualization")
        .and_then(|visualization| {
            visualization
                .get("chartRuntime")
                .or_else(|| visualization.get("runtime"))
                .or_else(|| {
                    visualization
                        .get("chartOptions")
                        .and_then(|chart_options| chart_options.get("chartRuntime"))
                })
                .or_else(|| {
                    visualization
                        .get("chartOptions")
                        .and_then(|chart_options| chart_options.get("runtime"))
                })
        })
        .or_else(|| module.get("chartRuntime"))
        .or_else(|| {
            module
                .get("chartOptions")
                .and_then(|chart_options| chart_options.get("chartRuntime"))
        })
        .and_then(Value::as_str)
        .map(str::trim);
    match runtime {
        Some("echarts") => "echarts",
        _ => "deterministic",
    }
}

pub(crate) fn static_page_visualization_needs_sample_rows(visualization_type: &str) -> bool {
    matches!(
        visualization_type,
        "kpi-cards" | "bar-chart" | "line-chart" | "donut-chart" | "table" | "risk-matrix"
    )
}

pub(crate) fn static_page_matching_field_candidate<'a>(
    field_candidates: &'a Value,
    source_id: &str,
    field_path: &str,
) -> Option<&'a Value> {
    field_candidates.as_array()?.iter().find(|candidate| {
        let candidate_field = candidate
            .get("fieldPath")
            .or_else(|| candidate.get("field_path"))
            .or_else(|| candidate.get("field"))
            .and_then(Value::as_str)
            .map(str::trim);
        if candidate_field != Some(field_path) {
            return false;
        }
        if source_id.is_empty() {
            return true;
        }
        candidate
            .get("sourceId")
            .or_else(|| candidate.get("source_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .map_or(true, |candidate_source| candidate_source == source_id)
    })
}

pub(crate) fn static_page_binding_string(binding: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| binding.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn static_page_sample_data_contains_kind(sample_data: &Value, kind: &str) -> bool {
    sample_data.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item.get("kind").and_then(Value::as_str) == Some(kind))
    })
}

fn static_page_binding_confidence(
    matched_candidate: Option<&Value>,
    sample_rows: usize,
    status: &str,
) -> Value {
    if let Some(confidence) = matched_candidate
        .and_then(|candidate| candidate.get("confidence"))
        .and_then(Value::as_f64)
    {
        return json!(confidence);
    }
    match (status, sample_rows) {
        ("confirmed", rows) if rows > 0 => json!(0.92),
        ("confirmed", _) => json!(0.72),
        ("partial", rows) if rows > 0 => json!(0.62),
        ("partial", _) => json!(0.48),
        _ => json!(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn static_page_module_binding_support_confirms_renderable_chart_rows() {
        let module = json!({
            "visualization": {
                "type": "bar-chart",
                "chartRuntime": "echarts"
            },
            "dataBinding": {
                "sourceId": "dataset",
                "fieldPath": "sales.monthly"
            }
        });
        let binding = module.get("dataBinding").unwrap();
        let field_candidates = json!([{
            "sourceId": "dataset",
            "fieldPath": "sales.monthly",
            "confidence": 0.81
        }]);
        let sample_data = json!([
            { "label": "A店", "value": 120.0 },
            { "label": "B店", "value": 90.0 }
        ]);

        let quality = static_page_module_binding_quality(
            &module,
            binding,
            &field_candidates,
            &sample_data,
            "module_data",
        );

        assert_eq!(quality.get("status"), Some(&json!("confirmed")));
        assert_eq!(quality.get("reason"), Some(&json!("renderable_data_rows")));
        assert_eq!(quality.get("chartDataFit"), Some(&json!("ready")));
        assert_eq!(quality.get("chartRuntime"), Some(&json!("echarts")));
        assert_eq!(quality.get("sampleRows"), Some(&json!(2)));
        assert_eq!(quality.get("confidence"), Some(&json!(0.81)));
    }

    #[test]
    fn static_page_module_binding_support_reports_matched_candidate_without_rows() {
        let module = json!({
            "visualization": { "type": "line-chart" },
            "dataBinding": {
                "source_id": "dataset",
                "field_path": "sales.trend"
            }
        });
        let binding = module.get("dataBinding").unwrap();
        let field_candidates = json!([{
            "source_id": "dataset",
            "field_path": "sales.trend",
            "confidence": 0.67
        }]);

        let quality = static_page_module_binding_quality(
            &module,
            binding,
            &field_candidates,
            &json!([]),
            "empty",
        );

        assert_eq!(quality.get("status"), Some(&json!("partial")));
        assert_eq!(
            quality.get("reason"),
            Some(&json!("matched_field_candidate_without_rows"))
        );
        assert_eq!(
            quality.get("chartDataFit"),
            Some(&json!("needs_sample_rows"))
        );
        assert_eq!(quality.get("confidence"), Some(&json!(0.67)));
        assert!(quality
            .get("matchedFieldCandidate")
            .is_some_and(Value::is_object));
    }

    #[test]
    fn static_page_module_binding_support_reports_missing_chart_binding() {
        let module = json!({
            "visualization": { "type": "donut-chart" }
        });

        let quality = static_page_module_binding_quality(
            &module,
            &Value::Null,
            &json!([]),
            &json!([]),
            "empty",
        );

        assert_eq!(quality.get("status"), Some(&json!("missing")));
        assert_eq!(quality.get("reason"), Some(&json!("chart_without_binding")));
        assert_eq!(quality.get("chartDataFit"), Some(&json!("missing_binding")));
        assert_eq!(quality.get("confidence"), Some(&json!(0.0)));
    }

    #[test]
    fn static_page_module_binding_support_accepts_media_sample_kind() {
        let module = json!({
            "visualization": { "type": "table" },
            "dataBinding": {
                "sourceId": "evidence",
                "fieldPath": "media.transcript_windows"
            }
        });
        let binding = module.get("dataBinding").unwrap();
        let sample_data = json!([{ "kind": "media_window", "label": "00:01.000", "value": 1.0 }]);

        let quality = static_page_module_binding_quality(
            &module,
            binding,
            &json!([]),
            &sample_data,
            "inferred",
        );

        assert_eq!(quality.get("status"), Some(&json!("confirmed")));
        assert_eq!(quality.get("chartDataFit"), Some(&json!("ready")));
        assert_eq!(quality.get("confidence"), Some(&json!(0.92)));
    }
}
