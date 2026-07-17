import assert from 'node:assert/strict';
import test from 'node:test';

import { buildDatasetDictionaryView } from './dataset-dictionary-view-model.js';

function evidence(sourceKind = 'spreadsheet') {
  return [{ source_kind: sourceKind, source_id: 'source-1', label: 'header' }];
}

test('file headers are authoritative and narrative facts cannot become dictionary fields', () => {
  const view = buildDatasetDictionaryView({
    tabularSchema: {
      tables: [{
        documentId: 'traffic-hourly',
        title: 'traffic_hourly.csv',
        contentType: 'text/csv',
        structuralSource: 'file_header',
        columns: [
          { ordinal: 1, name: 'day' },
          { ordinal: 2, name: 'trafficIn' },
          { ordinal: 3, name: 'averageStay' },
        ],
      }],
    },
    understanding: {
      fields: [
        {
          technical_name: 'trafficIn',
          label: '进入人次',
          value_type: 'integer',
          semantic_role: 'metric',
          label_source: 'spreadsheet_header',
          evidence_refs: evidence(),
        },
        {
          technical_name: '不能跨点位或跨小时直接相加作为期间去重人数',
          label: '不能跨点位或跨小时直接相加作为期间去重人数',
          label_source: 'document_fact',
          evidence_refs: [{ source_kind: 'document' }],
        },
      ],
    },
  });

  assert.equal(view.source, 'file_header');
  assert.equal(view.tableCount, 1);
  assert.equal(view.fieldCount, 3);
  assert.equal(view.businessClueCount, 1);
  assert.deepEqual(view.allFields.map((field) => field.technical_name), ['day', 'trafficIn', 'averageStay']);
  assert.equal(view.allFields[1].label, '进入人次');
  assert.match(view.allFields[2].description, /不可用/);
  assert.doesNotMatch(JSON.stringify(view), /不能跨点位或跨小时/);
});

test('dictionary never carries source value examples into the public view model', () => {
  const syntheticSecret = 'synthetic-token-1234567890-not-a-real-secret';
  const syntheticPersonId = '00000000-0000-4000-8000-000000000000';
  const view = buildDatasetDictionaryView({
    tabularSchema: {
      tables: [{
        documentId: 'profile',
        title: 'safe.csv',
        columns: [
          { ordinal: 1, name: 'point_id' },
          { ordinal: 2, name: 'PersonID' },
          { ordinal: 3, name: 'remark' },
        ],
      }],
    },
    understanding: {
      fields: [
        {
          technical_name: 'point_id',
          label_source: 'spreadsheet_header',
          evidence_refs: evidence(),
          examples: ['P01'],
        },
        {
          technical_name: 'PersonID',
          label_source: 'spreadsheet_header',
          evidence_refs: evidence(),
          examples: [syntheticPersonId],
        },
        {
          technical_name: 'remark',
          label_source: 'spreadsheet_header',
          evidence_refs: evidence(),
          examples: [syntheticSecret],
        },
      ],
    },
  });

  assert.deepEqual(view.allFields.map((field) => field.examples), [[], [], []]);
  assert.doesNotMatch(JSON.stringify(view), /P01/);
  assert.doesNotMatch(JSON.stringify(view), new RegExp(syntheticPersonId));
  assert.doesNotMatch(JSON.stringify(view), new RegExp(syntheticSecret));
});

test('semantic fallback accepts only structured identifiers and rejects document facts', () => {
  const view = buildDatasetDictionaryView({
    understanding: {
      fields: [
        {
          id: 'field-1',
          technical_name: 'trafficOut',
          label: '离开人次',
          label_source: 'spreadsheet_header',
          evidence_refs: evidence(),
        },
        {
          id: 'field-2',
          technical_name: '页面先用约呈现真实的全场趋势和小时数据',
          label_source: 'document_fact',
          evidence_refs: [{ source_kind: 'document' }],
        },
      ],
    },
  });

  assert.equal(view.source, 'semantic_fallback');
  assert.deepEqual(view.allFields.map((field) => field.technical_name), ['trafficOut']);
});
