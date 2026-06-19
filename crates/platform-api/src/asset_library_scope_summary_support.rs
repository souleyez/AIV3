use contracts::{
    AssetItemView, AssetLibraryDatasetMembershipView, AssetLibraryScopeSummaryResponse,
    AssetLibraryScopeSummaryView, AssetLibraryView, AssetProfileSupplyHintView, DatasetSummary,
};
use domain_model::DatasetId;
use uuid::Uuid;

use crate::{asset_library_validation_support::parse_asset_library_id, ApiError};

pub(crate) const ASSET_LIBRARY_SCOPE_POLICY: &str =
    "asset_library_memberships_intersect_authorized_datasets";

pub(crate) struct AssetLibraryScopeSummaryResponseInput {
    pub(crate) asset_library: AssetLibraryView,
    pub(crate) memberships: Vec<AssetLibraryDatasetMembershipView>,
    pub(crate) datasets: Vec<DatasetSummary>,
    pub(crate) dataset_ids: Vec<DatasetId>,
    pub(crate) assets: Vec<AssetItemView>,
    pub(crate) asset_profile_hints: Vec<AssetProfileSupplyHintView>,
    pub(crate) denied_dataset_count: usize,
    pub(crate) membership_count: usize,
    pub(crate) authorized_dataset_count: usize,
}

pub(crate) fn asset_library_scope_summary_response(
    input: AssetLibraryScopeSummaryResponseInput,
) -> AssetLibraryScopeSummaryResponse {
    let asset_count = input.assets.len();
    let asset_profile_hint_count = input.asset_profile_hints.len();

    AssetLibraryScopeSummaryResponse {
        summary: AssetLibraryScopeSummaryView {
            asset_library: input.asset_library,
            memberships: input.memberships,
            datasets: input.datasets,
            dataset_ids: input.dataset_ids,
            assets: input.assets,
            asset_profile_hints: input.asset_profile_hints,
            denied_dataset_count: input.denied_dataset_count,
            membership_count: input.membership_count,
            authorized_dataset_count: input.authorized_dataset_count,
            asset_count,
            asset_profile_hint_count,
            scope_policy: ASSET_LIBRARY_SCOPE_POLICY.to_string(),
        },
    }
}

pub(crate) fn parse_asset_library_scope_summary_path(
    asset_library_id: &str,
) -> std::result::Result<Uuid, ApiError> {
    parse_asset_library_id(asset_library_id)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::{DatasetId, DatasetLifecycle, DatasetVisibility};
    use serde_json::json;

    use super::*;

    fn timestamp() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 19, 10, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn dataset_id(value: u128) -> DatasetId {
        DatasetId(Uuid::from_u128(value))
    }

    fn library_view() -> AssetLibraryView {
        AssetLibraryView {
            id: Uuid::from_u128(1).to_string(),
            external_id: Some("fashion-main".to_string()),
            name: "服装设计资产库".to_string(),
            domain: "fashion".to_string(),
            description: Some("图库、视频、PPT".to_string()),
            visibility: "private".to_string(),
            metadata: json!({"owner": "design"}),
            dataset_count: 2,
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    #[test]
    fn scope_summary_response_sets_counts_and_policy() {
        let response =
            asset_library_scope_summary_response(AssetLibraryScopeSummaryResponseInput {
                asset_library: library_view(),
                memberships: vec![AssetLibraryDatasetMembershipView {
                    asset_library_id: Uuid::from_u128(1).to_string(),
                    dataset_id: dataset_id(2),
                    role: "source".to_string(),
                    priority: 10,
                    created_at: timestamp(),
                }],
                datasets: vec![DatasetSummary {
                    id: dataset_id(2),
                    key: "fashion-dataset".to_string(),
                    title: "服装数据集".to_string(),
                    lifecycle: DatasetLifecycle::Active,
                    visibility: DatasetVisibility::Private,
                    secret_binding_ids: vec![],
                    document_count: Some(1),
                    documents_count: Some(1),
                    estimated_word_count: None,
                    parse_status_summary: None,
                    content_type_summary: None,
                    latest_upload: None,
                    document_title_hints: vec![],
                    material_hints: vec![],
                    noun_term_hints: vec![],
                    section_title_hints: vec![],
                    document_understanding_strategies: vec![],
                    access_warning: None,
                }],
                dataset_ids: vec![dataset_id(2)],
                assets: vec![AssetItemView {
                    id: Uuid::from_u128(4).to_string(),
                    asset_library_id: Some(Uuid::from_u128(1).to_string()),
                    collection_id: None,
                    external_id: None,
                    title: "连衣裙灵感图".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    source_id: Some("doc-1".to_string()),
                    content_type: Some("image/png".to_string()),
                    object_key: None,
                    metadata: json!({}),
                    profile_count: 1,
                    created_at: timestamp(),
                    updated_at: timestamp(),
                }],
                asset_profile_hints: vec![AssetProfileSupplyHintView {
                    asset_id: Uuid::from_u128(4).to_string(),
                    title: "连衣裙灵感图".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "image_semantic".to_string(),
                    summary: "春夏连衣裙".to_string(),
                    noun_terms: vec!["连衣裙".to_string()],
                    facets: vec!["季节: 春夏".to_string()],
                }],
                denied_dataset_count: 1,
                membership_count: 2,
                authorized_dataset_count: 1,
            });

        assert_eq!(response.summary.asset_count, 1);
        assert_eq!(response.summary.asset_profile_hint_count, 1);
        assert_eq!(response.summary.denied_dataset_count, 1);
        assert_eq!(response.summary.membership_count, 2);
        assert_eq!(response.summary.authorized_dataset_count, 1);
        assert_eq!(response.summary.scope_policy, ASSET_LIBRARY_SCOPE_POLICY);
    }

    #[test]
    fn scope_summary_path_parser_keeps_asset_library_id_validation() {
        let asset_library_id = Uuid::from_u128(9);

        assert_eq!(
            parse_asset_library_scope_summary_path(&asset_library_id.to_string())
                .expect("valid asset library id should parse"),
            asset_library_id
        );

        let error = parse_asset_library_scope_summary_path("not-a-uuid")
            .expect_err("invalid asset library id should fail");
        assert_eq!(error.payload.code, "invalid_asset_library_id");
    }
}
