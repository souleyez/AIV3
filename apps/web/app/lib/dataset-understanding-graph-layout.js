const ROOT_SIZE = 42;
const OBJECT_RING_RADIUS = 210;
const FIELD_SECOND_RING_RADIUS = 340;
const FIELDS_PER_RING = 12;
const FIELD_RING_STEP = 92;
const FIELD_NODE_GAP = 8;

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

function fieldSlotsPerRing(primaryCount, maximumSymbolSize) {
  const availableSector = Math.min(0.9, (Math.PI * 2 / Math.max(1, primaryCount)) * 0.72);
  const desiredDistance = maximumSymbolSize + FIELD_NODE_GAP;
  const minimumAngle = 2 * Math.asin(Math.min(0.99, desiredDistance / (2 * FIELD_SECOND_RING_RADIUS)));
  return Math.max(2, Math.min(FIELDS_PER_RING, Math.floor(availableSector / minimumAngle) + 1));
}

function centerOutSlotOrder(count) {
  const center = (count - 1) / 2;
  return Array.from({ length: count }, (_, index) => index)
    .sort((left, right) => Math.abs(left - center) - Math.abs(right - center) || left - right);
}

function fieldFanAngle(parentAngle, index, primaryCount, slotsPerRing) {
  const availableSector = Math.min(0.9, (Math.PI * 2 / Math.max(1, primaryCount)) * 0.72);
  const slot = centerOutSlotOrder(slotsPerRing)[index % slotsPerRing];
  return parentAngle - availableSector / 2 + availableSector * (slot / (slotsPerRing - 1));
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
      const maximumSymbolSize = Math.max(
        12,
        ...group.map((field) => clamp(field.symbolSize, 12, 22, 17)),
      );
      const slotsPerRing = fieldSlotsPerRing(primaryNodes.length, maximumSymbolSize);
      group.forEach((field, index) => {
        const ring = Math.floor(index / slotsPerRing);
        const angle = fieldFanAngle(parentAngle, index, primaryNodes.length, slotsPerRing);
        positioned.set(field.id, {
          ...field,
          ...polarPosition(FIELD_SECOND_RING_RADIUS + ring * FIELD_RING_STEP, angle),
          fixed: false,
          layoutTier: ring === 0 ? 2 : 3,
          symbolSize: clamp(field.symbolSize, 12, 22, ring === 0 ? 17 : 15),
        });
      });
    });

  return nodes.map((node) => positioned.get(node.id) || node);
}
