import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildTabularDocumentPreview,
  inferTabularDelimiter,
  isSensitiveTabularColumn,
  isTabularDocument,
  maskTabularCell,
  parseDelimitedPreview,
} from './tabular-document-view.js';

test('inferTabularDelimiter recognizes comma and tab separated content', () => {
  assert.equal(inferTabularDelimiter('day,hour,trafficIn\n2026-07-01,10,18'), ',');
  assert.equal(inferTabularDelimiter('day\thour\ttrafficIn\n2026-07-01\t10\t18'), '\t');
});

test('parseDelimitedPreview handles quoted commas and duplicate headers', () => {
  const preview = parseDelimitedPreview([
    'point,area,area',
    'P01,"一层,东区",入口',
    'P02,二层,中庭',
  ].join('\n'));

  assert.deepEqual(preview.columns, ['point', 'area', 'area_2']);
  assert.deepEqual(preview.rows[0], ['P01', '一层,东区', '入口']);
});

test('parseDelimitedPreview limits rows and ignores repeated chunk headers', () => {
  const preview = parseDelimitedPreview([
    'day,hour,trafficIn',
    '2026-07-01,10,18',
    '',
    'day,hour,trafficIn',
    '2026-07-01,11,25',
    '2026-07-01,12,31',
  ].join('\n'), { maxRows: 2 });

  assert.equal(preview.rows.length, 2);
  assert.deepEqual(preview.rows[1], ['2026-07-01', '11', '25']);
});

test('buildTabularDocumentPreview only parses tabular document types', () => {
  const text = 'day,hour,trafficIn\n2026-07-01,10,18';
  assert.equal(buildTabularDocumentPreview({ title: 'README.md' }, text), null);
  assert.deepEqual(
    buildTabularDocumentPreview({ title: 'traffic_hourly.csv', content_type: 'text/csv' }, text)?.columns,
    ['day', 'hour', 'trafficIn'],
  );
});

test('isTabularDocument keeps single-column and malformed tables behind the raw-text guard', () => {
  assert.equal(isTabularDocument({ content_type: 'text/csv', title: 'person_ids.csv' }), true);
  assert.equal(isTabularDocument({ content_type: 'text/plain', title: 'README.md' }), false);
  assert.equal(buildTabularDocumentPreview({ content_type: 'text/csv' }, 'PersonID\nperson-001'), null);
});

test('tabular preview hides identity columns and token-like values', () => {
  assert.equal(isSensitiveTabularColumn('PersonID'), true);
  assert.equal(isSensitiveTabularColumn('point_id'), false);
  assert.deepEqual(maskTabularCell('phone', '13800138000'), { value: '已隐藏', masked: true });

  const preview = parseDelimitedPreview([
    'PersonID,point_id,remark',
    'person-001,P01,normal',
    'person-002,P02,a8f93c5e71b24f60a3948631f8b72d99',
  ].join('\n'));

  assert.equal(preview.hasSensitiveData, true);
  assert.deepEqual(preview.rows, [
    ['已隐藏', 'P01', 'normal'],
    ['已隐藏', 'P02', '已隐藏'],
  ]);
});
