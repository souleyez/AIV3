export const DATASET_GRAPH_DENSITY_PROFILES = Object.freeze({
  compact: Object.freeze({ targetNodes: 48, hardNodes: 48, maxEdges: 96 }),
  standard: Object.freeze({ targetNodes: 100, hardNodes: 120, maxEdges: 180 }),
  expanded: Object.freeze({ targetNodes: 160, hardNodes: 160, maxEdges: 240 }),
});

const FIELD_FLOOR = 6;
const FIELD_SOFT_CAP = 12;
const FIELD_HARD_CAP = 24;

function cleanId(value) {
  return typeof value === 'string' ? value.trim() : String(value || '').trim();
}

function stableTextCompare(left, right) {
  return cleanId(left).localeCompare(cleanId(right), 'en');
}

function nodeKindRank(node) {
  if (node?.kind === 'dataset') return 0;
  if (node?.entityType === 'object' || node?.kind === 'object') return 1;
  if (node?.entityType === 'field' || node?.kind === 'field') return 3;
  return 2;
}

function stableNodeCompare(left, right) {
  return nodeKindRank(left) - nodeKindRank(right)
    || stableTextCompare(left?.objectId, right?.objectId)
    || stableTextCompare(left?.id, right?.id);
}

function stableFieldCompare(left, right) {
  return (Number(right?.businessScore) || 0) - (Number(left?.businessScore) || 0)
    || stableTextCompare(left?.name, right?.name)
    || stableTextCompare(left?.id, right?.id);
}

function stableLinkKey(link) {
  return cleanId(link?.id)
    || `${cleanId(link?.source)}|${cleanId(link?.target)}|${cleanId(link?.relation)}`;
}

function linkPriority(link, selectedNodeId) {
  if (selectedNodeId && (link?.source === selectedNodeId || link?.target === selectedNodeId)) return 0;
  if (link?.structural || link?.rootRelation) return 1;
  if (link?.type === 'confirmed') return 2;
  if (link?.type === 'observed') return 3;
  if (link?.type === 'inferred') return 4;
  return 5;
}

function profileForDensity(density) {
  const normalized = cleanId(density).toLocaleLowerCase();
  return {
    density: DATASET_GRAPH_DENSITY_PROFILES[normalized] ? normalized : 'standard',
    profile: DATASET_GRAPH_DENSITY_PROFILES[normalized]
      || DATASET_GRAPH_DENSITY_PROFILES.standard,
  };
}

function graphNeighborhoodIds(nodes, links, selectedNodeId) {
  const selected = cleanId(selectedNodeId);
  const nodeIds = new Set(nodes.map((node) => cleanId(node?.id)).filter(Boolean));
  if (!selected || !nodeIds.has(selected)) return new Set();
  const protectedIds = new Set([selected]);
  links.forEach((link) => {
    if (link?.source === selected && nodeIds.has(link?.target)) protectedIds.add(link.target);
    if (link?.target === selected && nodeIds.has(link?.source)) protectedIds.add(link.source);
  });
  return protectedIds;
}

function reliableCrossLink(link) {
  const evidenceClass = cleanId(link?.evidenceClass || link?.type).toLowerCase();
  return link?.crossDataset === true
    && link?.relationSemantics !== 'similarity'
    && ['confirmed', 'observed'].includes(evidenceClass);
}

function qualityEligibleNode(node, allowTechnical) {
  if (!node || !cleanId(node.id)) return false;
  return allowTechnical || node.kind === 'dataset' || !node.technicalOnly;
}

function groupFields(fields) {
  const groups = new Map();
  fields.forEach((field) => {
    const objectId = cleanId(field.objectId) || `unscoped:${cleanId(field.id)}`;
    const group = groups.get(objectId) || { objectId, fields: [] };
    group.fields.push(field);
    groups.set(objectId, group);
  });
  return [...groups.values()]
    .sort((left, right) => stableTextCompare(left.objectId, right.objectId))
    .map((group) => ({
      ...group,
      fields: group.fields.sort(stableFieldCompare),
    }));
}

function fieldCounts(selectedIds, nodeById) {
  const counts = new Map();
  selectedIds.forEach((id) => {
    const node = nodeById.get(id);
    if (node?.entityType !== 'field' && node?.kind !== 'field') return;
    const objectId = cleanId(node.objectId) || `unscoped:${id}`;
    counts.set(objectId, (counts.get(objectId) || 0) + 1);
  });
  return counts;
}

function allocateFieldsRoundRobin({ groups, selectedIds, nodeById, limit, perObjectCap }) {
  const counts = fieldCounts(selectedIds, nodeById);
  let progress = true;
  while (selectedIds.size < limit && progress) {
    progress = false;
    for (const group of groups) {
      if (selectedIds.size >= limit) break;
      const currentCount = counts.get(group.objectId) || 0;
      if (currentCount >= perObjectCap) continue;
      const nextField = group.fields.find((field) => !selectedIds.has(field.id));
      if (!nextField) continue;
      selectedIds.add(nextField.id);
      counts.set(group.objectId, currentCount + 1);
      progress = true;
    }
  }
}

export function graphDensityForContainerWidth(containerWidth) {
  const width = Number(containerWidth);
  if (!Number.isFinite(width) || width <= 0) return 'standard';
  return width < 760 ? 'compact' : 'standard';
}

export function graphBudgetStatusText(stats) {
  const visibleNodeCount = Math.max(0, Number(stats?.visibleNodeCount) || 0);
  const availableNodeCount = Math.max(visibleNodeCount, Number(stats?.availableNodeCount) || 0);
  const prefix = `已展示 ${visibleNodeCount} / 可用 ${availableNodeCount} 个关键节点`;
  if (stats?.hardLimitReached) {
    return `${prefix} · 已达到节点硬上限，请聚焦对象或缩小图谱范围`;
  }
  if (stats?.truncated) return `${prefix} · 可切换展开查看更多`;
  return prefix;
}

export function applyDatasetUnderstandingGraphBudget(graph, options = {}) {
  const { density, profile } = profileForDensity(options.density);
  const allowTechnical = options.allowTechnical === true;
  const selectedNodeId = cleanId(options.selectedNodeId);
  const inputNodes = Array.isArray(graph?.nodes) ? graph.nodes : [];
  const inputLinks = Array.isArray(graph?.links) ? graph.links : [];
  const nodes = inputNodes
    .filter((node) => qualityEligibleNode(node, allowTechnical))
    .sort(stableNodeCompare);
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const eligibleIds = new Set(nodeById.keys());
  const links = inputLinks.filter((link) => (
    eligibleIds.has(link?.source) && eligibleIds.has(link?.target)
  ));
  const protectedIds = graphNeighborhoodIds(nodes, links, selectedNodeId);
  const criticalCrossIds = new Set();
  nodes.filter((node) => node.shared === true).forEach((node) => {
    protectedIds.add(node.id);
    criticalCrossIds.add(node.id);
  });
  links.filter(reliableCrossLink).forEach((link) => {
    [link.source, link.target].forEach((id) => {
      if (!eligibleIds.has(id)) return;
      protectedIds.add(id);
      criticalCrossIds.add(id);
    });
  });
  const nonFields = nodes.filter((node) => node.entityType !== 'field' && node.kind !== 'field');
  const fields = nodes.filter((node) => node.entityType === 'field' || node.kind === 'field');
  const groups = groupFields(fields);
  const selectedIds = new Set();

  nonFields.filter((node) => node.kind === 'dataset').forEach((node) => selectedIds.add(node.id));
  if (selectedNodeId && eligibleIds.has(selectedNodeId)) selectedIds.add(selectedNodeId);

  const protectedNodes = [...protectedIds]
    .map((id) => nodeById.get(id))
    .filter(Boolean)
    .sort((left, right) => {
      if (left.id === selectedNodeId) return -1;
      if (right.id === selectedNodeId) return 1;
      if (criticalCrossIds.has(left.id) !== criticalCrossIds.has(right.id)) {
        return criticalCrossIds.has(left.id) ? -1 : 1;
      }
      if ((left.entityType === 'field' || left.kind === 'field')
        && (right.entityType === 'field' || right.kind === 'field')) {
        return stableFieldCompare(left, right);
      }
      return stableNodeCompare(left, right);
    });
  const protectedFieldCounts = new Map();
  protectedNodes.forEach((node) => {
    if (selectedIds.size >= profile.hardNodes) return;
    if (node.entityType !== 'field' && node.kind !== 'field') {
      selectedIds.add(node.id);
      return;
    }
    const objectId = cleanId(node.objectId) || `unscoped:${node.id}`;
    const count = protectedFieldCounts.get(objectId) || 0;
    if (node.id === selectedNodeId || criticalCrossIds.has(node.id) || count < FIELD_HARD_CAP) {
      selectedIds.add(node.id);
      protectedFieldCounts.set(objectId, count + 1);
    }
  });

  nonFields.forEach((node) => {
    if (selectedIds.size < profile.targetNodes) selectedIds.add(node.id);
  });

  const allocationLimit = Math.min(
    profile.hardNodes,
    Math.max(profile.targetNodes, selectedIds.size),
  );
  allocateFieldsRoundRobin({
    groups,
    selectedIds,
    nodeById,
    limit: allocationLimit,
    perObjectCap: FIELD_FLOOR,
  });
  allocateFieldsRoundRobin({
    groups,
    selectedIds,
    nodeById,
    limit: allocationLimit,
    perObjectCap: FIELD_SOFT_CAP,
  });
  allocateFieldsRoundRobin({
    groups,
    selectedIds,
    nodeById,
    limit: allocationLimit,
    perObjectCap: FIELD_HARD_CAP,
  });

  const visibleNodes = nodes.filter((node) => selectedIds.has(node.id)).sort(stableNodeCompare);
  const visibleIds = new Set(visibleNodes.map((node) => node.id));
  const visibleLinks = links
    .filter((link) => visibleIds.has(link.source) && visibleIds.has(link.target))
    .sort((left, right) => (
      linkPriority(left, selectedNodeId) - linkPriority(right, selectedNodeId)
      || stableTextCompare(stableLinkKey(left), stableLinkKey(right))
    ))
    .slice(0, profile.maxEdges);

  return {
    nodes: visibleNodes,
    links: visibleLinks,
    stats: {
      density,
      targetNodeCount: profile.targetNodes,
      hardNodeCount: profile.hardNodes,
      maxEdgeCount: profile.maxEdges,
      visibleNodeCount: visibleNodes.length,
      availableNodeCount: nodes.length,
      visibleEdgeCount: visibleLinks.length,
      availableEdgeCount: links.filter((link) => (
        visibleIds.has(link.source) && visibleIds.has(link.target)
      )).length,
      truncated: visibleNodes.length < nodes.length,
      edgesTruncated: visibleLinks.length < links.filter((link) => (
        visibleIds.has(link.source) && visibleIds.has(link.target)
      )).length,
      hardLimitReached: visibleNodes.length >= profile.hardNodes && visibleNodes.length < nodes.length,
    },
  };
}
