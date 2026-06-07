import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildLocalUploadObjectKey,
  buildUploadDatasetPayload,
  classifyUploadTarget,
  inferUploadMediaKind,
  isPublicUploadClassification,
  summarizeUploadClassification,
} from './upload-classifier.js';

const datasets = [
  {
    id: 'dataset-orders',
    key: 'orders',
    title: '订单',
    description: '销售与营收',
  },
  {
    id: 'dataset-support',
    key: 'support',
    title: '客服',
    description: '工单和投诉',
  },
  {
    id: 'dataset-unclassified',
    key: 'unclassified',
    title: '未分类',
  },
];

test('upload classifier keeps selected dataset as strongest target', () => {
  const result = classifyUploadTarget({
    file: { name: 'support-ticket.csv', type: 'text/csv' },
    datasets,
    selectedDatasetId: 'dataset-orders',
  });

  assert.equal(result.datasetId, 'dataset-orders');
  assert.equal(result.source, 'user_selected');
  assert.equal(result.confidence, 'high');
  assert.equal(isPublicUploadClassification(result), false);
});

test('upload classifier matches common business hints by file name', () => {
  const result = classifyUploadTarget({
    file: { name: '客服投诉明细.xlsx', type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' },
    datasets,
  });

  assert.equal(result.datasetId, 'dataset-support');
  assert.equal(result.source, 'upload_classified');
  assert.match(result.reason, /客服/);
});

test('upload classifier suggests default dataset when category is missing', () => {
  const result = classifyUploadTarget({
    file: { name: '官网采集结果.json', type: 'application/json' },
    datasets: datasets.filter((dataset) => dataset.title !== '网页采集'),
  });
  const payload = buildUploadDatasetPayload(result);

  assert.equal(result.datasetId, '');
  assert.equal(payload.key, 'web_collection');
  assert.equal(payload.title, '网页采集');
});

test('upload classifier falls back to unclassified dataset', () => {
  const result = classifyUploadTarget({
    file: { name: 'random-notes.txt', type: 'text/plain' },
    datasets,
  });

  assert.equal(result.datasetId, 'dataset-unclassified');
  assert.equal(result.source, 'fallback_unclassified');
  assert.equal(isPublicUploadClassification(result), true);
});

test('upload object key is stable and safe enough for browser registration', () => {
  const key = buildLocalUploadObjectKey({ name: 'Q1 订单 明细.csv' }, 1710000000000);

  assert.equal(key, 'browser-uploads/1710000000000-Q1-订单-明细.csv');
});

test('upload classification summary names target dataset', () => {
  const result = classifyUploadTarget({
    file: { name: 'orders.csv', type: 'text/csv' },
    datasets,
  });
  const summary = summarizeUploadClassification({
    fileCount: 2,
    datasetTitle: '订单',
    classification: result,
  });

  assert.match(summary, /2 个文件/);
  assert.match(summary, /订单/);
});

test('upload classifier detects audio and video materials', () => {
  assert.equal(inferUploadMediaKind({ name: '客服通话录音.mp3', type: 'audio/mpeg' }), 'audio');
  assert.equal(inferUploadMediaKind({ name: '巡店视频.mov', type: 'video/quicktime' }), 'video');
  assert.equal(inferUploadMediaKind({ name: '课程回放.m4v', type: '' }), 'video');
  assert.equal(inferUploadMediaKind({ name: 'orders.csv', type: 'text/csv' }), '');
});
