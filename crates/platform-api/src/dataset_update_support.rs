use chrono::{DateTime, Utc};
use contracts::{DatasetSummary, UpdateDatasetRequest};
use domain_model::{DatasetId, DatasetLifecycle, SecretBindingId, UserId};
use serde_json::{json, Value};

use crate::{
    dataset_summary_support::dataset_summary, lifecycle_updates::parse_dataset_lifecycle_update,
    load_visible_dataset_for_user, resource_access::ensure_owner_managed_resource,
    text_normalization::trim_optional, validate_required, ApiError, AppState,
};

pub(crate) async fn update_dataset_and_load_summary(
    state: &AppState,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    request: UpdateDatasetRequest,
    updated_at: DateTime<Utc>,
) -> std::result::Result<DatasetSummary, ApiError> {
    let dataset = load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    ensure_owner_managed_resource(
        "dataset",
        dataset.id.to_string(),
        dataset.owner_user_id,
        current_user_id,
    )?;
    let update = dataset_update_fields_from_request(request, updated_at)?;

    let updated = state
        .storage
        .datasets()
        .update_state(
            state.tenant_id,
            dataset_id,
            update.title.as_deref(),
            update.description.as_deref(),
            update.lifecycle,
            &update.metadata_updates,
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok(dataset_summary(updated, None))
}

#[derive(Debug)]
struct DatasetUpdateFields {
    title: Option<String>,
    description: Option<String>,
    lifecycle: Option<DatasetLifecycle>,
    metadata_updates: Value,
}

fn dataset_update_fields_from_request(
    request: UpdateDatasetRequest,
    updated_at: DateTime<Utc>,
) -> std::result::Result<DatasetUpdateFields, ApiError> {
    let title = trim_optional(request.title);
    if let Some(title) = title.as_deref() {
        validate_required("title", title)?;
    }
    let description = trim_optional(request.description);
    let lifecycle = parse_dataset_lifecycle_update(request.lifecycle)?;
    let metadata_updates = if lifecycle == Some(DatasetLifecycle::Archived) {
        json!({ "archived_at": updated_at })
    } else {
        json!({})
    };

    Ok(DatasetUpdateFields {
        title,
        description,
        lifecycle,
        metadata_updates,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, TimeZone, Utc};

    use super::*;

    fn timestamp() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 19, 12, 30, 0)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn dataset_update_support_trims_fields_and_keeps_active_metadata_empty() {
        let fields = dataset_update_fields_from_request(
            UpdateDatasetRequest {
                title: Some(" 新标题 ".to_string()),
                description: Some(" 新描述 ".to_string()),
                lifecycle: Some(" active ".to_string()),
            },
            timestamp(),
        )
        .expect("active dataset update should build");

        assert_eq!(fields.title.as_deref(), Some("新标题"));
        assert_eq!(fields.description.as_deref(), Some("新描述"));
        assert_eq!(fields.lifecycle, Some(DatasetLifecycle::Active));
        assert_eq!(fields.metadata_updates, json!({}));
    }

    #[test]
    fn dataset_update_support_adds_archived_at_for_archive_updates() {
        let updated_at = timestamp();
        let fields = dataset_update_fields_from_request(
            UpdateDatasetRequest {
                title: None,
                description: None,
                lifecycle: Some(" archived ".to_string()),
            },
            updated_at,
        )
        .expect("archived dataset update should build");

        assert_eq!(fields.lifecycle, Some(DatasetLifecycle::Archived));
        assert_eq!(
            fields.metadata_updates["archived_at"],
            updated_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
        );
    }

    #[test]
    fn dataset_update_support_keeps_blank_title_as_noop() {
        let fields = dataset_update_fields_from_request(
            UpdateDatasetRequest {
                title: Some("   ".to_string()),
                description: None,
                lifecycle: None,
            },
            timestamp(),
        )
        .expect("blank title keeps existing trim_optional semantics");

        assert_eq!(fields.title, None);
        assert_eq!(fields.description, None);
        assert_eq!(fields.lifecycle, None);
        assert_eq!(fields.metadata_updates, json!({}));
    }

    #[test]
    fn dataset_update_support_rejects_invalid_lifecycle_with_existing_validation_error() {
        let error = dataset_update_fields_from_request(
            UpdateDatasetRequest {
                title: None,
                description: None,
                lifecycle: Some("deleted".to_string()),
            },
            timestamp(),
        )
        .expect_err("invalid lifecycle should fail");

        assert_eq!(error.payload.code, "validation_error");
        assert!(error.payload.message.contains("lifecycle"));
    }
}
