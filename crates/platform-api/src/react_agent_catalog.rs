use serde_json::{json, Map, Value};

const WEAK_ALLOWED_KEYS: &[&str] = &[
    "id",
    "dataset_id",
    "datasetId",
    "document_id",
    "documentId",
    "key",
    "title",
    "label",
    "name",
    "type",
    "kind",
    "category",
    "visibility",
    "lifecycle",
    "status",
    "parse_status",
    "parseStatus",
    "parse_status_summary",
    "parseStatusSummary",
    "vector_status",
    "vectorStatus",
    "profile_status",
    "profileStatus",
    "updated_at",
    "updatedAt",
    "created_at",
    "createdAt",
    "document_count",
    "documentCount",
    "documents_count",
    "documentsCount",
    "estimated_word_count",
    "estimatedWordCount",
    "word_count",
    "wordCount",
    "chunk_count",
    "chunkCount",
    "visibleDatasetCount",
    "visibleDocumentCount",
    "latestUpload",
    "latestActivity",
    "reportPlanCount",
    "publishedReportCount",
    "staticPageDraftCount",
    "styleDirection",
    "style_direction",
    "moduleCount",
    "module_count",
    "previewStatus",
    "preview_status",
    "finalRenderStatus",
    "final_render_status",
    "previewStale",
    "preview_stale",
    "intent",
    "retrievalPolicy",
    "retrieval_policy",
    "preferDetail",
    "prefer_detail",
    "noFakeData",
    "no_fake_data",
];

const WEAK_ARRAY_KEYS: &[&str] = &[
    "datasets",
    "datasetBriefs",
    "documents",
    "items",
    "material_hints",
    "materialHints",
    "selected",
    "scope_candidates",
    "scopeCandidates",
];

const WEAK_OBJECT_KEYS: &[&str] = &["supply_policy", "supplyPolicy"];

const SENSITIVE_OR_CONTENT_KEYS: &[&str] = &[
    "api_key",
    "apiKey",
    "secret",
    "token",
    "authorization",
    "provider_key",
    "providerKey",
    "local_access_key",
    "localAccessKey",
    "content",
    "body",
    "text",
    "chunk",
    "chunk_text",
    "chunkText",
    "ocr",
    "ocr_text",
    "ocrText",
    "table",
    "table_text",
    "tableText",
    "summary",
    "long_summary",
    "longSummary",
    "excerpt",
    "profile",
    "profile_values",
    "profileValues",
    "raw_profile",
    "rawProfile",
];

pub(crate) fn build_assistant_run_react_planning_catalog(
    startup_briefing: &Value,
    scope_candidates: &[Value],
    selected_scope: &Value,
    evidence_state: &Value,
) -> Value {
    json!({
        "kind": "assistant_run_react_planning_catalog",
        "answerableEvidence": false,
        "purpose": "tool_selection_only",
        "startup": summarize_weak_value(startup_briefing),
        "scopeCandidates": scope_candidates
            .iter()
            .map(summarize_weak_value)
            .collect::<Vec<_>>(),
        "selectedScope": summarize_weak_value(selected_scope),
        "evidenceState": summarize_evidence_state(evidence_state),
        "systemCapabilities": {
            "static_page": {
                "available": true,
                "actions": ["create_static_page_draft", "update_static_page_module", "submit_static_page_image_preview", "render_static_page"]
            },
            "report": {
                "available": true,
                "actions": ["list_report_options", "create_report_draft"]
            },
            "retrieval": {
                "available": true,
                "actions": ["retrieve_evidence", "read_document_detail"]
            },
            "conversation_memory": {
                "available": true,
                "actions": ["recall_conversation_memory"]
            },
            "openclaw_extension": {
                "available": false,
                "actions": ["openclaw_memory_recall", "openclaw_readonly_execution"],
                "note": "optional config-gated bridge"
            },
            "codex_host": {
                "available": false,
                "actions": ["codex_host_task"],
                "note": "disabled-by-default execution-kernel bridge; V3 validates task scope, memory, and allowlist before any external execution"
            }
        }
    })
}

fn summarize_evidence_state(evidence_state: &Value) -> Value {
    let supplied_items = evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let mut summary = Map::new();
                    copy_allowed_field(item, &mut summary, "type");
                    copy_allowed_field(item, &mut summary, "dataset_id");
                    copy_allowed_field(item, &mut summary, "datasetId");
                    copy_allowed_field(item, &mut summary, "document_id");
                    copy_allowed_field(item, &mut summary, "documentId");
                    copy_allowed_field(item, &mut summary, "retrieval_evidence_id");
                    copy_allowed_field(item, &mut summary, "retrievalEvidenceId");
                    copy_allowed_field(item, &mut summary, "status");
                    if let Some(media_summary) = summarize_media_context(item) {
                        summary.insert("media".to_string(), media_summary);
                    }
                    Value::Object(summary)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let detail_targets = evidence_state
        .get("detail_targets")
        .and_then(Value::as_array)
        .map(|targets| {
            targets
                .iter()
                .map(summarize_detail_target)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let recommended_actions = evidence_state
        .get("recommended_actions")
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter(|action| is_safe_scalar(action))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "status": evidence_state
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "suppliedCount": supplied_items.len(),
        "recommendedActions": recommended_actions,
        "detailTargets": detail_targets,
        "items": supplied_items,
    })
}

fn summarize_detail_target(target: &Value) -> Value {
    let mut summary = Map::new();
    for key in [
        "type",
        "dataset_id",
        "datasetId",
        "document_id",
        "documentId",
        "retrieval_evidence_id",
        "retrievalEvidenceId",
        "chunk_index",
        "chunkIndex",
        "reason",
        "has_media_context",
        "hasMediaContext",
        "has_timestamped_evidence",
        "hasTimestampedEvidence",
    ] {
        copy_allowed_field(target, &mut summary, key);
    }
    Value::Object(summary)
}

fn summarize_media_context(item: &Value) -> Option<Value> {
    let context = item
        .get("media_context")
        .filter(|value| value.is_object())?;
    let media_kind = context
        .get("media_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let parse_status = context
        .get("parse_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let transcript_window_count = array_len(context, "transcript_windows");
    let scene_window_count = array_len(context, "scene_windows");
    let keyframe_ocr_snippet_count = array_len(context, "keyframe_ocr_snippets");
    let provider_evidence_count = array_len(context, "provider_evidence");

    if media_kind == "unknown"
        && parse_status == "unknown"
        && transcript_window_count == 0
        && scene_window_count == 0
        && keyframe_ocr_snippet_count == 0
        && provider_evidence_count == 0
    {
        return None;
    }

    Some(json!({
        "media_kind": media_kind,
        "parse_status": parse_status,
        "has_timestamped_evidence": context
            .get("has_timestamped_evidence")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "transcriptWindowCount": transcript_window_count,
        "sceneWindowCount": scene_window_count,
        "keyframeOcrSnippetCount": keyframe_ocr_snippet_count,
        "providerEvidenceCount": provider_evidence_count,
    }))
}

fn array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default()
}

fn summarize_weak_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut summary = Map::new();
            for (key, field_value) in object {
                if SENSITIVE_OR_CONTENT_KEYS.contains(&key.as_str()) {
                    continue;
                }
                if WEAK_ALLOWED_KEYS.contains(&key.as_str()) && is_safe_scalar(field_value) {
                    summary.insert(key.clone(), field_value.clone());
                    continue;
                }
                if WEAK_ARRAY_KEYS.contains(&key.as_str()) {
                    if let Value::Array(items) = field_value {
                        summary.insert(
                            key.clone(),
                            Value::Array(items.iter().map(summarize_weak_value).collect()),
                        );
                    }
                }
                if WEAK_OBJECT_KEYS.contains(&key.as_str()) && field_value.is_object() {
                    summary.insert(key.clone(), summarize_weak_value(field_value));
                }
            }
            Value::Object(summary)
        }
        Value::Array(items) => Value::Array(items.iter().map(summarize_weak_value).collect()),
        value if is_safe_scalar(value) => value.clone(),
        _ => Value::Null,
    }
}

fn copy_allowed_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).filter(|value| is_safe_scalar(value)) {
        target.insert(key.to_string(), value.clone());
    }
}

fn is_safe_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_catalog_includes_weak_dataset_and_document_metadata() {
        let startup = json!({
            "visibleDatasetCount": 2,
            "latestUpload": "orders.csv",
            "providerKey": "secret-provider-key",
            "datasets": [{
                "id": "ds-public",
                "key": "orders",
                "title": "公开订单",
                "visibility": "public",
                "lifecycle": "active",
                "documentCount": 3,
                "estimatedWordCount": 1200,
                "parseStatusSummary": "completed:3",
                "category": "订单",
                "body": "不能进入规划目录的正文"
            }]
        });
        let candidates = vec![json!({
            "type": "dataset",
            "id": "ds-private",
            "key": "support",
            "title": "私密客服",
            "visibility": "private",
            "lifecycle": "active",
            "documentCount": 2,
            "estimatedWordCount": 900,
            "parseStatusSummary": "completed:2",
            "documents": [{
                "id": "doc-1",
                "title": "客服 FAQ",
                "parseStatus": "completed",
                "vectorStatus": "completed",
                "profileStatus": "ready",
                "updatedAt": "2026-04-29T10:00:00Z",
                "material_hints": ["audio_video", "transcript_possible"],
                "content": "不要把文档正文放进目录"
            }]
        })];
        let catalog = build_assistant_run_react_planning_catalog(
            &startup,
            &candidates,
            &json!({
                "mode": "selected",
                "intent": "static_page",
                "selected": [{"type": "dataset", "id": "ds-private"}],
                "supply_policy": {
                    "retrievalPolicy": "detail_first",
                    "preferDetail": true,
                    "noFakeData": true
                }
            }),
            &json!({"status": "not_requested", "supplied_items": []}),
        );

        assert_eq!(catalog["startup"]["visibleDatasetCount"], json!(2));
        assert_eq!(catalog["startup"]["datasets"][0]["id"], json!("ds-public"));
        assert_eq!(catalog["startup"]["datasets"][0]["key"], json!("orders"));
        assert_eq!(
            catalog["startup"]["datasets"][0]["lifecycle"],
            json!("active")
        );
        assert_eq!(
            catalog["startup"]["datasets"][0]["estimatedWordCount"],
            json!(1200)
        );
        assert_eq!(
            catalog["startup"]["datasets"][0]["parseStatusSummary"],
            json!("completed:3")
        );
        assert_eq!(catalog["scopeCandidates"][0]["id"], json!("ds-private"));
        assert_eq!(catalog["scopeCandidates"][0]["documentCount"], json!(2));
        assert_eq!(
            catalog["scopeCandidates"][0]["parseStatusSummary"],
            json!("completed:2")
        );
        assert_eq!(
            catalog["scopeCandidates"][0]["documents"][0]["parseStatus"],
            json!("completed")
        );
        assert_eq!(
            catalog["scopeCandidates"][0]["documents"][0]["material_hints"],
            json!(["audio_video", "transcript_possible"])
        );
        assert_eq!(
            catalog["systemCapabilities"]["retrieval"]["available"],
            json!(true)
        );
        assert_eq!(catalog["selectedScope"]["intent"], json!("static_page"));
        assert_eq!(
            catalog["selectedScope"]["supply_policy"]["retrievalPolicy"],
            json!("detail_first")
        );
        assert_eq!(
            catalog["systemCapabilities"]["codex_host"]["available"],
            json!(false)
        );
    }

    #[test]
    fn planning_catalog_keeps_static_page_artifact_state_without_module_content() {
        let catalog = build_assistant_run_react_planning_catalog(
            &json!({}),
            &[json!({
                "type": "static_page_draft",
                "id": "draft-1",
                "label": "当前静态页：经营简报",
                "status": "planning",
                "styleDirection": "data-command",
                "moduleCount": 5,
                "previewStatus": "stale",
                "finalRenderStatus": "rendered",
                "previewStale": true,
                "modules": [{
                    "id": "hero",
                    "title": "模块标题可以进弱目录",
                    "content": "模块正文不能进弱目录"
                }]
            })],
            &json!({"mode": "ordinary_chat"}),
            &json!({"status": "not_requested"}),
        );

        let candidate = &catalog["scopeCandidates"][0];
        assert_eq!(candidate["id"], json!("draft-1"));
        assert_eq!(candidate["styleDirection"], json!("data-command"));
        assert_eq!(candidate["moduleCount"], json!(5));
        assert_eq!(candidate["previewStatus"], json!("stale"));
        assert_eq!(candidate["finalRenderStatus"], json!("rendered"));
        assert_eq!(candidate["previewStale"], json!(true));

        let serialized = serde_json::to_string(&catalog).expect("catalog should serialize");
        assert!(!serialized.contains("模块正文不能进弱目录"));
    }

    #[test]
    fn planning_catalog_excludes_answerable_content_and_secrets() {
        let catalog = build_assistant_run_react_planning_catalog(
            &json!({
                "productTruth": "智能数据工作台",
                "apiKey": "secret",
                "longSummary": "这是一段不应该用于回答的长摘要",
                "rawProfile": {"revenue": 100},
            }),
            &[json!({
                "id": "doc-raw",
                "title": "原始文档",
                "body": "正文",
                "chunkText": "切片正文",
                "ocrText": "OCR 正文",
                "tableText": "表格正文",
                "summary": "长摘要",
                "profileValues": {"x": "y"},
                "localAccessKey": "local-key"
            })],
            &json!({"mode": "selected", "selected": [{"type": "document", "id": "doc-raw"}]}),
            &json!({
                "status": "supplied",
                "supplied_items": [{
                    "type": "retrieval_evidence",
                    "dataset_id": "ds-1",
                    "document_id": "doc-raw",
                    "summary": "证据摘要不进规划目录",
                    "excerpt": "证据原文不进规划目录"
                }]
            }),
        );

        let serialized = serde_json::to_string(&catalog).expect("catalog should serialize");
        assert!(serialized.contains("doc-raw"));
        assert!(serialized.contains("suppliedCount"));
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("local-key"));
        assert!(!serialized.contains("正文"));
        assert!(!serialized.contains("切片正文"));
        assert!(!serialized.contains("OCR 正文"));
        assert!(!serialized.contains("表格正文"));
        assert!(!serialized.contains("长摘要"));
        assert!(!serialized.contains("证据原文"));
        assert!(!serialized.contains("profileValues"));
    }

    #[test]
    fn planning_catalog_exposes_media_availability_without_media_content() {
        let catalog = build_assistant_run_react_planning_catalog(
            &json!({}),
            &[],
            &json!({"mode": "selected", "selected": [{"type": "dataset", "id": "ds-1"}]}),
            &json!({
                "status": "supplied",
                "recommended_actions": ["retrieve_evidence", "read_document_detail"],
                "detail_targets": [{
                    "type": "document_detail_target",
                    "dataset_id": "ds-1",
                    "document_id": "doc-media",
                    "retrieval_evidence_id": "ev-1",
                    "chunk_index": 2,
                    "source_locator": "00:12-00:28",
                    "reason": "timestamped_media_detail_available",
                    "has_media_context": true,
                    "has_timestamped_evidence": true
                }],
                "supplied_items": [{
                    "type": "retrieval_evidence",
                    "dataset_id": "ds-1",
                    "document_id": "doc-media",
                    "retrieval_evidence_id": "ev-1",
                    "media_context": {
                        "media_kind": "audio",
                        "parse_status": "completed",
                        "has_timestamped_evidence": true,
                        "transcript_windows": [{
                            "start_seconds": 12.0,
                            "end_seconds": 28.5,
                            "text": "客户真实转写不应进入规划目录"
                        }],
                        "scene_windows": [],
                        "keyframe_ocr_snippets": [{
                            "timestamp_seconds": 18.0,
                            "text": "屏幕文字也不应进入规划目录"
                        }],
                        "provider_evidence": [{
                            "provider": "minimax",
                            "detail": "供应商细节也不应进入规划目录"
                        }]
                    }
                }]
            }),
        );

        assert_eq!(
            catalog["evidenceState"]["recommendedActions"],
            json!(["retrieve_evidence", "read_document_detail"])
        );
        assert_eq!(
            catalog["evidenceState"]["detailTargets"][0]["document_id"],
            json!("doc-media")
        );
        assert_eq!(
            catalog["evidenceState"]["detailTargets"][0]["reason"],
            json!("timestamped_media_detail_available")
        );
        assert_eq!(
            catalog["evidenceState"]["detailTargets"][0]["has_timestamped_evidence"],
            json!(true)
        );
        let media = &catalog["evidenceState"]["items"][0]["media"];
        assert_eq!(media["media_kind"], json!("audio"));
        assert_eq!(media["parse_status"], json!("completed"));
        assert_eq!(media["has_timestamped_evidence"], json!(true));
        assert_eq!(media["transcriptWindowCount"], json!(1));
        assert_eq!(media["sceneWindowCount"], json!(0));
        assert_eq!(media["keyframeOcrSnippetCount"], json!(1));
        assert_eq!(media["providerEvidenceCount"], json!(1));

        let serialized = serde_json::to_string(&catalog).expect("catalog should serialize");
        assert!(!serialized.contains("客户真实转写"));
        assert!(!serialized.contains("屏幕文字"));
        assert!(!serialized.contains("供应商细节"));
        assert!(!serialized.contains("00:12-00:28"));
    }
}
