use serde_json::{json, Map, Value};

const WEAK_ALLOWED_KEYS: &[&str] = &[
    "id",
    "dataset_id",
    "datasetId",
    "document_id",
    "documentId",
    "title",
    "label",
    "name",
    "type",
    "kind",
    "visibility",
    "status",
    "parse_status",
    "parseStatus",
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
    "word_count",
    "wordCount",
    "chunk_count",
    "chunkCount",
    "visibleDatasetCount",
    "visibleDocumentCount",
    "latestUpload",
];

const WEAK_ARRAY_KEYS: &[&str] = &[
    "datasets",
    "documents",
    "items",
    "selected",
    "scope_candidates",
    "scopeCandidates",
];

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
                    Value::Object(summary)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "status": evidence_state
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "suppliedCount": supplied_items.len(),
        "items": supplied_items,
    })
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
                "title": "公开订单",
                "visibility": "public",
                "documentCount": 3,
                "body": "不能进入规划目录的正文"
            }]
        });
        let candidates = vec![json!({
            "type": "dataset",
            "id": "ds-private",
            "title": "私密客服",
            "visibility": "private",
            "documents": [{
                "id": "doc-1",
                "title": "客服 FAQ",
                "parseStatus": "completed",
                "vectorStatus": "completed",
                "profileStatus": "ready",
                "updatedAt": "2026-04-29T10:00:00Z",
                "content": "不要把文档正文放进目录"
            }]
        })];
        let catalog = build_assistant_run_react_planning_catalog(
            &startup,
            &candidates,
            &json!({"mode": "selected", "selected": [{"type": "dataset", "id": "ds-private"}]}),
            &json!({"status": "not_requested", "supplied_items": []}),
        );

        assert_eq!(catalog["startup"]["visibleDatasetCount"], json!(2));
        assert_eq!(catalog["startup"]["datasets"][0]["id"], json!("ds-public"));
        assert_eq!(catalog["scopeCandidates"][0]["id"], json!("ds-private"));
        assert_eq!(
            catalog["scopeCandidates"][0]["documents"][0]["parseStatus"],
            json!("completed")
        );
        assert_eq!(
            catalog["systemCapabilities"]["retrieval"]["available"],
            json!(true)
        );
        assert_eq!(
            catalog["systemCapabilities"]["codex_host"]["available"],
            json!(false)
        );
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
}
