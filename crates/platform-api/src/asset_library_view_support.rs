use contracts::{
    AssetItemView, AssetLibraryDatasetMembershipView, AssetLibraryView, AssetParseRunView,
    AssetProfileSupplyHintView, AssetProfileView, DatasetAssetMembershipView,
};
use storage::{
    AssetItemRecord, AssetLibraryDatasetMembershipRecord, AssetLibraryRecord, AssetParseRunRecord,
    AssetProfileRecord, DatasetAssetMembershipRecord,
};

use crate::asset_profile_supply_support;

pub(crate) fn asset_library_view(asset_library: AssetLibraryRecord) -> AssetLibraryView {
    AssetLibraryView {
        id: asset_library.id.to_string(),
        external_id: asset_library.external_id,
        name: asset_library.name,
        domain: asset_library.domain,
        description: asset_library.description,
        visibility: asset_library.visibility,
        metadata: asset_library.metadata,
        dataset_count: asset_library.dataset_count,
        created_at: asset_library.created_at,
        updated_at: asset_library.updated_at,
    }
}

pub(crate) fn asset_library_membership_view(
    membership: AssetLibraryDatasetMembershipRecord,
) -> AssetLibraryDatasetMembershipView {
    AssetLibraryDatasetMembershipView {
        asset_library_id: membership.asset_library_id.to_string(),
        dataset_id: membership.dataset_id,
        role: membership.role,
        priority: membership.priority,
        created_at: membership.created_at,
    }
}

pub(crate) fn asset_item_view(asset: AssetItemRecord) -> AssetItemView {
    AssetItemView {
        id: asset.id.to_string(),
        asset_library_id: asset.asset_library_id.map(|id| id.to_string()),
        collection_id: asset.collection_id.map(|id| id.to_string()),
        external_id: asset.external_id,
        title: asset.title,
        asset_kind: asset.asset_kind,
        source_kind: asset.source_kind,
        source_id: asset.source_id,
        content_type: asset.content_type,
        object_key: asset.object_key,
        metadata: asset.metadata,
        profile_count: asset.profile_count,
        created_at: asset.created_at,
        updated_at: asset.updated_at,
    }
}

pub(crate) fn asset_profile_supply_inputs(
    asset: &AssetItemRecord,
    profiles: &[AssetProfileRecord],
) -> Vec<asset_profile_supply_support::AssetProfileSupplyInput> {
    profiles
        .iter()
        .map(
            |profile| asset_profile_supply_support::AssetProfileSupplyInput {
                asset_id: asset.id.to_string(),
                title: asset.title.clone(),
                asset_kind: asset.asset_kind.clone(),
                source_kind: asset.source_kind.clone(),
                profile_kind: profile.profile_kind.clone(),
                attributes: profile.attributes.clone(),
            },
        )
        .collect()
}

pub(crate) fn asset_profile_supply_hint_view(
    hint: asset_profile_supply_support::AssetProfileSupplyHint,
) -> AssetProfileSupplyHintView {
    AssetProfileSupplyHintView {
        asset_id: hint.asset_id,
        title: hint.title,
        asset_kind: hint.asset_kind,
        source_kind: hint.source_kind,
        profile_kind: hint.profile_kind,
        summary: hint.summary,
        noun_terms: hint.noun_terms,
        facets: hint.facets,
    }
}

pub(crate) fn dataset_asset_membership_view(
    membership: DatasetAssetMembershipRecord,
) -> DatasetAssetMembershipView {
    DatasetAssetMembershipView {
        dataset_id: membership.dataset_id,
        asset_id: membership.asset_id.to_string(),
        membership_kind: membership.membership_kind,
        expires_at: membership.expires_at,
        created_at: membership.created_at,
    }
}

pub(crate) fn asset_profile_view(profile: AssetProfileRecord) -> AssetProfileView {
    AssetProfileView {
        id: profile.id.to_string(),
        asset_id: profile.asset_id.to_string(),
        profile_kind: profile.profile_kind,
        profile_version: profile.profile_version,
        attributes: profile.attributes,
        embedding_status: profile.embedding_status,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    }
}

pub(crate) fn asset_parse_run_view(parse_run: AssetParseRunRecord) -> AssetParseRunView {
    AssetParseRunView {
        id: parse_run.id.to_string(),
        asset_id: parse_run.asset_id.to_string(),
        parser_name: parse_run.parser_name,
        parser_version: parse_run.parser_version,
        status: parse_run.status,
        started_at: parse_run.started_at,
        finished_at: parse_run.finished_at,
        error_code: parse_run.error_code,
        error_message: parse_run.error_message,
        metadata: parse_run.metadata,
        created_at: parse_run.created_at,
        updated_at: parse_run.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::{DatasetId, TenantId};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn timestamp() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 17, 9, 30, 0)
            .single()
            .expect("valid timestamp")
    }

    fn tenant_id() -> TenantId {
        TenantId(Uuid::from_u128(1))
    }

    #[test]
    fn asset_library_view_support_maps_library_record() {
        let id = Uuid::from_u128(2);
        let view = asset_library_view(AssetLibraryRecord {
            id,
            tenant_id: tenant_id(),
            external_id: Some("fashion-library".to_string()),
            name: "服装设计资产库".to_string(),
            domain: "fashion".to_string(),
            description: Some("图库、PPT、视频素材".to_string()),
            visibility: "private".to_string(),
            metadata: json!({"owner": "design"}),
            dataset_count: 3,
            created_at: timestamp(),
            updated_at: timestamp(),
        });

        assert_eq!(view.id, id.to_string());
        assert_eq!(view.external_id.as_deref(), Some("fashion-library"));
        assert_eq!(view.domain, "fashion");
        assert_eq!(view.dataset_count, 3);
        assert_eq!(view.metadata["owner"], "design");
    }

    #[test]
    fn asset_library_view_support_maps_membership_and_asset_item() {
        let library_id = Uuid::from_u128(3);
        let dataset_id = DatasetId(Uuid::from_u128(4));
        let membership = asset_library_membership_view(AssetLibraryDatasetMembershipRecord {
            tenant_id: tenant_id(),
            asset_library_id: library_id,
            dataset_id,
            role: "source".to_string(),
            priority: 20,
            created_at: timestamp(),
        });
        assert_eq!(membership.asset_library_id, library_id.to_string());
        assert_eq!(membership.dataset_id, dataset_id);
        assert_eq!(membership.priority, 20);

        let asset_id = Uuid::from_u128(5);
        let collection_id = Uuid::from_u128(6);
        let item = asset_item_view(AssetItemRecord {
            id: asset_id,
            tenant_id: tenant_id(),
            asset_library_id: Some(library_id),
            collection_id: Some(collection_id),
            external_id: Some("look-001".to_string()),
            title: "春夏连衣裙灵感图".to_string(),
            asset_kind: "image".to_string(),
            source_kind: "upload".to_string(),
            source_id: Some("doc-001".to_string()),
            content_type: Some("image/png".to_string()),
            object_key: Some("objects/look-001.png".to_string()),
            metadata: json!({"season": "春夏"}),
            profile_count: 2,
            created_at: timestamp(),
            updated_at: timestamp(),
        });

        assert_eq!(item.id, asset_id.to_string());
        assert_eq!(item.asset_library_id, Some(library_id.to_string()));
        assert_eq!(item.collection_id, Some(collection_id.to_string()));
        assert_eq!(item.metadata["season"], "春夏");
        assert_eq!(item.profile_count, 2);
    }

    #[test]
    fn asset_library_view_support_builds_profile_supply_inputs_and_views() {
        let asset_id = Uuid::from_u128(7);
        let asset = AssetItemRecord {
            id: asset_id,
            tenant_id: tenant_id(),
            asset_library_id: None,
            collection_id: None,
            external_id: None,
            title: "门店陈列视频".to_string(),
            asset_kind: "video".to_string(),
            source_kind: "upload".to_string(),
            source_id: None,
            content_type: Some("video/mp4".to_string()),
            object_key: None,
            metadata: json!({}),
            profile_count: 1,
            created_at: timestamp(),
            updated_at: timestamp(),
        };
        let profiles = vec![AssetProfileRecord {
            id: Uuid::from_u128(8),
            tenant_id: tenant_id(),
            asset_id,
            profile_kind: "video_summary".to_string(),
            profile_version: "v1".to_string(),
            attributes: json!({"summary": "夏季女装区域陈列讲解"}),
            embedding_status: "pending".to_string(),
            created_at: timestamp(),
            updated_at: timestamp(),
        }];

        let inputs = asset_profile_supply_inputs(&asset, &profiles);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].asset_id, asset_id.to_string());
        assert_eq!(inputs[0].profile_kind, "video_summary");
        assert_eq!(inputs[0].attributes["summary"], "夏季女装区域陈列讲解");

        let hint_view =
            asset_profile_supply_hint_view(asset_profile_supply_support::AssetProfileSupplyHint {
                asset_id: asset_id.to_string(),
                title: "门店陈列视频".to_string(),
                asset_kind: "video".to_string(),
                source_kind: "upload".to_string(),
                profile_kind: "video_summary".to_string(),
                summary: "夏季女装区域陈列讲解".to_string(),
                noun_terms: vec!["女装".to_string()],
                facets: vec!["场景: 陈列".to_string()],
            });

        assert_eq!(hint_view.asset_id, asset_id.to_string());
        assert_eq!(hint_view.noun_terms, vec!["女装".to_string()]);
        assert_eq!(hint_view.facets, vec!["场景: 陈列".to_string()]);
    }

    #[test]
    fn asset_library_view_support_maps_dataset_asset_membership_and_profile() {
        let dataset_id = DatasetId(Uuid::from_u128(9));
        let asset_id = Uuid::from_u128(10);
        let membership = dataset_asset_membership_view(DatasetAssetMembershipRecord {
            tenant_id: tenant_id(),
            dataset_id,
            asset_id,
            membership_kind: "imported".to_string(),
            expires_at: None,
            created_at: timestamp(),
        });
        assert_eq!(membership.dataset_id, dataset_id);
        assert_eq!(membership.asset_id, asset_id.to_string());
        assert_eq!(membership.membership_kind, "imported");

        let profile = asset_profile_view(AssetProfileRecord {
            id: Uuid::from_u128(11),
            tenant_id: tenant_id(),
            asset_id,
            profile_kind: "fashion_design_image_v1".to_string(),
            profile_version: "v1".to_string(),
            attributes: json!({"category": "dress"}),
            embedding_status: "not_requested".to_string(),
            created_at: timestamp(),
            updated_at: timestamp(),
        });
        assert_eq!(profile.asset_id, asset_id.to_string());
        assert_eq!(profile.attributes["category"], "dress");

        let parse_run = asset_parse_run_view(AssetParseRunRecord {
            id: Uuid::from_u128(12),
            tenant_id: tenant_id(),
            asset_id,
            parser_name: "datamax-fashion-image-parser".to_string(),
            parser_version: "2026-06-17".to_string(),
            status: "pending".to_string(),
            started_at: None,
            finished_at: None,
            error_code: None,
            error_message: None,
            metadata: json!({"parse_method": "ocr_vlm_pending"}),
            created_at: timestamp(),
            updated_at: timestamp(),
        });
        assert_eq!(parse_run.asset_id, asset_id.to_string());
        assert_eq!(parse_run.status, "pending");
        assert_eq!(parse_run.metadata["parse_method"], "ocr_vlm_pending");
    }
}
