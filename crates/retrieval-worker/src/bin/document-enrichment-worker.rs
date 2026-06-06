use anyhow::{anyhow, Result};
use chrono::{Duration as ChronoDuration, Utc};
use domain_model::{Document, DocumentChunk, TenantId};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use storage::{DocumentEnrichmentRun, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::sleep;

const DEFAULT_POLL_INTERVAL_MS: u64 = 3_000;
const DEFAULT_ERROR_BACKOFF_SECONDS: i64 = 60;
const DEFAULT_SNAPSHOT_LIMIT_PER_TYPE: i64 = 50;

#[derive(Clone, Debug)]
struct WorkerConfig {
    enrichment_kind: Option<String>,
    poll_interval: Duration,
    error_backoff: ChronoDuration,
    once: bool,
    max_runs: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SectionCandidate {
    title: String,
    chunk_index: i32,
    source: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("document_enrichment_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.into());
    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "DOCUMENT_ENRICHMENT_WORKER_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    let config = WorkerConfig::from_env();

    tracing::info!(
        tenant_id = %tenant.id,
        tenant_key = %tenant.key,
        enrichment_kind = config.enrichment_kind.as_deref().unwrap_or("*"),
        poll_interval_ms = config.poll_interval.as_millis(),
        error_backoff_seconds = config.error_backoff.num_seconds(),
        once = config.once,
        max_runs = config.max_runs.unwrap_or(0),
        %database_url,
        "document enrichment worker polling started"
    );

    run_worker_loop(&storage, tenant.id, &config).await
}

async fn run_worker_loop(
    storage: &PgStorage,
    tenant_id: TenantId,
    config: &WorkerConfig,
) -> Result<()> {
    let mut processed_count = 0usize;

    loop {
        let claimed = storage
            .document_enrichment_runs()
            .claim_next_available(tenant_id, config.enrichment_kind.as_deref(), Utc::now())
            .await?;

        if let Some(run) = claimed {
            processed_count += 1;
            if let Err(error) = process_claimed_run(storage, tenant_id, run.clone()).await {
                let error_message = error.to_string();
                tracing::error!(
                    run_id = %run.id,
                    document_id = %run.document_id,
                    enrichment_kind = %run.enrichment_kind,
                    error = %error_message,
                    "document enrichment run failed"
                );
                storage
                    .document_enrichment_runs()
                    .requeue_after_error(
                        tenant_id,
                        run.id,
                        &error_message,
                        Utc::now() + config.error_backoff,
                        Utc::now(),
                    )
                    .await?;
            }

            if config.once
                || config
                    .max_runs
                    .is_some_and(|limit| processed_count >= limit)
            {
                break;
            }
            continue;
        }

        if config.once
            || config
                .max_runs
                .is_some_and(|limit| processed_count >= limit)
        {
            break;
        }
        sleep(config.poll_interval).await;
    }

    Ok(())
}

async fn process_claimed_run(
    storage: &PgStorage,
    tenant_id: TenantId,
    run: DocumentEnrichmentRun,
) -> Result<Value> {
    let document = storage
        .documents()
        .get_by_id(tenant_id, run.document_id)
        .await?
        .ok_or_else(|| anyhow!("document {} not found", run.document_id))?;
    let chunks = storage
        .document_chunks()
        .list_by_document_or_canonical(tenant_id, run.document_id)
        .await?;
    let generated_at = Utc::now();
    let summary = match run.enrichment_kind.as_str() {
        "structure_outline_v1" => build_structure_outline_summary(&document, &chunks, generated_at),
        "qa_seed_v1" => build_qa_seed_summary(&document, &chunks, generated_at),
        "entity_relation_v1" => build_entity_relation_summary(&document, &chunks, generated_at),
        "fact_index_v2" => {
            run_fact_index_enrichment(storage, tenant_id, &document, &chunks, generated_at).await?
        }
        other => return Err(anyhow!("unsupported document enrichment kind {other}")),
    };

    storage
        .document_enrichment_runs()
        .mark_succeeded(tenant_id, run.id, &summary, generated_at)
        .await?;
    tracing::info!(
        run_id = %run.id,
        document_id = %run.document_id,
        enrichment_kind = %run.enrichment_kind,
        "document enrichment run completed"
    );

    Ok(summary)
}

async fn run_fact_index_enrichment(
    storage: &PgStorage,
    tenant_id: TenantId,
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Result<Value> {
    if chunks.iter().any(|chunk| chunk.document_id != document.id) {
        return Ok(json!({
            "schema_version": "0.1.0",
            "enrichment_kind": "fact_index_v2",
            "status": "skipped",
            "skipped_reason": "canonical_alias_read_through",
            "document_id": document.id,
            "canonical_document_id": chunks.first().map(|chunk| chunk.document_id),
            "chunk_count": chunks.len(),
            "generated_at": generated_at.to_rfc3339(),
        }));
    }

    let facts =
        platform_api::fact_index::build_document_fact_candidates(document, chunks, generated_at);
    let fact_count_by_type = fact_count_by_type(&facts);
    let persisted_facts = storage
        .document_facts()
        .replace_document_facts(tenant_id, document.id, &facts)
        .await?;
    let snapshot = platform_api::fact_index::rebuild_dataset_entity_rows_snapshot(
        storage,
        tenant_id,
        document.dataset_id,
        DEFAULT_SNAPSHOT_LIMIT_PER_TYPE,
        generated_at,
    )
    .await?;

    Ok(json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "fact_index_v2",
        "status": "indexed",
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "chunk_count": chunks.len(),
        "fact_count": persisted_facts.len(),
        "fact_count_by_type": fact_count_by_type,
        "snapshot_kind": snapshot.snapshot_kind,
        "snapshot_key": snapshot.snapshot_key,
        "snapshot_source_fact_count": snapshot.source_fact_count,
        "snapshot_source_document_count": snapshot.source_document_count,
        "generated_at": generated_at.to_rfc3339(),
    }))
}

fn build_structure_outline_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let sections = extract_structure_sections(chunks, 80);
    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "structure_outline_v1",
        "status": if sections.is_empty() { "empty" } else { "extracted" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "chunk_count": chunks.len(),
        "section_count": sections.len(),
        "sections": sections.iter().map(section_candidate_json).collect::<Vec<_>>(),
        "generated_at": generated_at.to_rfc3339(),
    })
}

fn build_qa_seed_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let sections = extract_structure_sections(chunks, 12);
    let mut seeds = vec![
        json!({
            "question": format!("{} 的核心内容是什么？", document.title),
            "source": "document_title",
        }),
        json!({
            "question": format!("{} 中有哪些关键流程、规则或检查项？", document.title),
            "source": "document_title",
        }),
    ];
    for section in &sections {
        seeds.push(json!({
            "question": format!("关于{}，这份资料有哪些要求？", section.title),
            "source": section.source,
            "source_section": section.title,
            "chunk_index": section.chunk_index,
        }));
    }
    seeds.truncate(16);

    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "qa_seed_v1",
        "status": if seeds.is_empty() { "empty" } else { "generated" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "chunk_count": chunks.len(),
        "seed_count": seeds.len(),
        "seeds": seeds,
        "generated_at": generated_at.to_rfc3339(),
    })
}

fn build_entity_relation_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let facts =
        platform_api::fact_index::build_document_fact_candidates(document, chunks, generated_at);
    let fact_count_by_type = fact_count_by_type(&facts);
    let mut names_by_type = BTreeMap::<String, BTreeSet<String>>::new();
    let mut names_by_chunk = BTreeMap::<String, BTreeSet<String>>::new();

    for fact in &facts {
        names_by_type
            .entry(fact.fact_type.clone())
            .or_default()
            .insert(fact.name.clone());
        if let Some(locator) = fact.source_locator.as_ref() {
            names_by_chunk
                .entry(locator.clone())
                .or_default()
                .insert(fact.name.clone());
        }
    }

    let entity_samples_by_type = names_by_type
        .into_iter()
        .map(|(fact_type, names)| (fact_type, names.into_iter().take(12).collect::<Vec<_>>()))
        .collect::<BTreeMap<_, _>>();
    let relation_hints = names_by_chunk
        .into_iter()
        .filter_map(|(source_locator, names)| {
            let names = names.into_iter().take(8).collect::<Vec<_>>();
            (names.len() >= 2).then(|| {
                json!({
                    "source_locator": source_locator,
                    "co_occurring_entities": names,
                })
            })
        })
        .take(16)
        .collect::<Vec<_>>();

    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "entity_relation_v1",
        "status": if facts.is_empty() { "empty" } else { "extracted" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "chunk_count": chunks.len(),
        "fact_count": facts.len(),
        "fact_count_by_type": fact_count_by_type,
        "entity_samples_by_type": entity_samples_by_type,
        "relation_hint_count": relation_hints.len(),
        "relation_hints": relation_hints,
        "generated_at": generated_at.to_rfc3339(),
    })
}

fn extract_structure_sections(chunks: &[DocumentChunk], limit: usize) -> Vec<SectionCandidate> {
    let mut sections = Vec::new();
    let mut seen = BTreeSet::new();
    for chunk in chunks {
        for title in metadata_section_title_hints(chunk) {
            push_section_candidate(
                &mut sections,
                &mut seen,
                title,
                chunk.chunk_index,
                "section_title_hint",
                limit,
            );
        }
        for title in content_heading_candidates(&chunk.content) {
            push_section_candidate(
                &mut sections,
                &mut seen,
                title,
                chunk.chunk_index,
                "content_heading",
                limit,
            );
        }
        if sections.len() >= limit {
            break;
        }
    }
    sections
}

fn metadata_section_title_hints(chunk: &DocumentChunk) -> Vec<String> {
    let mut titles = Vec::new();
    for key in [
        "section_title_hints",
        "sectionTitleHints",
        "section_titles",
        "sectionTitles",
        "heading_hints",
        "headingHints",
    ] {
        if let Some(value) = chunk.metadata.get(key) {
            collect_value_strings(value, &mut titles);
        }
    }
    titles
}

fn collect_value_strings(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(raw) => {
            if let Some(title) = clean_title(raw) {
                output.push(title);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_value_strings(value, output);
            }
        }
        Value::Object(object) => {
            for key in ["title", "text", "name", "heading", "section"] {
                if let Some(value) = object.get(key) {
                    collect_value_strings(value, output);
                }
            }
        }
        _ => {}
    }
}

fn content_heading_candidates(content: &str) -> Vec<String> {
    content
        .lines()
        .take(80)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            if trimmed.starts_with('#') {
                return clean_title(trimmed.trim_start_matches('#'));
            }
            if looks_like_short_heading(trimmed) {
                return clean_title(trimmed);
            }
            None
        })
        .collect()
}

fn looks_like_short_heading(value: &str) -> bool {
    let char_count = value.chars().count();
    if !(3..=48).contains(&char_count) {
        return false;
    }
    if value.ends_with(['。', '.', '，', ',', ';', '；']) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    lower.starts_with("chapter ")
        || lower.starts_with("section ")
        || lower.starts_with("part ")
        || value.starts_with("第")
        || value.starts_with("一、")
        || value.starts_with("二、")
        || value.starts_with("三、")
        || value.starts_with("四、")
        || value.starts_with("五、")
        || value.starts_with("1.")
        || value.starts_with("1、")
        || value.starts_with("（一）")
        || value.starts_with("(一)")
}

fn clean_title(raw: &str) -> Option<String> {
    let cleaned = raw
        .trim()
        .trim_matches(|value: char| {
            value.is_ascii_whitespace()
                || value.is_ascii_punctuation()
                || matches!(value, '：' | '；' | '，' | '。' | '、')
        })
        .trim()
        .to_string();
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned.chars().take(80).collect())
}

fn push_section_candidate(
    sections: &mut Vec<SectionCandidate>,
    seen: &mut BTreeSet<String>,
    title: String,
    chunk_index: i32,
    source: &str,
    limit: usize,
) {
    if sections.len() >= limit {
        return;
    }
    let normalized = title.to_ascii_lowercase();
    if !seen.insert(normalized) {
        return;
    }
    sections.push(SectionCandidate {
        title,
        chunk_index,
        source: source.to_string(),
    });
}

fn section_candidate_json(section: &SectionCandidate) -> Value {
    json!({
        "title": section.title,
        "chunk_index": section.chunk_index,
        "source": section.source,
    })
}

fn fact_count_by_type(facts: &[storage::NewDocumentFact]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for fact in facts {
        *counts.entry(fact.fact_type.clone()).or_insert(0) += 1;
    }
    counts
}

impl WorkerConfig {
    fn from_env() -> Self {
        Self {
            enrichment_kind: optional_env("DOCUMENT_ENRICHMENT_WORKER_KIND"),
            poll_interval: Duration::from_millis(env_u64(
                "DOCUMENT_ENRICHMENT_WORKER_POLL_INTERVAL_MS",
                DEFAULT_POLL_INTERVAL_MS,
            )),
            error_backoff: ChronoDuration::seconds(env_i64(
                "DOCUMENT_ENRICHMENT_WORKER_ERROR_BACKOFF_SECONDS",
                DEFAULT_ERROR_BACKOFF_SECONDS,
            )),
            once: env_flag("DOCUMENT_ENRICHMENT_WORKER_ONCE", false),
            max_runs: optional_env("DOCUMENT_ENRICHMENT_WORKER_MAX_RUNS")
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0),
        }
    }
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_flag(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, DocumentLifecycle, TenantId,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn structure_outline_summary_extracts_section_hints() {
        let chunks = vec![
            test_chunk(
                0,
                BTreeMap::from_iter([(
                    "section_title_hints".to_string(),
                    json!(["Operating Summary", "Risk Checks"]),
                )]),
                "plain content",
            ),
            test_chunk(1, BTreeMap::new(), "# Action Plan\nFollow up stores"),
        ];

        let sections = extract_structure_sections(&chunks, 8);

        assert_eq!(
            sections,
            vec![
                SectionCandidate {
                    title: "Operating Summary".to_string(),
                    chunk_index: 0,
                    source: "section_title_hint".to_string(),
                },
                SectionCandidate {
                    title: "Risk Checks".to_string(),
                    chunk_index: 0,
                    source: "section_title_hint".to_string(),
                },
                SectionCandidate {
                    title: "Action Plan".to_string(),
                    chunk_index: 1,
                    source: "content_heading".to_string(),
                },
            ]
        );
    }

    #[test]
    fn qa_seed_summary_uses_sections() {
        let document = test_document("Operations Manual");
        let chunks = vec![test_chunk(
            0,
            BTreeMap::from_iter([(
                "sectionTitleHints".to_string(),
                json!([{ "title": "Medication Check" }]),
            )]),
            "section body",
        )];

        let summary = build_qa_seed_summary(&document, &chunks, Utc::now());
        let seeds = summary
            .get("seeds")
            .and_then(Value::as_array)
            .expect("seeds should be present");

        assert!(seeds.iter().any(|seed| {
            seed.get("question")
                .and_then(Value::as_str)
                .is_some_and(|question| question.contains("Medication Check"))
        }));
    }

    #[test]
    fn entity_relation_summary_counts_fact_types() {
        let document = test_document("Project Resume");
        let chunks = vec![test_chunk(
            0,
            BTreeMap::from_iter([(
                "section_title_hints".to_string(),
                json!(["Project Experience"]),
            )]),
            "Acme Technology Ltd built an access control platform in 2024.",
        )];

        let summary = build_entity_relation_summary(&document, &chunks, Utc::now());
        let counts = summary
            .get("fact_count_by_type")
            .and_then(Value::as_object)
            .expect("fact_count_by_type should be present");

        assert!(counts.get("section").and_then(Value::as_u64).unwrap_or(0) >= 1);
        assert!(
            summary
                .get("fact_count")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                >= 1
        );
    }

    fn test_document(title: &str) -> Document {
        let now = Utc::now();
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: title.to_string(),
            object_key: "uploads/test.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            lifecycle: DocumentLifecycle::Indexed,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    fn test_chunk(
        chunk_index: i32,
        metadata: BTreeMap<String, Value>,
        content: &str,
    ) -> DocumentChunk {
        let now = Utc::now();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunk_index,
            content: content.to_string(),
            token_count: content.split_whitespace().count() as i32,
            state: DocumentChunkState::Indexed,
            metadata,
            created_at: now,
            updated_at: now,
        }
    }
}
