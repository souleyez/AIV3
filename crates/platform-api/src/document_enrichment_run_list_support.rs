use contracts::DocumentEnrichmentRunView;
use domain_model::{DocumentId, SecretBindingId, UserId};
use storage::DocumentEnrichmentRun;

use crate::{
    document_view_support::to_document_enrichment_run_view,
    load_visible_document_for_user_with_local_scope, ApiError, AppState,
};

const DEFAULT_DOCUMENT_ENRICHMENT_RUN_LIMIT: i64 = 50;
const MIN_DOCUMENT_ENRICHMENT_RUN_LIMIT: i64 = 1;
const MAX_DOCUMENT_ENRICHMENT_RUN_LIMIT: i64 = 200;

pub(crate) async fn list_document_enrichment_run_views_for_user(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
    requested_limit: Option<i64>,
) -> std::result::Result<Vec<DocumentEnrichmentRunView>, ApiError> {
    load_visible_document_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await?;

    let runs = state
        .storage
        .document_enrichment_runs()
        .list_by_document(
            state.tenant_id,
            document_id,
            document_enrichment_run_list_limit(requested_limit),
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok(document_enrichment_run_views(runs))
}

fn document_enrichment_run_list_limit(requested_limit: Option<i64>) -> i64 {
    requested_limit
        .unwrap_or(DEFAULT_DOCUMENT_ENRICHMENT_RUN_LIMIT)
        .clamp(
            MIN_DOCUMENT_ENRICHMENT_RUN_LIMIT,
            MAX_DOCUMENT_ENRICHMENT_RUN_LIMIT,
        )
}

fn document_enrichment_run_views(
    runs: Vec<DocumentEnrichmentRun>,
) -> Vec<DocumentEnrichmentRunView> {
    runs.into_iter()
        .map(to_document_enrichment_run_view)
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DocumentId, TenantId};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn enrichment_run(document_id: DocumentId, kind: &str) -> DocumentEnrichmentRun {
        let now = Utc::now();
        DocumentEnrichmentRun {
            id: Uuid::new_v4(),
            tenant_id: TenantId::new(),
            document_id,
            enrichment_kind: kind.to_string(),
            parse_version: Some("parse-v1".to_string()),
            input_fingerprint: format!("fingerprint-{kind}"),
            status: "completed".to_string(),
            priority: 50,
            attempt_count: 1,
            max_attempts: 3,
            available_at: now,
            started_at: Some(now),
            finished_at: Some(now),
            error_message: None,
            output_summary: json!({ "kind": kind }),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn document_enrichment_run_limit_keeps_existing_default_and_clamp() {
        assert_eq!(document_enrichment_run_list_limit(None), 50);
        assert_eq!(document_enrichment_run_list_limit(Some(0)), 1);
        assert_eq!(document_enrichment_run_list_limit(Some(5)), 5);
        assert_eq!(document_enrichment_run_list_limit(Some(500)), 200);
    }

    #[test]
    fn document_enrichment_run_views_preserve_order_and_ids() {
        let document_id = DocumentId::new();
        let first = enrichment_run(document_id, "outline");
        let first_id = first.id.to_string();
        let second = enrichment_run(document_id, "facts");
        let second_id = second.id.to_string();

        let views = document_enrichment_run_views(vec![first, second]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, first_id);
        assert_eq!(views[0].enrichment_kind, "outline");
        assert_eq!(views[1].id, second_id);
        assert_eq!(views[1].enrichment_kind, "facts");
    }
}
