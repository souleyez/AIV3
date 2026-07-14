const ROOT_SIZE = 42;
const OBJECT_RING_RADIUS = 210;
const FIELD_SECOND_RING_RADIUS = 340;
const FIELD_THIRD_RING_RADIUS = 470;
const FIELDS_PER_RING = 12;

function cleanId(value) {
  return typeof value === 'string' ? value.trim() : String(value || '').trim();
}

function stableCompare(left, right) {
  const leftId = cleanId(left?.id);
  const rightId = cleanId(right?.id);
  return leftId < rightId ? -1 : leftId > rightId ? 1 : 0;
}

function stableFieldCompare(left, right) {
  return (Number(right?.businessScore) || 0) - (Number(left?.businessScore) || 0)
    || stableCompare(left, right);
}

function clamp(value, minimum, maximum, fallback) {
  const numeric = Number(value);
  return Math.max(minimum, Math.min(maximum, Number.isFinite(numeric) ? numeric : fallback));
}

function polarPosition(radius, angle) {
  return {
    x: Number((Math.cos(angle) * radius).toFixed(3)),
    y: Number((Math.sin(angle) * radius).toFixed(3)),
  };
}

function fieldFanAngle(parentAngle, index, count, primaryCount) {
  if (count <= 1) return parentAngle;
  const availableSector = Math.min(0.9, (Math.PI * 2 / Math.max(1, primaryCount)) * 0.72);
  return parentAngle - availableSector / 2 + availableSector * (index / (count - 1));
}

function nodeKindRank(node) {
  if (node?.kind === 'dataset') return 0;
  if (node?.entityType === 'object' || node?.kind === 'object') return 1;
  if (node?.entityType === 'field' || node?.kind === 'field') return 3;
  return 2;
}

export function datasetUnderstandingForceConfig(nodeCount) {
  const count = Math.max(0, Number(nodeCount) || 0);
  const progress = Math.max(0, Math.min(1, (count - 48) / (120 - 48)));
  return {
    repulsion: Math.round(280 + progress * 100),
    gravity: 0.025,
    edgeLength: [92, 176],
    friction: 0.32,
    initLayout: 'none',
    layoutAnimation: count <= 120,
  };
}

export function graphNodeLabelVisible(node, zoom, selectedNodeId = '') {
  if (cleanId(node?.id) === cleanId(selectedNodeId) && cleanId(selectedNodeId)) return true;
  const normalizedZoom = Number.isFinite(Number(zoom)) ? Number(zoom) : 1;
  const tier = Number(node?.layoutTier) || 0;
  if (normalizedZoom < 0.75) return tier <= 1;
  if (normalizedZoom <= 1.25) return tier <= 2;
  return true;
}

export function layoutDatasetUnderstandingGraph(inputNodes) {
  const nodes = (Array.isArray(inputNodes) ? inputNodes : [])
    .filter((node) => node && cleanId(node.id))
    .sort((left, right) => nodeKindRank(left) - nodeKindRank(right) || stableCompare(left, right));
  const root = nodes.find((node) => node.kind === 'dataset') || null;
  const fields = nodes.filter((node) => node.entityType === 'field' || node.kind === 'field');
  const primaryNodes = nodes.filter((node) => (
    node.id !== root?.id && node.entityType !== 'field' && node.kind !== 'field'
  ));
  const primaryAngles = new Map();
  primaryNodes.forEach((node, index) => {
    primaryAngles.set(node.id, -Math.PI / 2 + Math.PI * 2 * index / Math.max(1, primaryNodes.length));
  });

  const fieldsByObject = new Map();
  fields.forEach((field) => {
    const objectId = cleanId(field.objectId) || `unscoped:${field.id}`;
    const group = fieldsByObject.get(objectId) || [];
    group.push(field);
    fieldsByObject.set(objectId, group);
  });
  fieldsByObject.forEach((group) => group.sort(stableFieldCompare));

  const positioned = new Map();
  if (root) {
    positioned.set(root.id, {
      ...root,
      x: 0,
      y: 0,
      fixed: true,
      layoutTier: 0,
      symbolSize: ROOT_SIZE,
    });
  }
  primaryNodes.forEach((node) => {
    const position = polarPosition(OBJECT_RING_RADIUS, primaryAngles.get(node.id) || 0);
    const isObject = node.entityType === 'object' || node.kind === 'object';
    positioned.set(node.id, {
      ...node,
      ...position,
      fixed: false,
      layoutTier: 1,
      symbolSize: isObject
        ? clamp(node.symbolSize, 28, 44, 34)
        : clamp(node.symbolSize, 18, 34, 26),
    });
  });

  [...fieldsByObject.entries()]
    .sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
    .forEach(([objectId, group], groupIndex) => {
      const parentAngle = primaryAngles.get(objectId)
        ?? (-Math.PI / 2 + Math.PI * 2 * groupIndex / Math.max(1, fieldsByObject.size));
      const secondRing = group.slice(0, FIELDS_PER_RING);
      const thirdRing = group.slice(FIELDS_PER_RING);
      secondRing.forEach((field, index) => {
        const angle = fieldFanAngle(parentAngle, index, secondRing.length, primaryNodes.length);
        positioned.set(field.id, {
          ...field,
          ...polarPosition(FIELD_SECOND_RING_RADIUS, angle),
          fixed: false,
          layoutTier: 2,
          symbolSize: clamp(field.symbolSize, 12, 22, 17),
        });
      });
      thirdRing.forEach((field, index) => {
        const angle = fieldFanAngle(parentAngle, index, thirdRing.length, primaryNodes.length);
        positioned.set(field.id, {
          ...field,
          ...polarPosition(FIELD_THIRD_RING_RADIUS, angle),
          fixed: false,
          layoutTier: 3,
          symbolSize: clamp(field.symbolSize, 12, 22, 15),
        });
      });
    });

  return nodes.map((node) => positioned.get(node.id) || node);
}
