use std::collections::BTreeSet;

use domain_model::DatasetId;

pub(crate) fn normalize_document_dataset_ids(
    canonical_dataset_id: DatasetId,
    dataset_ids: Vec<DatasetId>,
) -> Vec<DatasetId> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    if seen.insert(canonical_dataset_id) {
        normalized.push(canonical_dataset_id);
    }
    for dataset_id in dataset_ids {
        if seen.insert(dataset_id) {
            normalized.push(dataset_id);
        }
    }
    normalized
}

pub(crate) fn infer_media_kind_from_content_type(content_type: &str) -> &'static str {
    let lower = content_type.trim().to_ascii_lowercase();
    if lower.starts_with("audio/") {
        "audio"
    } else if lower.starts_with("video/") {
        "video"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_dataset_ids_keep_canonical_first_and_deduplicate() {
        let canonical_dataset_id = DatasetId::new();
        let secondary_dataset_id = DatasetId::new();
        let tertiary_dataset_id = DatasetId::new();

        let normalized = normalize_document_dataset_ids(
            canonical_dataset_id,
            vec![
                secondary_dataset_id,
                canonical_dataset_id,
                tertiary_dataset_id,
                secondary_dataset_id,
            ],
        );

        assert_eq!(
            normalized,
            vec![
                canonical_dataset_id,
                secondary_dataset_id,
                tertiary_dataset_id
            ]
        );
    }

    #[test]
    fn media_kind_inference_keeps_existing_audio_video_prefix_rules() {
        assert_eq!(infer_media_kind_from_content_type("audio/mpeg"), "audio");
        assert_eq!(
            infer_media_kind_from_content_type("  VIDEO/MP4; charset=utf-8"),
            "video"
        );
        assert_eq!(
            infer_media_kind_from_content_type("application/pdf"),
            "unknown"
        );
        assert_eq!(infer_media_kind_from_content_type(""), "unknown");
    }
}
