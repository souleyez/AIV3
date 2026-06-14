use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn append_deduped_assistant_supply_items(
    target: &mut Vec<Value>,
    candidates: Vec<Value>,
) -> usize {
    let mut seen = target
        .iter()
        .filter_map(assistant_run_supply_item_identity)
        .collect::<BTreeSet<_>>();
    let before = target.len();
    for item in candidates {
        let Some(identity) = assistant_run_supply_item_identity(&item) else {
            target.push(item);
            continue;
        };
        if seen.insert(identity) {
            target.push(item);
        }
    }
    target.len().saturating_sub(before)
}

pub(crate) fn assistant_run_supply_item_identity(item: &Value) -> Option<String> {
    item.get("document_chunk_id")
        .or_else(|| item.get("documentChunkId"))
        .and_then(Value::as_str)
        .map(|value| format!("chunk:{value}"))
        .or_else(|| {
            item.get("retrieval_evidence_id")
                .or_else(|| item.get("retrievalEvidenceId"))
                .and_then(Value::as_str)
                .map(|value| format!("evidence:{value}"))
        })
        .or_else(|| {
            item.get("source_locator")
                .or_else(|| item.get("sourceLocator"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| format!("locator:{value}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn supply_item_identity_prefers_chunk_then_evidence_then_locator() {
        assert_eq!(
            assistant_run_supply_item_identity(&json!({
                "document_chunk_id": "chunk-1",
                "retrieval_evidence_id": "evidence-1",
                "source_locator": "doc#page=1"
            })),
            Some("chunk:chunk-1".to_string())
        );
        assert_eq!(
            assistant_run_supply_item_identity(&json!({
                "documentChunkId": "chunk-2"
            })),
            Some("chunk:chunk-2".to_string())
        );
        assert_eq!(
            assistant_run_supply_item_identity(&json!({
                "retrievalEvidenceId": "evidence-2",
                "sourceLocator": "doc#page=2"
            })),
            Some("evidence:evidence-2".to_string())
        );
        assert_eq!(
            assistant_run_supply_item_identity(&json!({
                "source_locator": "  doc#page=3  "
            })),
            Some("locator:doc#page=3".to_string())
        );
        assert_eq!(
            assistant_run_supply_item_identity(&json!({"source_locator": "   "})),
            None
        );
    }

    #[test]
    fn append_deduped_supply_items_skips_seen_identities_but_keeps_unidentified_items() {
        let mut target = vec![
            json!({"document_chunk_id": "chunk-1", "content_excerpt": "old"}),
            json!({"source_locator": "doc#page=1", "content_excerpt": "locator"}),
        ];
        let appended = append_deduped_assistant_supply_items(
            &mut target,
            vec![
                json!({"documentChunkId": "chunk-1", "content_excerpt": "duplicate"}),
                json!({"retrieval_evidence_id": "evidence-1", "content_excerpt": "new"}),
                json!({"sourceLocator": " doc#page=1 ", "content_excerpt": "duplicate locator"}),
                json!({"content_excerpt": "no identity"}),
            ],
        );

        assert_eq!(appended, 2);
        assert_eq!(target.len(), 4);
        assert_eq!(target[2]["retrieval_evidence_id"], json!("evidence-1"));
        assert_eq!(target[3]["content_excerpt"], json!("no identity"));
    }
}
