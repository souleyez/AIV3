use crate::truncate_assistant_supply_text;
use contracts::StaticPageImageJobView;
use serde_json::{json, Value};

pub(crate) fn external_static_page_image_asset_provenance_summary(
    image_job: &StaticPageImageJobView,
) -> Value {
    let raw = image_job
        .image_prompt_payload
        .pointer("/orchestrator/previewAssetProvenance")
        .or_else(|| {
            image_job
                .image_prompt_payload
                .pointer("/orchestrator/preview_asset_provenance")
        })
        .unwrap_or(&Value::Null);
    let render_asset_url = image_job
        .preview_asset_key
        .as_deref()
        .map(external_static_page_safe_preview_asset_ref)
        .unwrap_or(Value::Null);
    let persisted_preview_asset_key = raw
        .get("persistedPreviewAssetKey")
        .or_else(|| raw.get("persisted_preview_asset_key"))
        .and_then(Value::as_str)
        .map(external_static_page_safe_preview_asset_ref)
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| render_asset_url.clone());
    json!({
        "schema": "v3.static_page_preview_asset_provenance",
        "schemaVersion": 1,
        "renderAssetUrl": render_asset_url,
        "renderAssetPolicy": "use_persisted_v3_preview_asset_for_final_html",
        "sourceAssetKind": raw.get("sourceAssetKind")
            .or_else(|| raw.get("source_asset_kind"))
            .and_then(Value::as_str)
            .map(|value| truncate_assistant_supply_text(value, 64))
            .unwrap_or_else(|| "unknown".to_string()),
        "sourceAssetRef": raw.get("sourceAssetRef")
            .or_else(|| raw.get("source_asset_ref"))
            .and_then(Value::as_str)
            .map(external_static_page_safe_preview_asset_ref)
            .unwrap_or(Value::Null),
        "sourceAssetRefRedacted": raw.get("sourceAssetRefRedacted")
            .or_else(|| raw.get("source_asset_ref_redacted"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "sourceAssetHadQuery": raw.get("sourceAssetHadQuery")
            .or_else(|| raw.get("source_asset_had_query"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "persisted": raw.get("persisted").and_then(Value::as_bool).unwrap_or_else(|| image_job.preview_asset_key.is_some()),
        "persistedPreviewAssetKey": persisted_preview_asset_key,
        "storageStatus": raw.get("storageStatus")
            .or_else(|| raw.get("storage_status"))
            .and_then(Value::as_str)
            .map(|value| truncate_assistant_supply_text(value, 64))
            .unwrap_or_else(|| if image_job.preview_asset_key.is_some() { "ready".to_string() } else { "unknown".to_string() }),
        "byteSize": raw.get("byteSize").or_else(|| raw.get("byte_size")).and_then(Value::as_u64).map(Value::from).unwrap_or(Value::Null),
        "mimeType": raw.get("mimeType").or_else(|| raw.get("mime_type")).and_then(Value::as_str).map(|value| Value::String(truncate_assistant_supply_text(value, 80))).unwrap_or(Value::Null),
        "width": raw.get("width").and_then(Value::as_i64).map(Value::from).unwrap_or(Value::Null),
        "height": raw.get("height").and_then(Value::as_i64).map(Value::from).unwrap_or(Value::Null),
    })
}

pub(crate) fn external_static_page_safe_preview_asset_ref(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with("data:image/") || trimmed.starts_with("blob:") {
        return Value::Null;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Ok(mut url) = reqwest::Url::parse(trimmed) {
            url.set_query(None);
            url.set_fragment(None);
            return Value::String(truncate_assistant_supply_text(url.as_str(), 500));
        }
        return Value::Null;
    }
    Value::String(truncate_assistant_supply_text(trimmed, 500))
}

pub(crate) fn external_static_page_image_prompt_text(
    image_prompt_payload_summary: &Value,
    fallback_prompt: &str,
) -> String {
    image_prompt_payload_summary
        .get("prompt_text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_assistant_supply_text(value, 2000))
        .unwrap_or_else(|| truncate_assistant_supply_text(fallback_prompt, 2000))
}

pub(crate) fn external_static_page_image_prompt_payload_summary(payload: &Value) -> Value {
    let modules = payload
        .get("modules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(8)
                .map(external_static_page_image_module_summary)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let prompt_text = [
        payload.get("promptText"),
        payload.get("prompt_text"),
        payload.get("prompt"),
    ]
    .into_iter()
    .flatten()
    .find_map(Value::as_str)
    .map(|value| truncate_assistant_supply_text(value, 1200));
    json!({
        "truncated": true,
        "summary_for": "static_page_image2_data_publish",
        "title": payload.get("title").cloned().unwrap_or(Value::Null),
        "prompt_text": prompt_text,
        "style_direction": payload.get("style_direction").or_else(|| payload.get("styleDirection")).cloned().unwrap_or(Value::Null),
        "modules": modules,
        "module_count": payload
            .get("modules")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "preview_contract_excerpt": external_static_page_value_excerpt(
            payload
                .get("preview_contract")
                .or_else(|| payload.get("previewContract")),
            1000,
        ),
        "data_snapshot_excerpt": external_static_page_value_excerpt(
            payload
                .get("data_snapshot")
                .or_else(|| payload.get("dataSnapshot")),
            1000,
        ),
        "data_snapshot": external_static_page_image_prompt_payload_data_snapshot(payload),
    })
}

pub(crate) fn external_static_page_image_prompt_payload_data_snapshot(payload: &Value) -> Value {
    let snapshot = payload
        .get("data_snapshot")
        .or_else(|| payload.get("dataSnapshot"))
        .cloned()
        .unwrap_or(Value::Null);
    if snapshot.is_null() {
        return Value::Null;
    }
    json!({
        "source": snapshot.get("source").cloned().unwrap_or(Value::Null),
        "snapshotVersion": snapshot
            .get("snapshotVersion")
            .or_else(|| snapshot.get("snapshot_version"))
            .cloned()
            .unwrap_or(Value::Null),
        "updatedAt": snapshot
            .get("updatedAt")
            .or_else(|| snapshot.get("updated_at"))
            .cloned()
            .unwrap_or(Value::Null),
        "validation_summary": snapshot.get("validation_summary").cloned().unwrap_or(Value::Null),
        "sampleRowCount": snapshot
            .get("sampleRowCount")
            .or_else(|| snapshot.pointer("/validation_summary/sampleRowCount"))
            .cloned()
            .unwrap_or(Value::Null),
        "detailRowCount": snapshot
            .get("detailRowCount")
            .or_else(|| snapshot.pointer("/validation_summary/detailRowCount"))
            .cloned()
            .unwrap_or(Value::Null),
        "unitHints": snapshot
            .get("unitHints")
            .or_else(|| snapshot.pointer("/validation_summary/unitHints"))
            .cloned()
            .unwrap_or_else(|| json!([])),
        "data_source_candidates": snapshot
            .get("data_source_candidates")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "field_candidates": static_page_fixed_task_compact_array(
            snapshot.get("field_candidates"),
            12,
        ),
        "module_bindings": static_page_fixed_task_compact_module_bindings(
            snapshot.get("module_bindings"),
        ),
        "structure_signals": snapshot.get("structure_signals").cloned().unwrap_or(Value::Null),
        "refresh_policy": snapshot
            .get("refresh_policy")
            .or_else(|| snapshot.get("refresh"))
            .cloned()
            .unwrap_or(Value::Null),
    })
}

pub(crate) fn static_page_fixed_task_compact_array(value: Option<&Value>, limit: usize) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .map(|items| items.iter().take(limit).cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    )
}

pub(crate) fn static_page_fixed_task_compact_module_bindings(value: Option<&Value>) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .take(12)
                    .map(|item| {
                        let mut compact = item.clone();
                        if let Some(object) = compact.as_object_mut() {
                            object.insert(
                                "sampleData".to_string(),
                                static_page_fixed_task_compact_array(
                                    item.get("sampleData").or_else(|| item.get("sample_data")),
                                    12,
                                ),
                            );
                        }
                        compact
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    )
}

pub(crate) fn external_static_page_image_module_summary(module: &Value) -> Value {
    json!({
        "id": module.get("id").cloned().unwrap_or(Value::Null),
        "role": module.get("role").cloned().unwrap_or(Value::Null),
        "title": module.get("title").cloned().unwrap_or(Value::Null),
        "layout": module.get("layout").cloned().unwrap_or(Value::Null),
        "content": module
            .get("content")
            .and_then(Value::as_str)
            .map(|value| truncate_assistant_supply_text(value, 240)),
        "visualization": module.get("visualization").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn external_static_page_value_excerpt(value: Option<&Value>, max_chars: usize) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    let text = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    Value::String(truncate_assistant_supply_text(&text, max_chars))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::StaticPageImageJobStatusView;
    use domain_model::{AssistantRunId, StaticPageDraftId, StaticPageImageJobId};

    fn image_job_with_payload(
        payload: Value,
        preview_asset_key: Option<&str>,
    ) -> StaticPageImageJobView {
        let now = Utc::now();
        StaticPageImageJobView {
            id: StaticPageImageJobId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatusView::PreviewReady,
            queue_position: None,
            image_prompt_payload: payload,
            preview_asset_key: preview_asset_key.map(str::to_string),
            failure_reason: None,
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn safe_preview_asset_ref_strips_remote_query_and_rejects_inline_assets() {
        assert_eq!(
            external_static_page_safe_preview_asset_ref(
                " https://cdn.example.com/a.png?token=secret#frag "
            ),
            json!("https://cdn.example.com/a.png")
        );
        assert_eq!(
            external_static_page_safe_preview_asset_ref("static-page-previews/a.png"),
            json!("static-page-previews/a.png")
        );
        assert_eq!(
            external_static_page_safe_preview_asset_ref("data:image/png;base64,secret"),
            Value::Null
        );
        assert_eq!(
            external_static_page_safe_preview_asset_ref("blob:https://example.com/secret"),
            Value::Null
        );
    }

    #[test]
    fn asset_provenance_summary_redacts_urls_and_prefers_persisted_asset() {
        let job = image_job_with_payload(
            json!({
                "orchestrator": {
                    "previewAssetProvenance": {
                        "sourceAssetKind": "generated_image",
                        "sourceAssetRef": "https://source.example.com/raw.png?token=secret#frag",
                        "persistedPreviewAssetKey": "https://cdn.example.com/persisted.png?sig=secret#frag",
                        "storageStatus": "ready",
                        "byteSize": 1234,
                        "mimeType": "image/png",
                        "width": 390,
                        "height": 844
                    }
                }
            }),
            Some("https://cdn.example.com/render.png?sig=secret#frag"),
        );

        let summary = external_static_page_image_asset_provenance_summary(&job);

        assert_eq!(
            summary["renderAssetUrl"],
            json!("https://cdn.example.com/render.png")
        );
        assert_eq!(
            summary["sourceAssetRef"],
            json!("https://source.example.com/raw.png")
        );
        assert_eq!(
            summary["persistedPreviewAssetKey"],
            json!("https://cdn.example.com/persisted.png")
        );
        assert_eq!(summary["sourceAssetRefRedacted"], json!(true));
        assert_eq!(summary["persisted"], json!(true));
        assert_eq!(summary["byteSize"], json!(1234));
        assert_eq!(summary["mimeType"], json!("image/png"));
    }

    #[test]
    fn image_prompt_text_prefers_summary_prompt_and_falls_back_when_blank() {
        assert_eq!(
            external_static_page_image_prompt_text(
                &json!({"prompt_text": "  Build\nmobile report  "}),
                "fallback"
            ),
            "Build mobile report"
        );
        assert_eq!(
            external_static_page_image_prompt_text(
                &json!({"prompt_text": "   "}),
                " fallback\ntext "
            ),
            "fallback text"
        );
    }

    #[test]
    fn prompt_payload_summary_compacts_modules_and_data_snapshot() {
        let modules = (0..10)
            .map(|index| {
                json!({
                    "id": format!("module-{index}"),
                    "role": "chart",
                    "title": format!("Module {index}"),
                    "content": "x".repeat(320)
                })
            })
            .collect::<Vec<_>>();
        let field_candidates = (0..15)
            .map(|index| json!({"field": format!("f{index}")}))
            .collect::<Vec<_>>();
        let sample_data = (0..15)
            .map(|index| json!({"row": index}))
            .collect::<Vec<_>>();
        let payload = json!({
            "promptText": "Use the latest monthly data",
            "modules": modules,
            "data_snapshot": {
                "source": "database",
                "validation_summary": {
                    "sampleRowCount": 20,
                    "detailRowCount": 120,
                    "unitHints": ["yuan"]
                },
                "field_candidates": field_candidates,
                "module_bindings": [
                    {
                        "moduleId": "module-0",
                        "sampleData": sample_data
                    }
                ]
            }
        });

        let summary = external_static_page_image_prompt_payload_summary(&payload);

        assert_eq!(summary["module_count"], json!(10));
        assert_eq!(summary["modules"].as_array().unwrap().len(), 8);
        assert_eq!(
            summary.pointer("/data_snapshot/sampleRowCount"),
            Some(&json!(20))
        );
        assert_eq!(
            summary
                .pointer("/data_snapshot/field_candidates")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            12
        );
        assert_eq!(
            summary
                .pointer("/data_snapshot/module_bindings/0/sampleData")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            12
        );
    }

    #[test]
    fn value_excerpt_returns_null_for_missing_value_and_truncated_json_for_present_value() {
        assert_eq!(external_static_page_value_excerpt(None, 20), Value::Null);
        assert_eq!(
            external_static_page_value_excerpt(Some(&json!({"b": 2, "a": 1})), 50),
            json!("{\"a\":1,\"b\":2}")
        );
    }
}
