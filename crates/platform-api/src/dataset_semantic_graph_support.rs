use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json,
};
use domain_model::{Dataset, DatasetId, DatasetLifecycle, SecretBindingId, TenantId, UserId};
use futures_util::{
    future::join_all,
    stream::{self, StreamExt},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cross_dataset_semantic_graph::{
    scoped_semantic_endpoint_id, DatasetSemanticGraphDataset, DatasetSemanticGraphEdge,
    DatasetSemanticGraphNode, DatasetSemanticGraphNodeKind, DatasetSemanticGraphTruncation,
    DatasetSemanticGraphV1, DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION,
};
use crate::resource_access::filter_visible_datasets;
use crate::semantic_label_resolver::{
    classify_semantic_primary_label, safe_semantic_business_label,
};
use crate::semantic_understanding::{
    stable_semantic_id, DatasetSemanticUnderstanding, SemanticEvidenceClass,
    SemanticRelationSemantics, DATASET_SEMANTIC_SCHEMA_VERSION,
};
use crate::{
    active_secret_binding_ids_from_headers, current_auth_user_id,
    load_visible_dataset_for_user_with_local_scope, local_thread_id_from_headers, ApiError,
    AppState,
};

const DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV: &str = "DATASET_CROSS_SEMANTIC_GRAPH_ENABLED";
const DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV: &str =
    "DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST";
const DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV: &str =
    "DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST";
const DATASET_SEMANTIC_GRAPH_MAX_DATASETS: usize = 8;
const DATASET_SEMANTIC_GRAPH_MAX_NODES: usize = 360;
const DATASET_SEMANTIC_GRAPH_MAX_EDGES: usize = 600;
const DATASET_SEMANTIC_GRAPH_MAX_DEPTH: usize = 2;
const DATASET_SEMANTIC_GRAPH_MAX_AUTO_NEIGHBORS: usize = 4;
const DATASET_SEMANTIC_GRAPH_MAX_AUTO_NEIGHBOR_CANDIDATES: usize = 64;
const DATASET_SEMANTIC_GRAPH_AUTO_NEIGHBOR_CONCURRENCY: usize = 8;
const DATASET_SEMANTIC_GRAPH_CACHE_CONTROL: &str = "private, max-age=0, must-revalidate";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DatasetSemanticGraphQueryRequest {
    pub root_dataset_id: DatasetId,
    #[serde(default)]
    pub dataset_ids: Vec<DatasetId>,
    #[serde(default)]
    pub auto_neighbors: usize,
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
    #[serde(default = "default_max_edges")]
    pub max_edges: usize,
    #[serde(default = "default_depth")]
    pub depth: usize,
}

fn default_max_nodes() -> usize {
    160
}

fn default_max_edges() -> usize {
    240
}

fn default_depth() -> usize {
    1
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizedDatasetSemanticGraphQuery {
    root_dataset_id: DatasetId,
    explicit_dataset_ids: Vec<DatasetId>,
    auto_neighbors: usize,
    max_nodes: usize,
    max_edges: usize,
    depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueryValidationError {
    code: &'static str,
    message: &'static str,
}

fn normalize_query(
    request: DatasetSemanticGraphQueryRequest,
) -> std::result::Result<NormalizedDatasetSemanticGraphQuery, QueryValidationError> {
    let mut explicit_dataset_ids = vec![request.root_dataset_id];
    explicit_dataset_ids.extend(request.dataset_ids);
    let mut seen = BTreeSet::new();
    explicit_dataset_ids.retain(|dataset_id| seen.insert(*dataset_id));
    if explicit_dataset_ids.len() > DATASET_SEMANTIC_GRAPH_MAX_DATASETS {
        return Err(QueryValidationError {
            code: "dataset_semantic_graph_dataset_limit_exceeded",
            message: "dataset semantic graph supports at most 8 explicit datasets",
        });
    }
    let max_nodes = request.max_nodes.clamp(1, DATASET_SEMANTIC_GRAPH_MAX_NODES);
    let max_edges = request.max_edges.min(DATASET_SEMANTIC_GRAPH_MAX_EDGES);
    // Every automatically selected dataset must remain explainable in the
    // returned graph. Reserve at least two endpoints and one reliable edge per
    // automatic neighbor instead of selecting neighbors that the response
    // budget would immediately truncate away.
    let auto_neighbors = request
        .auto_neighbors
        .min(DATASET_SEMANTIC_GRAPH_MAX_AUTO_NEIGHBORS)
        .min(max_nodes / 2)
        .min(max_edges);
    Ok(NormalizedDatasetSemanticGraphQuery {
        root_dataset_id: request.root_dataset_id,
        explicit_dataset_ids,
        auto_neighbors,
        max_nodes,
        max_edges,
        depth: request.depth.clamp(1, DATASET_SEMANTIC_GRAPH_MAX_DEPTH),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CrossGraphAccess {
    Allowed,
    FeatureDisabled,
    TenantNotAllowlisted,
    DatasetNotAllowlisted,
}

fn cross_graph_access(tenant_id: TenantId, dataset_ids: &[DatasetId]) -> CrossGraphAccess {
    cross_graph_access_from_values(
        env_flag_value(DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV, false),
        std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        tenant_id,
        dataset_ids,
    )
}

fn cross_graph_access_from_values(
    enabled: bool,
    tenant_allowlist: Option<&str>,
    dataset_allowlist: Option<&str>,
    tenant_id: TenantId,
    dataset_ids: &[DatasetId],
) -> CrossGraphAccess {
    if !enabled {
        return CrossGraphAccess::FeatureDisabled;
    }
    if !uuid_csv_contains(tenant_allowlist, tenant_id.0) {
        return CrossGraphAccess::TenantNotAllowlisted;
    }
    if dataset_ids
        .iter()
        .any(|dataset_id| !uuid_csv_contains(dataset_allowlist, dataset_id.0))
    {
        return CrossGraphAccess::DatasetNotAllowlisted;
    }
    CrossGraphAccess::Allowed
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

fn uuid_csv_contains(csv: Option<&str>, expected: Uuid) -> bool {
    csv.into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.trim().parse::<Uuid>().ok())
        .any(|value| value == expected)
}

fn bounded_allowlisted_auto_neighbor_ids(
    dataset_allowlist: Option<&str>,
    selected_dataset_ids: &BTreeSet<DatasetId>,
) -> Vec<DatasetId> {
    dataset_allowlist
        .into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.trim().parse::<Uuid>().ok().map(DatasetId))
        .filter(|dataset_id| !selected_dataset_ids.contains(dataset_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(DATASET_SEMANTIC_GRAPH_MAX_AUTO_NEIGHBOR_CANDIDATES)
        .collect()
}

fn masked_graph_not_found() -> ApiError {
    ApiError::not_found(
        "dataset_semantic_graph_not_found",
        "dataset semantic graph was not found".to_string(),
    )
}

async fn load_all_explicit_datasets<F, Fut>(
    dataset_ids: &[DatasetId],
    loader: F,
) -> std::result::Result<Vec<Dataset>, ApiError>
where
    F: Fn(DatasetId) -> Fut,
    Fut: Future<Output = std::result::Result<Dataset, ApiError>>,
{
    let results = join_all(dataset_ids.iter().copied().map(loader)).await;
    if results.iter().any(|result| {
        result
            .as_ref()
            .is_err_and(|error| error.status == StatusCode::NOT_FOUND)
    }) {
        return Err(masked_graph_not_found());
    }
    results.into_iter().collect()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ReliableLinkCounts {
    confirmed: usize,
    observed: usize,
}

fn visible_auto_neighbor_candidates(
    datasets: Vec<Dataset>,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
    selected_dataset_ids: &BTreeSet<DatasetId>,
) -> Vec<Dataset> {
    let mut visible = filter_visible_datasets(
        datasets,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .into_iter()
    .filter(|dataset| dataset.lifecycle != DatasetLifecycle::Archived)
    .filter(|dataset| !selected_dataset_ids.contains(&dataset.id))
    .collect::<Vec<_>>();
    visible.sort_by_key(|dataset| dataset.id);
    visible
}

fn rank_reliable_auto_neighbors(
    candidates: Vec<Dataset>,
    counts: &BTreeMap<DatasetId, ReliableLinkCounts>,
    limit: usize,
) -> Vec<Dataset> {
    let mut ranked = candidates
        .into_iter()
        .filter(|dataset| {
            counts
                .get(&dataset.id)
                .is_some_and(|count| count.confirmed > 0 || count.observed > 0)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        let left_count = counts.get(&left.id).copied().unwrap_or_default();
        let right_count = counts.get(&right.id).copied().unwrap_or_default();
        right_count
            .confirmed
            .cmp(&left_count.confirmed)
            .then_with(|| right_count.observed.cmp(&left_count.observed))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.truncate(limit.min(DATASET_SEMANTIC_GRAPH_MAX_AUTO_NEIGHBORS));
    ranked
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DatasetSemanticGraphScope {
    current_user_id: Option<UserId>,
    active_secret_binding_ids: Vec<SecretBindingId>,
    local_thread_id: Option<String>,
}

fn dataset_semantic_graph_etag(
    query: &NormalizedDatasetSemanticGraphQuery,
    snapshot_ids: &[(DatasetId, Option<Uuid>)],
    link_ids: &[Uuid],
    state_versions: &[String],
    scope: &DatasetSemanticGraphScope,
) -> String {
    let mut snapshot_tokens = snapshot_ids
        .iter()
        .map(|(dataset_id, snapshot_id)| {
            format!(
                "dataset:{dataset_id}:snapshot:{}",
                snapshot_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            )
        })
        .collect::<Vec<_>>();
    snapshot_tokens.sort();
    snapshot_tokens.dedup();
    let mut link_tokens = link_ids
        .iter()
        .map(|link_id| format!("link:{link_id}"))
        .collect::<Vec<_>>();
    link_tokens.sort();
    link_tokens.dedup();

    let mut scope_tokens = scope
        .active_secret_binding_ids
        .iter()
        .map(|secret_id| format!("secret:{secret_id}"))
        .collect::<Vec<_>>();
    scope_tokens.push(format!(
        "user:{}",
        scope
            .current_user_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "anonymous".to_string())
    ));
    scope_tokens.push(format!(
        "thread:{}",
        scope.local_thread_id.as_deref().unwrap_or("none")
    ));
    scope_tokens.sort();
    let scope_refs = scope_tokens.iter().map(String::as_str).collect::<Vec<_>>();
    let scope_fingerprint = stable_semantic_id("dataset_graph_scope", &scope_refs);

    let mut tokens = snapshot_tokens;
    tokens.extend(link_tokens);
    tokens.extend(
        state_versions
            .iter()
            .map(|version| format!("state:{version}")),
    );
    tokens.push(format!("root:{}", query.root_dataset_id));
    tokens.push(format!("nodes:{}", query.max_nodes));
    tokens.push(format!("edges:{}", query.max_edges));
    tokens.push(format!("depth:{}", query.depth));
    tokens.push(format!("auto:{}", query.auto_neighbors));
    tokens.push(scope_fingerprint);
    tokens.sort();
    let refs = tokens.iter().map(String::as_str).collect::<Vec<_>>();
    format!("\"{}\"", stable_semantic_id("dataset_graph_etag", &refs))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CrossLinksStatus {
    Ready,
    Building,
    Stale,
    Empty,
}

fn cross_links_status(ready: usize, building: usize, stale: usize) -> CrossLinksStatus {
    if stale > 0 {
        CrossLinksStatus::Stale
    } else if building > 0 {
        CrossLinksStatus::Building
    } else if ready > 0 {
        CrossLinksStatus::Ready
    } else {
        CrossLinksStatus::Empty
    }
}

#[derive(Clone, Debug, Serialize)]
struct DatasetSemanticGraphQueryResponse {
    #[serde(flatten)]
    graph: DatasetSemanticGraphV1,
    cross_links_status: CrossLinksStatus,
}

fn request_etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag))
}

fn build_dataset_semantic_graph_response(
    headers: &HeaderMap,
    payload: &DatasetSemanticGraphQueryResponse,
    etag: Option<&str>,
) -> std::result::Result<Response, ApiError> {
    if etag.is_some_and(|value| request_etag_matches(headers, value)) {
        let mut builder = Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(header::CACHE_CONTROL, DATASET_SEMANTIC_GRAPH_CACHE_CONTROL);
        if let Some(etag) = etag {
            builder = builder.header(header::ETAG, etag);
        }
        return Ok(builder
            .body(Body::empty())
            .expect("not-modified semantic graph response should build"));
    }
    let body = serde_json::to_vec(payload).map_err(|error| {
        ApiError::internal(
            "dataset_semantic_graph_serialize_failed",
            format!("dataset semantic graph response serialization failed: {error}"),
        )
    })?;
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::CACHE_CONTROL, DATASET_SEMANTIC_GRAPH_CACHE_CONTROL);
    if let Some(etag) = etag {
        builder = builder.header(header::ETAG, etag);
    }
    Ok(builder
        .body(Body::from(body))
        .expect("semantic graph response should build"))
}

fn sanitize_graph_for_visible_scope(
    mut graph: DatasetSemanticGraphV1,
    visible_datasets: &[Dataset],
    query: &NormalizedDatasetSemanticGraphQuery,
) -> DatasetSemanticGraphV1 {
    let visible_dataset_ids = visible_datasets
        .iter()
        .map(|dataset| dataset.id)
        .collect::<BTreeSet<_>>();
    let datasets = visible_datasets
        .iter()
        .map(|dataset| DatasetSemanticGraphDataset {
            id: dataset.id,
            title: safe_dataset_title(&dataset.title),
            stale: graph.stale,
        })
        .collect::<Vec<_>>();

    let mut nodes_by_id = BTreeMap::<String, DatasetSemanticGraphNode>::new();
    for mut node in graph.nodes {
        node.dataset_refs
            .retain(|dataset_id| visible_dataset_ids.contains(dataset_id));
        node.dataset_refs.sort();
        node.dataset_refs.dedup();
        if node.dataset_refs.is_empty()
            || !is_safe_public_node_id(&node.id, node.kind, &visible_dataset_ids)
        {
            continue;
        }
        node.display_label = safe_node_label(&node.display_label, node.kind);
        node.visible_provenance_count = node.dataset_refs.len() as u64;
        nodes_by_id
            .entry(node.id.clone())
            .and_modify(|current| {
                current
                    .dataset_refs
                    .extend(node.dataset_refs.iter().copied());
                current.dataset_refs.sort();
                current.dataset_refs.dedup();
                current.visible_provenance_count = current.dataset_refs.len() as u64;
            })
            .or_insert(node);
    }

    let explicit_dataset_ids = query
        .explicit_dataset_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let auto_dataset_ids = visible_dataset_ids
        .difference(&explicit_dataset_ids)
        .copied()
        .collect::<Vec<_>>();
    // Exact cross-dataset identities are represented by one folded shared
    // node in the pair snapshot, so they intentionally have no identity edge.
    // For an automatically selected neighbor, project a safe dataset hub and
    // a confirmed membership edge. This makes the evidence visible without
    // exposing the internal identity that caused the fold.
    let mut synthesized_auto_edges = BTreeMap::<DatasetId, (String, String, String)>::new();
    for auto_dataset_id in &auto_dataset_ids {
        let shared_node_id = nodes_by_id
            .values()
            .filter(|node| node.id.starts_with("shared:"))
            .filter(|node| {
                node.dataset_refs.contains(&query.root_dataset_id)
                    && node.dataset_refs.contains(auto_dataset_id)
            })
            .map(|node| node.id.clone())
            .next();
        let Some(shared_node_id) = shared_node_id else {
            continue;
        };
        let hub_id = format!("d:{auto_dataset_id}:dataset:root");
        let hub_label = visible_datasets
            .iter()
            .find(|dataset| dataset.id == *auto_dataset_id)
            .map(|dataset| safe_node_label(&dataset.title, DatasetSemanticGraphNodeKind::Dataset))
            .unwrap_or_else(|| "数据集".to_string());
        nodes_by_id.insert(
            hub_id.clone(),
            DatasetSemanticGraphNode {
                id: hub_id.clone(),
                kind: DatasetSemanticGraphNodeKind::Dataset,
                display_label: hub_label,
                dataset_refs: vec![*auto_dataset_id],
                visible_provenance_count: 1,
            },
        );
        let safe_edge_id = stable_semantic_id(
            "cross_relation",
            &[
                &hub_id,
                &shared_node_id,
                "membership",
                evidence_name(SemanticEvidenceClass::Confirmed),
            ],
        );
        graph.edges.push(DatasetSemanticGraphEdge {
            id: safe_edge_id.clone(),
            source_id: hub_id,
            target_id: shared_node_id.clone(),
            relation_type: "membership".to_string(),
            label: "共享内容".to_string(),
            relation_semantics: SemanticRelationSemantics::Structure,
            evidence_class: SemanticEvidenceClass::Confirmed,
            confidence: 1.0,
            reason: "可见证据确认该共享节点属于此数据集。".to_string(),
            cross_dataset: true,
            supporting_dataset_ids: vec![query.root_dataset_id, *auto_dataset_id],
        });
        synthesized_auto_edges.insert(
            *auto_dataset_id,
            (
                safe_edge_id,
                format!("d:{auto_dataset_id}:dataset:root"),
                shared_node_id,
            ),
        );
    }

    let all_node_refs = nodes_by_id
        .iter()
        .map(|(node_id, node)| (node_id.clone(), node.dataset_refs.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut reliable_auto_candidates = graph
        .edges
        .iter()
        .filter_map(|edge| {
            let semantics = safe_relation_semantics(&edge.relation_type)?;
            if !edge.cross_dataset
                || semantics == SemanticRelationSemantics::Similarity
                || edge.evidence_class == SemanticEvidenceClass::Inferred
                || !all_node_refs.contains_key(&edge.source_id)
                || !all_node_refs.contains_key(&edge.target_id)
            {
                return None;
            }
            let supporting_dataset_ids = visible_supporting_dataset_ids(
                &edge.supporting_dataset_ids,
                &edge.source_id,
                &edge.target_id,
                &all_node_refs,
                &visible_dataset_ids,
            );
            if supporting_dataset_ids.len() < 2 {
                return None;
            }
            let safe_edge_id = stable_semantic_id(
                "cross_relation",
                &[
                    &edge.source_id,
                    &edge.target_id,
                    &edge.relation_type,
                    evidence_name(edge.evidence_class),
                ],
            );
            Some((
                evidence_rank(edge.evidence_class),
                edge.source_id.clone(),
                edge.target_id.clone(),
                edge.relation_type.clone(),
                safe_edge_id,
                supporting_dataset_ids,
            ))
        })
        .collect::<Vec<_>>();
    reliable_auto_candidates.sort();
    let mut required_auto_edge_ids = BTreeSet::new();
    let mut required_auto_endpoint_ids = BTreeSet::new();
    for auto_dataset_id in auto_dataset_ids {
        if let Some((safe_edge_id, source_id, target_id)) =
            synthesized_auto_edges.get(&auto_dataset_id)
        {
            required_auto_edge_ids.insert(safe_edge_id.clone());
            required_auto_endpoint_ids.insert(source_id.clone());
            required_auto_endpoint_ids.insert(target_id.clone());
            continue;
        }
        let Some((_, source_id, target_id, _, safe_edge_id, _)) = reliable_auto_candidates
            .iter()
            .find(|candidate| candidate.5.contains(&auto_dataset_id))
        else {
            continue;
        };
        required_auto_edge_ids.insert(safe_edge_id.clone());
        required_auto_endpoint_ids.insert(source_id.clone());
        required_auto_endpoint_ids.insert(target_id.clone());
    }

    let total_nodes = nodes_by_id.len();
    let mut nodes = nodes_by_id.into_values().collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        (!required_auto_endpoint_ids.contains(&left.id))
            .cmp(&(!required_auto_endpoint_ids.contains(&right.id)))
            .then_with(|| node_sort_rank(&left.id).cmp(&node_sort_rank(&right.id)))
            .then_with(|| left.id.cmp(&right.id))
    });
    nodes.truncate(query.max_nodes);
    let retained_node_refs = nodes
        .iter()
        .map(|node| (node.id.clone(), node.dataset_refs.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut edges_by_key =
        BTreeMap::<(String, String, String, u8), DatasetSemanticGraphEdge>::new();
    for edge in graph.edges {
        if !retained_node_refs.contains_key(&edge.source_id)
            || !retained_node_refs.contains_key(&edge.target_id)
        {
            continue;
        }
        let Some(relation_semantics) = safe_relation_semantics(&edge.relation_type) else {
            continue;
        };
        let evidence_class = if relation_semantics == SemanticRelationSemantics::Similarity {
            SemanticEvidenceClass::Inferred
        } else {
            edge.evidence_class
        };
        let supporting_dataset_ids = visible_supporting_dataset_ids(
            &edge.supporting_dataset_ids,
            &edge.source_id,
            &edge.target_id,
            &retained_node_refs,
            &visible_dataset_ids,
        );
        if supporting_dataset_ids.is_empty() {
            continue;
        }
        let evidence_rank = evidence_rank(evidence_class);
        let safe_edge = DatasetSemanticGraphEdge {
            id: stable_semantic_id(
                "cross_relation",
                &[
                    &edge.source_id,
                    &edge.target_id,
                    &edge.relation_type,
                    evidence_name(evidence_class),
                ],
            ),
            source_id: edge.source_id.clone(),
            target_id: edge.target_id.clone(),
            relation_type: edge.relation_type.clone(),
            label: safe_relation_label(&edge.relation_type, evidence_class).to_string(),
            relation_semantics,
            evidence_class,
            confidence: if evidence_class == SemanticEvidenceClass::Inferred {
                edge.confidence.min(0.6).clamp(0.0, 1.0)
            } else {
                edge.confidence.clamp(0.0, 1.0)
            },
            reason: safe_relation_reason(&edge.relation_type, relation_semantics, evidence_class)
                .to_string(),
            cross_dataset: edge.cross_dataset && supporting_dataset_ids.len() >= 2,
            supporting_dataset_ids,
        };
        edges_by_key
            .entry((
                safe_edge.source_id.clone(),
                safe_edge.target_id.clone(),
                safe_edge.relation_type.clone(),
                evidence_rank,
            ))
            .or_insert(safe_edge);
    }
    let total_edges = edges_by_key.len();
    let mut edges = edges_by_key.into_values().collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        (!required_auto_edge_ids.contains(&left.id))
            .cmp(&(!required_auto_edge_ids.contains(&right.id)))
            .then_with(|| {
                evidence_rank(left.evidence_class).cmp(&evidence_rank(right.evidence_class))
            })
            .then_with(|| left.source_id.cmp(&right.source_id))
            .then_with(|| left.target_id.cmp(&right.target_id))
            .then_with(|| left.relation_type.cmp(&right.relation_type))
    });
    edges.truncate(query.max_edges);

    DatasetSemanticGraphV1 {
        schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
        root_dataset_id: query.root_dataset_id,
        datasets,
        nodes,
        edges,
        truncated: DatasetSemanticGraphTruncation {
            datasets: 0,
            nodes: total_nodes.saturating_sub(query.max_nodes),
            edges: total_edges.saturating_sub(query.max_edges),
        },
        stale: graph.stale,
    }
}

fn supporting_dataset_ids_from_nodes(
    source_id: &str,
    target_id: &str,
    node_refs: &BTreeMap<String, Vec<DatasetId>>,
) -> Vec<DatasetId> {
    node_refs
        .get(source_id)
        .into_iter()
        .chain(node_refs.get(target_id))
        .flat_map(|dataset_ids| dataset_ids.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn visible_supporting_dataset_ids(
    reported_dataset_ids: &[DatasetId],
    source_id: &str,
    target_id: &str,
    node_refs: &BTreeMap<String, Vec<DatasetId>>,
    visible_dataset_ids: &BTreeSet<DatasetId>,
) -> Vec<DatasetId> {
    let endpoint_scope = supporting_dataset_ids_from_nodes(source_id, target_id, node_refs)
        .into_iter()
        .collect::<BTreeSet<_>>();
    reported_dataset_ids
        .iter()
        .copied()
        .filter(|dataset_id| visible_dataset_ids.contains(dataset_id))
        .filter(|dataset_id| endpoint_scope.contains(dataset_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn safe_dataset_title(value: &str) -> String {
    let value = value.trim();
    let quality = classify_semantic_primary_label(value);
    if value.is_empty() || looks_sensitive(value) || !quality.business_label {
        "数据集".to_string()
    } else {
        value.chars().take(80).collect()
    }
}

fn safe_node_label(value: &str, kind: DatasetSemanticGraphNodeKind) -> String {
    let Some(projected) = safe_semantic_business_label(value) else {
        return fallback_node_label(kind).to_string();
    };
    let cjk_count = projected
        .chars()
        .filter(|character| is_cjk(*character))
        .count();
    if cjk_count < 2
        || projected
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        || looks_sensitive(&projected)
    {
        return fallback_node_label(kind).to_string();
    }
    projected.chars().take(24).collect()
}

fn fallback_node_label(kind: DatasetSemanticGraphNodeKind) -> &'static str {
    match kind {
        DatasetSemanticGraphNodeKind::Dataset => "数据集",
        DatasetSemanticGraphNodeKind::Document => "资料",
        DatasetSemanticGraphNodeKind::Object => "业务对象",
        DatasetSemanticGraphNodeKind::Field => "业务字段",
        DatasetSemanticGraphNodeKind::Concept => "知识概念",
        DatasetSemanticGraphNodeKind::Structure => "数据结构",
    }
}

fn looks_sensitive(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    let windows_path = lower.as_bytes().get(1) == Some(&b':')
        && matches!(lower.as_bytes().get(2), Some(b'\\') | Some(b'/'));
    windows_path
        || [
            "/home/",
            "/users/",
            "/root/",
            "/etc/",
            "file:",
            "file://",
            "postgres://",
            "mysql://",
            "jdbc:",
            "hmac",
            "sha256",
            "content_identity",
            "source_system",
            "source_schema",
            "source_object",
            "source_field",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn is_safe_public_node_id(
    value: &str,
    kind: DatasetSemanticGraphNodeKind,
    visible_dataset_ids: &BTreeSet<DatasetId>,
) -> bool {
    if let Some(rest) = value.strip_prefix("shared:") {
        let Some((shared_kind, opaque)) = rest.split_once(':') else {
            return false;
        };
        return shared_kind == node_kind_name(kind)
            && opaque.len() == 32
            && opaque
                .chars()
                .all(|character| character.is_ascii_hexdigit());
    }
    let Some(rest) = value.strip_prefix("d:") else {
        return false;
    };
    let Some((dataset_id, local_id)) = rest.split_once(':') else {
        return false;
    };
    let Ok(dataset_id) = dataset_id.parse::<Uuid>().map(DatasetId) else {
        return false;
    };
    let local_lower = local_id.to_ascii_lowercase();
    let valid_namespace = [
        "dataset:",
        "document:",
        "doc:",
        "object:",
        "field:",
        "concept:",
        "structure:",
        "opaque:",
    ]
    .iter()
    .any(|prefix| local_lower.starts_with(prefix));
    visible_dataset_ids.contains(&dataset_id)
        && valid_namespace
        && local_id.len() <= 160
        && local_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_:-".contains(character))
        && !looks_sensitive(local_id)
}

fn node_kind_name(kind: DatasetSemanticGraphNodeKind) -> &'static str {
    match kind {
        DatasetSemanticGraphNodeKind::Dataset => "dataset",
        DatasetSemanticGraphNodeKind::Document => "document",
        DatasetSemanticGraphNodeKind::Object => "object",
        DatasetSemanticGraphNodeKind::Field => "field",
        DatasetSemanticGraphNodeKind::Concept => "concept",
        DatasetSemanticGraphNodeKind::Structure => "structure",
    }
}

fn node_sort_rank(id: &str) -> u8 {
    if id.starts_with("shared:") {
        0
    } else {
        1
    }
}

fn is_cjk(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{20000}'..='\u{2fa1f}'
    )
}

fn safe_relation_semantics(relation_type: &str) -> Option<SemanticRelationSemantics> {
    match relation_type {
        "identity" | "same_identity" => Some(SemanticRelationSemantics::Identity),
        "foreign_key" | "explicit_reference" | "reference" => {
            Some(SemanticRelationSemantics::Reference)
        }
        "parent_child"
        | "contains"
        | "collection_membership"
        | "membership"
        | "structure"
        | "dataset_contains_object"
        | "object_contains_field"
        | "field_expresses_concept" => Some(SemanticRelationSemantics::Structure),
        "label_similarity"
        | "shared_key"
        | "cooccurrence"
        | "text_similarity"
        | "similarity"
        | "temporal_complementarity" => Some(SemanticRelationSemantics::Similarity),
        _ => None,
    }
}

fn safe_relation_label(relation_type: &str, evidence_class: SemanticEvidenceClass) -> &'static str {
    match (relation_type, evidence_class) {
        ("foreign_key", SemanticEvidenceClass::Confirmed) => "已确认外键",
        ("foreign_key", _) => "外键关系线索",
        ("explicit_reference" | "reference", SemanticEvidenceClass::Confirmed) => "明确引用",
        ("explicit_reference" | "reference", _) => "引用关系线索",
        ("parent_child", _) => "父子结构",
        ("contains", _) => "包含字段",
        ("collection_membership", _) => "集合归属",
        ("membership", _) => "共享归属",
        ("dataset_contains_object", _) => "包含业务对象",
        ("object_contains_field", _) => "包含业务字段",
        ("field_expresses_concept", _) => "表达业务概念",
        ("temporal_complementarity", _) => "可按时间联合分析",
        ("identity" | "same_identity", SemanticEvidenceClass::Confirmed) => "身份一致",
        ("identity" | "same_identity", _) => "身份关系线索",
        ("label_similarity", _) => "名称相近",
        ("shared_key", _) => "可能通过共享键关联",
        ("cooccurrence", _) => "同组共现",
        ("text_similarity", _) => "文本相似",
        ("structure", _) => "结构关联",
        ("similarity", _) => "可能相关",
        _ => "语义关联",
    }
}

fn safe_relation_reason(
    relation_type: &str,
    semantics: SemanticRelationSemantics,
    evidence_class: SemanticEvidenceClass,
) -> &'static str {
    match relation_type {
        "dataset_contains_object" => {
            return "可见语义快照表明该业务对象属于此数据集。";
        }
        "object_contains_field" => {
            return "可见语义快照表明该字段属于此业务对象。";
        }
        "field_expresses_concept" => {
            return "字段的已复核名称或语义角色映射到该业务概念；这不是外键或记录关联。";
        }
        "temporal_complementarity" => {
            return "两个数据集都有时间维度并提供互补业务视角，形成汇总比较候选；仍需核验日期范围、时区与统计粒度，且不代表逐记录或逐人关联。";
        }
        _ => {}
    }
    match (semantics, evidence_class) {
        (SemanticRelationSemantics::Identity, SemanticEvidenceClass::Confirmed) => {
            "可见证据确认两个节点具有一致身份。"
        }
        (SemanticRelationSemantics::Reference, SemanticEvidenceClass::Confirmed) => {
            "可见证据确认两个节点存在引用关系。"
        }
        (SemanticRelationSemantics::Structure, SemanticEvidenceClass::Confirmed) => {
            "可见证据确认两个节点存在结构关系。"
        }
        (_, SemanticEvidenceClass::Observed) => "可见证据观察到该关系，尚未确认。",
        (_, SemanticEvidenceClass::Inferred) => "可见证据提示可能相关，不代表同一实体。",
        _ => "可见证据支持该语义关系。",
    }
}

fn evidence_name(class: SemanticEvidenceClass) -> &'static str {
    match class {
        SemanticEvidenceClass::Confirmed => "confirmed",
        SemanticEvidenceClass::Observed => "observed",
        SemanticEvidenceClass::Inferred => "inferred",
    }
}

fn evidence_rank(class: SemanticEvidenceClass) -> u8 {
    match class {
        SemanticEvidenceClass::Confirmed => 0,
        SemanticEvidenceClass::Observed => 1,
        SemanticEvidenceClass::Inferred => 2,
    }
}

fn graph_from_link_manifest(
    manifest: &serde_json::Value,
    allowed_pair: &BTreeSet<DatasetId>,
) -> Option<DatasetSemanticGraphV1> {
    let graph = serde_json::from_value::<DatasetSemanticGraphV1>(manifest.clone())
        .ok()
        .or_else(|| {
            manifest
                .get("graph")
                .cloned()
                .and_then(|graph| serde_json::from_value(graph).ok())
        })?;
    let manifest_dataset_ids = graph
        .datasets
        .iter()
        .map(|dataset| dataset.id)
        .collect::<BTreeSet<_>>();
    (graph.schema_version == DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION
        && graph.datasets.len() == allowed_pair.len()
        && manifest_dataset_ids == *allowed_pair
        && allowed_pair.contains(&graph.root_dataset_id))
    .then_some(graph)
}

fn understanding_from_snapshot_manifest(
    manifest: &serde_json::Value,
    expected_dataset_id: DatasetId,
) -> Option<DatasetSemanticUnderstanding> {
    serde_json::from_value::<DatasetSemanticUnderstanding>(manifest.clone())
        .ok()
        .filter(|understanding| {
            understanding.schema_version == DATASET_SEMANTIC_SCHEMA_VERSION
                && understanding.status == "ready"
                && understanding.dataset.id == expected_dataset_id
        })
}

fn reliable_link_counts(
    graph: &DatasetSemanticGraphV1,
    allowed_pair: &BTreeSet<DatasetId>,
) -> ReliableLinkCounts {
    if graph.stale {
        return ReliableLinkCounts::default();
    }
    let mut node_refs = BTreeMap::<String, Vec<DatasetId>>::new();
    for node in &graph.nodes {
        if !is_safe_public_node_id(&node.id, node.kind, allowed_pair) {
            continue;
        }
        let mut visible_refs = node
            .dataset_refs
            .iter()
            .copied()
            .filter(|dataset_id| allowed_pair.contains(dataset_id))
            .collect::<Vec<_>>();
        visible_refs.sort();
        visible_refs.dedup();
        if visible_refs.is_empty() {
            continue;
        }
        node_refs
            .entry(node.id.clone())
            .and_modify(|current| {
                current.extend(visible_refs.iter().copied());
                current.sort();
                current.dedup();
            })
            .or_insert(visible_refs);
    }

    let mut counts = ReliableLinkCounts {
        confirmed: node_refs
            .iter()
            .filter(|(node_id, dataset_refs)| {
                node_id.starts_with("shared:") && dataset_refs.len() >= 2
            })
            .count(),
        observed: 0,
    };
    for edge in &graph.edges {
        let Some(semantics) = safe_relation_semantics(&edge.relation_type) else {
            continue;
        };
        if !edge.cross_dataset
            || semantics == SemanticRelationSemantics::Similarity
            || edge.evidence_class == SemanticEvidenceClass::Inferred
            || !node_refs.contains_key(&edge.source_id)
            || !node_refs.contains_key(&edge.target_id)
            || visible_supporting_dataset_ids(
                &edge.supporting_dataset_ids,
                &edge.source_id,
                &edge.target_id,
                &node_refs,
                allowed_pair,
            )
            .len()
                < 2
        {
            continue;
        }
        match edge.evidence_class {
            SemanticEvidenceClass::Confirmed => counts.confirmed += 1,
            SemanticEvidenceClass::Observed => counts.observed += 1,
            SemanticEvidenceClass::Inferred => {}
        }
    }
    counts
}

fn project_single_understanding(
    dataset: &Dataset,
    understanding: &DatasetSemanticUnderstanding,
) -> DatasetSemanticGraphV1 {
    let mut nodes = Vec::new();
    let mut id_map = BTreeMap::<String, String>::new();
    for object in &understanding.objects {
        let node_id = scoped_semantic_endpoint_id(dataset.id, &object.id);
        id_map.insert(object.id.clone(), node_id.clone());
        nodes.push(DatasetSemanticGraphNode {
            id: node_id,
            kind: if object.kind.eq_ignore_ascii_case("document") {
                DatasetSemanticGraphNodeKind::Document
            } else {
                DatasetSemanticGraphNodeKind::Object
            },
            display_label: object.label.clone(),
            dataset_refs: vec![dataset.id],
            visible_provenance_count: 1,
        });
    }
    for field in &understanding.fields {
        let node_id = scoped_semantic_endpoint_id(dataset.id, &field.id);
        id_map.insert(field.id.clone(), node_id.clone());
        nodes.push(DatasetSemanticGraphNode {
            id: node_id,
            kind: DatasetSemanticGraphNodeKind::Field,
            display_label: field.label.clone(),
            dataset_refs: vec![dataset.id],
            visible_provenance_count: 1,
        });
    }

    let mut edges = Vec::new();
    for field in &understanding.fields {
        let (Some(source_id), Some(target_id)) =
            (id_map.get(&field.object_id), id_map.get(&field.id))
        else {
            continue;
        };
        edges.push(DatasetSemanticGraphEdge {
            id: stable_semantic_id("cross_relation", &[source_id, target_id, "contains"]),
            source_id: source_id.clone(),
            target_id: target_id.clone(),
            relation_type: "contains".to_string(),
            label: "包含字段".to_string(),
            relation_semantics: SemanticRelationSemantics::Structure,
            evidence_class: SemanticEvidenceClass::Observed,
            confidence: 0.9,
            reason: "可见证据观察到该字段属于此业务对象。".to_string(),
            cross_dataset: false,
            supporting_dataset_ids: vec![dataset.id],
        });
    }
    for relation in &understanding.relations {
        let (Some(source_id), Some(target_id), Some(relation_semantics)) = (
            id_map.get(&relation.source_id),
            id_map.get(&relation.target_id),
            safe_relation_semantics(&relation.relation_type),
        ) else {
            continue;
        };
        edges.push(DatasetSemanticGraphEdge {
            id: stable_semantic_id(
                "cross_relation",
                &[
                    source_id,
                    target_id,
                    &relation.relation_type,
                    evidence_name(relation.evidence_class),
                ],
            ),
            source_id: source_id.clone(),
            target_id: target_id.clone(),
            relation_type: relation.relation_type.clone(),
            label: safe_relation_label(&relation.relation_type, relation.evidence_class)
                .to_string(),
            relation_semantics,
            evidence_class: relation.evidence_class,
            confidence: relation.confidence,
            reason: safe_relation_reason(
                &relation.relation_type,
                relation_semantics,
                relation.evidence_class,
            )
            .to_string(),
            cross_dataset: false,
            supporting_dataset_ids: vec![dataset.id],
        });
    }

    DatasetSemanticGraphV1 {
        schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
        root_dataset_id: dataset.id,
        datasets: vec![DatasetSemanticGraphDataset {
            id: dataset.id,
            title: dataset.title.clone(),
            stale: understanding.stale,
        }],
        nodes,
        edges,
        truncated: DatasetSemanticGraphTruncation::default(),
        stale: understanding.stale,
    }
}

fn empty_single_graph(dataset: &Dataset, stale: bool) -> DatasetSemanticGraphV1 {
    DatasetSemanticGraphV1 {
        schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
        root_dataset_id: dataset.id,
        datasets: vec![DatasetSemanticGraphDataset {
            id: dataset.id,
            title: dataset.title.clone(),
            stale,
        }],
        nodes: Vec::new(),
        edges: Vec::new(),
        truncated: DatasetSemanticGraphTruncation::default(),
        stale,
    }
}

fn merge_graphs(
    root_dataset_id: DatasetId,
    graphs: Vec<DatasetSemanticGraphV1>,
) -> DatasetSemanticGraphV1 {
    let mut datasets = Vec::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut stale = false;
    for graph in graphs {
        datasets.extend(graph.datasets);
        nodes.extend(graph.nodes);
        edges.extend(graph.edges);
        stale |= graph.stale;
    }
    DatasetSemanticGraphV1 {
        schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
        root_dataset_id,
        datasets,
        nodes,
        edges,
        truncated: DatasetSemanticGraphTruncation::default(),
        stale,
    }
}

fn canonical_pair(left: DatasetId, right: DatasetId) -> (DatasetId, DatasetId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

pub(crate) async fn query_dataset_semantic_graph(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DatasetSemanticGraphQueryRequest>,
) -> std::result::Result<Response, ApiError> {
    let query = normalize_query(request)
        .map_err(|error| ApiError::bad_request(error.code, error.message.to_string()))?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let current_user_id = current_auth_user_id(&state, &headers).await?;
    let local_thread_id = local_thread_id_from_headers(&headers);
    let scope = DatasetSemanticGraphScope {
        current_user_id,
        active_secret_binding_ids: active_secret_binding_ids.clone(),
        local_thread_id: local_thread_id.clone(),
    };

    let mut visible_datasets =
        load_all_explicit_datasets(&query.explicit_dataset_ids, |dataset_id| {
            load_visible_dataset_for_user_with_local_scope(
                &state,
                dataset_id,
                &active_secret_binding_ids,
                current_user_id,
                local_thread_id.as_deref(),
            )
        })
        .await?;
    if cross_graph_access(state.tenant_id, &query.explicit_dataset_ids) != CrossGraphAccess::Allowed
    {
        return Err(masked_graph_not_found());
    }

    let mut selected_dataset_ids = visible_datasets
        .iter()
        .map(|dataset| dataset.id)
        .collect::<BTreeSet<_>>();
    let mut auto_ready_links =
        BTreeMap::<(DatasetId, DatasetId), Option<storage::DatasetSemanticLinkSnapshot>>::new();
    let remaining_dataset_capacity =
        DATASET_SEMANTIC_GRAPH_MAX_DATASETS.saturating_sub(visible_datasets.len());
    let auto_limit = query
        .auto_neighbors
        .min(remaining_dataset_capacity)
        .min(DATASET_SEMANTIC_GRAPH_MAX_AUTO_NEIGHBORS);
    if auto_limit > 0 {
        let dataset_allowlist =
            std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV).ok();
        let candidate_ids = bounded_allowlisted_auto_neighbor_ids(
            dataset_allowlist.as_deref(),
            &selected_dataset_ids,
        );
        let dataset_repository = state.storage.datasets();
        let tenant_id = state.tenant_id;
        let candidate_results = stream::iter(candidate_ids.into_iter().map(|candidate_id| {
            let dataset_repository = dataset_repository.clone();
            async move { dataset_repository.get_by_id(tenant_id, candidate_id).await }
        }))
        .buffer_unordered(DATASET_SEMANTIC_GRAPH_AUTO_NEIGHBOR_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
        let mut allowlisted_datasets = Vec::new();
        for result in candidate_results {
            if let Some(dataset) = result.map_err(ApiError::from_storage)? {
                allowlisted_datasets.push(dataset);
            }
        }
        let mut candidates = visible_auto_neighbor_candidates(
            allowlisted_datasets,
            &active_secret_binding_ids,
            current_user_id,
            local_thread_id.as_deref(),
            &selected_dataset_ids,
        );
        candidates.truncate(DATASET_SEMANTIC_GRAPH_MAX_AUTO_NEIGHBOR_CANDIDATES);
        let mut reliable_counts = BTreeMap::new();
        let link_repository = state.storage.dataset_semantic_links();
        let root_dataset_id = query.root_dataset_id;
        let candidate_link_ids = candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>();
        let link_results = stream::iter(candidate_link_ids.into_iter().map(|candidate_id| {
            let link_repository = link_repository.clone();
            async move {
                let pair = canonical_pair(root_dataset_id, candidate_id);
                let latest_ready = link_repository
                    .load_latest_ready(tenant_id, pair.0, pair.1)
                    .await;
                (candidate_id, pair, latest_ready)
            }
        }))
        .buffer_unordered(DATASET_SEMANTIC_GRAPH_AUTO_NEIGHBOR_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
        for (candidate_id, pair, latest_ready) in link_results {
            let latest_ready = latest_ready.map_err(ApiError::from_storage)?;
            let allowed_pair = BTreeSet::from([pair.0, pair.1]);
            if let Some(graph) = latest_ready
                .as_ref()
                .and_then(|snapshot| graph_from_link_manifest(&snapshot.manifest, &allowed_pair))
            {
                reliable_counts.insert(candidate_id, reliable_link_counts(&graph, &allowed_pair));
            }
            auto_ready_links.insert(pair, latest_ready);
        }
        let ranked = rank_reliable_auto_neighbors(candidates, &reliable_counts, auto_limit);
        for dataset in ranked {
            selected_dataset_ids.insert(dataset.id);
            visible_datasets.push(dataset);
        }
    }

    let mut graphs = Vec::new();
    let mut snapshot_tokens = Vec::<(DatasetId, Option<Uuid>)>::new();
    let mut state_versions = visible_datasets
        .iter()
        .map(|dataset| {
            format!(
                "dataset:{}:{}",
                dataset.id,
                dataset.updated_at.timestamp_micros()
            )
        })
        .collect::<Vec<_>>();
    for dataset in &visible_datasets {
        let latest_ready = state
            .storage
            .dataset_semantic_snapshots()
            .load_latest_ready(state.tenant_id, dataset.id)
            .await
            .map_err(ApiError::from_storage)?;
        let latest_attempt = state
            .storage
            .dataset_semantic_snapshots()
            .load_latest_attempt(state.tenant_id, dataset.id)
            .await
            .map_err(ApiError::from_storage)?;
        snapshot_tokens.push((
            dataset.id,
            latest_ready.as_ref().map(|snapshot| snapshot.id),
        ));
        if latest_attempt.as_ref().map(|snapshot| snapshot.id)
            != latest_ready.as_ref().map(|snapshot| snapshot.id)
        {
            snapshot_tokens.push((
                dataset.id,
                latest_attempt.as_ref().map(|snapshot| snapshot.id),
            ));
        }
        for snapshot in latest_ready.iter().chain(latest_attempt.iter()) {
            state_versions.push(format!(
                "snapshot:{}:{}:{}:{}",
                dataset.id,
                snapshot.id,
                snapshot.status,
                snapshot.updated_at.timestamp_micros()
            ));
        }
        let attempt_is_newer = match (&latest_ready, &latest_attempt) {
            (Some(ready), Some(attempt)) => {
                ready.id != attempt.id
                    && attempt.updated_at > ready.updated_at
                    && matches!(attempt.status.as_str(), "building" | "failed")
            }
            (None, Some(_)) => true,
            _ => false,
        };
        let parsed_understanding = latest_ready.as_ref().and_then(|snapshot| {
            understanding_from_snapshot_manifest(&snapshot.manifest, dataset.id)
        });
        let manifest_valid = parsed_understanding.is_some();
        let mut graph = parsed_understanding
            .map(|understanding| project_single_understanding(dataset, &understanding))
            .unwrap_or_else(|| empty_single_graph(dataset, latest_ready.is_some()));
        graph.stale |= attempt_is_newer || latest_ready.is_some() && !manifest_valid;
        graphs.push(graph);
    }

    let mut link_tokens = Vec::<Uuid>::new();
    let mut ready_count = 0;
    let mut building_count = 0;
    let mut stale_count = 0;
    for left_index in 0..visible_datasets.len() {
        for right_index in (left_index + 1)..visible_datasets.len() {
            let pair = canonical_pair(
                visible_datasets[left_index].id,
                visible_datasets[right_index].id,
            );
            let latest_ready = match auto_ready_links.get(&pair) {
                Some(snapshot) => snapshot.clone(),
                None => state
                    .storage
                    .dataset_semantic_links()
                    .load_latest_ready(state.tenant_id, pair.0, pair.1)
                    .await
                    .map_err(ApiError::from_storage)?,
            };
            let latest_attempt = state
                .storage
                .dataset_semantic_links()
                .load_latest_attempt(state.tenant_id, pair.0, pair.1)
                .await
                .map_err(ApiError::from_storage)?;
            if let Some(snapshot) = &latest_ready {
                link_tokens.push(snapshot.id);
            }
            if let Some(snapshot) = &latest_attempt {
                link_tokens.push(snapshot.id);
            }
            for snapshot in latest_ready.iter().chain(latest_attempt.iter()) {
                state_versions.push(format!(
                    "link:{}:{}:{}:{}:{}",
                    pair.0,
                    pair.1,
                    snapshot.id,
                    snapshot.status,
                    snapshot.updated_at.timestamp_micros()
                ));
            }
            let allowed_pair = BTreeSet::from([pair.0, pair.1]);
            let ready_graph = latest_ready
                .as_ref()
                .and_then(|snapshot| graph_from_link_manifest(&snapshot.manifest, &allowed_pair));
            let newer_attempt = match (&latest_ready, &latest_attempt) {
                (Some(ready), Some(attempt)) => {
                    ready.id != attempt.id
                        && attempt.updated_at > ready.updated_at
                        && matches!(attempt.status.as_str(), "building" | "failed")
                }
                _ => false,
            };
            match ready_graph {
                Some(mut graph) => {
                    if newer_attempt || graph.stale {
                        stale_count += 1;
                        graph.stale = true;
                    } else {
                        ready_count += 1;
                    }
                    graphs.push(graph);
                }
                None if latest_ready.is_some() => stale_count += 1,
                None => match latest_attempt
                    .as_ref()
                    .map(|attempt| attempt.status.as_str())
                {
                    Some("building") => building_count += 1,
                    Some(_) => stale_count += 1,
                    None => {}
                },
            }
        }
    }

    let status = cross_links_status(ready_count, building_count, stale_count);
    let mut merged = merge_graphs(query.root_dataset_id, graphs);
    merged.stale |= status == CrossLinksStatus::Stale;
    let graph = sanitize_graph_for_visible_scope(merged, &visible_datasets, &query);
    let payload = DatasetSemanticGraphQueryResponse {
        graph,
        cross_links_status: status,
    };
    let etag = dataset_semantic_graph_etag(
        &query,
        &snapshot_tokens,
        &link_tokens,
        &state_versions,
        &scope,
    );
    build_dataset_semantic_graph_response(&headers, &payload, Some(&etag))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, Mutex};

    use axum::http::{header, HeaderMap, StatusCode};
    use chrono::Utc;
    use domain_model::{
        Dataset, DatasetId, DatasetLifecycle, DatasetVisibility, SecretBindingId, TenantId, UserId,
    };
    use uuid::Uuid;

    use super::*;
    use crate::cross_dataset_semantic_graph::{
        DatasetSemanticGraphDataset, DatasetSemanticGraphEdge, DatasetSemanticGraphNode,
        DatasetSemanticGraphNodeKind, DatasetSemanticGraphTruncation, DatasetSemanticGraphV1,
        DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION,
    };
    use crate::semantic_understanding::{SemanticEvidenceClass, SemanticRelationSemantics};

    fn dataset(value: u128, visibility: DatasetVisibility) -> Dataset {
        Dataset {
            id: DatasetId(Uuid::from_u128(value)),
            tenant_id: TenantId(Uuid::from_u128(100)),
            owner_user_id: None,
            key: format!("dataset-{value}"),
            title: format!("数据集{value}"),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn request(root_dataset_id: DatasetId) -> DatasetSemanticGraphQueryRequest {
        DatasetSemanticGraphQueryRequest {
            root_dataset_id,
            dataset_ids: Vec::new(),
            auto_neighbors: 0,
            max_nodes: 160,
            max_edges: 240,
            depth: 1,
        }
    }

    #[test]
    fn query_limits_are_absolute_and_explicit_dataset_overflow_is_rejected() {
        let root = DatasetId(Uuid::from_u128(1));
        let mut oversized = request(root);
        oversized.dataset_ids = (2..=9)
            .map(|value| DatasetId(Uuid::from_u128(value)))
            .collect();
        let error = normalize_query(oversized).expect_err("nine explicit datasets must fail");
        assert_eq!(error.code, "dataset_semantic_graph_dataset_limit_exceeded");

        let mut bounded = request(root);
        bounded.dataset_ids = vec![root, DatasetId(Uuid::from_u128(2)), root];
        bounded.auto_neighbors = 99;
        bounded.max_nodes = 99_999;
        bounded.max_edges = 99_999;
        bounded.depth = 99;
        let normalized = normalize_query(bounded).expect("bounded request");

        assert_eq!(normalized.explicit_dataset_ids.len(), 2);
        assert_eq!(normalized.auto_neighbors, 4);
        assert_eq!(normalized.max_nodes, 360);
        assert_eq!(normalized.max_edges, 600);
        assert_eq!(normalized.depth, 2);

        let mut no_cross_budget = request(root);
        no_cross_budget.auto_neighbors = 4;
        no_cross_budget.max_nodes = 1;
        no_cross_budget.max_edges = 0;
        no_cross_budget.depth = 0;
        let no_cross_budget = normalize_query(no_cross_budget).expect("bounded no-cross query");
        assert_eq!(no_cross_budget.auto_neighbors, 0);
        assert_eq!(no_cross_budget.depth, 1);
    }

    #[test]
    fn public_node_label_projection_rejects_rows_and_removes_numeric_noise() {
        assert_eq!(
            safe_node_label("2024年销售额", DatasetSemanticGraphNodeKind::Field),
            "年销售额"
        );
        assert_eq!(
            safe_node_label("门店,100,200", DatasetSemanticGraphNodeKind::Field),
            "业务字段"
        );
        assert_eq!(
            safe_node_label("STORE_CODE", DatasetSemanticGraphNodeKind::Field),
            "业务字段"
        );
    }

    #[test]
    fn public_dataset_title_projection_rejects_hash_numeric_sql_rows_and_paths() {
        let unsafe_titles = [
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "550e8400-e29b-41d4-a716-446655440000",
            "20260714123456",
            "select customer_id from sales where active = true",
            "门店\t销售额\t利润额",
            "C:\\private\\raw-data.csv",
            "postgresql://user:password@example.invalid:5432/raw",
        ];
        for title in unsafe_titles {
            assert_eq!(safe_dataset_title(title), "数据集", "accepted {title}");
        }
        assert_eq!(safe_dataset_title("新百项目资料"), "新百项目资料");
        assert_eq!(safe_dataset_title("New Bai Project"), "New Bai Project");
    }

    #[test]
    fn joint_analysis_relation_vocabulary_preserves_structure_and_analysis_boundaries() {
        assert_eq!(
            safe_relation_semantics("dataset_contains_object"),
            Some(SemanticRelationSemantics::Structure)
        );
        assert_eq!(
            safe_relation_semantics("object_contains_field"),
            Some(SemanticRelationSemantics::Structure)
        );
        assert_eq!(
            safe_relation_semantics("field_expresses_concept"),
            Some(SemanticRelationSemantics::Structure)
        );
        assert_eq!(
            safe_relation_semantics("temporal_complementarity"),
            Some(SemanticRelationSemantics::Similarity)
        );
        assert_eq!(
            safe_relation_label("temporal_complementarity", SemanticEvidenceClass::Inferred),
            "可按时间联合分析"
        );
        assert!(safe_relation_reason(
            "field_expresses_concept",
            SemanticRelationSemantics::Structure,
            SemanticEvidenceClass::Observed,
        )
        .contains("不是外键或记录关联"));
        assert!(safe_relation_reason(
            "temporal_complementarity",
            SemanticRelationSemantics::Similarity,
            SemanticEvidenceClass::Inferred,
        )
        .contains("不代表逐记录或逐人关联"));
    }

    #[test]
    fn auto_neighbor_candidates_are_built_only_from_a_bounded_exact_allowlist() {
        let root = DatasetId(Uuid::from_u128(1));
        let selected = BTreeSet::from([root]);
        let allowlist = (1..=501)
            .map(|value| DatasetId(Uuid::from_u128(value)).to_string())
            .chain(["not-a-uuid".to_string(), root.to_string()])
            .collect::<Vec<_>>()
            .join(",");

        let candidates = bounded_allowlisted_auto_neighbor_ids(Some(&allowlist), &selected);

        assert_eq!(candidates.len(), 64);
        assert!(!candidates.contains(&root));
        assert_eq!(candidates[0], DatasetId(Uuid::from_u128(2)));
        assert_eq!(candidates[63], DatasetId(Uuid::from_u128(65)));
        assert!(candidates.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn cross_graph_gate_is_fail_closed_for_feature_tenant_and_every_dataset() {
        let tenant_id = TenantId(Uuid::from_u128(100));
        let left = DatasetId(Uuid::from_u128(1));
        let right = DatasetId(Uuid::from_u128(2));
        let tenant = tenant_id.to_string();
        let datasets = format!("{left},{right}");

        assert_eq!(
            cross_graph_access_from_values(
                false,
                Some(&tenant),
                Some(&datasets),
                tenant_id,
                &[left, right]
            ),
            CrossGraphAccess::FeatureDisabled
        );
        assert_eq!(
            cross_graph_access_from_values(true, Some("*"), Some("*"), tenant_id, &[left, right]),
            CrossGraphAccess::TenantNotAllowlisted
        );
        assert_eq!(
            cross_graph_access_from_values(
                true,
                Some(&tenant),
                Some(&left.to_string()),
                tenant_id,
                &[left, right]
            ),
            CrossGraphAccess::DatasetNotAllowlisted
        );
        assert_eq!(
            cross_graph_access_from_values(
                true,
                Some(&tenant),
                Some(&datasets),
                tenant_id,
                &[left, right]
            ),
            CrossGraphAccess::Allowed
        );
    }

    #[tokio::test]
    async fn every_explicit_dataset_is_loaded_and_any_denial_is_one_masked_404() {
        let ids = vec![
            DatasetId(Uuid::from_u128(1)),
            DatasetId(Uuid::from_u128(2)),
            DatasetId(Uuid::from_u128(3)),
        ];
        let denied_id = ids[1];
        let calls = Arc::new(Mutex::new(Vec::new()));
        let result = load_all_explicit_datasets(&ids, {
            let calls = Arc::clone(&calls);
            move |dataset_id| {
                let calls = Arc::clone(&calls);
                async move {
                    calls.lock().expect("calls lock").push(dataset_id);
                    if dataset_id == denied_id {
                        Err(crate::ApiError::not_found(
                            "dataset_not_found",
                            "隐藏数据集标题与编号都不能返回".to_string(),
                        ))
                    } else {
                        Ok(dataset(dataset_id.0.as_u128(), DatasetVisibility::Public))
                    }
                }
            }
        })
        .await
        .expect_err("mixed visibility must fail as one request");

        assert_eq!(*calls.lock().expect("calls lock"), ids);
        assert_eq!(result.status, StatusCode::NOT_FOUND);
        assert_eq!(result.payload.code, "dataset_semantic_graph_not_found");
        assert!(!result.payload.message.contains("隐藏数据集"));
        assert!(!result.payload.message.contains(&denied_id.to_string()));
    }

    #[test]
    fn auto_neighbors_filter_visibility_then_rank_reliable_evidence_only() {
        let root = DatasetId(Uuid::from_u128(1));
        let public_observed = dataset(2, DatasetVisibility::Public);
        let inferred_only = dataset(3, DatasetVisibility::Public);
        let mut hidden_private = dataset(4, DatasetVisibility::Private);
        hidden_private.title = "隐藏高分数据集".to_string();
        let candidates = visible_auto_neighbor_candidates(
            vec![
                public_observed.clone(),
                inferred_only.clone(),
                hidden_private,
            ],
            &[],
            None,
            None,
            &BTreeSet::from([root]),
        );
        assert_eq!(candidates.len(), 2);

        let counts = BTreeMap::from([
            (
                public_observed.id,
                ReliableLinkCounts {
                    confirmed: 0,
                    observed: 2,
                },
            ),
            (
                inferred_only.id,
                ReliableLinkCounts {
                    confirmed: 0,
                    observed: 0,
                },
            ),
        ]);
        let ranked = rank_reliable_auto_neighbors(candidates, &counts, 4);

        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].id, public_observed.id);
        assert!(ranked
            .iter()
            .all(|candidate| candidate.title != "隐藏高分数据集"));
    }

    #[test]
    fn auto_neighbor_reliability_uses_safe_endpoints_and_never_promotes_similarity() {
        let root = DatasetId(Uuid::from_u128(1));
        let neighbor = DatasetId(Uuid::from_u128(2));
        let source_id = format!("d:{root}:field:root_key");
        let target_id = format!("d:{neighbor}:field:neighbor_key");
        let nodes = vec![
            DatasetSemanticGraphNode {
                id: source_id.clone(),
                kind: DatasetSemanticGraphNodeKind::Field,
                display_label: "门店编号".to_string(),
                dataset_refs: vec![root],
                visible_provenance_count: 1,
            },
            DatasetSemanticGraphNode {
                id: target_id.clone(),
                kind: DatasetSemanticGraphNodeKind::Field,
                display_label: "门店编码".to_string(),
                dataset_refs: vec![neighbor],
                visible_provenance_count: 1,
            },
        ];
        let similarity = DatasetSemanticGraphEdge {
            id: "unsafe-confirmed-similarity".to_string(),
            source_id: source_id.clone(),
            target_id: target_id.clone(),
            relation_type: "label_similarity".to_string(),
            label: "名称一致".to_string(),
            relation_semantics: SemanticRelationSemantics::Similarity,
            evidence_class: SemanticEvidenceClass::Confirmed,
            confidence: 1.0,
            reason: "fixture".to_string(),
            cross_dataset: true,
            supporting_dataset_ids: vec![root, neighbor],
        };
        let mut graph = DatasetSemanticGraphV1 {
            schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
            root_dataset_id: root,
            datasets: Vec::new(),
            nodes,
            edges: vec![similarity],
            truncated: DatasetSemanticGraphTruncation::default(),
            stale: false,
        };
        let allowed_pair = BTreeSet::from([root, neighbor]);

        assert_eq!(
            reliable_link_counts(&graph, &allowed_pair),
            ReliableLinkCounts::default()
        );

        graph.edges.push(DatasetSemanticGraphEdge {
            id: "observed-membership".to_string(),
            source_id,
            target_id,
            relation_type: "membership".to_string(),
            label: "内部标签".to_string(),
            relation_semantics: SemanticRelationSemantics::Structure,
            evidence_class: SemanticEvidenceClass::Observed,
            confidence: 0.8,
            reason: "fixture".to_string(),
            cross_dataset: true,
            supporting_dataset_ids: vec![DatasetId(Uuid::from_u128(999))],
        });
        assert_eq!(
            reliable_link_counts(&graph, &allowed_pair),
            ReliableLinkCounts::default(),
            "out-of-scope reported support must not be promoted from endpoint refs"
        );
        graph
            .edges
            .last_mut()
            .expect("observed fixture edge")
            .supporting_dataset_ids = vec![root, neighbor];
        assert_eq!(
            reliable_link_counts(&graph, &allowed_pair),
            ReliableLinkCounts {
                confirmed: 0,
                observed: 1,
            }
        );

        let shared_only = DatasetSemanticGraphV1 {
            schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
            root_dataset_id: root,
            datasets: Vec::new(),
            nodes: vec![DatasetSemanticGraphNode {
                id: "shared:field:0123456789abcdef0123456789abcdef".to_string(),
                kind: DatasetSemanticGraphNodeKind::Field,
                display_label: "共享字段".to_string(),
                dataset_refs: vec![root, neighbor],
                visible_provenance_count: 2,
            }],
            edges: Vec::new(),
            truncated: DatasetSemanticGraphTruncation::default(),
            stale: false,
        };
        assert_eq!(
            reliable_link_counts(&shared_only, &allowed_pair),
            ReliableLinkCounts {
                confirmed: 1,
                observed: 0,
            }
        );
        let mut stale_shared = shared_only;
        stale_shared.stale = true;
        assert_eq!(
            reliable_link_counts(&stale_shared, &allowed_pair),
            ReliableLinkCounts::default(),
            "stale ready pair evidence must not auto-select a neighbor"
        );
    }

    #[test]
    fn response_budget_preserves_one_reliable_edge_for_each_selected_auto_neighbor() {
        let root = dataset(1, DatasetVisibility::Public);
        let neighbor = dataset(2, DatasetVisibility::Public);
        let shared_id = "shared:field:0123456789abcdef0123456789abcdef".to_string();
        let hub_id = format!("d:{}:dataset:root", neighbor.id);
        let mut nodes = (0..4)
            .map(|index| DatasetSemanticGraphNode {
                id: format!("shared:concept:{index:032x}"),
                kind: DatasetSemanticGraphNodeKind::Concept,
                display_label: "一般概念".to_string(),
                dataset_refs: vec![root.id],
                visible_provenance_count: 1,
            })
            .collect::<Vec<_>>();
        nodes.push(DatasetSemanticGraphNode {
            id: shared_id.clone(),
            kind: DatasetSemanticGraphNodeKind::Field,
            display_label: "共享字段".to_string(),
            dataset_refs: vec![root.id, neighbor.id],
            visible_provenance_count: 2,
        });
        let graph = DatasetSemanticGraphV1 {
            schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
            root_dataset_id: root.id,
            datasets: Vec::new(),
            nodes,
            edges: Vec::new(),
            truncated: DatasetSemanticGraphTruncation::default(),
            stale: false,
        };
        let mut bounded = request(root.id);
        bounded.auto_neighbors = 1;
        bounded.max_nodes = 2;
        bounded.max_edges = 1;
        let query = normalize_query(bounded).expect("bounded query");

        let projected = sanitize_graph_for_visible_scope(graph, &[root, neighbor.clone()], &query);

        assert_eq!(projected.nodes.len(), 2);
        assert!(projected.nodes.iter().any(|node| node.id == shared_id));
        assert!(projected.nodes.iter().any(|node| node.id == hub_id));
        assert_eq!(projected.edges.len(), 1);
        assert_eq!(
            projected.edges[0].evidence_class,
            SemanticEvidenceClass::Confirmed
        );
        assert_eq!(projected.edges[0].relation_type, "membership");
        assert!(projected.edges[0].cross_dataset);
        assert!(projected.edges[0]
            .supporting_dataset_ids
            .contains(&neighbor.id));
    }

    #[test]
    fn etag_is_deterministic_but_changes_with_scope_and_query_limits() {
        let root = DatasetId(Uuid::from_u128(1));
        let query = normalize_query(request(root)).expect("query");
        let snapshots = vec![
            (DatasetId(Uuid::from_u128(2)), Some(Uuid::from_u128(12))),
            (root, Some(Uuid::from_u128(11))),
        ];
        let links = vec![Uuid::from_u128(22), Uuid::from_u128(21)];
        let state_versions = vec!["snapshot:ready:100".to_string()];
        let owner = UserId(Uuid::from_u128(31));
        let secret = SecretBindingId(Uuid::from_u128(41));
        let first_scope = DatasetSemanticGraphScope {
            current_user_id: Some(owner),
            active_secret_binding_ids: vec![secret],
            local_thread_id: Some("thread-a".to_string()),
        };
        let second_scope = DatasetSemanticGraphScope {
            current_user_id: Some(owner),
            active_secret_binding_ids: vec![secret],
            local_thread_id: Some("thread-b".to_string()),
        };

        let first =
            dataset_semantic_graph_etag(&query, &snapshots, &links, &state_versions, &first_scope);
        let reordered = dataset_semantic_graph_etag(
            &query,
            &snapshots.iter().copied().rev().collect::<Vec<_>>(),
            &links.iter().copied().rev().collect::<Vec<_>>(),
            &state_versions,
            &first_scope,
        );
        let different_scope =
            dataset_semantic_graph_etag(&query, &snapshots, &links, &state_versions, &second_scope);
        let mut smaller = query.clone();
        smaller.max_nodes = 100;
        let different_limits = dataset_semantic_graph_etag(
            &smaller,
            &snapshots,
            &links,
            &state_versions,
            &first_scope,
        );
        let different_state = dataset_semantic_graph_etag(
            &query,
            &snapshots,
            &links,
            &["snapshot:building:101".to_string()],
            &first_scope,
        );

        assert_eq!(first, reordered);
        assert_ne!(first, different_scope);
        assert_ne!(first, different_limits);
        assert_ne!(first, different_state);
        assert!(first.starts_with('"') && first.ends_with('"'));
    }

    #[test]
    fn persisted_manifests_must_match_schema_and_dataset_identity() {
        let left = dataset(1, DatasetVisibility::Public);
        let right = dataset(2, DatasetVisibility::Public);
        let allowed_pair = BTreeSet::from([left.id, right.id]);
        let pair_graph = DatasetSemanticGraphV1 {
            schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
            root_dataset_id: left.id,
            datasets: vec![
                DatasetSemanticGraphDataset {
                    id: left.id,
                    title: left.title.clone(),
                    stale: false,
                },
                DatasetSemanticGraphDataset {
                    id: right.id,
                    title: right.title.clone(),
                    stale: false,
                },
            ],
            nodes: Vec::new(),
            edges: Vec::new(),
            truncated: DatasetSemanticGraphTruncation::default(),
            stale: false,
        };
        let mut pair_manifest = serde_json::to_value(pair_graph).expect("pair graph manifest");
        assert!(graph_from_link_manifest(&pair_manifest, &allowed_pair).is_some());
        pair_manifest["schema_version"] = serde_json::json!("unsupported");
        assert!(graph_from_link_manifest(&pair_manifest, &allowed_pair).is_none());

        let mut single_manifest = serde_json::json!({
            "schema_version": DATASET_SEMANTIC_SCHEMA_VERSION,
            "generation_version": "test",
            "status": "ready",
            "dataset": {"id": left.id, "title": left.title},
            "coverage": {
                "source_count": 0,
                "document_count": 0,
                "record_count": 0,
                "asset_count": 0,
                "retrieval_evidence_count": 0,
                "confirmed_fact_count": 0,
                "unresolved_field_count": 0
            },
            "summary": {"headline": "", "limitations": []},
            "objects": [],
            "fields": [],
            "relations": [],
            "source_groups": [],
            "pipeline": [],
            "generated_at": "2026-07-14T00:00:00Z",
            "stale": false,
            "truncated": {"objects": 0, "fields": 0, "relations": 0}
        });
        assert!(understanding_from_snapshot_manifest(&single_manifest, left.id).is_some());
        single_manifest["dataset"]["id"] = serde_json::json!(right.id);
        assert!(understanding_from_snapshot_manifest(&single_manifest, left.id).is_none());
    }

    #[test]
    fn public_projection_recomputes_visible_scope_and_drops_internal_material() {
        let visible = dataset(1, DatasetVisibility::Public);
        let hidden_id = DatasetId(Uuid::from_u128(999));
        let shared_id = "shared:field:0123456789abcdef0123456789abcdef";
        let scoped_id = format!("d:{}:field:one", visible.id);
        let graph = DatasetSemanticGraphV1 {
            schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
            root_dataset_id: visible.id,
            datasets: vec![
                DatasetSemanticGraphDataset {
                    id: visible.id,
                    title: visible.title.clone(),
                    stale: false,
                },
                DatasetSemanticGraphDataset {
                    id: hidden_id,
                    title: "隐藏数据集标题".to_string(),
                    stale: false,
                },
            ],
            nodes: vec![
                DatasetSemanticGraphNode {
                    id: shared_id.to_string(),
                    kind: DatasetSemanticGraphNodeKind::Field,
                    display_label: "C:\\private\\raw-hmac-value.txt".to_string(),
                    dataset_refs: vec![visible.id, hidden_id],
                    visible_provenance_count: 99,
                },
                DatasetSemanticGraphNode {
                    id: scoped_id.clone(),
                    kind: DatasetSemanticGraphNodeKind::Field,
                    display_label: "门店编号".to_string(),
                    dataset_refs: vec![visible.id],
                    visible_provenance_count: 88,
                },
            ],
            edges: vec![
                DatasetSemanticGraphEdge {
                    id: "raw-hmac-edge-id".to_string(),
                    source_id: shared_id.to_string(),
                    target_id: scoped_id.clone(),
                    relation_type: "explicit_reference".to_string(),
                    label: "内部 C:\\private\\source".to_string(),
                    relation_semantics: SemanticRelationSemantics::Reference,
                    evidence_class: SemanticEvidenceClass::Observed,
                    confidence: 0.72,
                    reason: "来自 /home/private/raw-value".to_string(),
                    cross_dataset: true,
                    supporting_dataset_ids: vec![visible.id, hidden_id],
                },
                DatasetSemanticGraphEdge {
                    id: "unsafe-upgraded-similarity".to_string(),
                    source_id: shared_id.to_string(),
                    target_id: scoped_id,
                    relation_type: "label_similarity".to_string(),
                    label: "错误确认相似".to_string(),
                    relation_semantics: SemanticRelationSemantics::Similarity,
                    evidence_class: SemanticEvidenceClass::Confirmed,
                    confidence: 1.0,
                    reason: "错误升级".to_string(),
                    cross_dataset: true,
                    supporting_dataset_ids: vec![visible.id, hidden_id],
                },
            ],
            truncated: DatasetSemanticGraphTruncation {
                datasets: 88,
                nodes: 77,
                edges: 66,
            },
            stale: false,
        };
        let normalized = normalize_query(request(visible.id)).expect("query");
        let projected = sanitize_graph_for_visible_scope(graph, &[visible.clone()], &normalized);
        let response = DatasetSemanticGraphQueryResponse {
            graph: projected,
            cross_links_status: CrossLinksStatus::Stale,
        };
        let value = serde_json::to_value(&response).expect("response JSON");
        let serialized = value.to_string();

        assert_eq!(
            value["schema_version"],
            DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION
        );
        assert_eq!(value["cross_links_status"], "stale");
        assert!(value.get("graph").is_none());
        assert_eq!(value["nodes"][0]["visible_provenance_count"], 1);
        assert_eq!(value["edges"][0]["cross_dataset"], false);
        let similarity = value["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|edge| edge["relation_type"] == "label_similarity")
            .expect("similarity edge");
        assert_eq!(similarity["evidence_class"], "inferred");
        assert_eq!(value["truncated"]["datasets"], 0);
        for secret in [
            "隐藏数据集标题",
            "raw-hmac",
            "C:\\private",
            "/home/private",
            "raw-value",
            "\"visible_provenance_count\":99",
        ] {
            assert!(!serialized.contains(secret), "leaked {secret}");
        }
    }

    #[test]
    fn public_projection_never_promotes_reported_single_dataset_support_to_cross_dataset() {
        let left = dataset(1, DatasetVisibility::Public);
        let right = dataset(2, DatasetVisibility::Public);
        let shared_id = "shared:field:0123456789abcdef0123456789abcdef";
        let left_id = format!("d:{}:field:left", left.id);
        let mut graph_request = request(left.id);
        graph_request.dataset_ids = vec![right.id];
        let query = normalize_query(graph_request).expect("two-dataset query");

        let project = |reported_dataset_ids: Vec<DatasetId>, reported_cross_dataset: bool| {
            sanitize_graph_for_visible_scope(
                DatasetSemanticGraphV1 {
                    schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
                    root_dataset_id: left.id,
                    datasets: Vec::new(),
                    nodes: vec![
                        DatasetSemanticGraphNode {
                            id: shared_id.to_string(),
                            kind: DatasetSemanticGraphNodeKind::Field,
                            display_label: "共享字段".to_string(),
                            dataset_refs: vec![left.id, right.id],
                            visible_provenance_count: 2,
                        },
                        DatasetSemanticGraphNode {
                            id: left_id.clone(),
                            kind: DatasetSemanticGraphNodeKind::Field,
                            display_label: "本地字段".to_string(),
                            dataset_refs: vec![left.id],
                            visible_provenance_count: 1,
                        },
                    ],
                    edges: vec![DatasetSemanticGraphEdge {
                        id: "unsafe-persisted-edge-id".to_string(),
                        source_id: shared_id.to_string(),
                        target_id: left_id.clone(),
                        relation_type: "explicit_reference".to_string(),
                        label: "内部标签".to_string(),
                        relation_semantics: SemanticRelationSemantics::Reference,
                        evidence_class: SemanticEvidenceClass::Observed,
                        confidence: 0.72,
                        reason: "内部依据".to_string(),
                        cross_dataset: reported_cross_dataset,
                        supporting_dataset_ids: reported_dataset_ids,
                    }],
                    truncated: DatasetSemanticGraphTruncation::default(),
                    stale: false,
                },
                &[left.clone(), right.clone()],
                &query,
            )
            .edges
            .into_iter()
            .next()
            .expect("safe projected edge")
        };

        let single_support = project(vec![left.id], false);
        assert_eq!(single_support.supporting_dataset_ids, vec![left.id]);
        assert!(!single_support.cross_dataset);

        let multi_support_structure = project(vec![left.id, right.id], false);
        assert_eq!(
            multi_support_structure.supporting_dataset_ids,
            vec![left.id, right.id]
        );
        assert!(!multi_support_structure.cross_dataset);

        let cross_support = project(vec![left.id, right.id], true);
        assert_eq!(
            cross_support.supporting_dataset_ids,
            vec![left.id, right.id]
        );
        assert!(cross_support.cross_dataset);
    }

    #[test]
    fn pair_missing_is_non_error_status_and_cache_response_supports_304() {
        assert_eq!(cross_links_status(0, 0, 0), CrossLinksStatus::Empty);
        assert_eq!(cross_links_status(0, 1, 0), CrossLinksStatus::Building);
        assert_eq!(cross_links_status(1, 0, 1), CrossLinksStatus::Stale);
        assert_eq!(cross_links_status(1, 0, 0), CrossLinksStatus::Ready);

        let root = dataset(1, DatasetVisibility::Public);
        let graph = sanitize_graph_for_visible_scope(
            DatasetSemanticGraphV1 {
                schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
                root_dataset_id: root.id,
                datasets: Vec::new(),
                nodes: Vec::new(),
                edges: Vec::new(),
                truncated: DatasetSemanticGraphTruncation::default(),
                stale: false,
            },
            &[root],
            &normalize_query(request(DatasetId(Uuid::from_u128(1)))).expect("query"),
        );
        let payload = DatasetSemanticGraphQueryResponse {
            graph,
            cross_links_status: CrossLinksStatus::Empty,
        };
        let etag = "\"scope-aware-etag\"";
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag.parse().unwrap());
        let not_modified = build_dataset_semantic_graph_response(&headers, &payload, Some(etag))
            .expect("304 response");

        assert_eq!(not_modified.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            not_modified.headers().get(header::CACHE_CONTROL).unwrap(),
            "private, max-age=0, must-revalidate"
        );
        assert_eq!(not_modified.headers().get(header::ETAG).unwrap(), etag);

        let ok = build_dataset_semantic_graph_response(&HeaderMap::new(), &payload, Some(etag))
            .expect("200 response");
        assert_eq!(ok.status(), StatusCode::OK);
        assert_eq!(
            ok.headers().get(header::CACHE_CONTROL).unwrap(),
            "private, max-age=0, must-revalidate"
        );
        assert_eq!(ok.headers().get(header::ETAG).unwrap(), etag);
    }

    #[test]
    fn absolute_node_and_edge_budget_stays_below_two_megabytes() {
        let visible = dataset(1, DatasetVisibility::Public);
        let nodes = (0..DATASET_SEMANTIC_GRAPH_MAX_NODES)
            .map(|index| DatasetSemanticGraphNode {
                id: format!("d:{}:field:{index}", visible.id),
                kind: DatasetSemanticGraphNodeKind::Field,
                display_label: "经营分析字段".to_string(),
                dataset_refs: vec![visible.id],
                visible_provenance_count: 1,
            })
            .collect::<Vec<_>>();
        let mut edges = Vec::new();
        'outer: for source in 0..DATASET_SEMANTIC_GRAPH_MAX_NODES {
            for target in (source + 1)..DATASET_SEMANTIC_GRAPH_MAX_NODES {
                edges.push(DatasetSemanticGraphEdge {
                    id: format!("unsafe-edge-{source}-{target}"),
                    source_id: format!("d:{}:field:{source}", visible.id),
                    target_id: format!("d:{}:field:{target}", visible.id),
                    relation_type: "contains".to_string(),
                    label: "包含字段".to_string(),
                    relation_semantics: SemanticRelationSemantics::Structure,
                    evidence_class: SemanticEvidenceClass::Observed,
                    confidence: 0.72,
                    reason: "fixture".to_string(),
                    cross_dataset: false,
                    supporting_dataset_ids: vec![visible.id],
                });
                if edges.len() == DATASET_SEMANTIC_GRAPH_MAX_EDGES {
                    break 'outer;
                }
            }
        }
        let mut bounded_request = request(visible.id);
        bounded_request.max_nodes = usize::MAX;
        bounded_request.max_edges = usize::MAX;
        let query = normalize_query(bounded_request).expect("bounded query");
        let graph = sanitize_graph_for_visible_scope(
            DatasetSemanticGraphV1 {
                schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
                root_dataset_id: visible.id,
                datasets: Vec::new(),
                nodes,
                edges,
                truncated: DatasetSemanticGraphTruncation::default(),
                stale: false,
            },
            &[visible],
            &query,
        );
        let body = serde_json::to_vec(&DatasetSemanticGraphQueryResponse {
            graph,
            cross_links_status: CrossLinksStatus::Ready,
        })
        .expect("bounded response JSON");

        assert!(body.len() < 2_000_000, "response was {} bytes", body.len());
        let value: serde_json::Value = serde_json::from_slice(&body).expect("response value");
        assert_eq!(value["nodes"].as_array().unwrap().len(), 360);
        assert_eq!(value["edges"].as_array().unwrap().len(), 600);
    }
}
