use super::collect_string_list;
use contracts::DatasetSummary;
use domain_model::Dataset;
use serde_json::Value;

pub(crate) fn dataset_summary(dataset: Dataset, access_warning: Option<String>) -> DatasetSummary {
    let document_count = dataset_metadata_usize_optional(
        &dataset,
        &[
            "document_count",
            "documentCount",
            "documents_count",
            "documentsCount",
        ],
    );
    let estimated_word_count = dataset_metadata_usize_optional(
        &dataset,
        &[
            "estimated_word_count",
            "estimatedWordCount",
            "word_count",
            "wordCount",
        ],
    );
    let parse_status_summary =
        dataset_metadata_string_optional(&dataset, &["parse_status_summary", "parseStatusSummary"]);
    let content_type_summary =
        dataset_metadata_string_optional(&dataset, &["content_type_summary", "contentTypeSummary"]);
    let latest_upload =
        dataset_metadata_string_optional(&dataset, &["latest_upload", "latestUpload"]);
    let document_title_hints = dataset_metadata_string_vec(
        &dataset,
        &["document_title_hints", "documentTitleHints"],
        12,
    );
    let material_hints =
        dataset_metadata_string_vec(&dataset, &["material_hints", "materialHints"], 8);
    let noun_term_hints = dataset_metadata_string_vec(
        &dataset,
        &[
            "noun_term_hints",
            "nounTermHints",
            "noun_terms",
            "nounTerms",
        ],
        16,
    );
    let section_title_hints =
        dataset_metadata_string_vec(&dataset, &["section_title_hints", "sectionTitleHints"], 16);
    let document_understanding_strategies = dataset_metadata_string_vec(
        &dataset,
        &[
            "document_understanding_strategies",
            "documentUnderstandingStrategies",
        ],
        6,
    );
    DatasetSummary {
        id: dataset.id,
        key: dataset.key,
        title: dataset.title,
        lifecycle: dataset.lifecycle,
        visibility: dataset.visibility,
        secret_binding_ids: dataset.default_secret_binding_ids,
        document_count,
        documents_count: document_count,
        estimated_word_count,
        parse_status_summary,
        content_type_summary,
        latest_upload,
        document_title_hints,
        material_hints,
        noun_term_hints,
        section_title_hints,
        document_understanding_strategies,
        access_warning,
    }
}

pub(crate) fn dataset_metadata_usize_optional(dataset: &Dataset, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|key| {
        dataset.metadata.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str()?.parse::<u64>().ok())
                .map(|number| number as usize)
        })
    })
}

fn dataset_metadata_string_optional(dataset: &Dataset, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        dataset
            .metadata
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn dataset_metadata_string_vec(dataset: &Dataset, keys: &[&str], limit: usize) -> Vec<String> {
    let mut values = Vec::new();
    for key in keys {
        if let Some(value) = dataset.metadata.get(*key) {
            collect_string_list(value, &mut values);
        }
    }
    values.truncate(limit);
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, DatasetLifecycle, DatasetVisibility, TenantId};
    use serde_json::json;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn test_dataset() -> Dataset {
        Dataset {
            id: DatasetId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: None,
            key: format!("dataset-summary-{}", Uuid::new_v4()),
            title: "Dataset Summary Test".to_string(),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility: DatasetVisibility::Public,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn dataset_metadata_helpers_preserve_alias_trim_and_limit_semantics() {
        let mut dataset = test_dataset();
        dataset
            .metadata
            .insert("documentCount".to_string(), json!("12"));
        dataset
            .metadata
            .insert("parseStatusSummary".to_string(), json!("  indexed  "));
        dataset.metadata.insert(
            "materialHints".to_string(),
            json!(["alpha", " ", ["beta", "gamma"], {"ignored": true}, 42]),
        );

        assert_eq!(
            dataset_metadata_usize_optional(&dataset, &["document_count", "documentCount"]),
            Some(12)
        );
        assert_eq!(
            dataset_metadata_string_optional(
                &dataset,
                &["parse_status_summary", "parseStatusSummary"]
            ),
            Some("indexed".to_string())
        );
        assert_eq!(
            dataset_metadata_string_vec(&dataset, &["material_hints", "materialHints"], 2),
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn dataset_summary_preserves_metadata_field_mapping() {
        let mut dataset = test_dataset();
        dataset
            .metadata
            .insert("documents_count".to_string(), json!(3));
        dataset
            .metadata
            .insert("wordCount".to_string(), json!("2048"));
        dataset
            .metadata
            .insert("content_type_summary".to_string(), json!(" docx "));
        dataset
            .metadata
            .insert("latestUpload".to_string(), json!("2026-06-13"));
        dataset
            .metadata
            .insert("documentTitleHints".to_string(), json!(["A", "B"]));
        dataset
            .metadata
            .insert("nounTerms".to_string(), json!(["term"]));
        dataset.metadata.insert(
            "documentUnderstandingStrategies".to_string(),
            json!(["table", "heading"]),
        );

        let summary = dataset_summary(dataset, Some("limited".to_string()));

        assert_eq!(summary.document_count, Some(3));
        assert_eq!(summary.documents_count, Some(3));
        assert_eq!(summary.estimated_word_count, Some(2048));
        assert_eq!(summary.content_type_summary, Some("docx".to_string()));
        assert_eq!(summary.latest_upload, Some("2026-06-13".to_string()));
        assert_eq!(
            summary.document_title_hints,
            vec!["A".to_string(), "B".to_string()]
        );
        assert_eq!(summary.noun_term_hints, vec!["term".to_string()]);
        assert_eq!(
            summary.document_understanding_strategies,
            vec!["table".to_string(), "heading".to_string()]
        );
        assert_eq!(summary.access_warning, Some("limited".to_string()));
    }
}
