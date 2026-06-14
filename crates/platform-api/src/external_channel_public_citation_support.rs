use std::collections::BTreeSet;

use contracts::{ExternalBotReplyTypeView, ExternalBotReplyView};
use serde_json::{json, Value};

use crate::{text_normalization::non_empty_trimmed_string, truncate_assistant_supply_text};

pub(crate) fn external_channel_reply_with_public_citations(
    mut reply: ExternalBotReplyView,
    evidence_state: &Value,
) -> ExternalBotReplyView {
    if reply.reply_type != ExternalBotReplyTypeView::Text {
        return reply;
    }
    let citations = external_channel_public_citations_from_evidence_state(evidence_state);
    if citations.is_empty() {
        return reply;
    }
    let citations = Value::Array(citations);
    match reply.card.as_mut() {
        Some(Value::Object(card)) => {
            card.entry("citations".to_string()).or_insert(citations);
        }
        Some(_) => {}
        None => {
            reply.card = Some(json!({
                "type": "answer_citations",
                "citations": citations,
            }));
        }
    }
    reply
}

fn external_channel_public_citations_from_evidence_state(evidence_state: &Value) -> Vec<Value> {
    const CITATION_LIMIT: usize = 8;
    const CITATION_TEXT_LIMIT: usize = 520;

    let Some(items) = evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut citations = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let Some(item_type) = item
            .get("type")
            .and_then(Value::as_str)
            .and_then(non_empty_trimmed_string)
        else {
            continue;
        };
        if matches!(item_type.as_str(), "dataset" | "external_channel") {
            continue;
        }
        let Some(text) = external_channel_public_citation_text(item, CITATION_TEXT_LIMIT) else {
            continue;
        };
        let source =
            external_channel_public_citation_source(item).unwrap_or_else(|| item_type.clone());
        let dedupe_key = format!("{item_type}\n{source}\n{text}");
        if !seen.insert(dedupe_key) {
            continue;
        }
        citations.push(json!({
            "type": item_type,
            "source": source,
            "text": text,
        }));
        if citations.len() >= CITATION_LIMIT {
            break;
        }
    }
    citations
}

fn external_channel_public_citation_text(item: &Value, limit: usize) -> Option<String> {
    for key in ["summary", "content_excerpt", "text", "note", "title"] {
        if let Some(value) = item
            .get(key)
            .and_then(Value::as_str)
            .and_then(non_empty_trimmed_string)
        {
            return Some(truncate_assistant_supply_text(&value, limit));
        }
    }
    None
}

fn external_channel_public_citation_source(item: &Value) -> Option<String> {
    if let Some(source) = external_channel_public_database_citation_source(item) {
        return Some(source);
    }
    for key in [
        "source_locator",
        "sourceLocator",
        "document_external_id",
        "documentExternalId",
        "source_id",
        "sourceId",
        "source",
        "dataset_key",
        "datasetKey",
    ] {
        if let Some(value) = item
            .get(key)
            .and_then(Value::as_str)
            .and_then(non_empty_trimmed_string)
        {
            return Some(truncate_assistant_supply_text(&value, 240));
        }
    }
    None
}

fn external_channel_public_database_citation_source(item: &Value) -> Option<String> {
    let item_type = item.get("type").and_then(Value::as_str)?;
    if !item_type.starts_with("database_") {
        return None;
    }
    let table = item
        .get("table")
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed_string)?;
    let source_id = item
        .get("source_id")
        .or_else(|| item.get("sourceId"))
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed_string)
        .unwrap_or_else(|| "database".to_string());
    let mut source = format!("database://{source_id}/{table}");
    if let Some(metric) = item
        .get("metric")
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed_string)
    {
        source.push('#');
        source.push_str(&metric);
    }
    Some(truncate_assistant_supply_text(&source, 240))
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ExternalBotReplyTypeView;

    fn text_reply(card: Option<Value>) -> ExternalBotReplyView {
        ExternalBotReplyView {
            target_conversation_external_id: "room-1".to_string(),
            reply_type: ExternalBotReplyTypeView::Text,
            text: Some("这是正常业务回答。".to_string()),
            card,
            artifact_links: Vec::new(),
            task_status: Some("answered".to_string()),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        }
    }

    fn evidence_state_with_citations() -> Value {
        json!({
            "status": "supplied",
            "supplied_items": [
                {
                    "type": "retrieval_evidence",
                    "source_locator": "documents/newbai.xlsx#chunk=0",
                    "summary": "固定提成取高资料说明"
                },
                {
                    "type": "retrieval_evidence",
                    "source_locator": "documents/newbai.xlsx#chunk=0",
                    "summary": "固定提成取高资料说明"
                },
                {
                    "type": "database_aggregate",
                    "source_id": "hy-sql-traffic-area",
                    "table": "bi_contract_warning",
                    "metric": "quekou",
                    "summary": "按门店聚合销售缺口，扫描上限 5000 行。"
                },
                {
                    "type": "dataset",
                    "summary": "数据集摘要不应作为 citation"
                },
                {
                    "type": "external_channel",
                    "summary": "第三方通道摘要不应作为 citation"
                },
                {
                    "type": "retrieval_evidence",
                    "source": "documents/empty.md"
                }
            ]
        })
    }

    #[test]
    fn public_citations_filter_dedupe_and_format_sources() {
        let citations =
            external_channel_public_citations_from_evidence_state(&evidence_state_with_citations());

        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0]["type"], json!("retrieval_evidence"));
        assert_eq!(
            citations[0]["source"],
            json!("documents/newbai.xlsx#chunk=0")
        );
        assert_eq!(citations[0]["text"], json!("固定提成取高资料说明"));
        assert_eq!(
            citations[1]["source"],
            json!("database://hy-sql-traffic-area/bi_contract_warning#quekou")
        );
    }

    #[test]
    fn public_citations_keep_existing_card_without_overwriting_citations() {
        let reply = text_reply(Some(json!({
            "type": "existing_card",
            "citations": [{"type": "manual", "source": "operator", "text": "保留"}],
        })));

        let reply =
            external_channel_reply_with_public_citations(reply, &evidence_state_with_citations());

        let card = reply.card.as_ref().expect("card");
        assert_eq!(card["type"], json!("existing_card"));
        assert_eq!(card["citations"][0]["type"], json!("manual"));
    }

    #[test]
    fn public_citations_attach_answer_card_for_plain_text_replies_only() {
        let reply = external_channel_reply_with_public_citations(
            text_reply(None),
            &evidence_state_with_citations(),
        );
        let card = reply.card.as_ref().expect("citation card");
        assert_eq!(card["type"], json!("answer_citations"));
        assert_eq!(card["citations"].as_array().unwrap().len(), 2);

        let mut non_text_reply = text_reply(None);
        non_text_reply.reply_type = ExternalBotReplyTypeView::TaskStatus;
        let non_text_reply = external_channel_reply_with_public_citations(
            non_text_reply,
            &evidence_state_with_citations(),
        );
        assert!(non_text_reply.card.is_none());
    }
}
