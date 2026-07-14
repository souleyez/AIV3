use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use chrono::{DateTime, SecondsFormat, Utc};
use domain_model::DocumentId;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetDocumentOwnership {
    pub document_id: DocumentId,
    pub direct: bool,
    pub membership: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDocumentSourceVersion {
    pub document_id: DocumentId,
    pub updated_at: DateTime<Utc>,
    pub parse_versions: Vec<String>,
    pub fact_snapshot_versions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAssetSourceVersion {
    pub asset_id: Uuid,
    pub updated_at: DateTime<Utc>,
    pub profile_versions: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticSourceFingerprintInput {
    pub documents: Vec<SemanticDocumentSourceVersion>,
    pub assets: Vec<SemanticAssetSourceVersion>,
    pub dataset_fact_snapshot_version: Option<String>,
    pub dictionary_versions: Vec<String>,
}

pub fn merge_dataset_document_ownership(
    direct_document_ids: &[DocumentId],
    membership_document_ids: &[DocumentId],
) -> Vec<DatasetDocumentOwnership> {
    let mut merged = BTreeMap::<Uuid, DatasetDocumentOwnership>::new();
    for document_id in direct_document_ids {
        merged
            .entry(document_id.0)
            .or_insert_with(|| DatasetDocumentOwnership {
                document_id: *document_id,
                direct: false,
                membership: false,
            })
            .direct = true;
    }
    for document_id in membership_document_ids {
        merged
            .entry(document_id.0)
            .or_insert_with(|| DatasetDocumentOwnership {
                document_id: *document_id,
                direct: false,
                membership: false,
            })
            .membership = true;
    }
    merged.into_values().collect()
}

pub fn source_fingerprint(input: &SemanticSourceFingerprintInput) -> String {
    let mut canonical = Vec::new();
    for document in &input.documents {
        canonical.push(format!(
            "document|{}|{}|{}|{}",
            document.document_id.0,
            document
                .updated_at
                .to_rfc3339_opts(SecondsFormat::Nanos, true),
            normalized_versions(&document.parse_versions).join(","),
            normalized_versions(&document.fact_snapshot_versions).join(",")
        ));
    }
    for asset in &input.assets {
        canonical.push(format!(
            "asset|{}|{}|{}",
            asset.asset_id,
            asset.updated_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
            normalized_versions(&asset.profile_versions).join(",")
        ));
    }
    canonical.push(format!(
        "dataset_fact_snapshot|{}",
        input
            .dataset_fact_snapshot_version
            .as_deref()
            .unwrap_or("none")
            .trim()
    ));
    for version in normalized_versions(&input.dictionary_versions) {
        canonical.push(format!("semantic_dictionary|{version}"));
    }
    canonical.sort();
    canonical.dedup();

    let mut digest = Sha256::new();
    for line in canonical {
        digest.update(line.as_bytes());
        digest.update(b"\n");
    }
    let mut encoded = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

fn normalized_versions(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::DocumentId;
    use uuid::Uuid;

    use super::*;

    fn document_id(value: u128) -> DocumentId {
        DocumentId(Uuid::from_u128(value))
    }

    #[test]
    fn direct_and_membership_ownership_are_unified_and_deduplicated() {
        let direct = vec![document_id(1), document_id(3)];
        let memberships = vec![document_id(2), document_id(3)];

        let merged = merge_dataset_document_ownership(&direct, &memberships);

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].document_id, document_id(1));
        assert!(merged[0].direct && !merged[0].membership);
        assert!(merged[1].membership && !merged[1].direct);
        assert!(merged[2].direct && merged[2].membership);
    }

    #[test]
    fn source_fingerprint_is_order_independent_and_version_sensitive() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 13, 20, 0, 0)
            .single()
            .expect("fixed timestamp");
        let document = |id, parse_version: &str| SemanticDocumentSourceVersion {
            document_id: document_id(id),
            updated_at: timestamp,
            parse_versions: vec![parse_version.to_string()],
            fact_snapshot_versions: vec!["facts-v1".to_string()],
        };
        let input = SemanticSourceFingerprintInput {
            documents: vec![document(2, "parser-v1"), document(1, "parser-v1")],
            assets: vec![SemanticAssetSourceVersion {
                asset_id: Uuid::from_u128(4),
                updated_at: timestamp,
                profile_versions: vec!["profile-v1".to_string()],
            }],
            dataset_fact_snapshot_version: Some("snapshot-v1".to_string()),
            dictionary_versions: vec!["dictionary-entry-1@v1".to_string()],
        };
        let mut reordered = input.clone();
        reordered.documents.reverse();
        reordered.documents[0].parse_versions =
            vec!["parser-v1".to_string(), "parser-v1".to_string()];

        assert_eq!(source_fingerprint(&input), source_fingerprint(&reordered));
        assert_eq!(source_fingerprint(&input).len(), 64);

        let mut changed = input;
        changed.documents[0].parse_versions = vec!["parser-v2".to_string()];
        assert_ne!(source_fingerprint(&changed), source_fingerprint(&reordered));

        let mut dictionary_changed = reordered.clone();
        dictionary_changed.dictionary_versions = vec!["dictionary-entry-1@v2".to_string()];
        assert_ne!(
            source_fingerprint(&dictionary_changed),
            source_fingerprint(&reordered)
        );
    }
}
