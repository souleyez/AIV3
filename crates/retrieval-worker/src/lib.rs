use anyhow::{anyhow, Result};
use domain_model::{DatasetId, DocumentId, TenantId};
use platform_api::cross_dataset_semantic_graph::{
    match_cross_dataset_semantic_graph, CrossDatasetCanonicalIdentity,
    CrossDatasetExplicitRelationInput, CrossDatasetExplicitRelationKind,
    CrossDatasetSemanticEndpointInput, CrossDatasetSemanticMatchInput,
    CrossDatasetSemanticMatcherLimits, CrossDatasetSemanticMatcherOptions,
    DatasetSemanticGraphDataset, DatasetSemanticGraphNodeKind, DatasetSemanticGraphV1,
    DATASET_SEMANTIC_GRAPH_GENERATION_VERSION,
};
use platform_api::semantic_label_resolver::{
    reviewed_mall_delivery_field_alias, reviewed_semantic_field_alias,
    reviewed_semantic_object_alias,
};
use platform_api::semantic_understanding::{
    stable_semantic_id, DatasetSemanticUnderstanding, SemanticEvidenceClass, SemanticField,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use storage::{DatasetSemanticLinkRun, DatasetSemanticSnapshot};

pub use platform_api::dataset_semantic_snapshot::{
    dataset_cross_semantic_graph_access, dataset_cross_semantic_graph_access_from_values,
    DatasetCrossSemanticGraphAccess, DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV,
    DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV, DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV,
};

const SIGNATURE_TERM_LIMIT: usize = 12;
const TERM_WEIGHT_LIMIT: usize = 16;
const CJK_NGRAM_MAX: usize = 6;

pub const DATASET_SEMANTIC_UNDERSTANDING_ENABLED_ENV: &str =
    "DATASET_SEMANTIC_UNDERSTANDING_ENABLED";
pub const DATASET_SEMANTIC_UNDERSTANDING_TENANT_ALLOWLIST_ENV: &str =
    "DATASET_SEMANTIC_UNDERSTANDING_TENANT_ALLOWLIST";
pub const DATASET_SEMANTIC_UNDERSTANDING_DATASET_ALLOWLIST_ENV: &str =
    "DATASET_SEMANTIC_UNDERSTANDING_DATASET_ALLOWLIST";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetSemanticUnderstandingAccess {
    Allowed,
    FeatureDisabled,
    TenantNotAllowlisted,
    DatasetNotAllowlisted,
}

impl DatasetSemanticUnderstandingAccess {
    pub fn is_allowed(self) -> bool {
        self == Self::Allowed
    }

    pub fn safe_reason(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::FeatureDisabled => "feature_disabled",
            Self::TenantNotAllowlisted => "tenant_not_allowlisted",
            Self::DatasetNotAllowlisted => "dataset_not_allowlisted",
        }
    }
}

pub fn dataset_semantic_understanding_access(
    tenant_id: TenantId,
    dataset_id: DatasetId,
) -> DatasetSemanticUnderstandingAccess {
    dataset_semantic_understanding_access_from_values(
        env_flag_value(DATASET_SEMANTIC_UNDERSTANDING_ENABLED_ENV, false),
        std::env::var(DATASET_SEMANTIC_UNDERSTANDING_TENANT_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        std::env::var(DATASET_SEMANTIC_UNDERSTANDING_DATASET_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        tenant_id,
        dataset_id,
    )
}

pub fn dataset_semantic_understanding_access_from_values(
    enabled: bool,
    tenant_allowlist: Option<&str>,
    dataset_allowlist: Option<&str>,
    tenant_id: TenantId,
    dataset_id: DatasetId,
) -> DatasetSemanticUnderstandingAccess {
    if !enabled {
        return DatasetSemanticUnderstandingAccess::FeatureDisabled;
    }
    if !uuid_csv_contains(tenant_allowlist, tenant_id.0) {
        return DatasetSemanticUnderstandingAccess::TenantNotAllowlisted;
    }
    if !uuid_csv_contains(dataset_allowlist, dataset_id.0) {
        return DatasetSemanticUnderstandingAccess::DatasetNotAllowlisted;
    }
    DatasetSemanticUnderstandingAccess::Allowed
}

pub fn dataset_cross_semantic_graph_worker_enabled(tenant_id: TenantId) -> bool {
    env_flag_value(DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV, false)
        && uuid_csv_contains(
            std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV)
                .ok()
                .as_deref(),
            tenant_id.0,
        )
}

pub fn dataset_cross_semantic_graph_allowed_dataset_ids() -> Vec<DatasetId> {
    dataset_cross_semantic_graph_allowed_dataset_ids_from_value(
        std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
    )
}

pub fn dataset_cross_semantic_graph_allowed_dataset_ids_from_value(
    value: Option<&str>,
) -> Vec<DatasetId> {
    value
        .into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.trim().parse::<uuid::Uuid>().ok())
        .map(DatasetId)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn uuid_csv_contains(csv: Option<&str>, expected: uuid::Uuid) -> bool {
    csv.into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.trim().parse::<uuid::Uuid>().ok())
        .any(|value| value == expected)
}

fn env_flag_value(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

#[derive(Clone, Debug)]
pub struct DatasetSemanticLinkMatchInput {
    pub left: DatasetSemanticUnderstanding,
    pub right: DatasetSemanticUnderstanding,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DatasetSemanticLinkSafeSummary {
    pub pair_id: String,
    pub generation_version: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub confirmed_edge_count: usize,
    pub observed_edge_count: usize,
    pub inferred_edge_count: usize,
    pub chinese_label_count: usize,
    pub generic_label_count: usize,
}

pub fn build_dataset_semantic_link_graph(
    input: &DatasetSemanticLinkMatchInput,
) -> Result<DatasetSemanticGraphV1> {
    let left_dataset_id = input.left.dataset.id;
    let right_dataset_id = input.right.dataset.id;
    if left_dataset_id == right_dataset_id {
        return Err(anyhow!(
            "semantic link snapshots must belong to distinct datasets"
        ));
    }
    if input.left.status != "ready" || input.right.status != "ready" {
        return Err(anyhow!(
            "semantic link matcher requires two ready snapshots"
        ));
    }

    let mut endpoints = Vec::new();
    let mut explicit_relations = Vec::new();
    let left_concepts =
        append_snapshot_semantic_endpoints(&mut endpoints, &mut explicit_relations, &input.left);
    let right_concepts =
        append_snapshot_semantic_endpoints(&mut endpoints, &mut explicit_relations, &input.right);
    if snapshots_are_temporally_complementary(&left_concepts, &right_concepts) {
        explicit_relations.push(CrossDatasetExplicitRelationInput {
            source_dataset_id: left_dataset_id,
            source_local_node_id: DATASET_ROOT_LOCAL_NODE_ID.to_string(),
            target_dataset_id: right_dataset_id,
            target_local_node_id: DATASET_ROOT_LOCAL_NODE_ID.to_string(),
            kind: CrossDatasetExplicitRelationKind::TemporalComplementarity,
            evidence_class: SemanticEvidenceClass::Inferred,
        });
    }
    for identity in shared_document_identities(&input.left, &input.right) {
        for dataset_id in [left_dataset_id, right_dataset_id] {
            endpoints.push(CrossDatasetSemanticEndpointInput {
                dataset_id,
                local_node_id: stable_semantic_id(
                    "document",
                    &[&dataset_id.to_string(), &identity],
                ),
                kind: DatasetSemanticGraphNodeKind::Document,
                display_label: "共享资料".to_string(),
                visible_provenance_count: 1,
                identities: vec![CrossDatasetCanonicalIdentity::SharedDocumentMembership {
                    shared_document_id: identity.clone(),
                }],
            });
        }
    }

    let graph = match_cross_dataset_semantic_graph(&CrossDatasetSemanticMatchInput {
        root_dataset_id: left_dataset_id.min(right_dataset_id),
        datasets: vec![
            DatasetSemanticGraphDataset {
                id: left_dataset_id,
                title: safe_semantic_link_label(&input.left.dataset.title, "数据集"),
                stale: input.left.stale,
            },
            DatasetSemanticGraphDataset {
                id: right_dataset_id,
                title: safe_semantic_link_label(&input.right.dataset.title, "数据集"),
                stale: input.right.stale,
            },
        ],
        endpoints,
        explicit_relations,
        options: CrossDatasetSemanticMatcherOptions::default(),
        limits: CrossDatasetSemanticMatcherLimits {
            max_datasets: 8,
            max_nodes: 240,
            max_edges: 360,
        },
        stale: input.left.stale || input.right.stale,
    });
    let manifest = serde_json::to_value(&graph)?;
    if !dataset_semantic_link_manifest_is_safe(&manifest) {
        return Err(anyhow!("semantic link manifest failed safe-output audit"));
    }
    Ok(graph)
}

fn shared_document_identities(
    left: &DatasetSemanticUnderstanding,
    right: &DatasetSemanticUnderstanding,
) -> Vec<String> {
    let left = semantic_document_source_ids(left);
    let right = semantic_document_source_ids(right);
    left.intersection(&right).cloned().collect()
}

fn semantic_document_source_ids(snapshot: &DatasetSemanticUnderstanding) -> BTreeSet<String> {
    snapshot
        .objects
        .iter()
        .flat_map(|object| object.evidence_refs.iter())
        .chain(
            snapshot
                .fields
                .iter()
                .flat_map(|field| field.evidence_refs.iter()),
        )
        .chain(
            snapshot
                .relations
                .iter()
                .flat_map(|relation| relation.evidence_refs.iter()),
        )
        .filter(|evidence| !evidence.source_kind.eq_ignore_ascii_case("asset"))
        .filter_map(|evidence| {
            evidence
                .source_id
                .trim()
                .parse::<uuid::Uuid>()
                .ok()
                .map(|id| id.to_string())
        })
        .collect()
}

const DATASET_ROOT_LOCAL_NODE_ID: &str = "dataset:root";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MallBusinessConcept {
    Time,
    Space,
    TrafficMetric,
    VisitorProfile,
    DataQuality,
    PrivacyBoundary,
    RetailFormat,
    StatisticalGrain,
    StatisticalScale,
}

impl MallBusinessConcept {
    fn key(self) -> &'static str {
        match self {
            Self::Time => "business-time",
            Self::Space => "mall-space",
            Self::TrafficMetric => "traffic-metric",
            Self::VisitorProfile => "visitor-profile",
            Self::DataQuality => "data-quality",
            Self::PrivacyBoundary => "privacy-boundary",
            Self::RetailFormat => "retail-format",
            Self::StatisticalGrain => "statistical-grain",
            Self::StatisticalScale => "statistical-scale",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Time => "时间维度",
            Self::Space => "空间点位",
            Self::TrafficMetric => "客流指标",
            Self::VisitorProfile => "人群画像",
            Self::DataQuality => "数据质量",
            Self::PrivacyBoundary => "隐私边界",
            Self::RetailFormat => "业态分类",
            Self::StatisticalGrain => "统计粒度",
            Self::StatisticalScale => "统计规模",
        }
    }

    fn local_node_id(self) -> String {
        format!("concept:{}", self.key())
    }
}

fn snapshot_matches_reviewed_mall_delivery(snapshot: &DatasetSemanticUnderstanding) -> bool {
    let signals = snapshot
        .fields
        .iter()
        .flat_map(|field| [&field.technical_name, &field.label])
        .map(|value| normalized_semantic_signal(value))
        .collect::<BTreeSet<_>>();
    let marker_count = |markers: &[&str]| {
        markers
            .iter()
            .filter(|&&marker| signals.contains(marker))
            .count()
    };
    let title_signal = normalized_semantic_signal(&snapshot.dataset.title);
    let title_hint = ["客流", "商场", "mall", "aibee"]
        .iter()
        .any(|marker| title_signal.contains(marker));
    let delivery_object_hint = snapshot.objects.iter().any(|object| {
        let signal = format!(
            "{} {}",
            normalized_semantic_signal(&object.technical_name),
            normalized_semantic_signal(&object.label)
        );
        [
            "traffichourly",
            "pointinventory",
            "agedistribution",
            "genderdistribution",
            "grouptypedistribution",
            "dailysummary",
        ]
        .iter()
        .any(|marker| signal.contains(marker))
    });
    let traffic_signature =
        marker_count(&["trafficin", "trafficout", "visitors", "averagestay"]) >= 2;
    let profile_marker_count = marker_count(&[
        "agebucket",
        "gender",
        "grouptype",
        "uniquepidcount",
        "uniquepersonidcount",
    ]);
    let profile_strong_marker = ["grouptype", "uniquepidcount", "uniquepersonidcount"]
        .iter()
        .any(|marker| signals.contains(*marker));
    let profile_signature = profile_strong_marker && profile_marker_count >= 2;
    (title_hint || delivery_object_hint) && (traffic_signature || profile_signature)
}

fn append_snapshot_semantic_endpoints(
    endpoints: &mut Vec<CrossDatasetSemanticEndpointInput>,
    explicit_relations: &mut Vec<CrossDatasetExplicitRelationInput>,
    snapshot: &DatasetSemanticUnderstanding,
) -> BTreeSet<MallBusinessConcept> {
    let reviewed_mall_delivery = snapshot_matches_reviewed_mall_delivery(snapshot);
    endpoints.push(CrossDatasetSemanticEndpointInput {
        dataset_id: snapshot.dataset.id,
        local_node_id: DATASET_ROOT_LOCAL_NODE_ID.to_string(),
        kind: DatasetSemanticGraphNodeKind::Dataset,
        display_label: safe_semantic_link_label(&snapshot.dataset.title, "数据集"),
        visible_provenance_count: snapshot.coverage.document_count.max(1),
        identities: Vec::new(),
    });

    let visible_fields = snapshot
        .fields
        .iter()
        .filter(|field| !semantic_field_is_direct_person_identifier(field))
        .collect::<Vec<_>>();
    let visible_object_ids = snapshot
        .objects
        .iter()
        .map(|object| object.id.trim())
        .filter(|id| !id.is_empty())
        .collect::<BTreeSet<_>>();

    for object in &snapshot.objects {
        endpoints.push(CrossDatasetSemanticEndpointInput {
            dataset_id: snapshot.dataset.id,
            local_node_id: object.id.clone(),
            kind: DatasetSemanticGraphNodeKind::Object,
            display_label: semantic_graph_object_label(
                snapshot,
                object.id.as_str(),
                reviewed_mall_delivery,
            ),
            visible_provenance_count: object.coverage_count.max(1),
            identities: Vec::new(),
        });
        explicit_relations.push(CrossDatasetExplicitRelationInput {
            source_dataset_id: snapshot.dataset.id,
            source_local_node_id: DATASET_ROOT_LOCAL_NODE_ID.to_string(),
            target_dataset_id: snapshot.dataset.id,
            target_local_node_id: object.id.clone(),
            kind: CrossDatasetExplicitRelationKind::DatasetContainsObject,
            evidence_class: SemanticEvidenceClass::Observed,
        });
    }

    let mut concepts = BTreeSet::new();
    for field in visible_fields {
        let identities = semantic_v3_opaque_field_identity(snapshot, &field.id)
            .into_iter()
            .collect();
        endpoints.push(CrossDatasetSemanticEndpointInput {
            dataset_id: snapshot.dataset.id,
            local_node_id: field.id.clone(),
            kind: DatasetSemanticGraphNodeKind::Field,
            display_label: semantic_graph_field_label(field, reviewed_mall_delivery),
            visible_provenance_count: field.non_empty_count.max(1),
            identities,
        });
        if visible_object_ids.contains(field.object_id.trim()) {
            explicit_relations.push(CrossDatasetExplicitRelationInput {
                source_dataset_id: snapshot.dataset.id,
                source_local_node_id: field.object_id.clone(),
                target_dataset_id: snapshot.dataset.id,
                target_local_node_id: field.id.clone(),
                kind: CrossDatasetExplicitRelationKind::ObjectContainsField,
                evidence_class: SemanticEvidenceClass::Observed,
            });
        }
        for concept in semantic_field_business_concepts(field, reviewed_mall_delivery) {
            concepts.insert(concept);
            explicit_relations.push(CrossDatasetExplicitRelationInput {
                source_dataset_id: snapshot.dataset.id,
                source_local_node_id: field.id.clone(),
                target_dataset_id: snapshot.dataset.id,
                target_local_node_id: concept.local_node_id(),
                kind: CrossDatasetExplicitRelationKind::FieldExpressesConcept,
                evidence_class: SemanticEvidenceClass::Observed,
            });
        }
    }

    for concept in &concepts {
        endpoints.push(CrossDatasetSemanticEndpointInput {
            dataset_id: snapshot.dataset.id,
            local_node_id: concept.local_node_id(),
            kind: DatasetSemanticGraphNodeKind::Concept,
            display_label: concept.label().to_string(),
            visible_provenance_count: snapshot
                .fields
                .iter()
                .filter(|field| {
                    semantic_field_business_concepts(field, reviewed_mall_delivery)
                        .contains(concept)
                })
                .count()
                .max(1) as u64,
            identities: vec![CrossDatasetCanonicalIdentity::ConfirmedConcept {
                concept_id: concept.key().to_string(),
            }],
        });
    }
    concepts
}

fn semantic_graph_object_label(
    snapshot: &DatasetSemanticUnderstanding,
    object_id: &str,
    reviewed_mall_delivery: bool,
) -> String {
    let Some(object) = snapshot
        .objects
        .iter()
        .find(|object| object.id == object_id)
    else {
        return "业务对象".to_string();
    };
    if reviewed_mall_delivery {
        if let Some(label) = reviewed_semantic_object_alias(&object.technical_name)
            .or_else(|| reviewed_semantic_object_alias(&object.label))
        {
            return label.to_string();
        }
    }
    let safe_label = safe_semantic_link_label(&object.label, "");
    if safe_label
        .chars()
        .filter(|character| is_cjk_business_character(*character))
        .count()
        >= 2
        && !matches!(safe_label.as_str(), "业务对象" | "待解释对象")
    {
        return safe_label;
    }

    let concepts = snapshot
        .fields
        .iter()
        .filter(|field| field.object_id == object_id)
        .flat_map(|field| semantic_field_business_concepts(field, reviewed_mall_delivery))
        .collect::<BTreeSet<_>>();
    if concepts.contains(&MallBusinessConcept::TrafficMetric)
        && concepts.contains(&MallBusinessConcept::Space)
    {
        "客流时空数据".to_string()
    } else if concepts.contains(&MallBusinessConcept::VisitorProfile) {
        "客流画像汇总".to_string()
    } else if concepts.contains(&MallBusinessConcept::DataQuality) {
        "数据质量校验".to_string()
    } else if concepts.contains(&MallBusinessConcept::TrafficMetric) {
        "客流指标数据".to_string()
    } else if concepts.contains(&MallBusinessConcept::Space) {
        "空间点位数据".to_string()
    } else {
        "业务对象".to_string()
    }
}

fn semantic_graph_field_label(field: &SemanticField, reviewed_mall_delivery: bool) -> String {
    reviewed_mall_delivery
        .then(|| reviewed_mall_delivery_field_alias(&field.technical_name))
        .flatten()
        .or_else(|| {
            reviewed_mall_delivery
                .then(|| reviewed_mall_delivery_field_alias(&field.label))
                .flatten()
        })
        .or_else(|| reviewed_semantic_field_alias(&field.technical_name))
        .or_else(|| reviewed_semantic_field_alias(&field.label))
        .map(str::to_string)
        .unwrap_or_else(|| safe_semantic_link_label(&field.label, "业务字段"))
}

fn semantic_field_is_direct_person_identifier(field: &SemanticField) -> bool {
    [&field.technical_name, &field.label]
        .into_iter()
        .any(|value| {
            !approved_person_identifier_aggregate(value)
                && contains_direct_person_identifier_marker(value)
        })
}

fn approved_person_identifier_aggregate(value: &str) -> bool {
    matches!(
        normalized_semantic_signal(value).as_str(),
        "uniquepidcount" | "uniquepersonidcount" | "missingpidcount" | "missingpersonidcount"
    )
}

fn semantic_field_business_concepts(
    field: &SemanticField,
    reviewed_mall_delivery: bool,
) -> Vec<MallBusinessConcept> {
    if !reviewed_mall_delivery || semantic_field_is_direct_person_identifier(field) {
        return Vec::new();
    }
    let technical = normalized_semantic_signal(&field.technical_name);
    let label = normalized_semantic_signal(&field.label);
    let signal = format!("{technical} {label}");
    let contains_any = |needles: &[&str]| needles.iter().any(|needle| signal.contains(needle));
    let equals_any = |needles: &[&str]| {
        needles
            .iter()
            .any(|needle| technical == *needle || label == *needle)
    };
    let quality = contains_any(&[
        "duplicate",
        "missing",
        "mismatch",
        "invalid",
        "quality",
        "validation",
        "error",
        "重复",
        "缺失",
        "不一致",
        "无效",
        "质量",
        "校验",
        "异常",
    ]);
    let mut concepts = BTreeSet::new();
    if !quality
        && contains_any(&[
            "businessdate",
            "timestamp",
            "datetime",
            "date",
            "day",
            "hour",
            "minute",
            "日期",
            "时间",
            "小时",
            "分钟",
        ])
    {
        concepts.insert(MallBusinessConcept::Time);
    }
    if contains_any(&[
        "sourcemallid",
        "mallid",
        "entityid",
        "entityname",
        "entitytype",
        "floor",
        "area",
        "location",
        "region",
        "store",
        "gate",
        "passage",
        "商场",
        "点位",
        "空间",
        "楼层",
        "区域",
        "门店",
        "出入口",
        "通道",
    ]) {
        concepts.insert(MallBusinessConcept::Space);
    }
    if contains_any(&[
        "trafficin",
        "trafficout",
        "visitors",
        "visitorcount",
        "averagestay",
        "进入人次",
        "离开人次",
        "去重到访",
        "平均停留",
        "客流数量",
    ]) {
        concepts.insert(MallBusinessConcept::TrafficMetric);
    }
    if equals_any(&["agebucket", "age", "年龄段", "年龄"])
        || contains_any(&[
            "gender",
            "grouptype",
            "share",
            "uniquepidcount",
            "uniquepersonidcount",
            "年龄段",
            "年龄",
            "性别",
            "同行关系",
            "结构占比",
            "人群画像",
        ])
    {
        concepts.insert(MallBusinessConcept::VisitorProfile);
    }
    if quality {
        concepts.insert(MallBusinessConcept::DataQuality);
    }
    if contains_any(&["uniquepidcount", "uniquepersonidcount", "日内去重画像数"]) {
        concepts.insert(MallBusinessConcept::PrivacyBoundary);
    }
    if contains_any(&["l1retailformat", "l2retailformat", "一级业态", "二级业态"]) {
        concepts.insert(MallBusinessConcept::RetailFormat);
    }
    if contains_any(&[
        "interval",
        "grain",
        "hour",
        "minute",
        "统计粒度",
        "小时",
        "分钟",
    ]) {
        concepts.insert(MallBusinessConcept::StatisticalGrain);
    }
    if contains_any(&[
        "recordcount",
        "visitorcount",
        "uniquepidcount",
        "uniquepersonidcount",
        "count",
        "share",
        "trafficin",
        "trafficout",
        "visitors",
        "记录数",
        "人次",
        "人数",
        "占比",
    ]) {
        concepts.insert(MallBusinessConcept::StatisticalScale);
    }
    concepts.into_iter().collect()
}

fn normalized_semantic_signal(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn snapshots_are_temporally_complementary(
    left: &BTreeSet<MallBusinessConcept>,
    right: &BTreeSet<MallBusinessConcept>,
) -> bool {
    left.contains(&MallBusinessConcept::Time)
        && right.contains(&MallBusinessConcept::Time)
        && ((left.contains(&MallBusinessConcept::TrafficMetric)
            && right.contains(&MallBusinessConcept::VisitorProfile))
            || (right.contains(&MallBusinessConcept::TrafficMetric)
                && left.contains(&MallBusinessConcept::VisitorProfile)))
}

fn semantic_v3_opaque_field_identity(
    snapshot: &DatasetSemanticUnderstanding,
    field_id: &str,
) -> Option<CrossDatasetCanonicalIdentity> {
    let opaque = field_id.trim().strip_prefix("field:")?;
    if snapshot.generation_version
        != platform_api::semantic_understanding::DATASET_SEMANTIC_GENERATION_VERSION
        || opaque.len() != 32
        || !opaque
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }
    Some(CrossDatasetCanonicalIdentity::ExactSourceField {
        source_system_key: "dataset-semantic-snapshot".to_string(),
        source_schema_key: "semantic-profile-v3".to_string(),
        source_object_key: "opaque-source-field".to_string(),
        source_field_key: field_id.trim().to_ascii_lowercase(),
    })
}

fn safe_semantic_link_label(value: &str, fallback: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = compact.to_ascii_lowercase();
    let windows_path = lower.as_bytes().get(1) == Some(&b':')
        && matches!(lower.as_bytes().get(2), Some(b'\\') | Some(b'/'));
    let internal = windows_path
        || lower.starts_with("\\\\")
        || contains_direct_person_identifier_marker(&lower)
        || [
            "/home/",
            "/users/",
            "/root/",
            "/tmp/",
            "/var/",
            "file://",
            "postgres://",
            "mysql://",
            "jdbc:",
            "s3://",
            "oss://",
            "sha256",
            "hmac",
            "http://",
            "https://",
            "token=",
            "password=",
            "passwd=",
            "secret=",
            "api_key=",
            "apikey=",
            "bearer ",
        ]
        .iter()
        .any(|marker| lower.contains(marker));
    if compact.is_empty()
        || compact.chars().count() > 48
        || compact.chars().any(char::is_control)
        || internal
        || looks_opaque_semantic_label(&compact)
    {
        fallback.to_string()
    } else {
        compact
    }
}

fn looks_opaque_semantic_label(value: &str) -> bool {
    if value.parse::<uuid::Uuid>().is_ok() {
        return true;
    }
    let compact = value
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | ':' | ' '))
        .collect::<String>();
    let hexadecimal = compact.len() >= 16
        && compact
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    let opaque_token = compact.len() >= 24
        && compact
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        && compact
            .chars()
            .any(|character| character.is_ascii_lowercase())
        && compact
            .chars()
            .any(|character| character.is_ascii_uppercase())
        && compact.chars().any(|character| character.is_ascii_digit());
    hexadecimal || opaque_token
}

pub fn dataset_semantic_link_manifest_is_safe(manifest: &Value) -> bool {
    let serialized = match serde_json::to_string(manifest) {
        Ok(serialized) => serialized.to_ascii_lowercase(),
        Err(_) => return false,
    };
    !contains_direct_person_identifier_marker(&serialized)
        && ![
            "sha256",
            "hmac",
            "content_identity",
            "source_identity",
            "technical_name",
            "observed_values",
            "examples",
            "raw_value",
            "object_key",
            "token=",
            "password=",
            "passwd=",
            "secret=",
            "api_key=",
            "apikey=",
            "bearer ",
            "file://",
            "http://",
            "https://",
            "postgres://",
            "mysql://",
            "jdbc:",
            "c:\\\\",
            "/home/",
            "/users/",
            "/root/",
        ]
        .iter()
        .any(|marker| serialized.contains(marker))
}

fn contains_direct_person_identifier_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if ["person_id", "person-id", "person id", "person_identifier"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return true;
    }
    let mut ascii_token = String::new();
    for character in lower.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_alphanumeric() {
            ascii_token.push(character.to_ascii_lowercase());
            continue;
        }
        if ascii_token_is_direct_person_identifier(&ascii_token) {
            return true;
        }
        ascii_token.clear();
    }
    false
}

fn ascii_token_is_direct_person_identifier(token: &str) -> bool {
    if matches!(token, "pid" | "personid" | "personidentifier")
        || token.contains("personid")
        || token.contains("personidentifier")
        || token.starts_with("pid")
    {
        return true;
    }
    token.contains("pid")
        && [
            "raw",
            "hash",
            "value",
            "source",
            "external",
            "customer",
            "visitor",
            "user",
            "subject",
            "member",
            "person",
            "key",
            "code",
            "identifier",
        ]
        .iter()
        .any(|marker| token.contains(marker))
}

pub fn dataset_semantic_link_public_pair_id(
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
    left_snapshot_id: uuid::Uuid,
    right_snapshot_id: uuid::Uuid,
) -> String {
    let (left_dataset_id, right_dataset_id, left_snapshot_id, right_snapshot_id) =
        storage::canonical_dataset_semantic_link_pair(
            left_dataset_id,
            right_dataset_id,
            left_snapshot_id,
            right_snapshot_id,
        )
        .expect("semantic link public pair id requires distinct endpoints");
    stable_semantic_id(
        "pair",
        &[
            &left_dataset_id.to_string(),
            &right_dataset_id.to_string(),
            &left_snapshot_id.to_string(),
            &right_snapshot_id.to_string(),
            DATASET_SEMANTIC_GRAPH_GENERATION_VERSION,
        ],
    )
}

pub fn dataset_semantic_link_safe_summary(
    pair_id: &str,
    graph: &DatasetSemanticGraphV1,
) -> DatasetSemanticLinkSafeSummary {
    let mut confirmed_edge_count = 0;
    let mut observed_edge_count = 0;
    let mut inferred_edge_count = 0;
    for edge in &graph.edges {
        match edge.evidence_class {
            SemanticEvidenceClass::Confirmed => confirmed_edge_count += 1,
            SemanticEvidenceClass::Observed => observed_edge_count += 1,
            SemanticEvidenceClass::Inferred => inferred_edge_count += 1,
        }
    }
    let chinese_label_count = graph
        .nodes
        .iter()
        .filter(|node| node.display_label.chars().any(is_cjk_business_character))
        .count();
    let generic_label_count = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.display_label.as_str(),
                "数据集" | "资料" | "业务对象" | "业务字段" | "知识概念" | "数据结构"
            )
        })
        .count();
    DatasetSemanticLinkSafeSummary {
        pair_id: pair_id.to_string(),
        generation_version: DATASET_SEMANTIC_GRAPH_GENERATION_VERSION.to_string(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        confirmed_edge_count,
        observed_edge_count,
        inferred_edge_count,
        chinese_label_count,
        generic_label_count,
    }
}

fn is_cjk_business_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
    )
}

pub fn build_dataset_semantic_link_graph_from_ready_snapshots(
    tenant_id: TenantId,
    left_snapshot: &DatasetSemanticSnapshot,
    right_snapshot: &DatasetSemanticSnapshot,
) -> Result<DatasetSemanticGraphV1> {
    if left_snapshot.tenant_id != tenant_id || right_snapshot.tenant_id != tenant_id {
        return Err(anyhow!("semantic link snapshot tenant mismatch"));
    }
    if left_snapshot.status != "ready" || right_snapshot.status != "ready" {
        return Err(anyhow!("semantic link inputs must both be ready"));
    }
    if left_snapshot.generation_version
        != platform_api::semantic_understanding::DATASET_SEMANTIC_GENERATION_VERSION
        || right_snapshot.generation_version
            != platform_api::semantic_understanding::DATASET_SEMANTIC_GENERATION_VERSION
    {
        return Err(anyhow!("semantic link input generation is unsupported"));
    }
    let left =
        serde_json::from_value::<DatasetSemanticUnderstanding>(left_snapshot.manifest.clone())?;
    let right =
        serde_json::from_value::<DatasetSemanticUnderstanding>(right_snapshot.manifest.clone())?;
    if left.dataset.id != left_snapshot.dataset_id || right.dataset.id != right_snapshot.dataset_id
    {
        return Err(anyhow!("semantic link manifest dataset identity mismatch"));
    }
    build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput { left, right })
}

pub fn semantic_link_run_inputs_are_current(
    run: &DatasetSemanticLinkRun,
    left_latest: &DatasetSemanticSnapshot,
    right_latest: &DatasetSemanticSnapshot,
) -> bool {
    run.generation_version == DATASET_SEMANTIC_GRAPH_GENERATION_VERSION
        && run.tenant_id == left_latest.tenant_id
        && run.tenant_id == right_latest.tenant_id
        && run.left_dataset_id == left_latest.dataset_id
        && run.right_dataset_id == right_latest.dataset_id
        && run.left_snapshot_id == left_latest.id
        && run.right_snapshot_id == right_latest.id
}

#[derive(Clone, Debug)]
pub struct RetrievalChunkInput {
    pub chunk_index: i32,
    pub content: String,
    pub token_count: usize,
}

#[derive(Clone, Debug)]
pub struct RetrievalIndexJob {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub chunks: Vec<RetrievalChunkInput>,
}

#[derive(Clone, Debug)]
pub struct RetrievalChunkProfile {
    pub chunk_index: i32,
    pub recall_score: f64,
    pub rank_hint: usize,
    pub signature_terms: Vec<String>,
    pub term_weights: BTreeMap<String, f64>,
    pub vector_norm: f64,
    pub token_count: usize,
}

#[derive(Clone, Debug)]
pub struct RetrievalIndexOutcome {
    pub embedded_chunks: u32,
    pub payload_filter_key: String,
    pub embedding_model: String,
    pub chunk_profiles: Vec<RetrievalChunkProfile>,
}

pub trait RetrievalIndexer {
    fn index(&self, job: &RetrievalIndexJob) -> RetrievalIndexOutcome;
}

#[derive(Clone, Debug)]
pub struct LocalLexicalRetrievalIndexer;

impl RetrievalIndexer for LocalLexicalRetrievalIndexer {
    fn index(&self, job: &RetrievalIndexJob) -> RetrievalIndexOutcome {
        let chunk_term_frequencies = job
            .chunks
            .iter()
            .map(|chunk| term_frequencies(&chunk.content))
            .collect::<Vec<_>>();
        let document_frequencies = document_frequencies(&chunk_term_frequencies);
        let chunk_count = job.chunks.len().max(1) as f64;

        let mut draft_profiles = job
            .chunks
            .iter()
            .zip(chunk_term_frequencies.iter())
            .map(|(chunk, frequencies)| {
                let weighted_terms =
                    weighted_terms_for_chunk(frequencies, &document_frequencies, chunk_count);
                let vector_norm = weighted_terms
                    .iter()
                    .map(|(_, weight)| weight * weight)
                    .sum::<f64>()
                    .sqrt();
                let signature_terms = weighted_terms
                    .iter()
                    .take(SIGNATURE_TERM_LIMIT)
                    .map(|(term, _)| term.clone())
                    .collect::<Vec<_>>();
                let term_weights = weighted_terms
                    .iter()
                    .take(TERM_WEIGHT_LIMIT)
                    .map(|(term, weight)| (term.clone(), round_metric(*weight)))
                    .collect::<BTreeMap<_, _>>();
                let lexical_salience =
                    lexical_salience_score(vector_norm, chunk.token_count, weighted_terms.len());

                (
                    chunk.chunk_index,
                    chunk.token_count,
                    signature_terms,
                    term_weights,
                    vector_norm,
                    lexical_salience,
                )
            })
            .collect::<Vec<_>>();

        let max_salience = draft_profiles
            .iter()
            .map(|(_, _, _, _, _, salience)| *salience)
            .fold(0.0_f64, f64::max);

        let mut ranked = draft_profiles
            .iter()
            .map(|(chunk_index, _, _, _, _, salience)| (*chunk_index, *salience))
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        let rank_hints = ranked
            .into_iter()
            .enumerate()
            .map(|(index, (chunk_index, _))| (chunk_index, index + 1))
            .collect::<BTreeMap<_, _>>();

        let chunk_profiles = draft_profiles
            .drain(..)
            .map(
                |(
                    chunk_index,
                    token_count,
                    signature_terms,
                    term_weights,
                    vector_norm,
                    lexical_salience,
                )| RetrievalChunkProfile {
                    chunk_index,
                    recall_score: if max_salience > 0.0 {
                        round_metric(lexical_salience / max_salience)
                    } else {
                        0.0
                    },
                    rank_hint: rank_hints.get(&chunk_index).copied().unwrap_or(1),
                    signature_terms,
                    term_weights,
                    vector_norm: round_metric(vector_norm),
                    token_count,
                },
            )
            .collect::<Vec<_>>();

        RetrievalIndexOutcome {
            embedded_chunks: job.chunks.len() as u32,
            payload_filter_key: format!("dataset/{}", job.dataset_id),
            embedding_model: "local-lexical-v1".to_string(),
            chunk_profiles,
        }
    }
}

fn document_frequencies(
    chunk_term_frequencies: &[BTreeMap<String, usize>],
) -> BTreeMap<String, usize> {
    let mut frequencies = BTreeMap::new();

    for terms in chunk_term_frequencies {
        for term in terms.keys().collect::<BTreeSet<_>>() {
            *frequencies.entry((*term).clone()).or_insert(0) += 1;
        }
    }

    frequencies
}

fn weighted_terms_for_chunk(
    term_frequencies: &BTreeMap<String, usize>,
    document_frequencies: &BTreeMap<String, usize>,
    chunk_count: f64,
) -> Vec<(String, f64)> {
    let mut weighted_terms = term_frequencies
        .iter()
        .map(|(term, count)| {
            let tf_weight = 1.0 + (*count as f64).ln();
            let document_frequency = *document_frequencies.get(term).unwrap_or(&1) as f64;
            let inverse_document_frequency =
                ((chunk_count + 1.0) / (document_frequency + 1.0)).ln() + 1.0;
            (
                term.clone(),
                tf_weight * inverse_document_frequency * cjk_phrase_boost(term),
            )
        })
        .collect::<Vec<_>>();
    weighted_terms.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    weighted_terms
}

fn lexical_salience_score(vector_norm: f64, token_count: usize, weighted_term_count: usize) -> f64 {
    if vector_norm <= 0.0 {
        return 0.0;
    }

    let token_factor = (token_count.max(1) as f64).ln() + 1.0;
    let diversity_factor = (weighted_term_count.max(1) as f64).sqrt();

    vector_norm * token_factor * diversity_factor
}

fn term_frequencies(content: &str) -> BTreeMap<String, usize> {
    let mut frequencies = BTreeMap::new();
    for token in tokenize(content) {
        *frequencies.entry(token).or_insert(0) += 1;
    }
    frequencies
}

fn tokenize(content: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii_token = String::new();
    let mut cjk_chars = Vec::new();

    for value in content.chars() {
        if value.is_ascii_alphanumeric() {
            flush_cjk_terms(&mut tokens, &mut cjk_chars);
            ascii_token.push(value.to_ascii_lowercase());
            continue;
        }
        flush_ascii_token(&mut tokens, &mut ascii_token);
        if is_cjk_token_char(value) {
            cjk_chars.push(value);
        } else {
            flush_cjk_terms(&mut tokens, &mut cjk_chars);
        }
    }
    flush_ascii_token(&mut tokens, &mut ascii_token);
    flush_cjk_terms(&mut tokens, &mut cjk_chars);
    tokens
}

pub fn lexical_search_terms(content: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    tokenize(content)
        .into_iter()
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

pub fn lexical_content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn flush_ascii_token(tokens: &mut Vec<String>, ascii_token: &mut String) {
    if let Some(token) = normalize_token(ascii_token) {
        tokens.push(token);
    }
    ascii_token.clear();
}

fn flush_cjk_terms(tokens: &mut Vec<String>, cjk_chars: &mut Vec<char>) {
    if cjk_chars.is_empty() {
        return;
    }

    for value in cjk_chars.iter() {
        tokens.push(value.to_string());
    }
    for ngram_size in 2..=CJK_NGRAM_MAX.min(cjk_chars.len()) {
        for window in cjk_chars.windows(ngram_size) {
            tokens.push(window.iter().collect::<String>());
        }
    }
    cjk_chars.clear();
}

fn is_cjk_token_char(value: char) -> bool {
    matches!(
        value as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
    )
}

fn cjk_phrase_boost(term: &str) -> f64 {
    let mut char_count = 0;
    let mut cjk_count = 0;
    for value in term.chars() {
        char_count += 1;
        if is_cjk_token_char(value) {
            cjk_count += 1;
        }
    }
    if char_count >= 2 && char_count == cjk_count {
        1.0 + ((char_count - 1) as f64 * 0.15).min(0.75)
    } else {
        1.0
    }
}

fn normalize_token(token: &str) -> Option<String> {
    if token.len() < 2 || is_stop_word(token) {
        return None;
    }

    Some(token.to_string())
}

fn is_stop_word(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "was"
            | "were"
            | "with"
    )
}

fn round_metric(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use platform_api::semantic_understanding::{
        DatasetSemanticIdentity, DatasetSemanticUnderstanding, SemanticCoverage,
        SemanticEvidenceRef, SemanticField, SemanticObject, SemanticStatus, SemanticSummary,
        SemanticTruncation, DATASET_SEMANTIC_GENERATION_VERSION, DATASET_SEMANTIC_SCHEMA_VERSION,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn semantic_snapshot(
        dataset_id: DatasetId,
        title: &str,
        object_id: &str,
        field_id: &str,
        field_label: &str,
        technical_name: &str,
    ) -> DatasetSemanticUnderstanding {
        DatasetSemanticUnderstanding {
            schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
            generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
            status: "ready".to_string(),
            dataset: DatasetSemanticIdentity {
                id: dataset_id,
                title: title.to_string(),
            },
            coverage: SemanticCoverage::default(),
            summary: SemanticSummary::default(),
            objects: vec![SemanticObject {
                id: object_id.to_string(),
                kind: "database_table".to_string(),
                label: "经营对象".to_string(),
                technical_name: "C:\\private\\raw-table".to_string(),
                description: "不进入跨集快照".to_string(),
                label_source: "fixture".to_string(),
                confidence: 1.0,
                coverage_count: 1,
                status: SemanticStatus::Observed,
                evidence_refs: Vec::new(),
            }],
            fields: vec![SemanticField {
                id: field_id.to_string(),
                object_id: object_id.to_string(),
                label: field_label.to_string(),
                technical_name: technical_name.to_string(),
                semantic_role: "identifier".to_string(),
                value_type: "text".to_string(),
                non_empty_count: 1,
                distinct_count: 1,
                examples: vec!["客户原始值-不可泄漏".to_string()],
                status: SemanticStatus::Observed,
                label_source: "fixture".to_string(),
                confidence: 1.0,
                evidence_refs: Vec::new(),
            }],
            relations: Vec::new(),
            source_groups: Vec::new(),
            pipeline: Vec::new(),
            generated_at: Utc::now(),
            stale: false,
            truncated: SemanticTruncation::default(),
        }
    }

    fn with_document_evidence(
        mut snapshot: DatasetSemanticUnderstanding,
        document_id: Uuid,
    ) -> DatasetSemanticUnderstanding {
        snapshot.objects[0].evidence_refs.push(SemanticEvidenceRef {
            source_kind: "database".to_string(),
            source_id: document_id.to_string(),
            label: "来源结构".to_string(),
        });
        snapshot
    }

    fn semantic_field(
        object_id: &str,
        field_id: &str,
        technical_name: &str,
        label: &str,
        semantic_role: &str,
    ) -> SemanticField {
        SemanticField {
            id: field_id.to_string(),
            object_id: object_id.to_string(),
            label: label.to_string(),
            technical_name: technical_name.to_string(),
            semantic_role: semantic_role.to_string(),
            value_type: "text".to_string(),
            non_empty_count: 15,
            distinct_count: 15,
            examples: vec!["raw-example-must-not-enter-link-graph".to_string()],
            status: SemanticStatus::Confirmed,
            label_source: "fixture".to_string(),
            confidence: 1.0,
            evidence_refs: Vec::new(),
        }
    }

    #[test]
    fn semantic_link_rollout_is_fail_closed_and_wildcard_is_not_special() {
        let tenant_id = TenantId::new();
        let left_dataset_id = DatasetId::new();
        let right_dataset_id = DatasetId::new();
        let tenant_allowlist = tenant_id.to_string();
        let dataset_allowlist = format!("{left_dataset_id},{right_dataset_id}");

        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                false,
                Some(&tenant_allowlist),
                Some(&dataset_allowlist),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::FeatureDisabled
        );
        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                true,
                Some("*"),
                Some("*"),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::TenantNotAllowlisted
        );
        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                true,
                Some(&tenant_allowlist),
                Some(&left_dataset_id.to_string()),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::RightDatasetNotAllowlisted
        );
        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                true,
                Some(&tenant_allowlist),
                Some(&dataset_allowlist),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::Allowed
        );
        assert!(dataset_cross_semantic_graph_allowed_dataset_ids_from_value(Some("*")).is_empty());
        let mut expected_dataset_ids = vec![left_dataset_id, right_dataset_id];
        expected_dataset_ids.sort();
        assert_eq!(
            dataset_cross_semantic_graph_allowed_dataset_ids_from_value(Some(&dataset_allowlist)),
            expected_dataset_ids
        );
    }

    #[test]
    fn semantic_link_matcher_shares_membership_documents_and_exact_opaque_fields_without_leaks() {
        let left_dataset_id = DatasetId(Uuid::from_u128(10));
        let right_dataset_id = DatasetId(Uuid::from_u128(20));
        let shared_field_id = "field:0123456789abcdef0123456789abcdef";
        let shared_document_id = Uuid::from_u128(99);
        let graph = build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput {
            left: with_document_evidence(
                semantic_snapshot(
                    left_dataset_id,
                    "项目资料",
                    "object:left",
                    shared_field_id,
                    "合同编号",
                    "raw_hmac_private_field",
                ),
                shared_document_id,
            ),
            right: with_document_evidence(
                semantic_snapshot(
                    right_dataset_id,
                    "经营分析",
                    "object:right",
                    shared_field_id,
                    "合同编号",
                    "postgres://private-host/raw",
                ),
                shared_document_id,
            ),
        })
        .expect("safe cross-dataset graph");
        let serialized = serde_json::to_string(&graph).expect("serializable graph");

        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.id.starts_with("shared:document:"))
                .count(),
            1
        );
        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.id.starts_with("shared:field:"))
                .count(),
            1
        );
        for forbidden in [
            &shared_document_id.to_string(),
            shared_field_id,
            "raw_hmac_private_field",
            "postgres://private-host/raw",
            "客户原始值-不可泄漏",
            "C:\\private\\raw-table",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
        assert!(dataset_semantic_link_manifest_is_safe(
            &serde_json::to_value(&graph).unwrap()
        ));
    }

    #[test]
    fn generic_count_share_and_date_fields_do_not_receive_mall_business_concepts() {
        let left_dataset_id = DatasetId(Uuid::from_u128(81));
        let right_dataset_id = DatasetId(Uuid::from_u128(82));
        let mut left = semantic_snapshot(
            left_dataset_id,
            "通用经营数据",
            "object:left",
            "field:count",
            "count",
            "count",
        );
        left.fields = vec![
            semantic_field("object:left", "field:count", "count", "count", "quantity"),
            semantic_field("object:left", "field:share", "share", "share", "unknown"),
            semantic_field("object:left", "field:date", "date", "date", "date"),
        ];
        let right = semantic_snapshot(
            right_dataset_id,
            "通用库存数据",
            "object:right",
            "field:count",
            "count",
            "count",
        );

        let graph =
            build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput { left, right })
                .expect("generic graph remains safe");

        assert!(!graph
            .nodes
            .iter()
            .any(|node| node.kind == DatasetSemanticGraphNodeKind::Concept));
        assert!(!graph.nodes.iter().any(|node| {
            matches!(
                node.display_label.as_str(),
                "分类记录数" | "结构占比" | "人群画像"
            )
        }));
        assert!(!graph
            .edges
            .iter()
            .any(|edge| edge.relation_type == "temporal_complementarity"));
    }

    #[test]
    fn mall_traffic_and_profile_pair_builds_grounded_structure_and_safe_analysis_bridge() {
        let traffic_dataset_id = DatasetId(Uuid::from_u128(101));
        let profile_dataset_id = DatasetId(Uuid::from_u128(202));
        let mut traffic = semantic_snapshot(
            traffic_dataset_id,
            "7月客流数据集",
            "object:traffic",
            "field:traffic-day",
            "day",
            "day",
        );
        traffic.objects[0].label = "traffic_hourly.csv".to_string();
        traffic.objects[0].technical_name = "C:\\restricted\\traffic_hourly.csv".to_string();
        traffic.fields = vec![
            semantic_field("object:traffic", "field:traffic-day", "day", "day", "date"),
            semantic_field(
                "object:traffic",
                "field:traffic-in",
                "trafficIn",
                "trafficIn",
                "metric",
            ),
            semantic_field(
                "object:traffic",
                "field:visitors",
                "visitors",
                "visitors",
                "metric",
            ),
            semantic_field(
                "object:traffic",
                "field:average-stay",
                "averageStay",
                "averageStay",
                "metric",
            ),
            semantic_field(
                "object:traffic",
                "field:entity-name",
                "entityName",
                "entityName",
                "location",
            ),
            semantic_field(
                "object:traffic",
                "field:retail-format",
                "l1RetailFormat",
                "l1RetailFormat",
                "category",
            ),
        ];

        let mut profile = semantic_snapshot(
            profile_dataset_id,
            "客流画像",
            "object:profile",
            "field:profile-date",
            "date",
            "date",
        );
        profile.objects[0].label = "daily_summary.csv".to_string();
        profile.objects[0].technical_name = "C:\\restricted\\daily_summary.csv".to_string();
        profile.fields = vec![
            semantic_field(
                "object:profile",
                "field:profile-date",
                "date",
                "date",
                "date",
            ),
            semantic_field(
                "object:profile",
                "field:age-bucket",
                "age_bucket",
                "age_bucket",
                "category",
            ),
            semantic_field(
                "object:profile",
                "field:unique-profile",
                "unique_pid_count",
                "unique_pid_count",
                "metric",
            ),
            semantic_field(
                "object:profile",
                "field:invalid-age",
                "invalid_age_count",
                "invalid_age_count",
                "metric",
            ),
            semantic_field(
                "object:profile",
                "field:raw-person",
                "PersonID",
                "PID",
                "identifier",
            ),
            semantic_field(
                "object:profile",
                "field:pid-hash",
                "PIDHash",
                "PIDHash",
                "identifier",
            ),
            semantic_field(
                "object:profile",
                "field:raw-person-value",
                "rawPersonIdValue",
                "rawPersonIdValue",
                "identifier",
            ),
            semantic_field(
                "object:profile",
                "field:visitor-pid",
                "visitorPid",
                "visitorPid",
                "identifier",
            ),
        ];

        let graph = build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput {
            left: traffic,
            right: profile,
        })
        .expect("safe mall semantic graph");
        let serialized = serde_json::to_string(&graph).unwrap();
        let lower = serialized.to_ascii_lowercase();

        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.kind == DatasetSemanticGraphNodeKind::Dataset)
                .count(),
            2
        );
        assert!(graph.nodes.iter().any(|node| {
            node.display_label == "时间维度"
                && node.dataset_refs == vec![traffic_dataset_id, profile_dataset_id]
        }));
        for expected in [
            "小时客流明细",
            "画像每日汇总",
            "进入人次",
            "去重到访人数",
            "日内去重画像数",
            "空间点位",
            "人群画像",
            "数据质量",
            "隐私边界",
            "业态分类",
        ] {
            assert!(
                graph
                    .nodes
                    .iter()
                    .any(|node| node.display_label == expected),
                "missing {expected}"
            );
        }

        let profile_unique_id =
            platform_api::cross_dataset_semantic_graph::scoped_semantic_endpoint_id(
                profile_dataset_id,
                "field:unique-profile",
            );
        let traffic_visitors_id =
            platform_api::cross_dataset_semantic_graph::scoped_semantic_endpoint_id(
                traffic_dataset_id,
                "field:visitors",
            );
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == traffic_visitors_id)
                .map(|node| node.display_label.as_str()),
            Some("去重到访人数")
        );
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == profile_unique_id)
                .map(|node| node.display_label.as_str()),
            Some("日内去重画像数")
        );
        let profile_concept_labels = graph
            .edges
            .iter()
            .filter(|edge| {
                edge.source_id == profile_unique_id
                    && edge.relation_type == "field_expresses_concept"
            })
            .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target_id))
            .map(|node| node.display_label.as_str())
            .collect::<BTreeSet<_>>();
        assert!(profile_concept_labels.contains("人群画像"));
        assert!(profile_concept_labels.contains("统计规模"));
        assert!(profile_concept_labels.contains("隐私边界"));
        assert!(!profile_concept_labels.contains("客流指标"));
        let traffic_visitors_concept_labels = graph
            .edges
            .iter()
            .filter(|edge| {
                edge.source_id == traffic_visitors_id
                    && edge.relation_type == "field_expresses_concept"
            })
            .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target_id))
            .map(|node| node.display_label.as_str())
            .collect::<BTreeSet<_>>();
        assert!(traffic_visitors_concept_labels.contains("客流指标"));
        assert!(traffic_visitors_concept_labels.contains("统计规模"));
        assert!(!traffic_visitors_concept_labels.contains("人群画像"));
        assert!(!traffic_visitors_concept_labels.contains("隐私边界"));
        let average_stay_id =
            platform_api::cross_dataset_semantic_graph::scoped_semantic_endpoint_id(
                traffic_dataset_id,
                "field:average-stay",
            );
        let average_stay_concept_labels = graph
            .edges
            .iter()
            .filter(|edge| {
                edge.source_id == average_stay_id && edge.relation_type == "field_expresses_concept"
            })
            .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target_id))
            .map(|node| node.display_label.as_str())
            .collect::<BTreeSet<_>>();
        assert!(average_stay_concept_labels.contains("客流指标"));
        assert!(!average_stay_concept_labels.contains("人群画像"));

        let temporal = graph
            .edges
            .iter()
            .find(|edge| edge.relation_type == "temporal_complementarity")
            .expect("temporal complementarity edge");
        assert_eq!(temporal.evidence_class, SemanticEvidenceClass::Inferred);
        assert!(temporal.cross_dataset);
        assert_eq!(
            temporal.supporting_dataset_ids,
            vec![traffic_dataset_id, profile_dataset_id]
        );
        assert!(temporal.reason.contains("不代表可以逐记录或逐人关联"));
        assert!(graph
            .edges
            .iter()
            .filter(|edge| {
                matches!(
                    edge.relation_type.as_str(),
                    "dataset_contains_object" | "object_contains_field" | "field_expresses_concept"
                )
            })
            .all(|edge| !edge.cross_dataset && edge.supporting_dataset_ids.len() == 1));
        assert!(!lower.contains("personid"));
        assert!(!lower.contains("pid"));
        assert!(!lower.contains("pidhash"));
        assert!(!lower.contains("rawpersonidvalue"));
        assert!(!lower.contains("visitorpid"));
        assert!(!serialized.contains("raw-example-must-not-enter-link-graph"));
        assert!(!serialized.contains("C:\\restricted"));
        assert!(dataset_semantic_link_manifest_is_safe(
            &serde_json::to_value(&graph).unwrap()
        ));
    }

    #[test]
    fn semantic_link_safe_summary_has_only_approved_receipt_fields() {
        let left_dataset_id = DatasetId(Uuid::from_u128(10));
        let right_dataset_id = DatasetId(Uuid::from_u128(20));
        let graph = build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput {
            left: semantic_snapshot(
                left_dataset_id,
                "项目资料",
                "object:left",
                "field:left",
                "合同金额",
                "amount",
            ),
            right: semantic_snapshot(
                right_dataset_id,
                "经营分析",
                "object:right",
                "field:right",
                "租赁面积",
                "area",
            ),
        })
        .unwrap();
        let summary = dataset_semantic_link_safe_summary("pair:opaque", &graph);
        let value = serde_json::to_value(summary).unwrap();
        let keys = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();

        assert_eq!(
            keys,
            BTreeSet::from([
                "chinese_label_count".to_string(),
                "confirmed_edge_count".to_string(),
                "edge_count".to_string(),
                "generation_version".to_string(),
                "generic_label_count".to_string(),
                "inferred_edge_count".to_string(),
                "node_count".to_string(),
                "observed_edge_count".to_string(),
                "pair_id".to_string(),
            ])
        );
        assert_eq!(value["pair_id"], json!("pair:opaque"));
    }

    #[test]
    fn semantic_link_run_generation_must_match_the_current_graph_builder() {
        let tenant_id = TenantId::new();
        let left_dataset_id = DatasetId::new();
        let right_dataset_id = DatasetId::new();
        let left_snapshot_id = Uuid::new_v4();
        let right_snapshot_id = Uuid::new_v4();
        let now = Utc::now();
        let snapshot = |id, dataset_id| DatasetSemanticSnapshot {
            id,
            tenant_id,
            dataset_id,
            schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
            generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
            source_fingerprint: "fixture".to_string(),
            status: "ready".to_string(),
            manifest: json!({}),
            source_document_count: 1,
            source_asset_count: 0,
            source_record_count: 1,
            node_count: 1,
            edge_count: 0,
            failure_code: None,
            generated_at: Some(now),
            created_at: now,
            updated_at: now,
        };
        let left = snapshot(left_snapshot_id, left_dataset_id);
        let right = snapshot(right_snapshot_id, right_dataset_id);
        let mut run = DatasetSemanticLinkRun {
            id: Uuid::new_v4(),
            tenant_id,
            left_dataset_id,
            right_dataset_id,
            left_snapshot_id,
            right_snapshot_id,
            generation_version: DATASET_SEMANTIC_GRAPH_GENERATION_VERSION.to_string(),
            source_fingerprint: "fixture".to_string(),
            status: "pending".to_string(),
            priority: 0,
            attempt_count: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            failure_code: None,
            created_at: now,
            updated_at: now,
        };

        assert!(semantic_link_run_inputs_are_current(&run, &left, &right));
        run.generation_version = "dataset_semantic_graph_v1".to_string();
        assert!(!semantic_link_run_inputs_are_current(&run, &left, &right));
    }

    #[test]
    fn semantic_link_labels_drop_opaque_ids_and_internal_paths() {
        assert_eq!(
            safe_semantic_link_label("0123456789abcdef0123456789abcdef", "资料"),
            "资料"
        );
        assert_eq!(
            safe_semantic_link_label("\\\\internal-server\\share\\report.xlsx", "资料"),
            "资料"
        );
        assert_eq!(
            safe_semantic_link_label("AbCdEfGh1234567890IjKlMn", "资料"),
            "资料"
        );
        assert_eq!(safe_semantic_link_label("合同金额", "业务字段"), "合同金额");
    }

    #[test]
    fn semantic_understanding_access_is_fail_closed_and_requires_both_allowlists() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();

        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                false,
                Some(&tenant_id.to_string()),
                Some(&dataset_id.to_string()),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::FeatureDisabled
        );
        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                None,
                Some(&dataset_id.to_string()),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::TenantNotAllowlisted
        );
        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                Some(&tenant_id.to_string()),
                None,
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::DatasetNotAllowlisted
        );
    }

    #[test]
    fn semantic_understanding_access_accepts_exact_uuid_csv_entries_only() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let other_tenant_id = TenantId::new();
        let other_dataset_id = DatasetId::new();
        let tenant_allowlist = format!("invalid, {}, {}", other_tenant_id, tenant_id);
        let dataset_allowlist = format!("{},{}", other_dataset_id, dataset_id);

        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                Some(&tenant_allowlist),
                Some(&dataset_allowlist),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::Allowed
        );
        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                Some("*"),
                Some("*"),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::TenantNotAllowlisted
        );
    }

    #[test]
    fn local_lexical_retrieval_indexer_uses_chunk_content() {
        let dataset_id = DatasetId::new();
        let outcome = LocalLexicalRetrievalIndexer.index(&RetrievalIndexJob {
            dataset_id,
            document_id: DocumentId::new(),
            chunks: vec![
                RetrievalChunkInput {
                    chunk_index: 0,
                    content: "Revenue revenue margin margin operating profit".to_string(),
                    token_count: 6,
                },
                RetrievalChunkInput {
                    chunk_index: 1,
                    content: "Product roadmap design system mobile navigation".to_string(),
                    token_count: 6,
                },
            ],
        });

        assert_eq!(outcome.embedded_chunks, 2);
        assert_eq!(outcome.payload_filter_key, format!("dataset/{dataset_id}"));
        assert_eq!(outcome.embedding_model, "local-lexical-v1");
        assert_eq!(outcome.chunk_profiles.len(), 2);
        assert_eq!(outcome.chunk_profiles[0].chunk_index, 0);
        assert_eq!(outcome.chunk_profiles[1].chunk_index, 1);
        assert_eq!(outcome.chunk_profiles[0].token_count, 6);
        assert!(!outcome.chunk_profiles[0].signature_terms.is_empty());
        assert!(outcome.chunk_profiles[0]
            .term_weights
            .contains_key("revenue"));
        assert!(outcome.chunk_profiles[1]
            .term_weights
            .contains_key("roadmap"));
        assert!(outcome
            .chunk_profiles
            .iter()
            .all(|profile| profile.rank_hint >= 1));
        assert!(outcome
            .chunk_profiles
            .iter()
            .all(|profile| (0.0..=1.0).contains(&profile.recall_score)));
    }

    #[test]
    fn tokenize_drops_stop_words_and_normalizes_terms() {
        assert_eq!(
            tokenize("The revenue, margin, and growth plan."),
            vec![
                "revenue".to_string(),
                "margin".to_string(),
                "growth".to_string(),
                "plan".to_string(),
            ]
        );
    }

    #[test]
    fn tokenize_keeps_cjk_terms_for_chinese_materials() {
        let tokens = tokenize("订单金额增长 revenue");

        assert!(tokens.contains(&"订".to_string()));
        assert!(tokens.contains(&"单".to_string()));
        assert!(tokens.contains(&"金".to_string()));
        assert!(tokens.contains(&"额".to_string()));
        assert!(tokens.contains(&"订单".to_string()));
        assert!(tokens.contains(&"金额".to_string()));
        assert!(tokens.contains(&"增长".to_string()));
        assert!(tokens.contains(&"订单金".to_string()));
        assert!(tokens.contains(&"金额增".to_string()));
        assert!(tokens.contains(&"订单金额增长".to_string()));
        assert!(tokens.contains(&"revenue".to_string()));
    }

    #[test]
    fn lexical_search_terms_are_unique_and_hash_is_stable() {
        let terms = lexical_search_terms("订单延期风险 订单延期风险 revenue");

        assert_eq!(
            terms.iter().filter(|term| *term == "订单延期风险").count(),
            1
        );
        assert!(terms.contains(&"revenue".to_string()));
        assert_eq!(
            lexical_content_hash("订单延期风险"),
            lexical_content_hash("订单延期风险")
        );
        assert_ne!(
            lexical_content_hash("订单延期风险"),
            lexical_content_hash("订单延期")
        );
    }

    #[test]
    fn local_lexical_retrieval_indexer_profiles_cjk_business_phrases() {
        let outcome = LocalLexicalRetrievalIndexer.index(&RetrievalIndexJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunks: vec![
                RetrievalChunkInput {
                    chunk_index: 0,
                    content: "订单延期风险 订单延期风险 仓库交接赔付".to_string(),
                    token_count: 8,
                },
                RetrievalChunkInput {
                    chunk_index: 1,
                    content: "客服满意度提升 会员复购增长".to_string(),
                    token_count: 7,
                },
            ],
        });

        let phrase_profile = outcome
            .chunk_profiles
            .iter()
            .find(|profile| profile.chunk_index == 0)
            .expect("phrase chunk profile should exist");

        assert!(phrase_profile.term_weights.contains_key("订单"));
        assert!(phrase_profile.term_weights.contains_key("延期"));
        assert!(phrase_profile.term_weights.contains_key("风险"));
        assert!(phrase_profile.term_weights.contains_key("订单延期风险"));
        assert!(phrase_profile.recall_score > 0.0);
    }

    #[test]
    fn local_lexical_retrieval_indexer_preserves_realistic_business_phrases() {
        let outcome = LocalLexicalRetrievalIndexer.index(&RetrievalIndexJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunks: vec![
                RetrievalChunkInput {
                    chunk_index: 0,
                    content: "订单延期风险集中在华东仓库交接，超过两天需要赔付提醒。".to_string(),
                    token_count: 22,
                },
                RetrievalChunkInput {
                    chunk_index: 1,
                    content: "客户满意度下降主要来自客服响应慢，工单需要升级处理。".to_string(),
                    token_count: 23,
                },
                RetrievalChunkInput {
                    chunk_index: 2,
                    content: "企业问答手册记录了员工报销流程和审批制度。".to_string(),
                    token_count: 18,
                },
            ],
        });

        let order_profile = outcome
            .chunk_profiles
            .iter()
            .find(|profile| profile.chunk_index == 0)
            .expect("order profile should exist");
        let support_profile = outcome
            .chunk_profiles
            .iter()
            .find(|profile| profile.chunk_index == 1)
            .expect("support profile should exist");

        assert!(order_profile.term_weights.contains_key("订单延期风险"));
        assert!(order_profile
            .signature_terms
            .contains(&"订单延期风险".to_string()));
        assert!(support_profile
            .term_weights
            .keys()
            .any(|term| term.contains("客户满意度")));
        assert!(support_profile
            .signature_terms
            .iter()
            .any(|term| term.contains("客户满意度")));
        assert!(order_profile.vector_norm > 0.0);
        assert!(support_profile.vector_norm > 0.0);
    }

    #[test]
    fn tokenize_keeps_cjk_ngrams_separate_across_ascii_boundaries() {
        let tokens = tokenize("订单risk取消");

        assert!(tokens.contains(&"订单".to_string()));
        assert!(tokens.contains(&"risk".to_string()));
        assert!(tokens.contains(&"取消".to_string()));
        assert!(!tokens.contains(&"单取".to_string()));
    }
}
