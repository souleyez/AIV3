use serde_json::{json, Map, Value};
use std::collections::HashSet;

pub(crate) fn assistant_run_detail_targets_for_supply(
    prefer_detail: bool,
    supplied_items: &[Value],
    limit: usize,
) -> Vec<Value> {
    if !prefer_detail {
        return Vec::new();
    }

    let mut seen_documents = HashSet::new();
    supplied_items
        .iter()
        .filter(|item| {
            item.get("type")
                .and_then(Value::as_str)
                .is_some_and(|item_type| item_type == "retrieval_evidence")
        })
        .filter_map(|item| {
            let document_id = item.get("document_id").or_else(|| item.get("documentId"))?;
            let document_key = document_id
                .as_str()
                .map(ToString::to_string)
                .unwrap_or_else(|| document_id.to_string());
            if document_key.trim().is_empty() || !seen_documents.insert(document_key) {
                return None;
            }

            let media_context = item
                .get("media_context")
                .or_else(|| item.get("mediaContext"));
            let has_media_context = media_context.is_some_and(Value::is_object);
            let has_timestamped_evidence = media_context
                .and_then(|context| context.get("has_timestamped_evidence"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let reason = if has_timestamped_evidence {
                "timestamped_media_detail_available"
            } else if has_media_context {
                "media_detail_available"
            } else {
                "detail_first_scope"
            };

            let mut target = Map::new();
            target.insert("type".to_string(), json!("document_detail_target"));
            for key in [
                "dataset_id",
                "datasetId",
                "document_id",
                "documentId",
                "retrieval_evidence_id",
                "retrievalEvidenceId",
                "chunk_index",
                "chunkIndex",
                "source_locator",
                "sourceLocator",
            ] {
                if let Some(value) = item.get(key).filter(|value| !value.is_null()) {
                    target.insert(key.to_string(), value.clone());
                }
            }
            target.insert("reason".to_string(), json!(reason));
            target.insert("has_media_context".to_string(), json!(has_media_context));
            target.insert(
                "has_timestamped_evidence".to_string(),
                json!(has_timestamped_evidence),
            );
            Some(Value::Object(target))
        })
        .take(limit)
        .collect()
}

pub(crate) fn assistant_run_detail_target_count(evidence_state: &Value) -> usize {
    evidence_state
        .get("detail_targets")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detail_targets_require_detail_preference() {
        let targets = assistant_run_detail_targets_for_supply(
            false,
            &[json!({
                "type": "retrieval_evidence",
                "document_id": "doc-1",
            })],
            3,
        );

        assert!(targets.is_empty());
    }

    #[test]
    fn detail_targets_dedupe_documents_preserve_alias_fields_and_limit() {
        let targets = assistant_run_detail_targets_for_supply(
            true,
            &[
                json!({
                    "type": "retrieval_evidence",
                    "datasetId": "ds-1",
                    "documentId": "doc-media",
                    "retrievalEvidenceId": "ev-1",
                    "chunkIndex": 4,
                    "sourceLocator": "00:12-00:28",
                    "mediaContext": {
                        "media_kind": "video",
                        "has_timestamped_evidence": true
                    }
                }),
                json!({
                    "type": "retrieval_evidence",
                    "documentId": "doc-media",
                    "retrievalEvidenceId": "ev-duplicate"
                }),
                json!({
                    "type": "retrieval_evidence",
                    "dataset_id": "ds-1",
                    "document_id": "doc-text",
                    "retrieval_evidence_id": "ev-2"
                }),
                json!({
                    "type": "retrieval_evidence",
                    "document_id": "doc-third",
                    "retrieval_evidence_id": "ev-3"
                }),
            ],
            2,
        );

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0]["documentId"], json!("doc-media"));
        assert_eq!(
            targets[0]["reason"],
            json!("timestamped_media_detail_available")
        );
        assert_eq!(targets[0]["has_media_context"], json!(true));
        assert_eq!(targets[0]["has_timestamped_evidence"], json!(true));
        assert_eq!(targets[1]["document_id"], json!("doc-text"));
        assert_eq!(targets[1]["reason"], json!("detail_first_scope"));
    }

    #[test]
    fn detail_target_count_reads_array_only() {
        assert_eq!(
            assistant_run_detail_target_count(&json!({"detail_targets": [{}, {}]})),
            2
        );
        assert_eq!(
            assistant_run_detail_target_count(&json!({"detail_targets": "2"})),
            0
        );
    }
}
