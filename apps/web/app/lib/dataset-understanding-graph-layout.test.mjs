import assert from 'node:assert/strict';
import test from 'node:test';

import {
  datasetUnderstandingForceConfig,
  graphNodeLabelVisible,
  layoutDatasetUnderstandingGraph,
} from './dataset-understanding-graph-layout.js';

function fixture() {
  const nodes = [{ id: 'dataset:root', kind: 'dataset', symbolSize: 68 }];
  for (let objectIndex = 0; objectIndex < 3; objectIndex += 1) {
    const objectId = `object:${objectIndex}`;
    nodes.push({
      id: objectId,
      kind: 'object',
      entityType: 'object',
      symbolSize: 54,
    });
    for (let fieldIndex = 0; fieldIndex < 16; fieldIndex += 1) {
      nodes.push({
        id: `field:${objectIndex}:${String(fieldIndex).padStart(2, '0')}`,
        kind: 'field',
        entityType: 'field',
        objectId,
        businessScore: 100 - fieldIndex,
        symbolSize: 31,
      });
    }
  }
  return nodes;
}

function positionMap(nodes) {
  return Object.fromEntries(nodes.map((node) => [node.id, {
    x: node.x,
    y: node.y,
    fixed: node.fixed,
    layoutTier: node.layoutTier,
    symbolSize: node.symbolSize,
  }]));
}

test('layout fixes the 42px dataset root at the origin', () => {
  const laidOut = layoutDatasetUnderstandingGraph(fixture());
  const root = laidOut.find((node) => node.kind === 'dataset');

  assert.deepEqual({ x: root.x, y: root.y, fixed: root.fixed }, { x: 0, y: 0, fixed: true });
  assert.equal(root.symbolSize, 42);
  assert.equal(root.layoutTier, 0);
});

test('objects occupy the first ring and fields fan into deterministic second and third rings', () => {
  const laidOut = layoutDatasetUnderstandingGraph(fixture());
  const objects = laidOut.filter((node) => node.entityType === 'object');
  const fields = laidOut.filter((node) => node.entityType === 'field');

  objects.forEach((node) => {
    const radius = Math.hypot(node.x, node.y);
    assert.ok(radius >= 190 && radius <= 230);
    assert.equal(node.layoutTier, 1);
    assert.ok(node.symbolSize >= 28 && node.symbolSize <= 44);
  });
  assert.equal(fields.filter((node) => node.layoutTier === 2).length, 36);
  assert.equal(fields.filter((node) => node.layoutTier === 3).length, 12);
  fields.forEach((node) => {
    assert.ok(node.symbolSize >= 12 && node.symbolSize <= 22);
    assert.ok(Math.hypot(node.x, node.y) > 280);
  });
});

test('layout positions do not depend on backend input order', () => {
  const nodes = fixture();
  const forward = layoutDatasetUnderstandingGraph(nodes);
  const reversed = layoutDatasetUnderstandingGraph([...nodes].reverse());

  assert.deepEqual(positionMap(forward), positionMap(reversed));
});

test('dense fields use fixed twelve-slot rings without overlap in the expanded budget', () => {
  const nodes = [
    { id: 'dataset:root', kind: 'dataset' },
    { id: 'object:only', kind: 'object', entityType: 'object' },
    ...Array.from({ length: 100 }, (_, index) => ({
      id: `field:${String(index).padStart(3, '0')}`,
      kind: 'field',
      entityType: 'field',
      objectId: 'object:only',
      businessScore: 100 - index,
      symbolSize: 15,
    })),
  ];
  const fields = layoutDatasetUnderstandingGraph(nodes)
    .filter((node) => node.entityType === 'field')
    .slice(0, 24);
  const minimumDistance = Math.min(...fields.flatMap((node, index) => (
    fields.slice(index + 1).map((other) => Math.hypot(node.x - other.x, node.y - other.y))
  )));

  assert.ok(minimumDistance >= 22, `expanded field spacing was ${minimumDistance}px`);
});

test('force config scales repulsion from 280 to 380 and disables layout animation above 120 nodes', () => {
  assert.equal(datasetUnderstandingForceConfig(48).repulsion, 280);
  assert.equal(datasetUnderstandingForceConfig(84).repulsion, 330);
  assert.equal(datasetUnderstandingForceConfig(120).repulsion, 380);
  assert.equal(datasetUnderstandingForceConfig(121).repulsion, 380);
  assert.equal(datasetUnderstandingForceConfig(120).layoutAnimation, true);
  assert.equal(datasetUnderstandingForceConfig(121).layoutAnimation, false);
  assert.deepEqual(datasetUnderstandingForceConfig(100).edgeLength, [92, 176]);
  assert.equal(datasetUnderstandingForceConfig(100).gravity, 0.025);
  assert.equal(datasetUnderstandingForceConfig(100).friction, 0.32);
});

test('zoom label LOD always keeps the selected node visible', () => {
  const laidOut = layoutDatasetUnderstandingGraph(fixture());
  const object = laidOut.find((node) => node.entityType === 'object');
  const firstLevelField = laidOut.find((node) => node.layoutTier === 2);
  const deeperField = laidOut.find((node) => node.layoutTier === 3);

  assert.equal(graphNodeLabelVisible(object, 0.6, ''), true);
  assert.equal(graphNodeLabelVisible(firstLevelField, 0.6, ''), false);
  assert.equal(graphNodeLabelVisible(firstLevelField, 1, ''), true);
  assert.equal(graphNodeLabelVisible(deeperField, 1, ''), false);
  assert.equal(graphNodeLabelVisible(deeperField, 1.4, ''), true);
  assert.equal(graphNodeLabelVisible(deeperField, 0.4, deeperField.id), true);
});
