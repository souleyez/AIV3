use serde_json::{json, Value};
use std::collections::BTreeSet;

const SUMMARY_LIMIT: usize = 240;
const TERMS_LIMIT: usize = 24;
const FACETS_LIMIT: usize = 12;
const RETRIEVAL_TEXT_LIMIT: usize = 1600;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetProfileSupplyInput {
    pub asset_id: String,
    pub title: String,
    pub asset_kind: String,
    pub source_kind: String,
    pub profile_kind: String,
    pub attributes: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetProfileSupplyHint {
    pub asset_id: String,
    pub title: String,
    pub asset_kind: String,
    pub source_kind: String,
    pub profile_kind: String,
    pub summary: String,
    pub noun_terms: Vec<String>,
    pub facets: Vec<String>,
}

pub(crate) fn build_asset_profile_supply_hints(
    profiles: &[AssetProfileSupplyInput],
    limit: usize,
) -> Vec<AssetProfileSupplyHint> {
    let mut hints = profiles
        .iter()
        .filter_map(build_asset_profile_supply_hint)
        .collect::<Vec<_>>();
    hints.sort_by(|left, right| {
        left.asset_kind
            .cmp(&right.asset_kind)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.profile_kind.cmp(&right.profile_kind))
            .then_with(|| left.asset_id.cmp(&right.asset_id))
    });
    hints.truncate(limit.min(100));
    hints
}

pub(crate) fn build_asset_profile_supply_hints_for_query(
    profiles: &[AssetProfileSupplyInput],
    query: &str,
    limit: usize,
) -> Vec<AssetProfileSupplyHint> {
    let mut ranked_hints = profiles
        .iter()
        .filter_map(build_asset_profile_supply_hint)
        .map(|hint| (asset_profile_query_score(&hint, query), hint))
        .collect::<Vec<_>>();
    ranked_hints.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.asset_kind.cmp(&right.asset_kind))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.profile_kind.cmp(&right.profile_kind))
            .then_with(|| left.asset_id.cmp(&right.asset_id))
    });
    ranked_hints.truncate(limit.min(100));
    ranked_hints
        .into_iter()
        .map(|(_, hint)| hint)
        .collect::<Vec<_>>()
}

pub(crate) fn materialize_asset_profile_retrieval_evidence_text(
    hint: &AssetProfileSupplyHint,
) -> String {
    let mut lines = Vec::new();
    push_labeled_line(&mut lines, "asset_title", &hint.title);
    push_labeled_line(&mut lines, "asset_kind", &hint.asset_kind);
    push_labeled_line(&mut lines, "source_kind", &hint.source_kind);
    push_labeled_line(&mut lines, "profile_kind", &hint.profile_kind);
    push_labeled_line(&mut lines, "summary", &hint.summary);
    push_labeled_line(&mut lines, "terms", &hint.noun_terms.join("、"));
    push_labeled_line(&mut lines, "facets", &hint.facets.join("；"));
    lines
        .join("\n")
        .chars()
        .take(RETRIEVAL_TEXT_LIMIT)
        .collect()
}

pub(crate) fn build_asset_profile_retrieval_evidence_write_plan(
    hint: &AssetProfileSupplyHint,
    parser_name: &str,
    parser_version: &str,
) -> Value {
    let asset_token = safe_plan_token(&hint.asset_id);
    let profile_token = safe_plan_token(&hint.profile_kind);
    let parser_token = safe_plan_token(parser_name);
    let parser_version_token = safe_plan_token(parser_version);
    let evidence_text = materialize_asset_profile_retrieval_evidence_text(hint);
    json!({
        "action": "upsert_retrieval_evidence",
        "source_kind": "asset_profile",
        "source_locator": format!("asset-profile://{asset_token}/{profile_token}"),
        "idempotency_key": format!(
            "asset-profile:{asset_token}:{profile_token}:{parser_token}:{parser_version_token}"
        ),
        "dedupe_scope": [
            "tenant_id",
            "dataset_id",
            "asset_id",
            "profile_kind",
            "parser_name",
            "parser_version"
        ],
        "profile_schema": hint.profile_kind,
        "parser_name": parser_name,
        "parser_version": parser_version,
        "write_policy": "upsert_by_idempotency_key_after_profile_available",
        "evidence_text_present": !evidence_text.is_empty(),
        "evidence_text_chars": evidence_text.chars().count(),
        "dry_run_only": true,
        "ready": !evidence_text.is_empty(),
    })
}

pub(crate) fn build_asset_profile_retrieval_evidence_adapter_dry_run(
    hint: &AssetProfileSupplyHint,
    parser_name: &str,
    parser_version: &str,
) -> Value {
    let evidence_text = materialize_asset_profile_retrieval_evidence_text(hint);
    let write_plan =
        build_asset_profile_retrieval_evidence_write_plan(hint, parser_name, parser_version);
    json!({
        "adapter_contract": "asset_profile_retrieval_evidence_writer_v1",
        "mode": "dry_run",
        "status": if evidence_text.is_empty() { "blocked" } else { "ready" },
        "source_kind": "asset_profile",
        "profile_schema": hint.profile_kind,
        "input_contract": {
            "asset_id_present": !hint.asset_id.is_empty(),
            "profile_schema_present": !hint.profile_kind.is_empty(),
            "requires_profile_ready": true,
            "requires_dataset_membership": true,
            "requires_materialized_text": true,
        },
        "evidence_draft": {
            "source_kind": "asset_profile",
            "source_locator_present": write_plan.get("source_locator").is_some(),
            "content_excerpt_present": !evidence_text.is_empty(),
            "summary_present": !hint.summary.is_empty(),
            "payload_filter_key": format!("asset_profile:{}", safe_plan_token(&hint.profile_kind)),
            "embedding_model": "asset_profile_text_v1",
            "recall_score": 1.0,
            "manifest": {
                "schema_version": "asset_profile_retrieval_evidence_dry_run_v1",
                "generator": "asset-profile-materializer",
                "asset_profile": {
                    "asset_id_present": !hint.asset_id.is_empty(),
                    "asset_kind": hint.asset_kind,
                    "source_kind": hint.source_kind,
                    "profile_schema": hint.profile_kind,
                },
                "lexical": {
                    "status": if evidence_text.is_empty() { "missing_text" } else { "materialized" },
                    "language": "simple",
                    "search_text_present": !evidence_text.is_empty(),
                    "search_terms_count": hint.noun_terms.len(),
                },
            },
        },
        "write_plan": {
            "action": write_plan.get("action").cloned().unwrap_or(Value::Null),
            "source_kind": write_plan.get("source_kind").cloned().unwrap_or(Value::Null),
            "write_policy": write_plan.get("write_policy").cloned().unwrap_or(Value::Null),
            "ready": write_plan.get("ready").cloned().unwrap_or(Value::Null),
            "dedupe_scope": write_plan.get("dedupe_scope").cloned().unwrap_or(Value::Null),
            "idempotency_key_present": write_plan.get("idempotency_key").is_some(),
        },
        "write_order": [
            {
                "step": 1,
                "action": "upsert_asset_profile",
                "required_before": "materialize_retrieval_evidence_text",
            },
            {
                "step": 2,
                "action": "materialize_retrieval_evidence_text",
                "requires": "asset_profile_upserted",
            },
            {
                "step": 3,
                "action": "upsert_retrieval_evidence",
                "requires": "materialized_retrieval_evidence_text",
                "idempotent": true,
            },
            {
                "step": 4,
                "action": "mark_parse_run_completed",
                "requires": "retrieval_evidence_upserted",
            },
        ],
        "completion_gate": {
            "mark_completed_after": [
                "asset_profile_upserted",
                "retrieval_evidence_upserted"
            ],
            "partial_failure_next_status": "retrying",
            "do_not_mark_completed_until_evidence_upsert_succeeds": true,
        },
        "dry_run_only": true,
    })
}

pub(crate) fn build_asset_profile_retrieval_storage_mapping_dry_run(
    hint: &AssetProfileSupplyHint,
) -> Value {
    json!({
        "mapping_contract": "asset_profile_retrieval_storage_mapping_v1",
        "mode": "dry_run",
        "source_kind": "asset_profile",
        "profile_schema": hint.profile_kind,
        "current_storage_constraints": {
            "retrieval_evidences_requires_document_id": true,
            "retrieval_evidences_requires_document_chunk_id": true,
            "retrieval_evidences_conflict_key": [
                "execution_id",
                "document_chunk_id"
            ],
            "retrieval_worker_input_source": "document_chunks",
            "asset_profile_has_document_chunk": false,
        },
        "options": [
            {
                "strategy": "reuse_retrieval_evidences_with_synthetic_document_chunk",
                "recommended": false,
                "reason_codes": [
                    "pollutes_document_corpus",
                    "fake_chunk_lifecycle_mismatch",
                    "permission_scope_mismatch",
                    "deletion_and_dedup_semantics_mismatch"
                ],
            },
            {
                "strategy": "extend_retrieval_evidences_with_nullable_asset_refs",
                "recommended": false,
                "reason_codes": [
                    "touches_existing_document_evidence_semantics",
                    "requires_existing_retrieval_worker_migration",
                    "higher_regression_surface"
                ],
            },
            {
                "strategy": "add_asset_retrieval_evidences_table_then_union_search",
                "recommended": true,
                "reason_codes": [
                    "keeps_asset_refs_native",
                    "avoids_document_spoofing",
                    "preserves_existing_document_retrieval_contract",
                    "supports_asset_lifecycle_and_permissions"
                ],
            },
        ],
        "selected_strategy": "add_asset_retrieval_evidences_table_then_union_search",
        "proposed_asset_evidence_table": {
            "table_name": "asset_retrieval_evidences",
            "fields": [
                "tenant_id",
                "dataset_id",
                "asset_id",
                "profile_kind",
                "parser_name",
                "parser_version",
                "source_locator",
                "content_excerpt",
                "summary",
                "payload_filter_key",
                "embedding_model",
                "recall_score",
                "evidence_manifest",
                "search_text",
                "search_terms",
                "indexed_content_hash",
                "created_at"
            ],
            "unique_key": [
                "tenant_id",
                "dataset_id",
                "asset_id",
                "profile_kind",
                "parser_name",
                "parser_version"
            ],
        },
        "search_integration": {
            "method": "union_document_and_asset_evidence_search",
            "requires_query_update": true,
            "returns_source_kind": "asset_profile",
            "document_evidence_table_unchanged": true,
        },
        "migration_guardrails": {
            "production_write_allowed": false,
            "requires_reviewed_migration": true,
            "requires_search_result_source_kind": true,
            "requires_asset_dataset_membership_check": true,
        },
        "ready_for_migration_design": true,
        "production_write_allowed": false,
    })
}

pub(crate) fn build_asset_retrieval_evidence_migration_sketch_dry_run() -> Value {
    json!({
        "migration_contract": "asset_retrieval_evidences_migration_sketch_v1",
        "mode": "reviewed_sketch_only",
        "table": {
            "name": "asset_retrieval_evidences",
            "purpose": "store searchable evidence materialized from asset profiles without spoofing document chunks",
            "columns": [
                {"name": "id", "kind": "uuid", "required": true},
                {"name": "tenant_id", "kind": "uuid", "required": true},
                {"name": "dataset_id", "kind": "uuid", "required": true},
                {"name": "asset_id", "kind": "uuid", "required": true},
                {"name": "profile_kind", "kind": "text", "required": true},
                {"name": "parser_name", "kind": "text", "required": true},
                {"name": "parser_version", "kind": "text", "required": true},
                {"name": "source_locator", "kind": "text", "required": true},
                {"name": "content_excerpt", "kind": "text", "required": true},
                {"name": "summary", "kind": "text", "required": false},
                {"name": "payload_filter_key", "kind": "text", "required": true},
                {"name": "embedding_model", "kind": "text", "required": false},
                {"name": "recall_score", "kind": "double precision", "required": false},
                {"name": "evidence_manifest", "kind": "jsonb", "required": true},
                {"name": "search_text", "kind": "text", "required": true},
                {"name": "search_terms", "kind": "text[]", "required": true},
                {"name": "search_tsv", "kind": "tsvector", "required": true},
                {"name": "indexed_content_hash", "kind": "text", "required": true},
                {"name": "created_at", "kind": "timestamptz", "required": true},
                {"name": "indexed_at", "kind": "timestamptz", "required": true}
            ],
            "unique_key": [
                "tenant_id",
                "dataset_id",
                "asset_id",
                "profile_kind",
                "parser_name",
                "parser_version"
            ],
        },
        "indexes": [
            {
                "name": "asset_retrieval_evidences_scope_profile_idx",
                "method": "btree",
                "columns": ["tenant_id", "dataset_id", "profile_kind", "created_at desc"]
            },
            {
                "name": "asset_retrieval_evidences_asset_profile_idx",
                "method": "btree",
                "columns": ["tenant_id", "asset_id", "profile_kind", "created_at desc"]
            },
            {
                "name": "asset_retrieval_evidences_search_terms_gin_idx",
                "method": "gin",
                "columns": ["search_terms"]
            },
            {
                "name": "asset_retrieval_evidences_search_tsv_gin_idx",
                "method": "gin",
                "columns": ["search_tsv"]
            },
            {
                "name": "asset_retrieval_evidences_content_hash_idx",
                "method": "btree",
                "columns": ["tenant_id", "asset_id", "indexed_content_hash", "indexed_at desc"]
            }
        ],
        "membership_guard": {
            "required": true,
            "membership_table": "dataset_asset_memberships",
            "join_keys": ["tenant_id", "dataset_id", "asset_id"],
            "expiry_filter_required": true,
            "hidden_asset_policy": "deny",
        },
        "search_result_contract": {
            "source_kind": "asset_profile",
            "source_id_field": "asset_id",
            "scope_id_field": "dataset_id",
            "profile_kind_field": "profile_kind",
            "content_field": "content_excerpt",
            "manifest_field": "evidence_manifest",
            "must_not_include": [
                "raw_storage_locator",
                "provider_payload",
                "auth_material",
                "session_material",
                "filesystem_path"
            ],
        },
        "review_gates": [
            "operator_reviewed_migration",
            "backfill_plan_reviewed",
            "rollback_plan_reviewed",
            "small_batch_first",
            "search_source_kind_regression_test"
        ],
        "production_migration_allowed": false,
        "production_write_allowed": false,
    })
}

pub(crate) fn build_asset_profile_union_search_no_write_adapter_draft(
    hint: &AssetProfileSupplyHint,
) -> Value {
    json!({
        "adapter_contract": "asset_profile_union_search_no_write_adapter_v1",
        "mode": "dry_run",
        "no_write": true,
        "source_kind": "asset_profile",
        "profile_schema": hint.profile_kind,
        "input_contract": {
            "requires_query_text": true,
            "requires_dataset_scope": true,
            "requires_asset_dataset_membership": true,
            "document_retrieval_input_unchanged": true,
            "asset_evidence_table_required": "asset_retrieval_evidences",
        },
        "search_plan": {
            "method": "union_document_and_asset_evidence_search",
            "steps": [
                {"step": 1, "action": "resolve_dataset_scope"},
                {"step": 2, "action": "run_existing_document_retrieval"},
                {
                    "step": 3,
                    "action": "query_asset_retrieval_evidences",
                    "table": "asset_retrieval_evidences",
                    "membership_guard": "dataset_asset_memberships"
                },
                {"step": 4, "action": "merge_rank_and_limit_results"}
            ],
            "query_terms_count": hint.noun_terms.len(),
            "ranking_policy": "score_then_source_kind_then_recency",
            "document_search_unchanged": true,
        },
        "membership_guard": {
            "required": true,
            "table": "dataset_asset_memberships",
            "join_keys": ["tenant_id", "dataset_id", "asset_id"],
            "deny_hidden_assets": true,
            "expiry_filter_required": true,
        },
        "result_contract": {
            "source_kind": "asset_profile",
            "source_id_field": "asset_id",
            "profile_kind_field": "profile_kind",
            "title_present": !hint.title.is_empty(),
            "summary_present": !hint.summary.is_empty(),
            "terms_count": hint.noun_terms.len(),
            "must_not_include_raw_locator": true,
            "must_not_include_provider_payload": true,
        },
        "guardrails": {
            "requires_search_result_source_kind": true,
            "requires_asset_dataset_membership_check": true,
            "requires_current_tenant_filter": true,
            "no_schema_migration_in_this_step": true,
            "production_write_allowed": false,
        },
        "ready_for_adapter_design": true,
        "production_write_allowed": false,
    })
}

pub(crate) fn build_asset_profile_union_search_merge_rank_fixture_dry_run(
    hint: &AssetProfileSupplyHint,
) -> Value {
    json!({
        "fixture_contract": "asset_profile_union_search_merge_rank_fixture_v1",
        "mode": "dry_run",
        "no_write": true,
        "input_results": {
            "document_results": [
                {
                    "source_kind": "document_chunk",
                    "source_id_present": true,
                    "title": "document evidence fixture",
                    "score": 0.72,
                    "recency_rank": 2,
                    "membership_guard_passed": true,
                    "raw_locator_included": false,
                    "provider_payload_included": false,
                }
            ],
            "asset_profile_results": [
                {
                    "source_kind": "asset_profile",
                    "source_id_present": !hint.asset_id.is_empty(),
                    "profile_schema": hint.profile_kind,
                    "title_present": !hint.title.is_empty(),
                    "score": 0.88,
                    "recency_rank": 1,
                    "membership_guard_passed": true,
                    "raw_locator_included": false,
                    "provider_payload_included": false,
                }
            ],
        },
        "merge_policy": {
            "method": "stable_score_desc_source_kind_recency",
            "max_results": 8,
            "requires_source_kind": true,
            "requires_membership_guard": true,
            "document_search_unchanged": true,
            "asset_search_no_write": true,
        },
        "merged_results": [
            {
                "rank": 1,
                "source_kind": "asset_profile",
                "source_id_present": !hint.asset_id.is_empty(),
                "profile_schema": hint.profile_kind,
                "score": 0.88,
                "sort_reason": "higher_score_then_recency",
                "membership_guard_passed": true,
                "raw_locator_included": false,
                "provider_payload_included": false,
            },
            {
                "rank": 2,
                "source_kind": "document_chunk",
                "source_id_present": true,
                "score": 0.72,
                "sort_reason": "document_evidence_retained",
                "membership_guard_passed": true,
                "raw_locator_included": false,
                "provider_payload_included": false,
            }
        ],
        "source_kind_regression": {
            "mixed_sources_present": true,
            "asset_profile_source_kind_preserved": true,
            "document_source_kind_preserved": true,
            "result_source_kind_required": true,
            "membership_guard_checked": true,
            "raw_locator_excluded": true,
            "provider_payload_excluded": true,
        },
        "ready_for_search_merge_regression": true,
        "production_write_allowed": false,
    })
}

pub(crate) fn build_asset_profile_union_search_explain_debug_summary_dry_run(
    hint: &AssetProfileSupplyHint,
) -> Value {
    let fixture = build_asset_profile_union_search_merge_rank_fixture_dry_run(hint);
    let merged_count = fixture
        .get("merged_results")
        .and_then(Value::as_array)
        .map(|results| results.len())
        .unwrap_or_default();
    json!({
        "debug_contract": "asset_profile_union_search_explain_debug_summary_v1",
        "mode": "dry_run",
        "no_write": true,
        "summary": {
            "query_scope_resolved": true,
            "document_result_count": 1,
            "asset_profile_result_count": 1,
            "merged_result_count": merged_count,
            "top_source_kind": fixture
                .pointer("/merged_results/0/source_kind")
                .cloned()
                .unwrap_or(Value::Null),
            "source_kind_mix": ["asset_profile", "document_chunk"],
            "membership_guard_status": "checked",
            "rank_reason_codes": [
                "asset_profile_higher_score",
                "document_evidence_retained",
                "source_kind_preserved"
            ],
            "asset_profile_is_exact_document_citation": false,
        },
        "safe_debug_fields": [
            "rank",
            "source_kind",
            "score_bucket",
            "reason_codes",
            "membership_guard_status"
        ],
        "redaction": {
            "raw_locator_excluded": true,
            "provider_payload_excluded": true,
            "secret_material_excluded": true,
        },
        "ready_for_union_search_debug_panel": true,
        "production_write_allowed": false,
    })
}

pub(crate) fn build_asset_profile_model_facing_supply_compression_dry_run(
    hint: &AssetProfileSupplyHint,
) -> Value {
    let compact_terms = hint.noun_terms.iter().take(8).cloned().collect::<Vec<_>>();
    let compact_facets = hint.facets.iter().take(4).cloned().collect::<Vec<_>>();
    let compact_text = format!(
        "asset signal: {}; profile: {}; summary: {}; terms: {}; facets: {}",
        hint.title,
        hint.profile_kind,
        hint.summary,
        compact_terms.join("、"),
        compact_facets.join("；")
    )
    .chars()
    .take(480)
    .collect::<String>();
    let compact_text_chars = compact_text.chars().count();
    json!({
        "compression_contract": "asset_profile_model_facing_supply_compression_v1",
        "mode": "dry_run",
        "no_write": true,
        "source_kind": "asset_profile",
        "profile_schema": hint.profile_kind,
        "model_supply": {
            "compressed_text": compact_text,
            "compressed_text_chars": compact_text_chars,
            "term_count": compact_terms.len(),
            "facet_count": compact_facets.len(),
            "citation_policy": "asset_profile_is_understanding_signal_not_exact_document_quote",
            "exact_claim_policy": "require_document_database_or_media_evidence_for_exact_claims",
        },
        "compression_checks": {
            "raw_locator_excluded": true,
            "provider_payload_excluded": true,
            "secret_material_excluded": true,
            "asset_profile_exact_citation_disallowed": true,
            "document_evidence_required_for_exact_claims": true,
        },
        "ready_for_model_supply_compression": !compact_text.is_empty(),
        "production_write_allowed": false,
    })
}

fn build_asset_profile_supply_hint(
    profile: &AssetProfileSupplyInput,
) -> Option<AssetProfileSupplyHint> {
    let title = normalize_text(&profile.title);
    let asset_id = normalize_text(&profile.asset_id);
    if title.is_empty() || asset_id.is_empty() || !profile.attributes.is_object() {
        return None;
    }

    let noun_terms = collect_noun_terms(&profile.attributes);
    let facets = collect_facets(&profile.attributes);
    let summary = build_summary(&profile.attributes, &facets)
        .unwrap_or_else(|| title.clone())
        .chars()
        .take(SUMMARY_LIMIT)
        .collect::<String>();

    Some(AssetProfileSupplyHint {
        asset_id,
        title,
        asset_kind: normalize_text(&profile.asset_kind),
        source_kind: normalize_text(&profile.source_kind),
        profile_kind: normalize_text(&profile.profile_kind),
        summary,
        noun_terms,
        facets,
    })
}

fn build_summary(attributes: &Value, facets: &[String]) -> Option<String> {
    for key in [
        "summary",
        "description",
        "caption",
        "ocr_text",
        "transcript_summary",
    ] {
        if let Some(value) = attributes.get(key).and_then(Value::as_str) {
            let text = normalize_text(value);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    if facets.is_empty() {
        None
    } else {
        Some(facets.join("；"))
    }
}

fn collect_noun_terms(attributes: &Value) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for key in [
        "noun_terms",
        "nounTermHints",
        "tags",
        "keywords",
        "objects",
        "entities",
        "colors",
        "color",
        "materials",
        "material",
        "scenes",
        "scene",
        "styles",
        "style",
        "audience",
        "gender",
        "seasons",
        "season",
        "silhouettes",
        "collars",
        "sleeves",
        "waists",
        "hems",
        "crafts",
        "processes",
        "patterns",
        "visible_text",
        "sku_text_marks",
    ] {
        collect_text_values(attributes.get(key), &mut terms);
    }
    for key in [
        "category",
        "season",
        "style",
        "silhouette",
        "material",
        "process",
        "theme",
    ] {
        if let Some(value) = attributes.get(key) {
            collect_text_values(Some(value), &mut terms);
        }
    }
    terms.into_iter().take(TERMS_LIMIT).collect()
}

fn collect_facets(attributes: &Value) -> Vec<String> {
    let mut facets = Vec::new();
    for (label, key) in [
        ("品类", "category"),
        ("受众", "audience"),
        ("季节", "season"),
        ("季节", "seasons"),
        ("风格", "style"),
        ("风格", "styles"),
        ("版型", "silhouette"),
        ("版型", "silhouettes"),
        ("领型", "collars"),
        ("袖型", "sleeves"),
        ("腰型", "waists"),
        ("下摆", "hems"),
        ("颜色", "colors"),
        ("面料", "materials"),
        ("工艺", "crafts"),
        ("图案", "patterns"),
        ("场景", "scenes"),
        ("状态", "status"),
    ] {
        let values = normalized_text_values(attributes.get(key));
        if !values.is_empty() {
            facets.push(format!("{label}: {}", values.join("、")));
        }
    }
    facets.truncate(FACETS_LIMIT);
    facets
}

fn collect_text_values(value: Option<&Value>, target: &mut BTreeSet<String>) {
    for text in normalized_text_values(value) {
        target.insert(text);
    }
}

fn asset_profile_query_score(hint: &AssetProfileSupplyHint, query: &str) -> i64 {
    let query = normalize_text(query).to_lowercase();
    if query.is_empty() {
        return 0;
    }

    let query_terms = asset_profile_query_terms(&query);
    let haystack = normalize_text(&format!(
        "{}\n{}\n{}\n{}",
        hint.title,
        hint.summary,
        hint.noun_terms.join("\n"),
        hint.facets.join("\n")
    ))
    .to_lowercase();

    let mut score = 0_i64;
    if haystack.contains(&query) {
        score += 80;
    }

    for term in &query_terms {
        if haystack.contains(term) {
            score += if term.chars().count() >= 2 { 10 } else { 1 };
        }
    }

    for term in hint
        .noun_terms
        .iter()
        .chain(hint.facets.iter())
        .map(|term| normalize_text(term).to_lowercase())
        .filter(|term| !term.is_empty())
    {
        if query.contains(&term) {
            score += if term.chars().count() >= 2 { 18 } else { 2 };
        }
    }

    score
}

fn asset_profile_query_terms(query: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    let mut ascii_token = String::new();
    let mut non_ascii_chars = Vec::new();

    for ch in query.chars() {
        if ch.is_ascii_alphanumeric() {
            ascii_token.push(ch.to_ascii_lowercase());
            continue;
        }
        if !ascii_token.is_empty() {
            terms.insert(std::mem::take(&mut ascii_token));
        }
        if !ch.is_whitespace() && !ch.is_ascii_punctuation() {
            let value = ch.to_string();
            terms.insert(value.clone());
            non_ascii_chars.push(value);
        }
    }
    if !ascii_token.is_empty() {
        terms.insert(ascii_token);
    }

    for window in non_ascii_chars.windows(2) {
        terms.insert(format!("{}{}", window[0], window[1]));
    }
    if query.chars().count() <= 32 {
        terms.insert(query.to_string());
    }

    terms.into_iter().take(64).collect()
}

fn normalized_text_values(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => {
            let text = normalize_text(text);
            if text.is_empty() {
                vec![]
            } else {
                vec![text]
            }
        }
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(normalize_text)
            .filter(|text| !text.is_empty())
            .collect(),
        _ => vec![],
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_labeled_line(lines: &mut Vec<String>, label: &str, value: &str) {
    let value = normalize_text(value);
    if !value.is_empty() {
        lines.push(format!("{label}: {value}"));
    }
}

fn safe_plan_token(value: &str) -> String {
    let token = normalize_text(value)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(160)
        .collect::<String>();
    if token.is_empty() {
        "unknown".to_string()
    } else {
        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn asset_profile_supply_extracts_multimodal_terms_and_facets() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "upload".to_string(),
                profile_kind: "image_semantic".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "style": "通勤",
                    "colors": ["绿色", "白色"],
                    "materials": ["棉麻"],
                    "collars": ["圆领"],
                    "sleeves": ["泡泡袖"],
                    "crafts": ["压褶"],
                    "patterns": ["碎花"],
                    "noun_terms": ["泡泡袖", "碎花", "连衣裙"]
                }),
            }],
            10,
        );

        assert_eq!(hints.len(), 1);
        assert!(hints[0].summary.contains("品类: 连衣裙"));
        assert!(hints[0].summary.contains("颜色: 绿色、白色"));
        assert!(hints[0].noun_terms.contains(&"泡泡袖".to_string()));
        assert!(hints[0].noun_terms.contains(&"棉麻".to_string()));
        assert!(hints[0].noun_terms.contains(&"压褶".to_string()));
        assert!(hints[0].facets.contains(&"领型: 圆领".to_string()));
        assert_eq!(hints[0].profile_kind, "image_semantic");
    }

    #[test]
    fn asset_profile_supply_prefers_existing_summaries_and_limits_output() {
        let hints = build_asset_profile_supply_hints(
            &[
                AssetProfileSupplyInput {
                    asset_id: "asset-b".to_string(),
                    title: "视频资产".to_string(),
                    asset_kind: "video".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "video_summary".to_string(),
                    attributes: json!({
                        "summary": "门店陈列视频，重点展示夏季女装区域和导购讲解。",
                        "entities": ["门店", "女装", "导购"]
                    }),
                },
                AssetProfileSupplyInput {
                    asset_id: "asset-a".to_string(),
                    title: "PPT资产".to_string(),
                    asset_kind: "presentation".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "slide_outline".to_string(),
                    attributes: json!({
                        "description": "季度经营复盘 PPT，包含销售趋势和库存风险。",
                        "keywords": ["销售趋势", "库存风险"]
                    }),
                },
            ],
            1,
        );

        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].asset_kind, "presentation");
        assert_eq!(
            hints[0].summary,
            "季度经营复盘 PPT，包含销售趋势和库存风险。"
        );
        assert!(hints[0].noun_terms.contains(&"库存风险".to_string()));
    }

    #[test]
    fn asset_profile_supply_query_ranking_promotes_matching_design_terms() {
        let profiles = vec![
            AssetProfileSupplyInput {
                asset_id: "asset-a".to_string(),
                title: "冬季西装参考".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "upload".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "西装",
                    "season": "秋冬",
                    "style": "商务",
                    "colors": ["黑色"]
                }),
            },
            AssetProfileSupplyInput {
                asset_id: "asset-z".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "upload".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "styles": ["通勤"],
                    "collars": ["圆领"],
                    "noun_terms": ["泡泡袖", "碎花", "连衣裙"]
                }),
            },
        ];

        let hints =
            build_asset_profile_supply_hints_for_query(&profiles, "找春夏连衣裙或泡泡袖款式", 10);

        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].asset_id, "asset-z");
        assert!(hints[0].noun_terms.contains(&"泡泡袖".to_string()));
    }

    #[test]
    fn asset_profile_supply_materializes_safe_retrieval_text() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "fashion_design_image_import".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "colors": ["绿色", "白色"],
                    "sleeves": ["泡泡袖"],
                    "patterns": ["碎花"],
                    "noun_terms": ["泡泡袖", "碎花", "连衣裙"],
                    "caption": "春夏通勤连衣裙设计图",
                    "object_key": "objects/private/look.png",
                    "raw_provider_payload": {"should_not_surface": true}
                }),
            }],
            10,
        );

        let text = materialize_asset_profile_retrieval_evidence_text(&hints[0]);

        assert!(text.contains("asset_title: 春夏连衣裙灵感图"));
        assert!(text.contains("profile_kind: fashion_design_image_v1"));
        assert!(text.contains("summary: 春夏通勤连衣裙设计图"));
        assert!(text.contains("terms: "));
        assert!(text.contains("泡泡袖"));
        assert!(text.contains("facets: "));
        assert!(!text.contains("objects/private"));
        assert!(!text.contains("raw_provider_payload"));
        assert!(text.chars().count() <= RETRIEVAL_TEXT_LIMIT);
    }

    #[test]
    fn asset_profile_supply_builds_idempotent_retrieval_write_plan() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "fashion_design_image_import".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "caption": "春夏通勤连衣裙设计图",
                    "object_key": "objects/private/look.png",
                    "raw_provider_payload": {"should_not_surface": true}
                }),
            }],
            10,
        );

        let plan = build_asset_profile_retrieval_evidence_write_plan(
            &hints[0],
            "datamax-fashion-image-parser",
            "2026-06-17",
        );
        let duplicate_plan = build_asset_profile_retrieval_evidence_write_plan(
            &hints[0],
            "datamax-fashion-image-parser",
            "2026-06-17",
        );

        assert_eq!(plan["action"], json!("upsert_retrieval_evidence"));
        assert_eq!(plan["source_kind"], json!("asset_profile"));
        assert_eq!(
            plan["source_locator"],
            json!("asset-profile://asset-1/fashion_design_image_v1")
        );
        assert_eq!(plan["idempotency_key"], duplicate_plan["idempotency_key"]);
        assert_eq!(
            plan["write_policy"],
            json!("upsert_by_idempotency_key_after_profile_available")
        );
        assert_eq!(plan["ready"], json!(true));
        assert!(plan["evidence_text_chars"].as_u64().unwrap_or_default() > 0);
        let serialized = serde_json::to_string(&plan).expect("plan should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn asset_profile_supply_builds_retrieval_adapter_dry_run_with_order_gate() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "fashion_design_image_import".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "caption": "春夏通勤连衣裙设计图",
                    "tags": ["泡泡袖", "碎花"],
                    "object_key": "objects/private/look.png",
                    "raw_provider_payload": {"should_not_surface": true}
                }),
            }],
            10,
        );

        let adapter = build_asset_profile_retrieval_evidence_adapter_dry_run(
            &hints[0],
            "datamax-fashion-image-parser",
            "2026-06-17",
        );

        assert_eq!(
            adapter["adapter_contract"],
            json!("asset_profile_retrieval_evidence_writer_v1")
        );
        assert_eq!(adapter["status"], json!("ready"));
        assert_eq!(
            adapter["evidence_draft"]["manifest"]["lexical"]["status"],
            json!("materialized")
        );
        assert_eq!(
            adapter["write_order"][0]["action"],
            json!("upsert_asset_profile")
        );
        assert_eq!(
            adapter["write_order"][2]["action"],
            json!("upsert_retrieval_evidence")
        );
        assert_eq!(
            adapter["write_order"][3]["requires"],
            json!("retrieval_evidence_upserted")
        );
        assert_eq!(
            adapter["completion_gate"]["do_not_mark_completed_until_evidence_upsert_succeeds"],
            json!(true)
        );
        assert_eq!(
            adapter["completion_gate"]["partial_failure_next_status"],
            json!("retrying")
        );
        assert_eq!(
            adapter["write_plan"]["idempotency_key_present"],
            json!(true)
        );
        let serialized = serde_json::to_string(&adapter).expect("adapter should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn asset_profile_supply_builds_storage_mapping_dry_run() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "fashion_design_image_import".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "caption": "春夏通勤连衣裙设计图",
                    "object_key": "objects/private/look.png",
                    "raw_provider_payload": {"should_not_surface": true}
                }),
            }],
            10,
        );

        let mapping = build_asset_profile_retrieval_storage_mapping_dry_run(&hints[0]);

        assert_eq!(
            mapping["mapping_contract"],
            json!("asset_profile_retrieval_storage_mapping_v1")
        );
        assert_eq!(
            mapping["current_storage_constraints"]["retrieval_evidences_requires_document_id"],
            json!(true)
        );
        assert_eq!(
            mapping["current_storage_constraints"]["asset_profile_has_document_chunk"],
            json!(false)
        );
        assert_eq!(
            mapping["options"][0]["strategy"],
            json!("reuse_retrieval_evidences_with_synthetic_document_chunk")
        );
        assert_eq!(mapping["options"][0]["recommended"], json!(false));
        assert_eq!(
            mapping["selected_strategy"],
            json!("add_asset_retrieval_evidences_table_then_union_search")
        );
        assert_eq!(
            mapping["proposed_asset_evidence_table"]["table_name"],
            json!("asset_retrieval_evidences")
        );
        assert_eq!(
            mapping["proposed_asset_evidence_table"]["unique_key"][2],
            json!("asset_id")
        );
        assert_eq!(
            mapping["search_integration"]["method"],
            json!("union_document_and_asset_evidence_search")
        );
        assert_eq!(
            mapping["migration_guardrails"]["requires_asset_dataset_membership_check"],
            json!(true)
        );
        assert_eq!(mapping["ready_for_migration_design"], json!(true));
        assert_eq!(mapping["production_write_allowed"], json!(false));
        let serialized = serde_json::to_string(&mapping).expect("mapping should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn asset_profile_supply_builds_migration_sketch_and_union_search_draft() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "fashion_design_image_import".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "caption": "春夏通勤连衣裙设计图",
                    "tags": ["泡泡袖", "碎花"],
                    "object_key": "objects/private/look.png",
                    "raw_provider_payload": {"should_not_surface": true}
                }),
            }],
            10,
        );

        let migration = build_asset_retrieval_evidence_migration_sketch_dry_run();
        let adapter = build_asset_profile_union_search_no_write_adapter_draft(&hints[0]);

        assert_eq!(
            migration["migration_contract"],
            json!("asset_retrieval_evidences_migration_sketch_v1")
        );
        assert_eq!(
            migration["table"]["name"],
            json!("asset_retrieval_evidences")
        );
        assert_eq!(
            migration["membership_guard"]["membership_table"],
            json!("dataset_asset_memberships")
        );
        assert_eq!(migration["membership_guard"]["required"], json!(true));
        assert_eq!(
            migration["search_result_contract"]["source_kind"],
            json!("asset_profile")
        );
        assert!(migration["indexes"]
            .as_array()
            .is_some_and(|indexes| indexes.iter().any(|index| index["method"] == json!("gin"))));
        assert_eq!(migration["production_migration_allowed"], json!(false));
        assert_eq!(migration["production_write_allowed"], json!(false));

        assert_eq!(
            adapter["adapter_contract"],
            json!("asset_profile_union_search_no_write_adapter_v1")
        );
        assert_eq!(adapter["no_write"], json!(true));
        assert_eq!(
            adapter["search_plan"]["method"],
            json!("union_document_and_asset_evidence_search")
        );
        assert_eq!(
            adapter["membership_guard"]["table"],
            json!("dataset_asset_memberships")
        );
        assert_eq!(
            adapter["membership_guard"]["deny_hidden_assets"],
            json!(true)
        );
        assert_eq!(
            adapter["result_contract"]["source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            adapter["guardrails"]["no_schema_migration_in_this_step"],
            json!(true)
        );
        assert_eq!(adapter["production_write_allowed"], json!(false));

        let serialized = serde_json::to_string(&json!({
            "migration": migration,
            "adapter": adapter,
        }))
        .expect("migration and adapter should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn asset_profile_supply_builds_union_search_merge_rank_fixture() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "fashion_design_image_import".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "caption": "春夏通勤连衣裙设计图",
                    "tags": ["泡泡袖", "碎花"],
                    "object_key": "objects/private/look.png",
                    "raw_provider_payload": {"should_not_surface": true}
                }),
            }],
            10,
        );

        let fixture = build_asset_profile_union_search_merge_rank_fixture_dry_run(&hints[0]);

        assert_eq!(
            fixture["fixture_contract"],
            json!("asset_profile_union_search_merge_rank_fixture_v1")
        );
        assert_eq!(fixture["no_write"], json!(true));
        assert_eq!(
            fixture["merged_results"][0]["source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            fixture["merged_results"][1]["source_kind"],
            json!("document_chunk")
        );
        assert_eq!(
            fixture["merge_policy"]["requires_membership_guard"],
            json!(true)
        );
        assert_eq!(
            fixture["source_kind_regression"]["mixed_sources_present"],
            json!(true)
        );
        assert_eq!(
            fixture["source_kind_regression"]["asset_profile_source_kind_preserved"],
            json!(true)
        );
        assert_eq!(
            fixture["source_kind_regression"]["document_source_kind_preserved"],
            json!(true)
        );
        assert_eq!(
            fixture["source_kind_regression"]["raw_locator_excluded"],
            json!(true)
        );
        assert_eq!(fixture["production_write_allowed"], json!(false));

        let serialized = serde_json::to_string(&fixture).expect("fixture should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn asset_profile_supply_builds_explain_debug_and_model_compression_dry_runs() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "fashion_design_image_import".to_string(),
                profile_kind: "fashion_design_image_v1".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "caption": "春夏通勤连衣裙设计图",
                    "tags": ["泡泡袖", "碎花", "轻熟"],
                    "object_key": "objects/private/look.png",
                    "raw_provider_payload": {"should_not_surface": true}
                }),
            }],
            10,
        );

        let explain = build_asset_profile_union_search_explain_debug_summary_dry_run(&hints[0]);
        let compression = build_asset_profile_model_facing_supply_compression_dry_run(&hints[0]);

        assert_eq!(
            explain["debug_contract"],
            json!("asset_profile_union_search_explain_debug_summary_v1")
        );
        assert_eq!(explain["no_write"], json!(true));
        assert_eq!(
            explain["summary"]["top_source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            explain["summary"]["asset_profile_is_exact_document_citation"],
            json!(false)
        );
        assert_eq!(explain["redaction"]["raw_locator_excluded"], json!(true));
        assert_eq!(explain["production_write_allowed"], json!(false));

        assert_eq!(
            compression["compression_contract"],
            json!("asset_profile_model_facing_supply_compression_v1")
        );
        assert_eq!(compression["no_write"], json!(true));
        assert_eq!(
            compression["model_supply"]["citation_policy"],
            json!("asset_profile_is_understanding_signal_not_exact_document_quote")
        );
        assert_eq!(
            compression["compression_checks"]["asset_profile_exact_citation_disallowed"],
            json!(true)
        );
        assert_eq!(
            compression["compression_checks"]["document_evidence_required_for_exact_claims"],
            json!(true)
        );
        assert_eq!(compression["production_write_allowed"], json!(false));

        let serialized = serde_json::to_string(&json!({
            "explain": explain,
            "compression": compression,
        }))
        .expect("explain and compression dry-runs should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn asset_profile_supply_drops_empty_or_non_object_profiles() {
        let hints = build_asset_profile_supply_hints(
            &[
                AssetProfileSupplyInput {
                    asset_id: "".to_string(),
                    title: "missing id".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "image_semantic".to_string(),
                    attributes: json!({"category": "连衣裙"}),
                },
                AssetProfileSupplyInput {
                    asset_id: "asset-2".to_string(),
                    title: "bad attrs".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "image_semantic".to_string(),
                    attributes: json!("raw text should not be used directly"),
                },
            ],
            10,
        );

        assert!(hints.is_empty());
    }
}
