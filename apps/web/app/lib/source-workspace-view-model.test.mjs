import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  buildConnectedSourceCards,
  connectedDocumentCount,
  documentDatasetIds,
  documentUpdatedAt,
  sourceDisplayName,
  sourceFieldHints,
} from './source-workspace-view-model.js';

describe('source workspace view model', () => {
  it('uses a local display alias without changing the source title', () => {
    const source = { id: 'dataset-traffic', title: '7月客流数据集' };

    assert.equal(sourceDisplayName(source, { 'dataset-traffic': '商场客流' }), '商场客流');
    assert.equal(sourceDisplayName(source, { 'dataset-traffic': '   ' }), '7月客流数据集');
    assert.equal(sourceDisplayName(source, {}), '7月客流数据集');
    assert.equal(source.title, '7月客流数据集');
  });

  it('normalizes multi-dataset memberships, field hints, and document time', () => {
    assert.deepEqual(documentDatasetIds({
      dataset_id: ' dataset-a ',
      datasetId: 'dataset-a',
      dataset_ids: ['dataset-b', ''],
      datasetIds: ['dataset-c', 'dataset-b'],
    }), ['dataset-a', 'dataset-b', 'dataset-c']);

    assert.deepEqual(sourceFieldHints({
      document_title_hints: [
        'imports/traffic_hourly.csv',
      ],
      noun_term_hints: [
        '客流',
        ' 销售额 ',
        '客流',
        '08cd41d48f5d9202d2edb8933ee3fa55',
        '"coreDataFile": "data-core.js",',
        '20260701,SZ02,point-01,132,128',
        'token=demo-secret',
        'https://example.test/path?access_token=demo',
        'customer@example.com',
        'AbC123xYz987QwErTyUiOpAsDfGh',
        '550e8400-e29b-41d4-a716-446655440000',
        '11010519491231002X',
      ],
      sectionTitleHints: ['经营概览', '销售额'],
      material_hints: ['CSV', 'API', 'CSV', '摄像头', '小时', '去重', '停留时长'],
    }), ['客流', '销售额', '经营概览', 'CSV', 'API', '摄像头', '小时', '去重']);

    assert.equal(
      documentUpdatedAt({ updated_at: '2026-07-17T16:30:00+08:00' }),
      '2026-07-17T08:30:00.000Z',
    );
    assert.equal(documentUpdatedAt({ updated_at: 'not-a-date' }), '');
  });

  it('groups and deduplicates documents while retaining shared dataset membership', () => {
    const cards = buildConnectedSourceCards([
      {
        id: 'dataset-a',
        title: '客流数据',
        estimated_word_count: 12500,
        noun_term_hints: ['trafficIn', 'trafficOut', 'visitors'],
        section_title_hints: ['小时汇总', '点位汇总'],
        material_hints: ['API', 'CSV', 'trafficIn', 'camera', 'mall', 'hour'],
      },
      {
        id: 'dataset-b',
        title: '客流画像',
        nounTermHints: ['年龄段', '性别'],
      },
    ], [
      {
        id: 'shared',
        title: '共享资料（旧）',
        dataset_ids: ['dataset-a', 'dataset-b'],
        content_type: 'text/csv',
        parse_status: 'parsed',
        updated_at: '2026-07-17T07:30:00.000Z',
      },
      {
        id: 'shared',
        title: '共享资料（新）',
        dataset_id: 'dataset-a',
        content_type: 'text/csv',
        parse_status: 'indexed',
        updated_at: '2026-07-17T08:30:00.000Z',
      },
      {
        id: 'a-pdf',
        title: '接口说明',
        datasetId: 'dataset-a',
        contentType: 'application/pdf',
        parseStatus: 'parsed',
        updatedAt: '2026-07-16T06:00:00.000Z',
      },
      {
        id: 'outside',
        dataset_id: 'dataset-outside',
        content_type: 'text/plain',
        parse_status: 'parsed',
        updated_at: '2026-07-17T09:00:00.000Z',
      },
    ], { now: new Date('2026-07-17T09:00:00.000Z') });

    assert.deepEqual(cards.map((card) => card.id), ['dataset-a', 'dataset-b']);
    assert.equal(connectedDocumentCount([
      { id: 'dataset-a' },
      { id: 'dataset-b' },
    ], [
      { id: 'shared', dataset_ids: ['dataset-a', 'dataset-b'] },
      { id: 'shared', dataset_id: 'dataset-a' },
      { id: 'a-pdf', dataset_id: 'dataset-a' },
      { id: 'outside', dataset_id: 'dataset-outside' },
    ]), 2);

    const traffic = cards[0];
    assert.equal(traffic.title, '客流数据');
    assert.equal(traffic.documentCount, 2);
    assert.equal(traffic.estimatedWordCount, 12500);
    assert.deepEqual(traffic.fieldHints, [
      'trafficIn',
      'trafficOut',
      'visitors',
      '小时汇总',
      '点位汇总',
      'API',
      'CSV',
      'camera',
    ]);
    assert.deepEqual(traffic.contentTypes, ['text/csv', 'application/pdf']);
    assert.deepEqual(traffic.parseCounts, { indexed: 1, parsed: 1 });
    assert.equal(traffic.latestUpdatedAt, '2026-07-17T08:30:00.000Z');
    assert.equal(traffic.recent24hCount, 1);
    assert.deepEqual(traffic.recentDocuments.map((document) => document.id), ['shared', 'a-pdf']);
    assert.equal(traffic.recentDocuments[0].title, '共享资料（新）');

    const profile = cards[1];
    assert.equal(profile.documentCount, 1);
    assert.deepEqual(profile.parseCounts, { indexed: 1 });
    assert.equal(profile.latestUpdatedAt, '2026-07-17T08:30:00.000Z');
    assert.equal(profile.recentDocuments[0].title, '共享资料（新）');
  });

  it('sorts cards and recent documents by latest change and limits the timeline to four', () => {
    const cards = buildConnectedSourceCards([
      { id: 'older', title: '较早数据源' },
      { id: 'newer', title: '较新数据源' },
      { id: 'empty', title: '空数据源' },
    ], [
      ...Array.from({ length: 5 }, (_, index) => ({
        id: `older-${index + 1}`,
        dataset_id: 'older',
        content_type: index % 2 ? 'text/csv' : 'application/pdf',
        parse_status: index === 4 ? '' : 'parsed',
        estimated_word_count: 100 + index,
        updated_at: `2026-07-${String(10 + index).padStart(2, '0')}T08:00:00.000Z`,
      })),
      {
        id: 'newer-1',
        dataset_id: 'newer',
        content_type: 'application/json',
        parse_status: 'pending',
        estimatedWordCount: 250,
        updated_at: '2026-07-17T08:45:00.000Z',
      },
    ], { now: '2026-07-17T09:00:00.000Z' });

    assert.deepEqual(cards.map((card) => card.id), ['newer', 'older', 'empty']);
    assert.equal(cards[0].recent24hCount, 1);
    assert.equal(cards[0].estimatedWordCount, 250);
    assert.equal(cards[1].estimatedWordCount, 510);
    assert.deepEqual(
      cards[1].recentDocuments.map((document) => document.id),
      ['older-5', 'older-4', 'older-3', 'older-2'],
    );
    assert.deepEqual(cards[1].parseCounts, { parsed: 4, unknown: 1 });
    assert.equal(cards[2].documentCount, 0);
    assert.equal(cards[2].latestUpdatedAt, '');
    assert.deepEqual(cards[2].recentDocuments, []);
  });
});
