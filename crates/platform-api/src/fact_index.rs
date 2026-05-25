use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use domain_model::{DatasetId, Document, DocumentChunk, DocumentId, TenantId};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use storage::{
    DatasetFactSnapshot, DocumentFactAggregate, NewDocumentFact, NewDocumentFactSource, PgStorage,
};

const FACT_INDEX_FACT_LIMIT: usize = 256;
pub const ENTITY_ROWS_BY_TYPE_SNAPSHOT_KIND: &str = "entity_rows_by_type";
pub const ENTITY_ROWS_BY_TYPE_SNAPSHOT_KEY: &str = "default";
const DATASET_ENTITY_FACT_TYPES: &[&str] = &[
    "organization",
    "person",
    "role_position",
    "skill_technology",
    "project_product_system",
    "location_area",
    "education_certificate",
    "date_period",
    "section",
    "keyword",
];

pub fn build_document_fact_candidates(
    document: &Document,
    chunks: &[DocumentChunk],
    created_at: DateTime<Utc>,
) -> Vec<NewDocumentFact> {
    let parse_version = document_parse_version(document);
    let mut facts = Vec::new();
    let mut seen = BTreeSet::<String>::new();

    for chunk in chunks {
        for section in document_chunk_section_title_hints(chunk) {
            push_document_fact_candidate(
                &mut facts,
                &mut seen,
                document,
                chunk,
                "section",
                &section,
                "section_title_hint",
                parse_version.clone(),
                created_at,
            );
            if facts.len() >= FACT_INDEX_FACT_LIMIT {
                return facts;
            }
        }

        for company_name in crate::extract_company_names_from_text(&chunk.content, 12) {
            push_document_fact_candidate(
                &mut facts,
                &mut seen,
                document,
                chunk,
                "organization",
                &company_name,
                "text_company_name",
                parse_version.clone(),
                created_at,
            );
            if facts.len() >= FACT_INDEX_FACT_LIMIT {
                return facts;
            }
        }

        for term in document_chunk_noun_terms(chunk) {
            if !fact_term_is_useful(&term) {
                continue;
            }
            let fact_type = classify_fact_type(&term, &chunk.content);
            push_document_fact_candidate(
                &mut facts,
                &mut seen,
                document,
                chunk,
                fact_type,
                &term,
                "chunk_understanding",
                parse_version.clone(),
                created_at,
            );
            if facts.len() >= FACT_INDEX_FACT_LIMIT {
                return facts;
            }
        }

        for year in extract_year_fact_terms(&chunk.content, 8) {
            push_document_fact_candidate(
                &mut facts,
                &mut seen,
                document,
                chunk,
                "date_period",
                &year,
                "text_year",
                parse_version.clone(),
                created_at,
            );
            if facts.len() >= FACT_INDEX_FACT_LIMIT {
                return facts;
            }
        }
    }

    facts
}

pub async fn rebuild_dataset_entity_rows_snapshot(
    storage: &PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    limit_per_type: i64,
    created_at: DateTime<Utc>,
) -> Result<DatasetFactSnapshot> {
    let mut aggregates_by_type = BTreeMap::<String, Vec<DocumentFactAggregate>>::new();
    for fact_type in DATASET_ENTITY_FACT_TYPES {
        let aggregates = storage
            .document_facts()
            .aggregate_document_facts_by_dataset(
                tenant_id,
                dataset_id,
                fact_type,
                limit_per_type.max(1),
            )
            .await?;
        if !aggregates.is_empty() {
            aggregates_by_type.insert((*fact_type).to_string(), aggregates);
        }
    }

    let (manifest, source_fact_count, source_document_count) =
        build_dataset_entity_rows_snapshot_manifest(dataset_id, created_at, &aggregates_by_type);
    storage
        .dataset_fact_snapshots()
        .upsert_dataset_fact_snapshot(
            tenant_id,
            dataset_id,
            ENTITY_ROWS_BY_TYPE_SNAPSHOT_KIND,
            ENTITY_ROWS_BY_TYPE_SNAPSHOT_KEY,
            &manifest,
            source_fact_count,
            source_document_count,
            created_at,
        )
        .await
}

pub async fn build_dataset_entity_rows_snapshot_manifest_for_documents(
    storage: &PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    document_ids: &[DocumentId],
    limit_per_type: i64,
    created_at: DateTime<Utc>,
) -> Result<(Value, i64, i64)> {
    if document_ids.is_empty() {
        return Ok(build_dataset_entity_rows_snapshot_manifest(
            dataset_id,
            created_at,
            &BTreeMap::new(),
        ));
    }

    let mut aggregates_by_type = BTreeMap::<String, Vec<DocumentFactAggregate>>::new();
    for fact_type in DATASET_ENTITY_FACT_TYPES {
        let aggregates = storage
            .document_facts()
            .aggregate_document_facts_by_documents(
                tenant_id,
                document_ids,
                fact_type,
                limit_per_type.max(1),
            )
            .await?;
        if !aggregates.is_empty() {
            aggregates_by_type.insert((*fact_type).to_string(), aggregates);
        }
    }

    Ok(build_dataset_entity_rows_snapshot_manifest(
        dataset_id,
        created_at,
        &aggregates_by_type,
    ))
}

pub fn build_dataset_entity_rows_snapshot_manifest(
    dataset_id: DatasetId,
    created_at: DateTime<Utc>,
    aggregates_by_type: &BTreeMap<String, Vec<DocumentFactAggregate>>,
) -> (Value, i64, i64) {
    let mut source_fact_count = 0_i64;
    let mut source_document_ids = BTreeSet::<DocumentId>::new();
    let mut rows_by_type = serde_json::Map::new();
    let mut row_count_by_type = serde_json::Map::new();

    for (fact_type, aggregates) in aggregates_by_type {
        let rows = aggregates
            .iter()
            .map(|aggregate| {
                source_fact_count += aggregate.fact_count;
                source_document_ids.extend(aggregate.source_document_ids.iter().copied());
                document_fact_aggregate_row_json(aggregate)
            })
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            row_count_by_type.insert(fact_type.clone(), Value::from(rows.len()));
            rows_by_type.insert(fact_type.clone(), Value::Array(rows));
        }
    }

    let source_document_count = source_document_ids.len() as i64;
    (
        json!({
            "schema_version": "0.1.0",
            "snapshot_kind": ENTITY_ROWS_BY_TYPE_SNAPSHOT_KIND,
            "snapshot_key": ENTITY_ROWS_BY_TYPE_SNAPSHOT_KEY,
            "dataset_id": dataset_id,
            "generated_at": created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "scanned_document_count": source_document_count,
            "source_fact_count": source_fact_count,
            "row_count_by_type": row_count_by_type,
            "entity_rows_by_type": rows_by_type,
            "model_note": "Use entity_rows_by_type as the authoritative dataset-level aggregate for count/list/rank questions. Do not infer totals from retrieval top-k chunks.",
        }),
        source_fact_count,
        source_document_count,
    )
}

fn document_fact_aggregate_row_json(aggregate: &DocumentFactAggregate) -> Value {
    json!({
        "fact_type": aggregate.fact_type,
        "name": aggregate.name,
        "normalized_name": aggregate.normalized_name,
        "fact_count": aggregate.fact_count,
        "document_count": aggregate.document_count,
        "source_document_ids": aggregate
            .source_document_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "source_locators": aggregate
            .source_locators
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>(),
    })
}

#[allow(clippy::too_many_arguments)]
fn push_document_fact_candidate(
    facts: &mut Vec<NewDocumentFact>,
    seen: &mut BTreeSet<String>,
    document: &Document,
    chunk: &DocumentChunk,
    fact_type: &str,
    name: &str,
    source_kind: &str,
    parse_version: Option<String>,
    created_at: DateTime<Utc>,
) {
    let normalized_name = normalize_fact_name(name);
    if normalized_name.is_empty() {
        return;
    }
    let seen_key = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        fact_type, normalized_name, document.id, chunk.chunk_index
    );
    if !seen.insert(seen_key) {
        return;
    }

    let source_locator = document_fact_source_locator(document.id, chunk);
    let attributes = json!({
        "document_title": &document.title,
        "chunk_index": chunk.chunk_index,
        "extraction_method": "post_ingest_cleanup_v1",
        "term_source": source_kind,
    });

    facts.push(NewDocumentFact {
        dataset_id: document.dataset_id,
        document_id: document.id,
        fact_type: fact_type.to_string(),
        name: name.trim().to_string(),
        normalized_name,
        value_text: Some(name.trim().to_string()),
        value_number: None,
        value_date: None,
        attributes: attributes.clone(),
        confidence: fact_confidence(source_kind),
        source_kind: source_kind.to_string(),
        source_locator: Some(source_locator.clone()),
        source_chunk_id: Some(chunk.id),
        parse_version,
        created_at,
        sources: vec![NewDocumentFactSource {
            source_kind: source_kind.to_string(),
            source_locator: Some(source_locator),
            source_chunk_id: Some(chunk.id),
            attributes,
            created_at,
        }],
    });
}

fn classify_fact_type(term: &str, context: &str) -> &'static str {
    if fact_term_is_valid_organization(term) {
        return "organization";
    }
    if contains_any(
        term,
        &["工程师", "经理", "主管", "负责人", "岗位", "顾问", "总监"],
    ) {
        return "role_position";
    }
    if contains_any(
        term,
        &[
            "系统",
            "平台",
            "产品",
            "项目",
            "梯控",
            "电梯",
            "智能家居",
            "门禁",
        ],
    ) {
        return "project_product_system";
    }
    if contains_any(
        term,
        &["楼", "层", "点位", "区域", "园区", "地点", "房间", "小区"],
    ) {
        return "location_area";
    }
    if contains_any(term, &["学校", "大学", "学院", "学历", "证书", "认证"]) {
        return "education_certificate";
    }
    if looks_like_year_or_period(term)
        || (looks_like_year_or_period(context) && term.contains('年'))
    {
        return "date_period";
    }
    "keyword"
}

fn fact_term_is_valid_organization(term: &str) -> bool {
    let normalized = crate::normalize_document_entity_value(term);
    if crate::is_valid_company_name(&normalized) {
        return true;
    }

    let lower = normalized.to_ascii_lowercase();
    let ascii_char_count = lower.chars().count();
    ascii_char_count >= 4
        && ascii_char_count <= 80
        && (lower.ends_with(" inc") || lower.ends_with(" ltd"))
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn fact_term_is_useful(term: &str) -> bool {
    let trimmed = term.trim();
    let char_count = trimmed.chars().count();
    char_count >= 2
        && char_count <= 80
        && trimmed
            .chars()
            .any(|value| value.is_alphabetic() || is_fact_cjk_char(value))
}

fn is_fact_cjk_char(value: char) -> bool {
    matches!(
        value as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

fn normalize_fact_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_ascii_lowercase()
}

fn fact_confidence(source_kind: &str) -> f64 {
    match source_kind {
        "section_title_hint" => 0.92,
        "text_year" => 0.9,
        _ => 0.82,
    }
}

fn document_parse_version(document: &Document) -> Option<String> {
    document
        .metadata
        .get("ingest")
        .and_then(|value| value.get("parse_method"))
        .and_then(Value::as_str)
        .or_else(|| {
            document
                .metadata
                .get("parse_method")
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned)
}

fn document_fact_source_locator(
    document_id: domain_model::DocumentId,
    chunk: &DocumentChunk,
) -> String {
    format!("document://{document_id}/chunks/{}", chunk.chunk_index)
}

fn document_chunk_section_title_hints(chunk: &DocumentChunk) -> Vec<String> {
    let mut hints = Vec::new();
    for key in [
        "section_title_hints",
        "sectionTitleHints",
        "section_titles",
        "sectionTitles",
        "heading_hints",
        "headingHints",
    ] {
        if let Some(value) = chunk.metadata.get(key) {
            collect_string_list(value, &mut hints);
        }
    }
    if let Some(value) = chunk
        .metadata
        .get("parse_metadata")
        .and_then(|value| value.get("section_title_hints"))
    {
        collect_string_list(value, &mut hints);
    }
    hints.truncate(6);
    hints
}

fn document_chunk_noun_terms(chunk: &DocumentChunk) -> Vec<String> {
    let mut terms = Vec::new();
    if let Some(value) = chunk
        .metadata
        .get("understanding")
        .and_then(|value| value.get("noun_terms").or_else(|| value.get("nounTerms")))
    {
        collect_string_list(value, &mut terms);
    }
    for key in ["noun_terms", "nounTerms", "term_hints", "termHints"] {
        if let Some(value) = chunk.metadata.get(key) {
            collect_string_list(value, &mut terms);
        }
    }
    terms.truncate(64);
    terms
}

fn collect_string_list(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            push_string_hint(output, text);
        }
        Value::Array(items) => {
            for item in items {
                collect_string_list(item, output);
            }
        }
        _ => {}
    }
}

fn push_string_hint(output: &mut Vec<String>, text: impl AsRef<str>) {
    let normalized = text.as_ref().trim().chars().take(80).collect::<String>();
    if !normalized.is_empty() && !output.iter().any(|existing| existing == &normalized) {
        output.push(normalized);
    }
}

fn extract_year_fact_terms(content: &str, limit: usize) -> Vec<String> {
    let mut years = Vec::new();
    let mut digits = String::new();
    for ch in content.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        if digits.len() == 4 {
            if let Ok(year) = digits.parse::<u16>() {
                if (1900..=2099).contains(&year) {
                    push_string_hint(&mut years, format!("{year}年"));
                }
            }
        }
        digits.clear();
        if years.len() >= limit {
            break;
        }
    }
    years
}

fn looks_like_year_or_period(value: &str) -> bool {
    !extract_year_fact_terms(value, 1).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, DocumentLifecycle, TenantId,
    };

    fn document_with_metadata(metadata: Value) -> Document {
        let now = Utc::now();
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: "External Doc".to_string(),
            object_key: "external/src/doc".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: DocumentLifecycle::Extracted,
            secret_binding_ids: Vec::new(),
            metadata: metadata
                .as_object()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            created_at: now,
            updated_at: now,
        }
    }

    fn document_chunk(document: &Document, metadata: Value) -> DocumentChunk {
        let now = Utc::now();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: document.tenant_id,
            dataset_id: document.dataset_id,
            document_id: document.id,
            chunk_index: 0,
            content: "## 工作经历\n北京星河科技有限公司 2020年 任后端工程师，负责智能梯控系统。"
                .to_string(),
            token_count: 32,
            state: DocumentChunkState::Extracted,
            metadata: metadata
                .as_object()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn document_fact_candidates_keep_normalized_names_and_sources() {
        let document = document_with_metadata(json!({
            "ingest": { "parse_method": "pdf-paddleocr" }
        }));
        let chunk = document_chunk(
            &document,
            json!({
                "section_title_hints": ["工作经历"],
                "understanding": {
                    "noun_terms": [
                        "北京星河科技有限公司",
                        "后端工程师",
                        "智能梯控系统"
                    ]
                }
            }),
        );

        let facts = build_document_fact_candidates(&document, &[chunk], Utc::now());

        assert!(facts.iter().any(|fact| fact.fact_type == "section"
            && fact.normalized_name == "工作经历"
            && fact.source_kind == "section_title_hint"));
        assert!(facts.iter().any(|fact| fact.fact_type == "organization"
            && fact.normalized_name == "北京星河科技有限公司"));
        assert!(facts
            .iter()
            .any(|fact| fact.fact_type == "role_position" && fact.normalized_name == "后端工程师"));
        assert!(facts
            .iter()
            .any(|fact| fact.fact_type == "project_product_system"
                && fact.normalized_name == "智能梯控系统"));
        assert!(facts
            .iter()
            .any(|fact| fact.fact_type == "date_period" && fact.normalized_name == "2020年"));
        assert!(facts.iter().all(|fact| fact
            .source_locator
            .as_deref()
            .unwrap_or_default()
            .contains("/chunks/")));
        assert!(facts
            .iter()
            .all(|fact| fact.parse_version.as_deref() == Some("pdf-paddleocr")));
    }

    #[test]
    fn document_fact_candidates_do_not_promote_company_suffix_noise() {
        let document = document_with_metadata(json!({}));
        let chunk = document_chunk(
            &document,
            json!({
                "understanding": {
                    "noun_terms": [
                        "公司",
                        "有限公司",
                        "份有限公司",
                        "互联网公司",
                        "网络有限公司",
                        "北京星河科技有限公司"
                    ]
                }
            }),
        );

        let facts = build_document_fact_candidates(&document, &[chunk], Utc::now());
        let organizations = facts
            .iter()
            .filter(|fact| fact.fact_type == "organization")
            .map(|fact| fact.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(organizations, vec!["北京星河科技有限公司"]);
    }

    #[test]
    fn document_fact_candidates_extract_company_names_from_chunk_text() {
        let document = document_with_metadata(json!({}));
        let mut chunk = document_chunk(&document, json!({ "understanding": { "noun_terms": [] } }));
        chunk.content =
            "任职公司：深圳星拓智能科技有限公司\n2019-2023 广州云岚数码有限公司。".to_string();

        let facts = build_document_fact_candidates(&document, &[chunk], Utc::now());
        let organizations = facts
            .iter()
            .filter(|fact| fact.fact_type == "organization")
            .map(|fact| (fact.name.as_str(), fact.source_kind.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            organizations,
            vec![
                ("深圳星拓智能科技有限公司", "text_company_name"),
                ("广州云岚数码有限公司", "text_company_name")
            ]
        );
    }

    #[test]
    fn dataset_entity_rows_snapshot_manifest_groups_aggregate_rows_by_type() {
        let dataset_id = DatasetId::new();
        let document_a = DocumentId::new();
        let document_b = DocumentId::new();
        let mut aggregates_by_type = BTreeMap::new();
        aggregates_by_type.insert(
            "organization".to_string(),
            vec![DocumentFactAggregate {
                fact_type: "organization".to_string(),
                normalized_name: "北京星河科技有限公司".to_string(),
                name: "北京星河科技有限公司".to_string(),
                fact_count: 3,
                document_count: 2,
                source_document_ids: vec![document_a, document_b],
                source_locators: vec![
                    format!("document://{document_a}/chunks/0"),
                    format!("document://{document_b}/chunks/1"),
                ],
            }],
        );
        aggregates_by_type.insert(
            "role_position".to_string(),
            vec![DocumentFactAggregate {
                fact_type: "role_position".to_string(),
                normalized_name: "后端工程师".to_string(),
                name: "后端工程师".to_string(),
                fact_count: 1,
                document_count: 1,
                source_document_ids: vec![document_a],
                source_locators: vec![format!("document://{document_a}/chunks/2")],
            }],
        );

        let (manifest, source_fact_count, source_document_count) =
            build_dataset_entity_rows_snapshot_manifest(
                dataset_id,
                Utc::now(),
                &aggregates_by_type,
            );

        assert_eq!(source_fact_count, 4);
        assert_eq!(source_document_count, 2);
        assert_eq!(manifest["snapshot_kind"], json!("entity_rows_by_type"));
        assert_eq!(manifest["dataset_id"], json!(dataset_id));
        assert_eq!(manifest["row_count_by_type"]["organization"], json!(1));
        assert_eq!(
            manifest["entity_rows_by_type"]["organization"][0]["name"],
            json!("北京星河科技有限公司")
        );
        assert_eq!(
            manifest["entity_rows_by_type"]["role_position"][0]["document_count"],
            json!(1)
        );
        assert!(manifest["model_note"]
            .as_str()
            .expect("model note")
            .contains("authoritative dataset-level aggregate"));
    }
}
