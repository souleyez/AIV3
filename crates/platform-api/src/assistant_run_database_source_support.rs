use crate::{ApiError, AppState};
use anyhow::Error;
use domain_model::DatasetId;
use sqlx::Row;

pub(crate) const ASSISTANT_RUN_DATABASE_SOURCE_LIMIT: usize = 2;

pub(crate) async fn load_dataset_external_source_ids(
    state: &AppState,
    dataset_id: DatasetId,
) -> std::result::Result<Vec<String>, ApiError> {
    let rows = sqlx::query(
        r#"
        with document_sources as (
            select metadata #>> '{external_source,source_id}' as source_id,
                   count(*)::bigint as document_count,
                   0 as source_priority
            from documents
            where tenant_id = $1
              and dataset_id = $2
              and metadata #>> '{external_source,source_id}' is not null
            group by source_id
        ),
        configured_database_sources as (
            select id as source_id,
                   0::bigint as document_count,
                   1 as source_priority
            from external_source_connections
            where tenant_id = $1
              and disabled_at is null
              and lower(trim(connector_kind)) in ('mysql', 'mysql_source', 'database_source')
              and coalesce(
                    config_redacted #>> '{database_source,default_dataset_id}',
                    config_redacted #>> '{databaseSource,defaultDatasetId}',
                    config_redacted #>> '{mysql_source,default_dataset_id}',
                    config_redacted #>> '{mysqlSource,defaultDatasetId}'
                  ) = $2::text
        ),
        combined_sources as (
            select source_id, document_count, source_priority
            from document_sources
            union all
            select source_id, document_count, source_priority
            from configured_database_sources
        )
        select source_id
        from combined_sources
        where source_id is not null and btrim(source_id) <> ''
        group by source_id
        order by min(source_priority) asc, max(document_count) desc, source_id asc
        limit $3
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(dataset_id.0)
    .bind(ASSISTANT_RUN_DATABASE_SOURCE_LIMIT as i64)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(Error::new(error)))?;

    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<Option<String>, _>("source_id").ok().flatten())
        .map(|source_id| source_id.trim().to_string())
        .filter(|source_id| !source_id.is_empty())
        .collect())
}
