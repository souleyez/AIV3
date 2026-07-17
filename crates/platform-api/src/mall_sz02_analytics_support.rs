use std::collections::{BTreeMap, BTreeSet};

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;

use crate::{ApiError, AppState};

const PUBLIC_MALL_ALLOWLIST_ENV: &str = "PLATFORM_PUBLIC_MALL_ANALYTICS_IDS";
const PUBLIC_MALL_TRAFFIC_DATASETS_ENV: &str = "PLATFORM_PUBLIC_MALL_TRAFFIC_DATASETS";
const PUBLIC_MALL_PROFILE_DATASETS_ENV: &str = "PLATFORM_PUBLIC_MALL_PROFILE_DATASETS";
const TRAFFIC_SOURCE_KIND: &str = "aibee_traffic_hourly";
const PROFILE_SOURCE_KIND: &str = "aibee_profile_aggregate";
const MAX_PUBLIC_ANALYTICS_ROWS: i64 = 120_000;
const TRAFFIC_COLUMNS: [&str; 7] = [
    "dayIndex",
    "hour",
    "entityIndex",
    "trafficIn",
    "trafficOut",
    "visitors",
    "averageStay",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MallTrafficReportResponse {
    schema_version: &'static str,
    title: &'static str,
    dataset_title: &'static str,
    generated_at: String,
    source: MallTrafficSourceView,
    range: MallTrafficRangeView,
    columns: Vec<&'static str>,
    entities: Vec<MallTrafficEntityView>,
    rows: Vec<Vec<Value>>,
    types: BTreeMap<String, MallTrafficTypeStatisticsView>,
    quality: MallTrafficQualityView,
    reference: MallTrafficReferenceView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficSourceView {
    provider: &'static str,
    endpoint: &'static str,
    mall_id: String,
    interval: &'static str,
    row_count: usize,
    token_included: bool,
    storage: &'static str,
    relation: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficRangeView {
    start: String,
    end: String,
    days: Vec<String>,
    hours: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficEntityView {
    key: String,
    #[serde(rename = "type")]
    entity_type: String,
    type_label: String,
    name: String,
    entity_id: String,
    aibee_entity_id: String,
    floor: String,
    floor_name: String,
    area: String,
    l1_retail_format: String,
    l2_retail_format: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficTypeStatisticsView {
    label: String,
    rows: usize,
    entities: usize,
    nonzero_rows: MallTrafficMetricCountsView,
    coverage: MallTrafficMetricCoverageView,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficMetricCountsView {
    traffic_in: usize,
    traffic_out: usize,
    visitors: usize,
    average_stay: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficMetricCoverageView {
    traffic_in: f64,
    traffic_out: f64,
    visitors: f64,
    average_stay: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficQualityView {
    duplicate_key_count: usize,
    primary_hourly_metrics: [&'static str; 2],
    visitors_policy: &'static str,
    average_stay_policy: &'static str,
    cross_level_policy: &'static str,
    camera_policy: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallTrafficReferenceView {
    mall_traffic_in: i64,
    mall_traffic_out: i64,
    mall_peak_day: String,
    mall_peak_day_traffic_in: i64,
    active_entities: usize,
}

#[derive(Clone, Debug)]
struct TrafficFactRow {
    business_date: String,
    hour: i32,
    entity_type: String,
    entity_type_name: String,
    entity_name: String,
    source_entity_id: String,
    aibee_entity_id: String,
    floor_code: String,
    floor_name: String,
    area: String,
    l1_retail_format: String,
    l2_retail_format: String,
    traffic_in: i64,
    traffic_out: i64,
    visitors: i64,
    average_stay: f64,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MallProfileSummaryResponse {
    schema_version: &'static str,
    mall_id: String,
    generated_at: String,
    storage: MallProfileStorageView,
    privacy: MallProfilePrivacyView,
    daily_summary: Vec<MallProfileDailySummaryView>,
    daily_distributions: Vec<MallProfileDailyDistributionView>,
    period_distributions: Vec<MallProfilePeriodDistributionView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallProfileStorageView {
    engine: &'static str,
    schema: &'static str,
    relations: MallProfileRelationsView,
    tenant_scoped: bool,
    mall_scoped: bool,
    row_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallProfileRelationsView {
    daily_summary: &'static str,
    daily_distributions: &'static str,
    period_distributions: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallProfilePrivacyView {
    personal_identifiers_included: bool,
    aggregation_level: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallProfileDailySummaryView {
    business_date: String,
    record_count: i64,
    unique_visitor_count: i64,
    duplicate_record_count: i64,
    missing_identifier_count: i64,
    date_mismatch_count: i64,
    invalid_age_count: i64,
    #[serde(skip)]
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallProfileDailyDistributionView {
    business_date: String,
    dimension_key: String,
    bucket_key: String,
    visitor_count: i64,
    share: f64,
    #[serde(skip)]
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MallProfilePeriodDistributionView {
    period_start: String,
    period_end: String,
    dimension_key: String,
    bucket_key: String,
    visitor_count: i64,
    share: f64,
    #[serde(skip)]
    updated_at: DateTime<Utc>,
}

pub(crate) async fn get_public_mall_traffic_report_data(
    State(state): State<AppState>,
    Path(mall_id): Path<String>,
) -> std::result::Result<Json<MallTrafficReportResponse>, ApiError> {
    let mall_id = authorize_public_mall_id(&mall_id)?;
    let dataset_id = configured_public_dataset_id(PUBLIC_MALL_TRAFFIC_DATASETS_ENV, &mall_id)?;
    let batch_id = latest_ready_batch_id(&state, &mall_id, dataset_id, TRAFFIC_SOURCE_KIND)
        .await?
        .ok_or_else(|| {
            ApiError::not_found(
                "mall_traffic_not_found",
                format!("no public hourly traffic data are available for mall {mall_id}"),
            )
        })?;
    let row_count = traffic_row_count(&state, &mall_id, dataset_id, batch_id).await?;
    reject_row_limit("mall_traffic_row_limit_exceeded", row_count)?;
    if row_count == 0 {
        return Err(ApiError::not_found(
            "mall_traffic_not_found",
            format!("no public hourly traffic data are available for mall {mall_id}"),
        ));
    }

    let rows = load_traffic_rows(&state, &mall_id, dataset_id, batch_id).await?;
    reject_row_limit("mall_traffic_row_limit_exceeded", rows.len() as i64)?;
    if rows.is_empty() {
        return Err(ApiError::not_found(
            "mall_traffic_not_found",
            format!("no public hourly traffic data are available for mall {mall_id}"),
        ));
    }
    Ok(Json(build_traffic_report_response(mall_id, rows)?))
}

pub(crate) async fn get_public_mall_profile_summary(
    State(state): State<AppState>,
    Path(mall_id): Path<String>,
) -> std::result::Result<Json<MallProfileSummaryResponse>, ApiError> {
    let mall_id = authorize_public_mall_id(&mall_id)?;
    let dataset_id = configured_public_dataset_id(PUBLIC_MALL_PROFILE_DATASETS_ENV, &mall_id)?;
    let batch_id = latest_ready_batch_id(&state, &mall_id, dataset_id, PROFILE_SOURCE_KIND)
        .await?
        .ok_or_else(|| {
            ApiError::not_found(
                "mall_profile_not_found",
                format!("no public aggregate profile data are available for mall {mall_id}"),
            )
        })?;
    let daily_summary = load_profile_daily_summary(&state, &mall_id, dataset_id, batch_id).await?;
    let daily_distributions =
        load_profile_daily_distributions(&state, &mall_id, dataset_id, batch_id).await?;
    let period_distributions =
        load_profile_period_distributions(&state, &mall_id, dataset_id, batch_id).await?;
    let row_count = daily_summary
        .len()
        .saturating_add(daily_distributions.len())
        .saturating_add(period_distributions.len());
    reject_row_limit("mall_profile_row_limit_exceeded", row_count as i64)?;
    if row_count == 0 {
        return Err(ApiError::not_found(
            "mall_profile_not_found",
            format!("no public aggregate profile data are available for mall {mall_id}"),
        ));
    }

    let generated_at = daily_summary
        .iter()
        .map(|row| row.updated_at)
        .chain(daily_distributions.iter().map(|row| row.updated_at))
        .chain(period_distributions.iter().map(|row| row.updated_at))
        .max()
        .unwrap_or_else(Utc::now);

    Ok(Json(MallProfileSummaryResponse {
        schema_version: "1.0",
        mall_id,
        generated_at: generated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        storage: MallProfileStorageView {
            engine: "postgresql",
            schema: "mall_sz02",
            relations: MallProfileRelationsView {
                daily_summary: "mall_sz02.profile_daily_summary",
                daily_distributions: "mall_sz02.profile_distribution_daily",
                period_distributions: "mall_sz02.profile_distribution_period",
            },
            tenant_scoped: true,
            mall_scoped: true,
            row_count,
        },
        privacy: MallProfilePrivacyView {
            personal_identifiers_included: false,
            aggregation_level: "daily_or_period_aggregate",
        },
        daily_summary,
        daily_distributions,
        period_distributions,
    }))
}

fn authorize_public_mall_id(raw_mall_id: &str) -> std::result::Result<String, ApiError> {
    let mall_id = normalize_mall_id(raw_mall_id).ok_or_else(|| {
        ApiError::bad_request(
            "invalid_mall_id",
            "mall_id must contain 1 to 32 ASCII letters, digits, hyphens, or underscores"
                .to_string(),
        )
    })?;
    let allowlist = std::env::var(PUBLIC_MALL_ALLOWLIST_ENV).ok();
    if !public_mall_allowlist_contains(allowlist.as_deref(), &mall_id) {
        return Err(ApiError::forbidden(
            "public_mall_analytics_not_allowed",
            "mall analytics are not publicly available".to_string(),
        ));
    }
    Ok(mall_id)
}

fn configured_public_dataset_id(
    env_name: &str,
    mall_id: &str,
) -> std::result::Result<Uuid, ApiError> {
    let mapping = match std::env::var(env_name) {
        Ok(mapping) => mapping,
        Err(std::env::VarError::NotPresent) => {
            return Err(ApiError::not_found(
                "public_mall_dataset_not_configured",
                "mall analytics are not publicly available".to_string(),
            ));
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(ApiError::service_unavailable(
                "public_mall_dataset_config_invalid",
                "public mall dataset configuration is invalid".to_string(),
            ));
        }
    };
    match parse_public_dataset_mapping(Some(&mapping), mall_id) {
        Ok(Some(dataset_id)) => Ok(dataset_id),
        Ok(None) => Err(ApiError::not_found(
            "public_mall_dataset_not_configured",
            "mall analytics are not publicly available".to_string(),
        )),
        Err(message) => {
            tracing::error!(
                env_name,
                error = message,
                "invalid public mall dataset mapping"
            );
            Err(ApiError::service_unavailable(
                "public_mall_dataset_config_invalid",
                "public mall dataset configuration is invalid".to_string(),
            ))
        }
    }
}

fn parse_public_dataset_mapping(
    raw_mapping: Option<&str>,
    mall_id: &str,
) -> std::result::Result<Option<Uuid>, &'static str> {
    let target_mall_id = normalize_mall_id(mall_id).ok_or("invalid target mall id")?;
    let mut mappings = BTreeMap::<String, Uuid>::new();
    for entry in raw_mapping.unwrap_or_default().split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (raw_mall_id, raw_dataset_id) = entry
            .split_once('=')
            .ok_or("mapping entry must use MALL_ID=DATASET_UUID")?;
        if raw_dataset_id.contains('=') {
            return Err("mapping entry contains more than one equals sign");
        }
        let mapped_mall_id = normalize_mall_id(raw_mall_id).ok_or("invalid mapped mall id")?;
        let dataset_id =
            Uuid::parse_str(raw_dataset_id.trim()).map_err(|_| "invalid dataset uuid")?;
        if mappings.insert(mapped_mall_id, dataset_id).is_some() {
            return Err("duplicate mall dataset mapping");
        }
    }
    Ok(mappings.get(&target_mall_id).copied())
}

async fn latest_ready_batch_id(
    state: &AppState,
    mall_id: &str,
    dataset_id: Uuid,
    source_kind: &str,
) -> std::result::Result<Option<Uuid>, ApiError> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        select b.id
        from mall_sz02.ingest_batch b
        join public.datasets d
          on d.tenant_id = b.tenant_id and d.id = b.dataset_id
        where b.tenant_id = $1
          and b.dataset_id = $2
          and upper(btrim(b.mall_code)) = $3
          and b.source_kind = $4
          and b.status = 'ready'
          and d.lifecycle = 'active'
          and coalesce(d.metadata ->> 'visibility', 'public') = 'public'
          and d.owner_user_id is null
          and coalesce(d.metadata -> 'local_only', 'false'::jsonb) <> 'true'::jsonb
        order by b.completed_at desc nulls last, b.created_at desc, b.id desc
        limit 1
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(dataset_id)
    .bind(mall_id)
    .bind(source_kind)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))
}

fn normalize_mall_id(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_ascii_uppercase();
    (!normalized.is_empty()
        && normalized.len() <= 32
        && normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
    .then_some(normalized)
}

fn public_mall_allowlist_contains(raw_allowlist: Option<&str>, mall_id: &str) -> bool {
    let Some(mall_id) = normalize_mall_id(mall_id) else {
        return false;
    };
    raw_allowlist
        .unwrap_or_default()
        .split(',')
        .filter_map(normalize_mall_id)
        .any(|candidate| candidate == mall_id)
}

fn reject_row_limit(code: &str, row_count: i64) -> std::result::Result<(), ApiError> {
    if row_count <= MAX_PUBLIC_ANALYTICS_ROWS {
        return Ok(());
    }
    Err(ApiError::bad_request_with_details(
        code,
        format!(
            "public analytics row count {row_count} exceeds the limit {MAX_PUBLIC_ANALYTICS_ROWS}"
        ),
        json!({
            "rowCount": row_count,
            "maxRows": MAX_PUBLIC_ANALYTICS_ROWS,
        }),
    ))
}

async fn traffic_row_count(
    state: &AppState,
    mall_id: &str,
    dataset_id: Uuid,
    batch_id: Uuid,
) -> std::result::Result<i64, ApiError> {
    sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)::bigint
        from mall_sz02.traffic_hourly_fact
        where tenant_id = $1
          and upper(btrim(mall_code)) = $2
          and dataset_id = $3
          and batch_id = $4
          and upper(btrim(interval_code)) = 'H'
          and minute = 0
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(mall_id)
    .bind(dataset_id)
    .bind(batch_id)
    .fetch_one(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))
}

async fn load_traffic_rows(
    state: &AppState,
    mall_id: &str,
    dataset_id: Uuid,
    batch_id: Uuid,
) -> std::result::Result<Vec<TrafficFactRow>, ApiError> {
    let rows = sqlx::query(
        r#"
        select f.business_date::text as business_date,
               f.hour::integer as hour,
               f.entity_type::text as entity_type,
               coalesce(nullif(btrim(p.entity_type_name), ''),
                        nullif(btrim(f.entity_type_name), ''),
                        f.entity_type::text) as entity_type_name,
               coalesce(nullif(btrim(p.entity_name), ''), f.entity_name) as entity_name,
               coalesce(nullif(btrim(p.source_entity_id), ''), f.source_entity_id) as source_entity_id,
               f.aibee_entity_id,
               coalesce(p.floor_code, '') as floor_code,
               coalesce(p.floor_name, '') as floor_name,
               coalesce(p.area::text, '') as area,
               coalesce(p.l1_retail_format, '') as l1_retail_format,
               coalesce(p.l2_retail_format, '') as l2_retail_format,
               f.traffic_in::bigint as traffic_in,
               f.traffic_out::bigint as traffic_out,
               f.visitors::bigint as visitors,
               f.average_stay::double precision as average_stay,
               f.updated_at
        from mall_sz02.traffic_hourly_fact f
        left join mall_sz02.traffic_point_dim p
          on p.tenant_id = f.tenant_id
         and p.dataset_id = f.dataset_id
         and p.mall_code = f.mall_code
         and p.entity_type = f.entity_type
         and p.aibee_entity_id = f.aibee_entity_id
        where f.tenant_id = $1
          and upper(btrim(f.mall_code)) = $2
          and f.dataset_id = $3
          and f.batch_id = $4
          and upper(btrim(f.interval_code)) = 'H'
          and f.minute = 0
        order by f.business_date asc, f.hour asc, f.entity_type asc, f.aibee_entity_id asc
        limit $5
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(mall_id)
    .bind(dataset_id)
    .bind(batch_id)
    .bind(MAX_PUBLIC_ANALYTICS_ROWS + 1)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?;

    rows.into_iter()
        .map(traffic_fact_from_row)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))
}

fn traffic_fact_from_row(row: PgRow) -> std::result::Result<TrafficFactRow, sqlx::Error> {
    Ok(TrafficFactRow {
        business_date: row.try_get("business_date")?,
        hour: row.try_get("hour")?,
        entity_type: row.try_get("entity_type")?,
        entity_type_name: row.try_get("entity_type_name")?,
        entity_name: row.try_get("entity_name")?,
        source_entity_id: row.try_get("source_entity_id")?,
        aibee_entity_id: row.try_get("aibee_entity_id")?,
        floor_code: row.try_get("floor_code")?,
        floor_name: row.try_get("floor_name")?,
        area: row.try_get("area")?,
        l1_retail_format: row.try_get("l1_retail_format")?,
        l2_retail_format: row.try_get("l2_retail_format")?,
        traffic_in: row.try_get("traffic_in")?,
        traffic_out: row.try_get("traffic_out")?,
        visitors: row.try_get("visitors")?,
        average_stay: row.try_get("average_stay")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn build_traffic_report_response(
    mall_id: String,
    facts: Vec<TrafficFactRow>,
) -> std::result::Result<MallTrafficReportResponse, ApiError> {
    let days = facts
        .iter()
        .map(|row| row.business_date.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let hours = facts
        .iter()
        .map(|row| row.hour)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let day_indexes = days
        .iter()
        .enumerate()
        .map(|(index, day)| (day.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let mut entities_by_key = BTreeMap::new();
    for fact in &facts {
        let key = traffic_entity_key(&fact.entity_type, &fact.aibee_entity_id);
        if fact.aibee_entity_id.trim().is_empty() {
            return Err(ApiError::internal(
                "mall_traffic_invalid_entity",
                "hourly traffic data contain an empty entity identifier".to_string(),
            ));
        }
        entities_by_key
            .entry(key.clone())
            .or_insert_with(|| MallTrafficEntityView {
                key,
                entity_type: fact.entity_type.clone(),
                type_label: fact.entity_type_name.clone(),
                name: fact.entity_name.clone(),
                entity_id: fact.source_entity_id.clone(),
                aibee_entity_id: fact.aibee_entity_id.clone(),
                floor: fact.floor_code.clone(),
                floor_name: fact.floor_name.clone(),
                area: fact.area.clone(),
                l1_retail_format: fact.l1_retail_format.clone(),
                l2_retail_format: fact.l2_retail_format.clone(),
            });
    }
    let entities = entities_by_key.into_values().collect::<Vec<_>>();
    let entity_indexes = entities
        .iter()
        .enumerate()
        .map(|(index, entity)| (entity.key.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let mut rows = Vec::with_capacity(facts.len());
    for fact in &facts {
        let Some(day_index) = day_indexes.get(&fact.business_date) else {
            continue;
        };
        let key = traffic_entity_key(&fact.entity_type, &fact.aibee_entity_id);
        let Some(entity_index) = entity_indexes.get(&key) else {
            continue;
        };
        rows.push(vec![
            json!(*day_index),
            json!(fact.hour),
            json!(entity_index),
            json!(fact.traffic_in),
            json!(fact.traffic_out),
            json!(fact.visitors),
            json!(fact.average_stay),
        ]);
    }

    let generated_at = facts
        .iter()
        .map(|row| row.updated_at)
        .max()
        .unwrap_or_else(Utc::now);
    let types = build_traffic_type_statistics(&facts);
    let reference = build_traffic_reference(&facts, entities.len());

    Ok(MallTrafficReportResponse {
        schema_version: "1.0",
        title: "某商场 · 7月客流运营驾驶舱",
        dataset_title: "7月客流数据集",
        generated_at: generated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        source: MallTrafficSourceView {
            provider: "AIBeeMall",
            endpoint: "/traffic_summary",
            mall_id,
            interval: "H",
            row_count: rows.len(),
            token_included: false,
            storage: "postgresql",
            relation: "mall_sz02.traffic_hourly_fact",
        },
        range: MallTrafficRangeView {
            start: days.first().cloned().unwrap_or_default(),
            end: days.last().cloned().unwrap_or_default(),
            days,
            hours,
        },
        columns: TRAFFIC_COLUMNS.to_vec(),
        entities,
        rows,
        types,
        quality: build_traffic_quality(),
        reference,
    })
}

fn traffic_entity_key(entity_type: &str, aibee_entity_id: &str) -> String {
    format!("{}:{}", entity_type.trim(), aibee_entity_id.trim())
}

fn build_traffic_type_statistics(
    facts: &[TrafficFactRow],
) -> BTreeMap<String, MallTrafficTypeStatisticsView> {
    #[derive(Default)]
    struct Accumulator {
        label: String,
        rows: usize,
        entities: BTreeSet<String>,
        nonzero: MallTrafficMetricCountsView,
    }

    let mut accumulators = BTreeMap::<String, Accumulator>::new();
    for fact in facts {
        let accumulator = accumulators.entry(fact.entity_type.clone()).or_default();
        if accumulator.label.is_empty() {
            accumulator.label = fact.entity_type_name.clone();
        }
        accumulator.rows = accumulator.rows.saturating_add(1);
        accumulator
            .entities
            .insert(traffic_entity_key(&fact.entity_type, &fact.aibee_entity_id));
        accumulator.nonzero.traffic_in += usize::from(fact.traffic_in > 0);
        accumulator.nonzero.traffic_out += usize::from(fact.traffic_out > 0);
        accumulator.nonzero.visitors += usize::from(fact.visitors > 0);
        accumulator.nonzero.average_stay += usize::from(fact.average_stay > 0.0);
    }

    accumulators
        .into_iter()
        .map(|(entity_type, accumulator)| {
            let rows = accumulator.rows;
            let coverage = MallTrafficMetricCoverageView {
                traffic_in: rounded_coverage(accumulator.nonzero.traffic_in, rows),
                traffic_out: rounded_coverage(accumulator.nonzero.traffic_out, rows),
                visitors: rounded_coverage(accumulator.nonzero.visitors, rows),
                average_stay: rounded_coverage(accumulator.nonzero.average_stay, rows),
            };
            (
                entity_type,
                MallTrafficTypeStatisticsView {
                    label: accumulator.label,
                    rows,
                    entities: accumulator.entities.len(),
                    nonzero_rows: accumulator.nonzero,
                    coverage,
                },
            )
        })
        .collect()
}

fn rounded_coverage(nonzero_rows: usize, rows: usize) -> f64 {
    if rows == 0 {
        return 0.0;
    }
    ((nonzero_rows as f64 / rows as f64) * 1_000_000.0).round() / 1_000_000.0
}

fn build_traffic_quality() -> MallTrafficQualityView {
    MallTrafficQualityView {
        duplicate_key_count: 0,
        primary_hourly_metrics: ["trafficIn", "trafficOut"],
        visitors_policy: "仅在覆盖完整的空间层级用于小时分析；其他层级显示为口径不可用。",
        average_stay_policy: "当前小时数据均为 0，按缺失口径处理，不解释为真实停留时长。",
        cross_level_policy: "不同空间层级不可相加解释为全场客流；页面每次仅聚合一个层级。",
        camera_policy: "点位为业务空间实体，不等同于单摄像头。",
    }
}

fn build_traffic_reference(
    facts: &[TrafficFactRow],
    active_entities: usize,
) -> MallTrafficReferenceView {
    let mut mall_traffic_in = 0i64;
    let mut mall_traffic_out = 0i64;
    let mut daily_traffic_in = BTreeMap::<String, i64>::new();
    for fact in facts.iter().filter(|row| row.entity_type == "20") {
        mall_traffic_in = mall_traffic_in.saturating_add(fact.traffic_in);
        mall_traffic_out = mall_traffic_out.saturating_add(fact.traffic_out);
        let total = daily_traffic_in
            .entry(fact.business_date.clone())
            .or_default();
        *total = total.saturating_add(fact.traffic_in);
    }
    let (mall_peak_day, mall_peak_day_traffic_in) = daily_traffic_in
        .into_iter()
        .max_by(|(left_day, left_value), (right_day, right_value)| {
            left_value
                .cmp(right_value)
                .then_with(|| right_day.cmp(left_day))
        })
        .unwrap_or_default();

    MallTrafficReferenceView {
        mall_traffic_in,
        mall_traffic_out,
        mall_peak_day,
        mall_peak_day_traffic_in,
        active_entities,
    }
}

async fn load_profile_daily_summary(
    state: &AppState,
    mall_id: &str,
    dataset_id: Uuid,
    batch_id: Uuid,
) -> std::result::Result<Vec<MallProfileDailySummaryView>, ApiError> {
    let rows = sqlx::query(
        r#"
        select business_date::text as business_date,
               record_count::bigint as record_count,
               unique_pid_count::bigint as unique_visitor_count,
               duplicate_record_count::bigint as duplicate_record_count,
               missing_pid_count::bigint as missing_identifier_count,
               date_mismatch_count::bigint as date_mismatch_count,
               invalid_age_count::bigint as invalid_age_count,
               updated_at
        from mall_sz02.profile_daily_summary
        where tenant_id = $1
          and upper(btrim(mall_code)) = $2
          and dataset_id = $3
          and batch_id = $4
        order by business_date asc
        limit $5
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(mall_id)
    .bind(dataset_id)
    .bind(batch_id)
    .bind(MAX_PUBLIC_ANALYTICS_ROWS + 1)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?;
    rows.into_iter()
        .map(|row| {
            Ok(MallProfileDailySummaryView {
                business_date: row.try_get("business_date")?,
                record_count: row.try_get("record_count")?,
                unique_visitor_count: row.try_get("unique_visitor_count")?,
                duplicate_record_count: row.try_get("duplicate_record_count")?,
                missing_identifier_count: row.try_get("missing_identifier_count")?,
                date_mismatch_count: row.try_get("date_mismatch_count")?,
                invalid_age_count: row.try_get("invalid_age_count")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<std::result::Result<Vec<_>, sqlx::Error>>()
        .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))
}

async fn load_profile_daily_distributions(
    state: &AppState,
    mall_id: &str,
    dataset_id: Uuid,
    batch_id: Uuid,
) -> std::result::Result<Vec<MallProfileDailyDistributionView>, ApiError> {
    let rows = sqlx::query(
        r#"
        select business_date::text as business_date,
               dimension_key,
               bucket_key,
               visitor_count::bigint as visitor_count,
               share::double precision as share,
               updated_at
        from mall_sz02.profile_distribution_daily
        where tenant_id = $1
          and upper(btrim(mall_code)) = $2
          and dataset_id = $3
          and batch_id = $4
        order by business_date asc, dimension_key asc, bucket_key asc
        limit $5
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(mall_id)
    .bind(dataset_id)
    .bind(batch_id)
    .bind(MAX_PUBLIC_ANALYTICS_ROWS + 1)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?;
    rows.into_iter()
        .map(|row| {
            Ok(MallProfileDailyDistributionView {
                business_date: row.try_get("business_date")?,
                dimension_key: row.try_get("dimension_key")?,
                bucket_key: row.try_get("bucket_key")?,
                visitor_count: row.try_get("visitor_count")?,
                share: row.try_get("share")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<std::result::Result<Vec<_>, sqlx::Error>>()
        .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))
}

async fn load_profile_period_distributions(
    state: &AppState,
    mall_id: &str,
    dataset_id: Uuid,
    batch_id: Uuid,
) -> std::result::Result<Vec<MallProfilePeriodDistributionView>, ApiError> {
    let rows = sqlx::query(
        r#"
        select period_start_date::text as period_start,
               period_end_date::text as period_end,
               dimension_key,
               bucket_key,
               visitor_count::bigint as visitor_count,
               share::double precision as share,
               updated_at
        from mall_sz02.profile_distribution_period
        where tenant_id = $1
          and upper(btrim(mall_code)) = $2
          and dataset_id = $3
          and batch_id = $4
        order by period_start_date asc, period_end_date asc, dimension_key asc, bucket_key asc
        limit $5
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(mall_id)
    .bind(dataset_id)
    .bind(batch_id)
    .bind(MAX_PUBLIC_ANALYTICS_ROWS + 1)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?;
    rows.into_iter()
        .map(|row| {
            Ok(MallProfilePeriodDistributionView {
                period_start: row.try_get("period_start")?,
                period_end: row.try_get("period_end")?,
                dimension_key: row.try_get("dimension_key")?,
                bucket_key: row.try_get("bucket_key")?,
                visitor_count: row.try_get("visitor_count")?,
                share: row.try_get("share")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<std::result::Result<Vec<_>, sqlx::Error>>()
        .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn traffic_fact(
        entity_type: &str,
        entity_id: &str,
        day: &str,
        traffic_in: i64,
        traffic_out: i64,
        visitors: i64,
        average_stay: f64,
    ) -> TrafficFactRow {
        TrafficFactRow {
            business_date: day.to_string(),
            hour: 9,
            entity_type: entity_type.to_string(),
            entity_type_name: if entity_type == "20" {
                "全场"
            } else {
                "店铺"
            }
            .to_string(),
            entity_name: format!("entity-{entity_id}"),
            source_entity_id: entity_id.to_string(),
            aibee_entity_id: entity_id.to_string(),
            floor_code: String::new(),
            floor_name: String::new(),
            area: String::new(),
            l1_retail_format: String::new(),
            l2_retail_format: String::new(),
            traffic_in,
            traffic_out,
            visitors,
            average_stay,
            updated_at: Utc.with_ymd_and_hms(2026, 7, 17, 8, 0, 0).unwrap(),
        }
    }

    #[test]
    fn public_mall_allowlist_defaults_to_deny_and_normalizes_case() {
        assert!(!public_mall_allowlist_contains(None, "SZ02"));
        assert!(!public_mall_allowlist_contains(Some(""), "SZ02"));
        assert!(public_mall_allowlist_contains(
            Some(" mall01, sz02 ,MALL03"),
            "Sz02"
        ));
        assert!(!public_mall_allowlist_contains(Some("SZ020"), "SZ02"));
        assert_eq!(normalize_mall_id(" sz02 ").as_deref(), Some("SZ02"));
        assert_eq!(normalize_mall_id("../SZ02"), None);
    }

    #[test]
    fn public_dataset_mapping_is_strict_and_keeps_traffic_and_profile_scopes_distinct() {
        let traffic_id = Uuid::parse_str("c5ba0e8e-7905-4a32-9ab4-4f9a82016c59").unwrap();
        let profile_id = Uuid::parse_str("e15ce18e-11e4-4326-a456-98ade948c8ef").unwrap();
        assert_eq!(parse_public_dataset_mapping(None, "SZ02").unwrap(), None);
        assert_eq!(
            parse_public_dataset_mapping(Some("SZ02=c5ba0e8e-7905-4a32-9ab4-4f9a82016c59"), "sz02")
                .unwrap(),
            Some(traffic_id)
        );
        assert_eq!(
            parse_public_dataset_mapping(Some("SZ02=e15ce18e-11e4-4326-a456-98ade948c8ef"), "SZ02")
                .unwrap(),
            Some(profile_id)
        );
        assert!(parse_public_dataset_mapping(Some("SZ02=not-a-uuid"), "SZ02").is_err());
        assert!(parse_public_dataset_mapping(
            Some(
                "SZ02=c5ba0e8e-7905-4a32-9ab4-4f9a82016c59,SZ02=e15ce18e-11e4-4326-a456-98ade948c8ef"
            ),
            "SZ02"
        )
        .is_err());
        assert_eq!(
            parse_public_dataset_mapping(
                Some("MALL01=c5ba0e8e-7905-4a32-9ab4-4f9a82016c59"),
                "SZ02"
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn sql_contract_binds_dataset_and_latest_ready_batch_for_every_business_query() {
        let source = include_str!("mall_sz02_analytics_support.rs");
        for predicate in [
            "and dataset_id = $3\n          and batch_id = $4",
            "and f.dataset_id = $3\n          and f.batch_id = $4",
            "and p.entity_type = f.entity_type",
            "and b.dataset_id = $2",
            "and b.source_kind = $4",
            "and b.status = 'ready'",
            "and d.lifecycle = 'active'",
            "and d.owner_user_id is null",
            "and coalesce(d.metadata -> 'local_only', 'false'::jsonb) <> 'true'::jsonb",
        ] {
            assert!(source.contains(predicate), "missing SQL scope: {predicate}");
        }
        assert_eq!(TRAFFIC_SOURCE_KIND, "aibee_traffic_hourly");
        assert_eq!(PROFILE_SOURCE_KIND, "aibee_profile_aggregate");
    }

    #[test]
    fn traffic_entity_key_matches_report_contract() {
        assert_eq!(traffic_entity_key("20", "mall-main"), "20:mall-main");
    }

    #[test]
    fn traffic_statistics_and_quality_match_existing_report_semantics() {
        let facts = vec![
            traffic_fact("20", "mall", "2026-07-01", 10, 5, 100, 0.0),
            traffic_fact("20", "mall", "2026-07-02", 0, 1, 0, 0.0),
        ];
        let stats = build_traffic_type_statistics(&facts);
        let mall = stats.get("20").expect("mall type statistics");
        assert_eq!(mall.rows, 2);
        assert_eq!(mall.entities, 1);
        assert_eq!(mall.nonzero_rows.traffic_in, 1);
        assert_eq!(mall.nonzero_rows.traffic_out, 2);
        assert_eq!(mall.nonzero_rows.visitors, 1);
        assert_eq!(mall.coverage.traffic_in, 0.5);
        assert_eq!(mall.coverage.average_stay, 0.0);

        let quality = serde_json::to_value(build_traffic_quality()).unwrap();
        assert_eq!(quality["duplicateKeyCount"], json!(0));
        assert_eq!(
            quality["primaryHourlyMetrics"],
            json!(["trafficIn", "trafficOut"])
        );
    }

    #[test]
    fn serialized_traffic_response_contains_no_sensitive_values_or_provenance_keys() {
        let response = build_traffic_report_response(
            "SZ02".to_string(),
            vec![traffic_fact("20", "mall", "2026-07-01", 10, 5, 100, 0.0)],
        )
        .unwrap();
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["title"], json!("某商场 · 7月客流运营驾驶舱"));
        assert_eq!(value["rows"][0][0], json!(0));
        assert!(value["rows"][0][0].is_u64());
        let serialized = serde_json::to_string(&response).unwrap();
        for forbidden_key in [
            "\"token\":",
            "\"pid\":",
            "\"batchId\":",
            "\"documentId\":",
            "\"objectKey\":",
        ] {
            assert!(!serialized
                .to_ascii_lowercase()
                .contains(&forbidden_key.to_ascii_lowercase()));
        }
        assert!(serialized.contains("\"tokenIncluded\":false"));
        assert!(serialized.contains("\"storage\":\"postgresql\""));
    }

    #[test]
    fn serialized_profile_response_exposes_aggregates_without_identifier_fields() {
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 17, 8, 0, 0).unwrap();
        let response = MallProfileSummaryResponse {
            schema_version: "1.0",
            mall_id: "SZ02".to_string(),
            generated_at: updated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            storage: MallProfileStorageView {
                engine: "postgresql",
                schema: "mall_sz02",
                relations: MallProfileRelationsView {
                    daily_summary: "mall_sz02.profile_daily_summary",
                    daily_distributions: "mall_sz02.profile_distribution_daily",
                    period_distributions: "mall_sz02.profile_distribution_period",
                },
                tenant_scoped: true,
                mall_scoped: true,
                row_count: 1,
            },
            privacy: MallProfilePrivacyView {
                personal_identifiers_included: false,
                aggregation_level: "daily_or_period_aggregate",
            },
            daily_summary: vec![MallProfileDailySummaryView {
                business_date: "2026-07-01".to_string(),
                record_count: 10,
                unique_visitor_count: 9,
                duplicate_record_count: 1,
                missing_identifier_count: 0,
                date_mismatch_count: 0,
                invalid_age_count: 0,
                updated_at,
            }],
            daily_distributions: Vec::new(),
            period_distributions: Vec::new(),
        };
        let serialized = serde_json::to_string(&response).unwrap();
        let lowercase = serialized.to_ascii_lowercase();
        for forbidden_key in [
            "\"pid",
            "\"token\":",
            "\"batchid\":",
            "\"documentid\":",
            "\"objectkey\":",
        ] {
            assert!(!lowercase.contains(forbidden_key));
        }
        assert!(serialized.contains("\"uniqueVisitorCount\":9"));
        assert!(serialized.contains("\"personalIdentifiersIncluded\":false"));
    }

    #[test]
    fn row_limit_rejects_oversized_public_responses() {
        let projected_full_july_rows = 100_794;
        assert!(reject_row_limit("limit", projected_full_july_rows).is_ok());
        assert!(reject_row_limit("limit", MAX_PUBLIC_ANALYTICS_ROWS).is_ok());
        let error = reject_row_limit("limit", MAX_PUBLIC_ANALYTICS_ROWS + 1).unwrap_err();
        assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "limit");
    }
}
