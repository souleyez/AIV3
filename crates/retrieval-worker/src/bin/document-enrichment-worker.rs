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
        "structure_outline_v1" | "section_outline_v1" | "section_outline" => {
            build_structure_outline_summary(&document, &chunks, generated_at)
        }
        "qa_seed_v1" => build_qa_seed_summary(&document, &chunks, generated_at),
        "entity_relation_v1" => build_entity_relation_summary(&document, &chunks, generated_at),
        "entity_terms_v1" | "entity_terms" => {
            build_entity_terms_summary(&document, &chunks, generated_at)
        }
        "table_structure_v1" | "table_structure" => {
            build_table_structure_summary(&document, &chunks, generated_at)
        }
        "procedure_steps_v1" | "procedure_steps" => {
            build_procedure_steps_summary(&document, &chunks, generated_at)
        }
        "resume_profile_v1" | "resume_profile" => {
            build_resume_profile_summary(&document, &chunks, generated_at)
        }
        "spreadsheet_metrics_v1" | "spreadsheet_metrics" => {
            build_spreadsheet_metrics_summary(&document, &chunks, generated_at)
        }
        "fact_index_v2" => {
            run_fact_index_enrichment(storage, tenant_id, &document, &chunks, generated_at).await?
        }
        "semantic_profile_v1" => {
            run_semantic_profile_enrichment(storage, tenant_id, &document, generated_at).await?
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

async fn run_semantic_profile_enrichment(
    storage: &PgStorage,
    tenant_id: TenantId,
    document: &Document,
    generated_at: chrono::DateTime<Utc>,
) -> Result<Value> {
    if !env_flag("DATASET_SEMANTIC_UNDERSTANDING_ENABLED", false) {
        return Ok(json!({
            "schema_version": "0.1.0",
            "enrichment_kind": "semantic_profile_v1",
            "status": "skipped",
            "skipped_reason": "feature_disabled",
            "dataset_count": 0,
            "generated_at": generated_at.to_rfc3339(),
        }));
    }
    let mut dataset_ids = storage
        .dataset_document_memberships()
        .list_dataset_ids_by_document(tenant_id, document.id)
        .await?;
    dataset_ids.push(document.dataset_id);
    dataset_ids.sort_by_key(|id| id.0);
    dataset_ids.dedup();

    let mut ready_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed_count = 0usize;
    let mut fingerprints = Vec::new();
    for dataset_id in &dataset_ids {
        let outcome = platform_api::dataset_semantic_snapshot::rebuild_dataset_semantic_snapshot_from_storage(
            storage,
            tenant_id,
            *dataset_id,
            generated_at,
        )
        .await?;
        match outcome.status.as_str() {
            "ready" => ready_count += 1,
            "skipped" => skipped_count += 1,
            _ => failed_count += 1,
        }
        fingerprints.push(outcome.source_fingerprint);
    }
    Ok(json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "semantic_profile_v1",
        "status": if failed_count > 0 { "partial" } else { "completed" },
        "dataset_count": dataset_ids.len(),
        "ready_count": ready_count,
        "skipped_count": skipped_count,
        "failed_count": failed_count,
        "fingerprints": fingerprints,
        "generated_at": generated_at.to_rfc3339(),
    }))
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

fn build_entity_terms_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let facts =
        platform_api::fact_index::build_document_fact_candidates(document, chunks, generated_at);
    let mut terms_by_type = BTreeMap::<String, BTreeSet<String>>::new();
    for fact in &facts {
        terms_by_type
            .entry(fact.fact_type.clone())
            .or_default()
            .insert(fact.name.clone());
    }
    let term_rows = terms_by_type
        .into_iter()
        .map(|(fact_type, terms)| {
            json!({
                "fact_type": fact_type,
                "term_count": terms.len(),
                "terms": terms.into_iter().take(32).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "entity_terms_v1",
        "status": if term_rows.is_empty() { "empty" } else { "extracted" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "chunk_count": chunks.len(),
        "fact_count": facts.len(),
        "term_type_count": term_rows.len(),
        "term_rows": term_rows,
        "generated_at": generated_at.to_rfc3339(),
    })
}

fn build_table_structure_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let tables = extract_table_candidates(chunks, 24);
    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "table_structure_v1",
        "status": if tables.is_empty() { "empty" } else { "extracted" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "chunk_count": chunks.len(),
        "table_count": tables.len(),
        "tables": tables,
        "generated_at": generated_at.to_rfc3339(),
    })
}

fn build_procedure_steps_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let steps = extract_procedure_steps(chunks, 80);
    let threshold_terms = steps
        .iter()
        .filter_map(|step| step.get("threshold").cloned())
        .collect::<Vec<_>>();
    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "procedure_steps_v1",
        "status": if steps.is_empty() { "empty" } else { "extracted" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "chunk_count": chunks.len(),
        "step_count": steps.len(),
        "threshold_count": threshold_terms.len(),
        "threshold_terms": threshold_terms,
        "steps": steps,
        "generated_at": generated_at.to_rfc3339(),
    })
}

fn build_resume_profile_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let text = joined_chunk_text(chunks, 80_000);
    let facts =
        platform_api::fact_index::build_document_fact_candidates(document, chunks, generated_at);
    let organizations = facts
        .iter()
        .filter(|fact| fact.fact_type == "organization")
        .map(|fact| fact.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(24)
        .collect::<Vec<_>>();
    let projects = facts
        .iter()
        .filter(|fact| fact.fact_type == "project_product_system")
        .map(|fact| fact.name.clone())
        .chain(extract_resume_project_lines(&text, 16))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(24)
        .collect::<Vec<_>>();
    let skills = extract_known_skill_terms(&text, 40);
    let years = extract_year_terms_from_text(&text, 16);
    let candidate_name = resume_candidate_name(document);
    let cities = extract_known_terms(
        &text,
        &[
            "北京", "上海", "广州", "深圳", "杭州", "南京", "成都", "武汉", "西安", "苏州",
        ],
        12,
    );
    let certificates = extract_lines_containing(&text, &["证书", "认证", "资格"], 12);

    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "resume_profile_v1",
        "status": if organizations.is_empty() && projects.is_empty() && skills.is_empty() { "sparse" } else { "extracted" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "candidate_name": candidate_name,
        "chunk_count": chunks.len(),
        "organization_count": organizations.len(),
        "organizations": organizations,
        "project_count": projects.len(),
        "projects": projects,
        "skill_count": skills.len(),
        "skills": skills,
        "timeline_years": years,
        "cities": cities,
        "certificates": certificates,
        "generated_at": generated_at.to_rfc3339(),
    })
}

fn build_spreadsheet_metrics_summary(
    document: &Document,
    chunks: &[DocumentChunk],
    generated_at: chrono::DateTime<Utc>,
) -> Value {
    let rows = extract_spreadsheet_metric_rows(chunks, 120);
    let absence_rows = rows
        .iter()
        .filter(|row| row.get("counts_as_absence").and_then(Value::as_bool) == Some(true))
        .cloned()
        .collect::<Vec<_>>();
    let work_hour_rows = rows
        .iter()
        .filter(|row| row.get("work_hours").and_then(Value::as_f64).is_some())
        .cloned()
        .collect::<Vec<_>>();
    let longest_work_hour = work_hour_rows
        .iter()
        .max_by(|left, right| {
            left.get("work_hours")
                .and_then(Value::as_f64)
                .partial_cmp(&right.get("work_hours").and_then(Value::as_f64))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let shortest_work_hour = work_hour_rows
        .iter()
        .min_by(|left, right| {
            left.get("work_hours")
                .and_then(Value::as_f64)
                .partial_cmp(&right.get("work_hours").and_then(Value::as_f64))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();

    json!({
        "schema_version": "0.1.0",
        "enrichment_kind": "spreadsheet_metrics_v1",
        "status": if rows.is_empty() { "empty" } else { "extracted" },
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "document_title": document.title,
        "chunk_count": chunks.len(),
        "row_count": rows.len(),
        "absence_count": absence_rows.len(),
        "work_hour_row_count": work_hour_rows.len(),
        "longest_work_hour": longest_work_hour,
        "shortest_work_hour": shortest_work_hour,
        "absence_rows": absence_rows.into_iter().take(24).collect::<Vec<_>>(),
        "sample_rows": rows.into_iter().take(40).collect::<Vec<_>>(),
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

fn extract_table_candidates(chunks: &[DocumentChunk], limit: usize) -> Vec<Value> {
    let mut tables = Vec::new();
    for chunk in chunks {
        for table in metadata_table_candidates(chunk, limit.saturating_sub(tables.len())) {
            tables.push(table);
            if tables.len() >= limit {
                return tables;
            }
        }
        for table in markdown_table_candidates(chunk, limit.saturating_sub(tables.len())) {
            tables.push(table);
            if tables.len() >= limit {
                return tables;
            }
        }
    }
    tables
}

fn metadata_table_candidates(chunk: &DocumentChunk, limit: usize) -> Vec<Value> {
    if limit == 0 {
        return Vec::new();
    }
    let mut tables = Vec::new();
    for key in ["tables", "table_rows", "tableRows", "structured_tables"] {
        let Some(value) = chunk.metadata.get(key) else {
            continue;
        };
        match value {
            Value::Array(rows) => {
                if rows.is_empty() {
                    continue;
                }
                tables.push(json!({
                    "chunk_index": chunk.chunk_index,
                    "source": format!("metadata.{key}"),
                    "row_count": rows.len(),
                    "column_count": estimate_json_table_column_count(rows),
                    "headers": estimate_json_table_headers(rows),
                    "sample_rows": rows.iter().take(5).cloned().collect::<Vec<_>>(),
                }));
            }
            Value::Object(object) => {
                let rows = object
                    .get("rows")
                    .or_else(|| object.get("data"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let headers = object
                    .get("headers")
                    .or_else(|| object.get("columns"))
                    .cloned()
                    .unwrap_or_else(|| Value::Array(estimate_json_table_headers(&rows)));
                tables.push(json!({
                    "chunk_index": chunk.chunk_index,
                    "source": format!("metadata.{key}"),
                    "row_count": rows.len(),
                    "column_count": estimate_json_table_column_count(&rows),
                    "headers": headers,
                    "sample_rows": rows.into_iter().take(5).collect::<Vec<_>>(),
                }));
            }
            _ => {}
        }
        if tables.len() >= limit {
            break;
        }
    }
    tables
}

fn estimate_json_table_column_count(rows: &[Value]) -> usize {
    rows.iter()
        .map(|row| match row {
            Value::Array(values) => values.len(),
            Value::Object(object) => object.len(),
            _ => 1,
        })
        .max()
        .unwrap_or(0)
}

fn estimate_json_table_headers(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .find_map(|row| match row {
            Value::Object(object) => Some(
                object
                    .keys()
                    .take(24)
                    .map(|key| Value::String(key.clone()))
                    .collect::<Vec<_>>(),
            ),
            Value::Array(values) => Some(
                (0..values.len().min(24))
                    .map(|index| Value::String(format!("column_{}", index + 1)))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn markdown_table_candidates(chunk: &DocumentChunk, limit: usize) -> Vec<Value> {
    if limit == 0 {
        return Vec::new();
    }
    let mut tables = Vec::new();
    let mut current = Vec::<String>::new();
    for line in chunk.content.lines() {
        let trimmed = line.trim();
        if trimmed.matches('|').count() >= 2 {
            current.push(trimmed.to_string());
            continue;
        }
        flush_markdown_table(chunk.chunk_index, &mut current, &mut tables, limit);
        if tables.len() >= limit {
            return tables;
        }
    }
    flush_markdown_table(chunk.chunk_index, &mut current, &mut tables, limit);
    tables
}

fn flush_markdown_table(
    chunk_index: i32,
    current: &mut Vec<String>,
    tables: &mut Vec<Value>,
    limit: usize,
) {
    if current.len() < 2 || tables.len() >= limit {
        current.clear();
        return;
    }
    let rows = current
        .iter()
        .filter(|line| !is_markdown_separator_row(line))
        .map(|line| split_markdown_table_row(line))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        current.clear();
        return;
    }
    let headers = rows.first().cloned().unwrap_or_default();
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    tables.push(json!({
        "chunk_index": chunk_index,
        "source": "content.markdown_table",
        "row_count": rows.len().saturating_sub(1),
        "column_count": column_count,
        "headers": headers,
        "sample_rows": rows.into_iter().take(6).collect::<Vec<_>>(),
    }));
    current.clear();
}

fn is_markdown_separator_row(line: &str) -> bool {
    line.chars()
        .all(|value| matches!(value, '|' | '-' | ':' | ' ' | '\t'))
}

fn split_markdown_table_row(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(80).collect())
        .collect()
}

fn extract_procedure_steps(chunks: &[DocumentChunk], limit: usize) -> Vec<Value> {
    let mut steps = Vec::new();
    let mut seen = BTreeSet::new();
    for chunk in chunks {
        for sentence in procedure_sentence_candidates(&chunk.content) {
            if steps.len() >= limit {
                return steps;
            }
            let Some(step) = clean_procedure_step(&sentence) else {
                continue;
            };
            let normalized = normalize_for_seen(&step);
            if !seen.insert(normalized) {
                continue;
            }
            steps.push(json!({
                "chunk_index": chunk.chunk_index,
                "domain": procedure_domain(&step),
                "step": step,
                "threshold": extract_threshold_term(&sentence),
                "source": if looks_like_ordered_step(&sentence) { "ordered_line" } else { "procedure_sentence" },
            }));
        }
    }
    steps
}

fn procedure_sentence_candidates(content: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for line in content.lines() {
        for sentence in line.split(['。', '；', ';']) {
            let trimmed = sentence.trim();
            if trimmed.is_empty() {
                continue;
            }
            if looks_like_ordered_step(trimmed) || contains_any(trimmed, PROCEDURE_SIGNALS) {
                candidates.push(trimmed.to_string());
            }
        }
    }
    candidates
}

fn clean_procedure_step(value: &str) -> Option<String> {
    let cleaned = value
        .trim()
        .trim_matches(|item: char| matches!(item, '-' | '*' | ' ' | '\t'))
        .trim()
        .to_string();
    let char_count = cleaned.chars().count();
    if !(4..=180).contains(&char_count) {
        return None;
    }
    Some(cleaned)
}

fn procedure_domain(value: &str) -> &'static str {
    if contains_any(value, &["发药", "服药", "药品", "医嘱", "剂量"]) {
        "medication"
    } else if contains_any(value, &["交接班", "巡查", "护理记录"]) {
        "handover"
    } else if contains_any(value, &["翻身", "压疮", "皮肤", "卧床"]) {
        "bedridden_care"
    } else if contains_any(value, &["跌倒", "噎食", "突发", "应急", "处置"]) {
        "emergency"
    } else {
        "general"
    }
}

fn looks_like_ordered_step(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with(|item: char| item.is_ascii_digit())
        || trimmed.starts_with("第")
        || trimmed.starts_with("（")
        || trimmed.starts_with('(')
        || trimmed.starts_with("一、")
        || trimmed.starts_with("二、")
        || trimmed.starts_with("三、")
        || trimmed.starts_with("四、")
}

fn extract_threshold_term(value: &str) -> Option<String> {
    if !(value.contains("小时") || value.contains("分钟") || value.contains("天")) {
        return None;
    }
    let mut terms = Vec::new();
    for token in value.split(|item: char| item.is_whitespace() || matches!(item, '，' | ',' | '、'))
    {
        if token.contains("小时") || token.contains("分钟") || token.contains("天") {
            terms.push(token.trim_matches(|item: char| matches!(item, '。' | '；' | ';')));
        }
    }
    (!terms.is_empty()).then(|| terms.join(" "))
}

fn joined_chunk_text(chunks: &[DocumentChunk], limit: usize) -> String {
    let mut output = String::new();
    for chunk in chunks {
        if output.len() >= limit {
            break;
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&chunk.content);
    }
    output.chars().take(limit).collect()
}

fn resume_candidate_name(document: &Document) -> Option<String> {
    let raw = document
        .title
        .trim_end_matches(".pdf")
        .trim_end_matches(".docx")
        .trim_end_matches(".doc")
        .trim_end_matches(".PDF")
        .trim_end_matches(".DOCX")
        .trim_end_matches(".DOC");
    for part in raw.split(['_', '-', '—', ' ']) {
        let cleaned = part
            .replace("简历", "")
            .replace("个人", "")
            .replace("resume", "")
            .trim()
            .to_string();
        let cjk_count = cleaned.chars().filter(|value| is_cjk_char(*value)).count();
        if (2..=4).contains(&cjk_count) && cleaned.chars().count() <= 6 {
            return Some(cleaned);
        }
    }
    None
}

fn extract_resume_project_lines(text: &str, limit: usize) -> Vec<String> {
    extract_lines_containing(text, &["项目", "系统", "平台", "产品", "中台"], limit)
}

fn extract_known_skill_terms(text: &str, limit: usize) -> Vec<String> {
    extract_known_terms(
        text,
        &[
            "Rust",
            "Java",
            "Python",
            "JavaScript",
            "TypeScript",
            "React",
            "Vue",
            "Node",
            "SQL",
            "PostgreSQL",
            "MySQL",
            "Redis",
            "Docker",
            "Kubernetes",
            "AI",
            "大模型",
            "物联网",
            "数据分析",
            "项目管理",
            "产品设计",
        ],
        limit,
    )
}

fn extract_known_terms(text: &str, terms: &[&str], limit: usize) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    terms
        .iter()
        .filter(|term| {
            if term.is_ascii() {
                lower.contains(&term.to_ascii_lowercase())
            } else {
                text.contains(*term)
            }
        })
        .take(limit)
        .map(|term| (*term).to_string())
        .collect()
}

fn extract_lines_containing(text: &str, needles: &[&str], limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut lines = Vec::new();
    for line in text.lines() {
        let cleaned = line.trim();
        if cleaned.is_empty() || !contains_any(cleaned, needles) {
            continue;
        }
        let short = cleaned.chars().take(140).collect::<String>();
        if seen.insert(normalize_for_seen(&short)) {
            lines.push(short);
        }
        if lines.len() >= limit {
            break;
        }
    }
    lines
}

fn extract_year_terms_from_text(text: &str, limit: usize) -> Vec<String> {
    let mut years = BTreeSet::new();
    let chars = text.chars().collect::<Vec<_>>();
    for window in chars.windows(4) {
        if window.iter().all(|value| value.is_ascii_digit()) {
            let year = window.iter().collect::<String>();
            if year
                .parse::<i32>()
                .is_ok_and(|value| (1990..=2035).contains(&value))
            {
                years.insert(year);
            }
        }
        if years.len() >= limit {
            break;
        }
    }
    years.into_iter().collect()
}

fn extract_spreadsheet_metric_rows(chunks: &[DocumentChunk], limit: usize) -> Vec<Value> {
    let mut rows = Vec::new();
    for chunk in chunks {
        for line in chunk.content.lines() {
            if rows.len() >= limit {
                return rows;
            }
            if !looks_like_spreadsheet_metric_line(line) {
                continue;
            }
            rows.push(json!({
                "chunk_index": chunk.chunk_index,
                "date": extract_date_token(line),
                "employee": extract_employee_token(line),
                "work_hours": extract_work_hours(line),
                "work_hours_label": extract_work_hours(line).map(|hours| format!("{hours:.2}小时")),
                "counts_as_absence": line.contains("缺勤") || line.contains("未打卡") || line.contains("不考勤"),
                "status": spreadsheet_status(line),
                "source_text": line.trim().chars().take(180).collect::<String>(),
            }));
        }
    }
    rows
}

fn looks_like_spreadsheet_metric_line(line: &str) -> bool {
    contains_any(line, &["缺勤", "未打卡", "不考勤", "工时", "小时", "考勤"])
        || extract_date_token(line).is_some()
}

fn extract_date_token(line: &str) -> Option<String> {
    for token in line.split(|item: char| item.is_whitespace() || matches!(item, ',' | '，' | '|'))
    {
        let normalized = token.replace('/', "-");
        let parts = normalized.split('-').collect::<Vec<_>>();
        if parts.len() == 3
            && parts[0].len() == 4
            && parts[1].len() <= 2
            && parts[2].len() <= 2
            && parts
                .iter()
                .all(|part| part.chars().all(|item| item.is_ascii_digit()))
        {
            let month = parts[1].parse::<u32>().ok()?;
            let day = parts[2].parse::<u32>().ok()?;
            if (1..=12).contains(&month) && (1..=31).contains(&day) {
                return Some(format!("{}-{month:02}-{day:02}", parts[0]));
            }
        }
    }
    None
}

fn extract_employee_token(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|token| {
            let mut chars = token.chars();
            matches!(chars.next(), Some('A' | 'B' | 'C' | 'D' | 'E'))
                && chars.all(|item| item.is_ascii_digit())
        })
        .map(|value| value.to_string())
}

fn extract_work_hours(line: &str) -> Option<f64> {
    let marker = line.find("小时")?;
    let before = &line[..marker];
    let mut number = String::new();
    for value in before.chars().rev() {
        if value.is_ascii_digit() || value == '.' {
            number.insert(0, value);
        } else if !number.is_empty() {
            break;
        }
    }
    number.parse::<f64>().ok()
}

fn spreadsheet_status(line: &str) -> &'static str {
    if line.contains("缺勤") || line.contains("未打卡") || line.contains("不考勤") {
        "absence_or_missing_punch"
    } else if line.contains("正常") {
        "normal"
    } else {
        "unknown"
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn normalize_for_seen(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_cjk_char(value: char) -> bool {
    matches!(
        value as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

const PROCEDURE_SIGNALS: &[&str] = &[
    "应",
    "必须",
    "需要",
    "核对",
    "检查",
    "登记",
    "记录",
    "观察",
    "通知",
    "上报",
    "翻身",
    "发药",
    "服药",
    "交接班",
    "处置",
    "巡查",
    "评估",
];

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
    fn semantic_profile_feature_flag_defaults_off() {
        assert!(!env_flag("DATAMAX_TEST_MISSING_SEMANTIC_FLAG", false));
    }

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

    #[test]
    fn table_structure_summary_extracts_markdown_tables() {
        let document = test_document("经营表格.md");
        let chunks = vec![test_chunk(
            0,
            BTreeMap::new(),
            "| 门店 | 品牌 | 销售额 |\n| --- | --- | --- |\n| 新街口 | A品牌 | 1000 |",
        )];

        let summary = build_table_structure_summary(&document, &chunks, Utc::now());

        assert_eq!(summary["table_count"], json!(1));
        assert_eq!(summary["tables"][0]["column_count"], json!(3));
        assert_eq!(summary["tables"][0]["headers"][0], json!("门店"));
    }

    #[test]
    fn procedure_steps_summary_extracts_care_steps_and_thresholds() {
        let document = test_document("养老机构实操手册.docx");
        let chunks = vec![test_chunk(
            0,
            BTreeMap::new(),
            "2.4 发药：发药前应核对老年人姓名、床号、药品名称、剂量、时间和方法。\n帮助无自主翻身能力的老年人翻身，应至少每2小时翻身1次。",
        )];

        let summary = build_procedure_steps_summary(&document, &chunks, Utc::now());
        let steps = summary["steps"]
            .as_array()
            .expect("steps should be present");

        assert!(steps
            .iter()
            .any(|step| step["domain"] == json!("medication")));
        assert!(steps
            .iter()
            .any(|step| step["domain"] == json!("bedridden_care")));
        assert!(summary["threshold_count"].as_u64().unwrap_or(0) >= 1);
    }

    #[test]
    fn entity_terms_summary_groups_fact_terms() {
        let document = test_document("候选人简历.pdf");
        let chunks = vec![test_chunk(
            0,
            BTreeMap::from_iter([("section_title_hints".to_string(), json!(["项目经历"]))]),
            "Acme Technology Ltd 负责智能梯控平台，2024年上线。",
        )];

        let summary = build_entity_terms_summary(&document, &chunks, Utc::now());

        assert!(summary["term_type_count"].as_u64().unwrap_or(0) >= 1);
        assert!(summary["term_rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["fact_type"] == json!("section")));
    }

    #[test]
    fn resume_profile_summary_extracts_dimensions() {
        let document = test_document("郑宇宁_AI全栈产品技术主管_优化简历.pdf");
        let chunks = vec![test_chunk(
            0,
            BTreeMap::from_iter([(
                "section_title_hints".to_string(),
                json!(["项目经历"]),
            )]),
            "郑宇宁\n项目经历：智能家居平台、AI数据分析系统。\n技能：Python Rust React SQL 大模型 产品设计。\n2020-2024 在 Acme Technology Ltd 负责产品与技术管理。",
        )];

        let summary = build_resume_profile_summary(&document, &chunks, Utc::now());

        assert_eq!(summary["candidate_name"], json!("郑宇宁"));
        assert!(summary["skill_count"].as_u64().unwrap_or(0) >= 4);
        assert!(summary["project_count"].as_u64().unwrap_or(0) >= 1);
        assert!(summary["timeline_years"]
            .as_array()
            .unwrap()
            .contains(&json!("2024")));
    }

    #[test]
    fn spreadsheet_metrics_summary_extracts_absence_and_work_hour_extremes() {
        let document = test_document("考勤表.xlsx");
        let chunks = vec![test_chunk(
            0,
            BTreeMap::new(),
            "A5 2026/5/13 坐班0900 未打卡 不考勤\nA8 2026-02-07 坐班0900 12.55小时 正常考勤\nA8 2026-02-25 坐班0900 4.30小时 正常考勤",
        )];

        let summary = build_spreadsheet_metrics_summary(&document, &chunks, Utc::now());

        assert_eq!(summary["absence_count"], json!(1));
        assert_eq!(summary["longest_work_hour"]["work_hours"], json!(12.55));
        assert_eq!(summary["shortest_work_hour"]["work_hours"], json!(4.3));
        assert_eq!(summary["sample_rows"][0]["date"], json!("2026-05-13"));
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
