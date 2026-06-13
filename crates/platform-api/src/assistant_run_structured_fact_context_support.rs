use serde_json::{json, Map, Value};

use crate::{
    assistant_run_fact_snapshot_can_replace_dataset_entity_scan,
    ASSISTANT_RUN_DATASET_ENTITY_SCAN_ROW_LIMIT, ASSISTANT_RUN_RESUME_PROFILE_ROW_LIMIT,
    ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ROW_LIMIT,
};

pub(crate) fn assistant_run_compact_dataset_entity_scan_payloads_for_prompt(
    evidence_state: &Value,
    prompt: &str,
) -> Vec<Value> {
    assistant_run_compact_dataset_entity_scan_payloads_with_fact_snapshots(
        evidence_state,
        assistant_run_fact_snapshot_can_replace_dataset_entity_scan(prompt),
    )
}

pub(crate) fn assistant_run_compact_dataset_entity_scan_payloads_with_fact_snapshots(
    evidence_state: &Value,
    include_fact_snapshots: bool,
) -> Vec<Value> {
    evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item.get("type").and_then(Value::as_str) {
                    Some("dataset_entity_scan") => {
                        assistant_run_compact_dataset_entity_scan_payload(item)
                    }
                    Some("dataset_fact_snapshot") if include_fact_snapshots => {
                        assistant_run_compact_dataset_fact_snapshot_as_scan_payload(item)
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn assistant_run_compact_dataset_fact_snapshot_payloads(
    evidence_state: &Value,
) -> Vec<Value> {
    evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("type").and_then(Value::as_str) == Some("dataset_fact_snapshot")
                })
                .filter_map(assistant_run_compact_dataset_fact_snapshot_payload)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn assistant_run_compact_dataset_fact_snapshot_payload(item: &Value) -> Option<Value> {
    let entity_rows_by_type =
        assistant_run_compact_fact_rows_by_type(item.get("entity_rows_by_type")?);
    if entity_rows_by_type
        .as_object()
        .map_or(true, |object| object.is_empty())
    {
        return None;
    }
    Some(json!({
        "type": "dataset_fact_snapshot",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "dataset_key": item.get("dataset_key").cloned().unwrap_or(Value::Null),
        "snapshot_kind": item.get("snapshot_kind").cloned().unwrap_or(Value::Null),
        "snapshot_key": item.get("snapshot_key").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").cloned().unwrap_or(Value::Null),
        "scanned_document_count": item.get("scanned_document_count").cloned().unwrap_or(Value::Null),
        "source_document_count": item.get("source_document_count").cloned().unwrap_or(Value::Null),
        "source_fact_count": item.get("source_fact_count").cloned().unwrap_or(Value::Null),
        "row_count_by_type": item.get("row_count_by_type").cloned().unwrap_or(Value::Null),
        "entity_rows_by_type": entity_rows_by_type,
        "model_note": item.get("model_note").cloned().unwrap_or_else(|| json!("Use this as the authoritative dataset-level aggregate for count/list/rank questions. Cite scanned_document_count, source_document_count, source_fact_count, and row_count_by_type when giving totals. Use retrieval evidence only for examples, quotes, and validation; never infer totals from retrieval chunks.")),
    }))
}

fn assistant_run_compact_dataset_fact_snapshot_as_scan_payload(item: &Value) -> Option<Value> {
    let payload = assistant_run_compact_dataset_fact_snapshot_payload(item)?;
    let rows_by_type = payload
        .get("entity_rows_by_type")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let company_rows = assistant_run_fact_rows_for_type(&rows_by_type, "organization");
    let skill_rows = assistant_run_fact_rows_for_type(&rows_by_type, "skill_technology");
    let project_rows = assistant_run_fact_rows_for_type(&rows_by_type, "project_product_system");
    let position_rows = assistant_run_fact_rows_for_type(&rows_by_type, "role_position");
    let location_rows = assistant_run_fact_rows_for_type(&rows_by_type, "location_area");
    let person_rows = assistant_run_fact_rows_for_type(&rows_by_type, "person");
    let certificate_rows = assistant_run_fact_rows_for_type(&rows_by_type, "education_certificate");
    let keyword_rows = assistant_run_fact_rows_for_type(&rows_by_type, "keyword");
    let year_rows = assistant_run_fact_rows_for_type(&rows_by_type, "date_period");
    let section_rows = assistant_run_fact_rows_for_type(&rows_by_type, "section");
    if company_rows.is_empty()
        && skill_rows.is_empty()
        && project_rows.is_empty()
        && position_rows.is_empty()
        && location_rows.is_empty()
        && person_rows.is_empty()
        && certificate_rows.is_empty()
        && keyword_rows.is_empty()
        && year_rows.is_empty()
        && section_rows.is_empty()
    {
        return None;
    }
    Some(json!({
        "type": "dataset_entity_scan",
        "source": "dataset_fact_snapshot",
        "dataset_id": payload.get("dataset_id").cloned().unwrap_or(Value::Null),
        "summary": payload.get("summary").cloned().unwrap_or(Value::Null),
        "scanned_document_count": payload.get("scanned_document_count").cloned().unwrap_or(Value::Null),
        "company_count": company_rows.len(),
        "company_rows": company_rows,
        "skill_rows": skill_rows,
        "project_rows": project_rows,
        "position_rows": position_rows,
        "location_rows": location_rows,
        "person_rows": person_rows,
        "school_rows": [],
        "degree_rows": [],
        "certificate_rows": certificate_rows,
        "keyword_rows": keyword_rows,
        "year_rows": year_rows,
        "section_rows": section_rows,
        "paragraph_rows": [],
        "table_rows": [],
        "entity_rows_by_type": rows_by_type,
        "resume_profile_rows": [],
        "answer_guidance": "Authoritative rows converted from dataset_fact_snapshot. Cite scanned_document_count and row counts for totals. Use retrieval evidence only for examples, quotes, and validation.",
        "model_note": "This compact scan is fact-snapshot backed. Use scanned_document_count as the document total; cite row_count_by_type/entity row counts for list totals; do not infer totals from retrieval chunks or summed row counts.",
    }))
}

fn assistant_run_compact_fact_rows_by_type(rows_by_type: &Value) -> Value {
    let Some(object) = rows_by_type.as_object() else {
        return json!({});
    };
    let mut compact = Map::new();
    for (fact_type, rows) in object {
        let Some(rows) = rows.as_array() else {
            continue;
        };
        let compact_rows = rows
            .iter()
            .filter_map(assistant_run_compact_fact_row)
            .take(ASSISTANT_RUN_DATASET_ENTITY_SCAN_ROW_LIMIT)
            .collect::<Vec<_>>();
        if !compact_rows.is_empty() {
            compact.insert(fact_type.clone(), Value::Array(compact_rows));
        }
    }
    Value::Object(compact)
}

fn assistant_run_compact_fact_row(row: &Value) -> Option<Value> {
    let name = row.get("name").and_then(Value::as_str)?.trim();
    if name.is_empty() {
        return None;
    }
    Some(json!({
        "name": name,
        "document_count": row.get("document_count").cloned().unwrap_or(Value::Null),
        "fact_count": row.get("fact_count").cloned().unwrap_or(Value::Null),
        "source_document_ids": row.get("source_document_ids").cloned().unwrap_or(Value::Null),
        "source_locators": row.get("source_locators").cloned().unwrap_or(Value::Null),
    }))
}

fn assistant_run_fact_rows_for_type(rows_by_type: &Value, fact_type: &str) -> Vec<Value> {
    rows_by_type
        .get(fact_type)
        .and_then(Value::as_array)
        .map(|rows| rows.to_vec())
        .unwrap_or_default()
}

fn assistant_run_compact_entity_scan_rows(item: &Value, key: &str) -> Vec<Value> {
    item.get(key)
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let name = row.get("name").and_then(Value::as_str)?.trim();
                    if name.is_empty() {
                        return None;
                    }
                    Some(json!({
                        "name": name,
                        "document_count": row.get("document_count").cloned().unwrap_or(Value::Null),
                    }))
                })
                .take(ASSISTANT_RUN_DATASET_ENTITY_SCAN_ROW_LIMIT)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn assistant_run_compact_dataset_entity_scan_payload(item: &Value) -> Option<Value> {
    let company_rows = assistant_run_compact_entity_scan_rows(item, "company_rows");
    let skill_rows = assistant_run_compact_entity_scan_rows(item, "skill_rows");
    let project_rows = assistant_run_compact_entity_scan_rows(item, "project_rows");
    let position_rows = assistant_run_compact_entity_scan_rows(item, "position_rows");
    let location_rows = assistant_run_compact_entity_scan_rows(item, "location_rows");
    let person_rows = assistant_run_compact_entity_scan_rows(item, "person_rows");
    let school_rows = assistant_run_compact_entity_scan_rows(item, "school_rows");
    let degree_rows = assistant_run_compact_entity_scan_rows(item, "degree_rows");
    let certificate_rows = assistant_run_compact_entity_scan_rows(item, "certificate_rows");
    let keyword_rows = assistant_run_compact_entity_scan_rows(item, "keyword_rows");
    let year_rows = assistant_run_compact_entity_scan_rows(item, "year_rows");
    let section_rows = assistant_run_compact_entity_scan_rows(item, "section_rows");
    let paragraph_rows = assistant_run_compact_entity_scan_rows(item, "paragraph_rows");
    let table_rows = assistant_run_compact_entity_scan_rows(item, "table_rows");
    let resume_project_delivery_rows = item
        .get("resume_project_delivery_rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .take(ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ROW_LIMIT)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let resume_profile_rows = item
        .get("resume_profile_rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .take(ASSISTANT_RUN_RESUME_PROFILE_ROW_LIMIT)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if company_rows.is_empty()
        && skill_rows.is_empty()
        && project_rows.is_empty()
        && position_rows.is_empty()
        && location_rows.is_empty()
        && person_rows.is_empty()
        && school_rows.is_empty()
        && degree_rows.is_empty()
        && certificate_rows.is_empty()
        && keyword_rows.is_empty()
        && year_rows.is_empty()
        && section_rows.is_empty()
        && paragraph_rows.is_empty()
        && table_rows.is_empty()
        && resume_project_delivery_rows.is_empty()
        && resume_profile_rows.is_empty()
    {
        return None;
    }
    let entity_rows_by_type = json!({
        "organization": company_rows.clone(),
        "skill": skill_rows.clone(),
        "project": project_rows.clone(),
        "position": position_rows.clone(),
        "location": location_rows.clone(),
        "person": person_rows.clone(),
        "school": school_rows.clone(),
        "degree": degree_rows.clone(),
        "certificate": certificate_rows.clone(),
        "keyword": keyword_rows.clone(),
        "year": year_rows.clone(),
        "section": section_rows.clone(),
        "paragraph": paragraph_rows.clone(),
        "table": table_rows.clone(),
    });

    Some(json!({
        "type": "dataset_entity_scan",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").cloned().unwrap_or(Value::Null),
        "scanned_document_count": item.get("scanned_document_count").cloned().unwrap_or(Value::Null),
        "company_count": item.get("company_count").cloned().unwrap_or(Value::Null),
        "company_rows": company_rows,
        "skill_rows": skill_rows,
        "project_rows": project_rows,
        "position_rows": position_rows,
        "location_rows": location_rows,
        "person_rows": person_rows,
        "school_rows": school_rows,
        "degree_rows": degree_rows,
        "certificate_rows": certificate_rows,
        "keyword_rows": keyword_rows,
        "year_rows": year_rows,
        "section_rows": section_rows,
        "paragraph_rows": paragraph_rows,
        "table_rows": table_rows,
        "entity_rows_by_type": entity_rows_by_type,
        "resume_profile_rows": resume_profile_rows,
        "resume_project_delivery_rows": resume_project_delivery_rows,
        "answer_guidance": item.get("answer_guidance").cloned().unwrap_or(Value::Null),
        "model_note": "Answer entity/document dimension questions from *_rows, keyword_rows, year_rows, section_rows, paragraph_rows, table_rows, resume_profile_rows, and resume_project_delivery_rows only. Cite scanned_document_count and the number of returned rows when giving totals. For multi-resume project delivery/detail questions, prefer resume_project_delivery_rows. Do not use omitted candidate_terms, entities, document_hits, retrieval chunks, or summed row counts for totals.",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_scan_payload_keeps_resume_rows_and_omits_raw_candidates() {
        let item = json!({
            "type": "dataset_entity_scan",
            "source": "visible_document_scan",
            "dataset_id": "dataset-1",
            "company_rows": [
                {"name": " Acme Corp ", "document_count": 2},
                {"name": "", "document_count": 1}
            ],
            "resume_profile_rows": [
                {"document_id": "doc-1", "candidate_name": "Alice"}
            ],
            "resume_project_delivery_rows": [
                {"document_id": "doc-1", "project_name": "Knowledge Base", "delivery": "API"}
            ],
            "candidate_terms": [{"name": "raw"}],
            "document_hits": [{"document_id": "doc-1"}]
        });

        let compact = assistant_run_compact_dataset_entity_scan_payload(&item)
            .expect("entity scan should compact");

        assert_eq!(compact["company_rows"][0]["name"], json!("Acme Corp"));
        assert_eq!(
            compact["resume_profile_rows"][0]["candidate_name"],
            json!("Alice")
        );
        assert_eq!(
            compact["resume_project_delivery_rows"][0]["project_name"],
            json!("Knowledge Base")
        );
        assert!(compact.get("candidate_terms").is_none());
        assert!(compact.get("document_hits").is_none());
    }

    #[test]
    fn fact_snapshot_payload_compacts_rows_and_scan_alias() {
        let item = json!({
            "type": "dataset_fact_snapshot",
            "source": "dataset_fact_snapshots",
            "dataset_id": "dataset-1",
            "snapshot_kind": "entity_rows_by_type",
            "summary": "snapshot",
            "scanned_document_count": 3,
            "row_count_by_type": {"organization": 1, "skill_technology": 1},
            "entity_rows_by_type": {
                "organization": [
                    {
                        "name": "Acme Corp",
                        "document_count": 2,
                        "fact_count": 3,
                        "source_document_ids": ["doc-1", "doc-2"],
                        "source_locators": ["document://doc-1/chunks/0"]
                    }
                ],
                "skill_technology": [
                    {"name": "Rust", "document_count": 1, "fact_count": 1}
                ],
                "ignored_empty": [
                    {"name": "   ", "document_count": 1}
                ]
            }
        });

        let compact = assistant_run_compact_dataset_fact_snapshot_payload(&item)
            .expect("fact snapshot should compact");
        assert_eq!(compact["type"], json!("dataset_fact_snapshot"));
        assert_eq!(
            compact["entity_rows_by_type"]["organization"][0]["source_document_ids"][0],
            json!("doc-1")
        );
        assert!(compact["entity_rows_by_type"]
            .get("ignored_empty")
            .is_none());

        let evidence = json!({"supplied_items": [item]});
        let scans =
            assistant_run_compact_dataset_entity_scan_payloads_with_fact_snapshots(&evidence, true);
        assert_eq!(scans[0]["type"], json!("dataset_entity_scan"));
        assert_eq!(scans[0]["source"], json!("dataset_fact_snapshot"));
        assert_eq!(scans[0]["company_rows"][0]["name"], json!("Acme Corp"));
        assert_eq!(scans[0]["skill_rows"][0]["name"], json!("Rust"));
    }

    #[test]
    fn fact_snapshot_include_flag_controls_scan_conversion() {
        let evidence = json!({
            "supplied_items": [
                {
                    "type": "dataset_entity_scan",
                    "company_rows": [{"name": "Visible Scan", "document_count": 1}]
                },
                {
                    "type": "dataset_fact_snapshot",
                    "entity_rows_by_type": {
                        "organization": [{"name": "Snapshot Org", "document_count": 2}]
                    }
                }
            ]
        });

        let without_snapshots =
            assistant_run_compact_dataset_entity_scan_payloads_with_fact_snapshots(
                &evidence, false,
            );
        assert_eq!(without_snapshots.len(), 1);
        assert_eq!(
            without_snapshots[0]["company_rows"][0]["name"],
            json!("Visible Scan")
        );

        let with_snapshots =
            assistant_run_compact_dataset_entity_scan_payloads_with_fact_snapshots(&evidence, true);
        assert_eq!(with_snapshots.len(), 2);
        assert_eq!(
            with_snapshots[1]["company_rows"][0]["name"],
            json!("Snapshot Org")
        );
    }
}
