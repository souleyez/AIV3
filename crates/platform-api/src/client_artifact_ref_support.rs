use axum::http::HeaderMap;
use contracts::ClientArtifactView;
use domain_model::UserId;

use crate::{
    client_artifact_contract_support::validate_v3_client_ref_list,
    client_artifact_error_support::client_artifact_not_found_error,
    client_artifact_scope_support::validate_v3_client_scope_refs,
    client_artifact_view_support::load_client_artifact_view, ApiError, AppState,
};

pub(crate) fn client_artifact_ref_update_sql(
    field: &'static str,
) -> std::result::Result<&'static str, ApiError> {
    match field {
        "dataset_ids" => Ok(r#"
            update v3_client_artifacts
            set dataset_ids = case
                    when $3 = any(dataset_ids) then dataset_ids
                    else array_append(dataset_ids, $3)
                end,
                updated_at = now()
            where tenant_id = $1 and artifact_id = $2
            "#),
        "asset_library_ids" => Ok(r#"
            update v3_client_artifacts
            set asset_library_ids = case
                    when $3 = any(asset_library_ids) then asset_library_ids
                    else array_append(asset_library_ids, $3)
                end,
                updated_at = now()
            where tenant_id = $1 and artifact_id = $2
            "#),
        _ => Err(ApiError::internal(
            "invalid_client_artifact_ref_field",
            format!("unsupported client artifact ref field {field}"),
        )),
    }
}

pub(crate) async fn append_client_artifact_ref(
    state: &AppState,
    artifact_id: &str,
    field: &'static str,
    value: &str,
) -> std::result::Result<(), ApiError> {
    let sql = client_artifact_ref_update_sql(field)?;
    let result = sqlx::query(sql)
        .bind(state.tenant_id.0)
        .bind(artifact_id)
        .bind(value)
        .execute(state.storage.pool())
        .await
        .map_err(|error| ApiError::from_storage(error.into()))?;
    if result.rows_affected() == 0 {
        return Err(client_artifact_not_found_error(artifact_id));
    }
    Ok(())
}

pub(crate) fn client_artifact_ref_scope_values(
    field: &'static str,
    value: &str,
) -> std::result::Result<(Vec<String>, Vec<String>), ApiError> {
    client_artifact_ref_update_sql(field)?;
    let value = value.trim().to_string();
    Ok(match field {
        "dataset_ids" => (vec![value], Vec::new()),
        "asset_library_ids" => (Vec::new(), vec![value]),
        _ => unreachable!("validated by client_artifact_ref_update_sql"),
    })
}

pub(crate) async fn attach_client_artifact_ref_and_load_view(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    artifact_id: &str,
    field: &'static str,
    value: &str,
) -> std::result::Result<ClientArtifactView, ApiError> {
    let value = value.trim().to_string();
    validate_v3_client_ref_list(field, std::slice::from_ref(&value))?;
    let (dataset_ids, asset_library_ids) = client_artifact_ref_scope_values(field, &value)?;
    validate_v3_client_scope_refs(
        state,
        headers,
        Some(current_user_id),
        &dataset_ids,
        &asset_library_ids,
    )
    .await?;
    append_client_artifact_ref(state, artifact_id, field, &value).await?;
    load_client_artifact_view(state, artifact_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_artifact_ref_support_returns_dataset_ref_update_sql() {
        let sql = client_artifact_ref_update_sql("dataset_ids")
            .expect("dataset refs should be supported");

        assert!(sql.contains("set dataset_ids = case"));
        assert!(sql.contains("array_append(dataset_ids, $3)"));
        assert!(sql.contains("where tenant_id = $1 and artifact_id = $2"));
    }

    #[test]
    fn client_artifact_ref_support_returns_asset_library_ref_update_sql() {
        let sql = client_artifact_ref_update_sql("asset_library_ids")
            .expect("asset library refs should be supported");

        assert!(sql.contains("set asset_library_ids = case"));
        assert!(sql.contains("array_append(asset_library_ids, $3)"));
        assert!(sql.contains("where tenant_id = $1 and artifact_id = $2"));
    }

    #[test]
    fn client_artifact_ref_support_rejects_unknown_fields() {
        let error =
            client_artifact_ref_update_sql("task_id").expect_err("unknown field should fail");

        assert_eq!(error.payload.code, "invalid_client_artifact_ref_field");
        assert!(error.payload.message.contains("task_id"));
    }

    #[test]
    fn client_artifact_ref_support_routes_dataset_scope_values() {
        let (dataset_ids, asset_library_ids) =
            client_artifact_ref_scope_values("dataset_ids", " dataset-1 ")
                .expect("dataset refs should route");

        assert_eq!(dataset_ids, vec!["dataset-1".to_string()]);
        assert!(asset_library_ids.is_empty());
    }

    #[test]
    fn client_artifact_ref_support_routes_asset_library_scope_values() {
        let (dataset_ids, asset_library_ids) =
            client_artifact_ref_scope_values("asset_library_ids", " asset-1 ")
                .expect("asset library refs should route");

        assert!(dataset_ids.is_empty());
        assert_eq!(asset_library_ids, vec!["asset-1".to_string()]);
    }
}
