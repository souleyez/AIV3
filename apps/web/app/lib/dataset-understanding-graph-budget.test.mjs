import assert from 'node:assert/strict';
import test from 'node:test';

import {
  applyDatasetUnderstandingGraphBudget,
  DATASET_GRAPH_DENSITY_PROFILES,
  graphBudgetStatusText,
  graphDensityForContainerWidth,
} from './dataset-understanding-graph-budget.js';

function graphFixture({ objectCount = 9, fieldCount = 155, lowQualityCount = 0 } = {}) {
  const nodes = [{
    id: 'dataset:root',
    kind: 'dataset',
    entityType: 'dataset',
    name: '测试数据集',
  }];
  const links = [];

  for (let index = 0; index < objectCount; index += 1) {
    const objectId = `object:${String(index).padStart(2, '0')}`;
    nodes.push({
      id: objectId,
      kind: 'object',
      entityType: 'object',
      name: `业务对象${index}`,
    });
    links.push({
      id: `root:${objectId}`,
      source: 'dataset:root',
      target: objectId,
      structural: true,
      rootRelation: true,
      type: 'observed',
    });
  }

  for (let index = 0; index < fieldCount; index += 1) {
    const objectIndex = index % objectCount;
    const objectId = `object:${String(objectIndex).padStart(2, '0')}`;
    const fieldId = `field:${String(index).padStart(3, '0')}`;
    nodes.push({
      id: fieldId,
      kind: 'field',
      entityType: 'field',
      objectId,
      name: `关键字段${index}`,
      businessScore: 1000 - index,
      technicalOnly: index >= fieldCount - lowQualityCount,
    });
    links.push({
      id: `membership:${fieldId}`,
      source: objectId,
      target: fieldId,
      structural: true,
      rootRelation: false,
      type: 'observed',
    });
  }

  return { nodes, links };
}

function fieldCountsByObject(nodes) {
  const counts = new Map();
  nodes.filter((node) => node.entityType === 'field').forEach((node) => {
    counts.set(node.objectId, (counts.get(node.objectId) || 0) + 1);
  });
  return [...counts.values()];
}

test('density profiles keep the explicit compact, standard and expanded limits', () => {
  assert.deepEqual(DATASET_GRAPH_DENSITY_PROFILES.compact, {
    targetNodes: 48,
    hardNodes: 48,
    maxEdges: 96,
  });
  assert.deepEqual(DATASET_GRAPH_DENSITY_PROFILES.standard, {
    targetNodes: 100,
    hardNodes: 120,
    maxEdges: 180,
  });
  assert.deepEqual(DATASET_GRAPH_DENSITY_PROFILES.expanded, {
    targetNodes: 160,
    hardNodes: 160,
    maxEdges: 240,
  });
});

test('standard budget projects the 9-object 155-field fixture to 100 balanced nodes', () => {
  const graph = graphFixture();
  const result = applyDatasetUnderstandingGraphBudget(graph, { density: 'standard' });
  const fieldCounts = fieldCountsByObject(result.nodes);

  assert.equal(result.nodes.length, 100);
  assert.ok(result.nodes.length <= 120);
  assert.equal(result.stats.availableNodeCount, 165);
  assert.equal(result.stats.visibleNodeCount, 100);
  assert.equal(result.stats.truncated, true);
  assert.equal(fieldCounts.length, 9);
  assert.ok(fieldCounts.every((count) => count >= 6));
  assert.ok(fieldCounts.every((count) => count <= 12));
  assert.ok(Math.max(...fieldCounts) - Math.min(...fieldCounts) <= 1);
});

test('expanded budget allocates beyond the soft cap fairly but never beyond 24 fields per object', () => {
  const graph = graphFixture();
  const result = applyDatasetUnderstandingGraphBudget(graph, { density: 'expanded' });
  const fieldCounts = fieldCountsByObject(result.nodes);

  assert.equal(result.nodes.length, 160);
  assert.ok(fieldCounts.some((count) => count > 12));
  assert.ok(fieldCounts.every((count) => count <= 24));
  assert.ok(Math.max(...fieldCounts) - Math.min(...fieldCounts) <= 1);
  assert.ok(result.links.length <= 240);
});

test('compact budget remains fair when there is not enough room for six fields per object', () => {
  const graph = graphFixture();
  const result = applyDatasetUnderstandingGraphBudget(graph, { density: 'compact' });
  const fieldCounts = fieldCountsByObject(result.nodes);

  assert.equal(result.nodes.length, 48);
  assert.ok(Math.max(...fieldCounts) - Math.min(...fieldCounts) <= 1);
});

test('selected node and its one-hop neighbors displace lower-priority budget items', () => {
  const graph = graphFixture();
  graph.links.push({
    id: 'selected:neighbor',
    source: 'field:154',
    target: 'field:153',
    structural: false,
    type: 'confirmed',
  });

  const baseline = applyDatasetUnderstandingGraphBudget(graph, { density: 'standard' });
  const selected = applyDatasetUnderstandingGraphBudget(graph, {
    density: 'standard',
    selectedNodeId: 'field:154',
  });

  assert.equal(baseline.nodes.some((node) => node.id === 'field:154'), false);
  assert.equal(selected.nodes.some((node) => node.id === 'field:154'), true);
  assert.equal(selected.nodes.some((node) => node.id === 'field:153'), true);
  assert.ok(selected.nodes.length <= 120);
});

test('low-quality fields never enter the graph merely to fill a node target', () => {
  const graph = graphFixture({ objectCount: 2, fieldCount: 50, lowQualityCount: 30 });
  const result = applyDatasetUnderstandingGraphBudget(graph, { density: 'standard' });

  assert.equal(result.nodes.length, 23);
  assert.equal(result.stats.availableNodeCount, 23);
  assert.equal(result.stats.truncated, false);
  assert.equal(result.nodes.some((node) => node.technicalOnly), false);
});

test('budget projection is stable when backend node and edge order changes', () => {
  const graph = graphFixture();
  const forward = applyDatasetUnderstandingGraphBudget(graph, { density: 'standard' });
  const reversed = applyDatasetUnderstandingGraphBudget({
    nodes: [...graph.nodes].reverse(),
    links: [...graph.links].reverse(),
  }, { density: 'standard' });

  assert.deepEqual(
    forward.nodes.map((node) => node.id),
    reversed.nodes.map((node) => node.id),
  );
  assert.deepEqual(
    forward.links.map((link) => link.id),
    reversed.links.map((link) => link.id),
  );
});

test('expanded dense graphs cap edges at 240 with deterministic evidence priority', () => {
  const graph = graphFixture();
  const fieldIds = graph.nodes.filter((node) => node.entityType === 'field').map((node) => node.id);
  for (let left = 0; left < 30; left += 1) {
    for (let right = left + 1; right < 30; right += 1) {
      graph.links.push({
        id: `dense:${String(left).padStart(2, '0')}:${String(right).padStart(2, '0')}`,
        source: fieldIds[left],
        target: fieldIds[right],
        structural: false,
        type: (left + right) % 3 === 0 ? 'confirmed' : 'inferred',
      });
    }
  }

  const result = applyDatasetUnderstandingGraphBudget(graph, { density: 'expanded' });

  assert.equal(result.links.length, 240);
  assert.ok(result.links.some((link) => link.structural));
});

test('container width selects compact or standard without user-agent checks', () => {
  assert.equal(graphDensityForContainerWidth(520), 'compact');
  assert.equal(graphDensityForContainerWidth(759), 'compact');
  assert.equal(graphDensityForContainerWidth(760), 'standard');
  assert.equal(graphDensityForContainerWidth(1440), 'standard');
  assert.equal(graphDensityForContainerWidth(Number.NaN), 'standard');
});

test('budget status copy reports visible and available nodes plus the hard-limit action', () => {
  assert.equal(graphBudgetStatusText({
    visibleNodeCount: 100,
    availableNodeCount: 165,
    truncated: true,
    hardLimitReached: false,
  }), '已展示 100 / 可用 165 个关键节点 · 可切换展开查看更多');
  assert.equal(graphBudgetStatusText({
    visibleNodeCount: 160,
    availableNodeCount: 165,
    truncated: true,
    hardLimitReached: true,
  }), '已展示 160 / 可用 165 个关键节点 · 已达到节点硬上限，请聚焦对象或缩小图谱范围');
});
