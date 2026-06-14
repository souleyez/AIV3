use serde_json::Value;

use crate::{
    external_channel_static_page_baseline_public_url,
    external_channel_static_page_provisional_existing_artifact,
};

pub(crate) fn external_channel_static_page_accepted_template_baseline(
    card: Option<&Value>,
) -> bool {
    let Some(card) = card else {
        return false;
    };
    let accepted_reason = card
        .get("provisional_existing_artifact_reason")
        .or_else(|| card.get("image2_skip_reason"))
        .and_then(Value::as_str)
        == Some("accepted_dataset_overlap_template_baseline");
    let accepted_marker = external_channel_static_page_provisional_existing_artifact(Some(card))
        || card
            .get("image2_skipped")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    accepted_reason
        && accepted_marker
        && external_channel_static_page_baseline_public_url(card).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepted_template_baseline_requires_reason_marker_and_public_url() {
        let card = json!({
            "provisional_existing_artifact": true,
            "provisional_existing_artifact_reason": "accepted_dataset_overlap_template_baseline",
            "public_url": "https://v3.elepcloud.com/generated-artifacts/reports/current/index.html"
        });

        assert!(external_channel_static_page_accepted_template_baseline(
            Some(&card)
        ));
    }

    #[test]
    fn accepted_template_baseline_accepts_image2_skip_marker_and_baseline_url() {
        let card = json!({
            "image2_skipped": true,
            "image2_skip_reason": "accepted_dataset_overlap_template_baseline",
            "relaxed_template_match": {
                "baseline_public_url": "https://v3.elepcloud.com/generated-artifacts/reports/baseline/index.html"
            }
        });

        assert!(external_channel_static_page_accepted_template_baseline(
            Some(&card)
        ));
    }

    #[test]
    fn accepted_template_baseline_rejects_incomplete_markers() {
        let missing_reason = json!({
            "provisional_existing_artifact": true,
            "public_url": "https://v3.elepcloud.com/generated-artifacts/reports/current/index.html"
        });
        let missing_marker = json!({
            "provisional_existing_artifact_reason": "accepted_dataset_overlap_template_baseline",
            "public_url": "https://v3.elepcloud.com/generated-artifacts/reports/current/index.html"
        });
        let missing_url = json!({
            "provisional_existing_artifact": true,
            "provisional_existing_artifact_reason": "accepted_dataset_overlap_template_baseline"
        });

        assert!(!external_channel_static_page_accepted_template_baseline(
            None
        ));
        assert!(!external_channel_static_page_accepted_template_baseline(
            Some(&missing_reason)
        ));
        assert!(!external_channel_static_page_accepted_template_baseline(
            Some(&missing_marker)
        ));
        assert!(!external_channel_static_page_accepted_template_baseline(
            Some(&missing_url)
        ));
    }
}
