use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use chrono::{DateTime, SecondsFormat, Utc};
use domain_model::{DatasetId, DocumentId, SecretBindingId, TenantId, UserId};
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
    pub source_dataset_id: DatasetId,
    pub owner_user_id: Option<UserId>,
    pub secret_binding_ids: Vec<SecretBindingId>,
    pub updated_at: DateTime<Utc>,
    pub parse_versions: Vec<String>,
    pub fact_snapshot_versions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDatasetScopeVersion {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub owner_user_id: Option<UserId>,
    pub visibility: String,
    pub default_secret_binding_ids: Vec<SecretBindingId>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDatasetMembershipVersion {
    pub document_id: DocumentId,
    pub membership_kind: String,
    pub source: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSourceIdentityVersion {
    pub source_kind: String,
    pub source_system_key: String,
    pub source_schema_key: String,
    pub source_object_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAssetSourceVersion {
    pub asset_id: Uuid,
    pub updated_at: DateTime<Utc>,
    pub profile_versions: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticSourceFingerprintInput {
    pub dataset_scope: Option<SemanticDatasetScopeVersion>,
    pub documents: Vec<SemanticDocumentSourceVersion>,
    pub assets: Vec<SemanticAssetSourceVersion>,
    pub memberships: Vec<SemanticDatasetMembershipVersion>,
    pub source_identities: Vec<SemanticSourceIdentityVersion>,
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
    if let Some(scope) = &input.dataset_scope {
        canonical.push(canonical_component(
            "dataset_scope",
            &[
                scope.tenant_id.to_string(),
                scope.dataset_id.to_string(),
                optional_user_id(scope.owner_user_id),
                normalized_identity_part(&scope.visibility),
                normalized_secret_binding_ids(&scope.default_secret_binding_ids).join(","),
                scope.updated_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
            ],
        ));
    }
    for document in &input.documents {
        canonical.push(canonical_component(
            "document",
            &[
                document.document_id.to_string(),
                document.source_dataset_id.to_string(),
                optional_user_id(document.owner_user_id),
                normalized_secret_binding_ids(&document.secret_binding_ids).join(","),
                document
                    .updated_at
                    .to_rfc3339_opts(SecondsFormat::Nanos, true),
                normalized_versions(&document.parse_versions).join(","),
                normalized_versions(&document.fact_snapshot_versions).join(","),
            ],
        ));
    }
    for asset in &input.assets {
        canonical.push(canonical_component(
            "asset",
            &[
                asset.asset_id.to_string(),
                asset.updated_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
                normalized_versions(&asset.profile_versions).join(","),
            ],
        ));
    }
    for membership in &input.memberships {
        canonical.push(canonical_component(
            "membership",
            &[
                membership.document_id.to_string(),
                normalized_identity_part(&membership.membership_kind),
                membership.source.trim().to_string(),
                membership
                    .expires_at
                    .map(|value| value.to_rfc3339_opts(SecondsFormat::Nanos, true))
                    .unwrap_or_else(|| "none".to_string()),
                membership
                    .created_at
                    .to_rfc3339_opts(SecondsFormat::Nanos, true),
            ],
        ));
    }
    for identity in &input.source_identities {
        canonical.push(canonical_component(
            "source_identity",
            &[
                normalized_identity_part(&identity.source_kind),
                normalized_identity_part(&identity.source_system_key),
                normalized_identity_part(&identity.source_schema_key),
                normalized_identity_part(&identity.source_object_key),
            ],
        ));
    }
    canonical.push(canonical_component(
        "dataset_fact_snapshot",
        &[input
            .dataset_fact_snapshot_version
            .as_deref()
            .unwrap_or("none")
            .trim()
            .to_string()],
    ));
    for version in normalized_versions(&input.dictionary_versions) {
        canonical.push(canonical_component("semantic_dictionary", &[version]));
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

fn canonical_component(kind: &str, parts: &[String]) -> String {
    let mut output = format!("{}:{}", kind.len(), kind);
    for part in parts {
        output.push('|');
        output.push_str(&part.len().to_string());
        output.push(':');
        output.push_str(part);
    }
    output
}

fn optional_user_id(value: Option<UserId>) -> String {
    value
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn normalized_secret_binding_ids(values: &[SecretBindingId]) -> Vec<String> {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalized_identity_part(value: &str) -> String {
    value.trim().to_ascii_lowercase()
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
    use domain_model::{DatasetId, DocumentId, SecretBindingId, TenantId, UserId};
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
            source_dataset_id: DatasetId(Uuid::from_u128(20)),
            owner_user_id: None,
            secret_binding_ids: Vec::new(),
            updated_at: timestamp,
            parse_versions: vec![parse_version.to_string()],
            fact_snapshot_versions: vec!["facts-v1".to_string()],
        };
        let input = SemanticSourceFingerprintInput {
            dataset_scope: Some(SemanticDatasetScopeVersion {
                tenant_id: TenantId(Uuid::from_u128(10)),
                dataset_id: DatasetId(Uuid::from_u128(20)),
                owner_user_id: None,
                visibility: "public".to_string(),
                default_secret_binding_ids: Vec::new(),
                updated_at: timestamp,
            }),
            documents: vec![document(2, "parser-v1"), document(1, "parser-v1")],
            assets: vec![SemanticAssetSourceVersion {
                asset_id: Uuid::from_u128(4),
                updated_at: timestamp,
                profile_versions: vec!["profile-v1".to_string()],
            }],
            memberships: vec![SemanticDatasetMembershipVersion {
                document_id: document_id(2),
                membership_kind: "linked".to_string(),
                source: "dataset_graph".to_string(),
                expires_at: None,
                created_at: timestamp,
            }],
            source_identities: vec![SemanticSourceIdentityVersion {
                source_kind: "database".to_string(),
                source_system_key: "erp-a".to_string(),
                source_schema_key: "finance".to_string(),
                source_object_key: "lease_contract".to_string(),
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

        let mut membership_changed = reordered.clone();
        membership_changed.memberships[0].membership_kind = "canonical".to_string();
        assert_ne!(
            source_fingerprint(&membership_changed),
            source_fingerprint(&reordered)
        );

        let mut dataset_scope_changed = reordered.clone();
        let scope = dataset_scope_changed
            .dataset_scope
            .as_mut()
            .expect("dataset scope");
        scope.visibility = "private".to_string();
        scope.owner_user_id = Some(UserId(Uuid::from_u128(30)));
        scope.default_secret_binding_ids = vec![SecretBindingId(Uuid::from_u128(40))];
        assert_ne!(
            source_fingerprint(&dataset_scope_changed),
            source_fingerprint(&reordered)
        );

        let mut source_identity_changed = reordered.clone();
        source_identity_changed.source_identities[0].source_schema_key = "retail".to_string();
        assert_ne!(
            source_fingerprint(&source_identity_changed),
            source_fingerprint(&reordered)
        );

        let mut document_scope_changed = reordered.clone();
        document_scope_changed.documents[0].source_dataset_id = DatasetId(Uuid::from_u128(99));
        assert_ne!(
            source_fingerprint(&document_scope_changed),
            source_fingerprint(&reordered)
        );
    }
}
