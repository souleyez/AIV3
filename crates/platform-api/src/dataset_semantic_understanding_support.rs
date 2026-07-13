use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use chrono::Utc;
use domain_model::{Dataset, DatasetId, SecretBindingId, TenantId, UserId};

use crate::semantic_label_resolver::safe_public_examples;
use crate::semantic_understanding::{
    DatasetSemanticIdentity, DatasetSemanticUnderstanding, SemanticCoverage, SemanticSummary,
    SemanticTruncation, DATASET_SEMANTIC_GENERATION_VERSION, DATASET_SEMANTIC_SCHEMA_VERSION,
};
use crate::{
    active_secret_binding_ids_from_headers, current_auth_user_id, dataset_is_visible_for_request,
    load_visible_dataset_for_user_with_local_scope, local_thread_id_from_headers, ApiError,
    AppState,
};

pub(crate) async fn get_dataset_semantic_understanding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<DatasetId>,
) -> std::result::Result<Response, ApiError> {
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let current_user_id = current_auth_user_id(&state, &headers).await?;
    let local_thread_id = local_thread_id_from_headers(&headers);
    let dataset = load_visible_dataset_for_user_with_local_scope(
        &state,
        dataset_id,
        &active_secret_binding_ids,
        current_user_id,
        local_thread_id.as_deref(),
    )
    .await?;

    let latest_ready = state
        .storage
        .dataset_semantic_snapshots()
        .load_latest_ready(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;
    let latest_attempt = state
        .storage
        .dataset_semantic_snapshots()
        .load_latest_attempt(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;

    let (mut understanding, etag) = match latest_ready {
        Some(snapshot) => {
            let understanding = serde_json::from_value::<DatasetSemanticUnderstanding>(
                snapshot.manifest,
            )
            .map_err(|error| {
                ApiError::internal(
                    "dataset_understanding_manifest_invalid",
                    format!("dataset understanding manifest is invalid: {error}"),
                )
            })?;
            (
                understanding,
                Some(format!("\"{}\"", snapshot.source_fingerprint)),
            )
        }
        None => (empty_understanding_contract(&dataset), None),
    };
    apply_latest_attempt_state(&mut understanding, latest_attempt.as_ref());
    sanitize_understanding_for_public_response(&mut understanding);

    if etag
        .as_deref()
        .is_some_and(|etag| request_etag_matches(&headers, etag))
    {
        return Ok(Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(header::CACHE_CONTROL, "private, max-age=0, must-revalidate")
            .body(Body::empty())
            .expect("not-modified response should build"));
    }
    let body = serde_json::to_vec(&understanding).map_err(|error| {
        ApiError::internal(
            "dataset_understanding_serialize_failed",
            format!("dataset understanding response serialization failed: {error}"),
        )
    })?;
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::CACHE_CONTROL, "private, max-age=0, must-revalidate");
    if let Some(etag) = etag {
        builder = builder.header(header::ETAG, etag);
    }
    Ok(builder
        .body(Body::from(body))
        .expect("dataset understanding response should build"))
}

fn empty_understanding_contract(dataset: &Dataset) -> DatasetSemanticUnderstanding {
    DatasetSemanticUnderstanding {
        schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
        generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
        status: "empty".to_string(),
        dataset: DatasetSemanticIdentity {
            id: dataset.id,
            title: dataset.title.clone(),
        },
        coverage: SemanticCoverage::default(),
        summary: SemanticSummary {
            headline: "该数据集尚未生成可解释的语义快照。".to_string(),
            limitations: vec!["数据仍可用于现有检索；语义理解将在后台生成后显示。".to_string()],
        },
        objects: Vec::new(),
        fields: Vec::new(),
        relations: Vec::new(),
        source_groups: Vec::new(),
        pipeline: Vec::new(),
        generated_at: Utc::now(),
        stale: false,
        truncated: SemanticTruncation::default(),
    }
}

fn apply_latest_attempt_state(
    understanding: &mut DatasetSemanticUnderstanding,
    latest_attempt: Option<&storage::DatasetSemanticSnapshot>,
) {
    let Some(attempt) = latest_attempt else {
        return;
    };
    if matches!(attempt.status.as_str(), "building" | "failed")
        && attempt.updated_at > understanding.generated_at
    {
        understanding.stale = true;
        let code = attempt
            .failure_code
            .as_deref()
            .map(safe_failure_code)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "rebuild_in_progress".to_string());
        understanding.summary.limitations.push(format!(
            "当前展示上一版结果；最新重建状态为 {}（{}）。",
            attempt.status, code
        ));
    }
}

fn sanitize_understanding_for_public_response(understanding: &mut DatasetSemanticUnderstanding) {
    for field in &mut understanding.fields {
        field.examples = safe_public_examples(&field.examples);
        field.evidence_refs.retain(safe_evidence_ref);
    }
    for object in &mut understanding.objects {
        object.evidence_refs.retain(safe_evidence_ref);
    }
    for relation in &mut understanding.relations {
        relation.evidence_refs.retain(safe_evidence_ref);
    }
    understanding
        .summary
        .limitations
        .retain(|item| !looks_internal(item));
}

fn safe_evidence_ref(reference: &crate::semantic_understanding::SemanticEvidenceRef) -> bool {
    !looks_internal(&reference.source_id) && !looks_internal(&reference.label)
}

fn looks_internal(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    let windows_path = lower.as_bytes().get(1) == Some(&b':')
        && matches!(lower.as_bytes().get(2), Some(b'\\') | Some(b'/'));
    windows_path
        || [
            "/home/",
            "/users/",
            "/root/",
            "/etc/",
            "file://",
            "postgres://",
            "mysql://",
            "jdbc:",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn request_etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|item| item.trim() == etag))
}

fn safe_failure_code(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        .take(64)
        .collect()
}

#[cfg(test)]
fn semantic_dataset_access_allowed(
    dataset: &Dataset,
    request_tenant_id: TenantId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> bool {
    dataset.tenant_id == request_tenant_id
        && dataset_is_visible_for_request(dataset, active_secret_binding_ids, current_user_id, None)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use domain_model::{DatasetLifecycle, DatasetVisibility};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    use super::*;
    use crate::semantic_understanding::{SemanticEvidenceRef, SemanticField, SemanticStatus};

    fn dataset(visibility: DatasetVisibility) -> Dataset {
        Dataset {
            id: DatasetId(Uuid::from_u128(1)),
            tenant_id: TenantId(Uuid::from_u128(2)),
            owner_user_id: None,
            key: "fixture".to_string(),
            title: "数据集".to_string(),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn public_private_secret_and_cross_tenant_access_share_existing_visibility_policy() {
        let tenant_id = TenantId(Uuid::from_u128(2));
        assert!(semantic_dataset_access_allowed(
            &dataset(DatasetVisibility::Public),
            tenant_id,
            &[],
            None
        ));

        let mut private = dataset(DatasetVisibility::Private);
        let owner = UserId(Uuid::from_u128(3));
        private.owner_user_id = Some(owner);
        assert!(semantic_dataset_access_allowed(
            &private,
            tenant_id,
            &[],
            Some(owner)
        ));
        assert!(!semantic_dataset_access_allowed(
            &private,
            tenant_id,
            &[],
            None
        ));

        let secret = SecretBindingId(Uuid::from_u128(4));
        private.owner_user_id = None;
        private.default_secret_binding_ids = vec![secret];
        assert!(semantic_dataset_access_allowed(
            &private,
            tenant_id,
            &[secret],
            None
        ));
        assert!(!semantic_dataset_access_allowed(
            &private,
            TenantId(Uuid::from_u128(99)),
            &[secret],
            None
        ));
    }

    #[test]
    fn no_snapshot_returns_honest_empty_contract() {
        let contract = empty_understanding_contract(&dataset(DatasetVisibility::Public));
        assert_eq!(contract.status, "empty");
        assert!(contract.objects.is_empty());
        assert!(contract.summary.headline.contains("尚未生成"));
    }

    #[test]
    fn response_projection_filters_sensitive_examples_and_internal_paths() {
        let mut contract = empty_understanding_contract(&dataset(DatasetVisibility::Public));
        contract.fields.push(SemanticField {
            id: "field:1".to_string(),
            object_id: "object:1".to_string(),
            label: "示例".to_string(),
            technical_name: "sample".to_string(),
            semantic_role: "text".to_string(),
            value_type: "text".to_string(),
            non_empty_count: 2,
            distinct_count: 2,
            examples: vec!["普通值".to_string(), "person@example.com".to_string()],
            status: SemanticStatus::Observed,
            label_source: "fixture".to_string(),
            confidence: 1.0,
            evidence_refs: vec![SemanticEvidenceRef {
                source_kind: "file".to_string(),
                source_id: "C:\\private\\source.csv".to_string(),
                label: "内部路径".to_string(),
            }],
        });
        sanitize_understanding_for_public_response(&mut contract);
        assert_eq!(contract.fields[0].examples, vec!["普通值"]);
        assert!(contract.fields[0].evidence_refs.is_empty());
    }

    #[test]
    fn etag_and_stale_attempt_are_projected_without_replacing_ready() {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, "\"fingerprint\"".parse().unwrap());
        assert!(request_etag_matches(&headers, "\"fingerprint\""));

        let mut contract = empty_understanding_contract(&dataset(DatasetVisibility::Public));
        contract.status = "ready".to_string();
        contract.generated_at = Utc::now() - Duration::minutes(5);
        let attempt = storage::DatasetSemanticSnapshot {
            id: Uuid::from_u128(8),
            tenant_id: TenantId(Uuid::from_u128(2)),
            dataset_id: DatasetId(Uuid::from_u128(1)),
            schema_version: "1.0.0".to_string(),
            generation_version: "semantic_profile_v1".to_string(),
            source_fingerprint: "x".repeat(64),
            status: "failed".to_string(),
            manifest: serde_json::json!({}),
            source_document_count: 0,
            source_asset_count: 0,
            source_record_count: 0,
            node_count: 0,
            edge_count: 0,
            failure_code: Some("source_parse_failed".to_string()),
            generated_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        apply_latest_attempt_state(&mut contract, Some(&attempt));
        assert!(contract.stale);
        assert_eq!(contract.status, "ready");
        assert!(contract.summary.limitations[1].contains("source_parse_failed"));
    }
}
