use std::collections::HashSet;

use domain_model::{DatasetVisibility, TenantId};
use serde_json::{json, Value};
use storage::{NewDataset, PgStorage};

use crate::ApiError;

const DEFAULT_PUBLIC_DATASETS: &[(&str, &str, &str)] = &[
    ("orders", "订单", "默认公开订单数据集。"),
    ("customer-service", "客服", "默认公开客服数据集。"),
    ("enterprise-qa", "企业问答", "默认公开企业问答数据集。"),
    ("web-capture", "网页采集", "默认公开网页采集数据集。"),
    ("unclassified", "未分类", "默认公开未分类数据集。"),
];

pub(crate) async fn ensure_default_public_datasets(
    storage: &PgStorage,
    tenant_id: TenantId,
) -> std::result::Result<(), ApiError> {
    let existing = storage
        .datasets()
        .list_by_tenant(tenant_id)
        .await
        .map_err(ApiError::from_storage)?;
    let existing_keys: HashSet<String> = existing.into_iter().map(|dataset| dataset.key).collect();
    for (key, title, description) in DEFAULT_PUBLIC_DATASETS {
        if existing_keys.contains(*key) {
            continue;
        }
        storage
            .datasets()
            .create_with_metadata(
                tenant_id,
                NewDataset {
                    key: (*key).to_string(),
                    title: (*title).to_string(),
                    description: Some((*description).to_string()),
                    owner_user_id: None,
                },
                default_public_dataset_metadata(),
            )
            .await
            .map_err(ApiError::from_storage)?;
    }
    Ok(())
}

fn default_public_dataset_metadata() -> Value {
    json!({
        "visibility": DatasetVisibility::Public.as_str(),
        "default_secret_binding_ids": [],
        "default_dataset": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_public_dataset_specs_keep_stable_unique_keys() {
        let keys = DEFAULT_PUBLIC_DATASETS
            .iter()
            .map(|(key, _title, _description)| *key)
            .collect::<HashSet<_>>();
        assert_eq!(keys.len(), DEFAULT_PUBLIC_DATASETS.len());
        assert!(keys.contains("orders"));
        assert!(keys.contains("customer-service"));
        assert!(keys.contains("enterprise-qa"));
        assert!(keys.contains("web-capture"));
        assert!(keys.contains("unclassified"));
    }

    #[test]
    fn default_public_dataset_metadata_marks_public_default_without_secret_bindings() {
        let metadata = default_public_dataset_metadata();
        assert_eq!(metadata["visibility"], DatasetVisibility::Public.as_str());
        assert_eq!(metadata["default_dataset"], true);
        assert_eq!(
            metadata["default_secret_binding_ids"]
                .as_array()
                .expect("secret binding ids should be an array")
                .len(),
            0
        );
    }
}
