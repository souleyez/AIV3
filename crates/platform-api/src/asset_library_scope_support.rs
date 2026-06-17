#![allow(dead_code)]

use std::collections::BTreeSet;

use domain_model::DatasetId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetLibraryDatasetMembership {
    pub(crate) dataset_id: DatasetId,
    pub(crate) role: String,
    pub(crate) priority: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetLibraryScopeSummary {
    pub(crate) dataset_ids: Vec<DatasetId>,
    pub(crate) denied_dataset_ids: Vec<DatasetId>,
    pub(crate) membership_count: usize,
    pub(crate) authorized_dataset_count: usize,
}

pub(crate) fn resolve_asset_library_dataset_scope(
    memberships: &[AssetLibraryDatasetMembership],
    authorized_dataset_ids: &[DatasetId],
) -> AssetLibraryScopeSummary {
    let authorized = authorized_dataset_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut ordered_memberships = memberships.iter().collect::<Vec<_>>();
    ordered_memberships.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.dataset_id.cmp(&right.dataset_id))
    });

    let mut dataset_ids = Vec::new();
    let mut denied_dataset_ids = Vec::new();
    for membership in ordered_memberships {
        if authorized.contains(&membership.dataset_id) {
            if !dataset_ids.contains(&membership.dataset_id) {
                dataset_ids.push(membership.dataset_id);
            }
        } else if !denied_dataset_ids.contains(&membership.dataset_id) {
            denied_dataset_ids.push(membership.dataset_id);
        }
    }

    AssetLibraryScopeSummary {
        dataset_ids,
        denied_dataset_ids,
        membership_count: memberships.len(),
        authorized_dataset_count: authorized.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn membership(dataset_id: DatasetId, priority: i32) -> AssetLibraryDatasetMembership {
        AssetLibraryDatasetMembership {
            dataset_id,
            role: "member".to_string(),
            priority,
        }
    }

    #[test]
    fn asset_library_scope_returns_only_authorized_member_datasets() {
        let allowed_a = DatasetId::new();
        let denied_b = DatasetId::new();
        let allowed_c = DatasetId::new();
        let unrelated = DatasetId::new();
        let summary = resolve_asset_library_dataset_scope(
            &[
                membership(allowed_a, 10),
                membership(denied_b, 20),
                membership(allowed_c, 30),
            ],
            &[allowed_a, allowed_c, unrelated],
        );

        assert_eq!(summary.dataset_ids, vec![allowed_a, allowed_c]);
        assert_eq!(summary.denied_dataset_ids, vec![denied_b]);
        assert_eq!(summary.membership_count, 3);
        assert_eq!(summary.authorized_dataset_count, 3);
    }

    #[test]
    fn asset_library_scope_dedupes_memberships_and_orders_by_priority() {
        let dataset_a = DatasetId::new();
        let dataset_b = DatasetId::new();
        let summary = resolve_asset_library_dataset_scope(
            &[
                membership(dataset_b, 20),
                membership(dataset_a, 10),
                membership(dataset_a, 30),
                membership(dataset_b, 5),
            ],
            &[dataset_a, dataset_b],
        );

        assert_eq!(summary.dataset_ids, vec![dataset_b, dataset_a]);
        assert!(summary.denied_dataset_ids.is_empty());
        assert_eq!(summary.membership_count, 4);
    }

    #[test]
    fn asset_library_scope_never_expands_beyond_memberships() {
        let authorized_only = DatasetId::new();
        let summary = resolve_asset_library_dataset_scope(&[], &[authorized_only]);

        assert!(summary.dataset_ids.is_empty());
        assert!(summary.denied_dataset_ids.is_empty());
        assert_eq!(summary.membership_count, 0);
        assert_eq!(summary.authorized_dataset_count, 1);
    }
}
