import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  documentDatasetIds,
  documentDatasetSelectionUpdate,
  documentMembershipCurrentDatasetIds,
  documentMembershipResponseDatasetIds,
  filterRecordsByDatasetIds,
  normalizeDatasetIds,
  reportRecordDatasetIds,
  sameDatasetIds,
  sortByDateDesc,
  sortDatasets,
  staticPageDraftDatasetIds,
  toggleSelectedDatasetIds,
} from './dataset-record-scope.js';

describe('dataset record scope helpers', () => {
  it('normalizes dataset ids with trimming, dedupe, and original order', () => {
    assert.deepEqual(
      normalizeDatasetIds([' ds-a ', '', null, 'ds-b', 'ds-a', 0, '0']),
      ['ds-a', 'ds-b', '0'],
    );
    assert.deepEqual(normalizeDatasetIds(' single '), ['single']);
  });

  it('keeps existing dataset id equality order-sensitive', () => {
    assert.equal(sameDatasetIds(['a', 'b', 'a'], ['a', 'b']), true);
    assert.equal(sameDatasetIds(['a', 'b'], ['b', 'a']), false);
    assert.equal(sameDatasetIds([], null), true);
  });

  it('toggles selected dataset ids while preserving existing ordering', () => {
    assert.deepEqual(
      toggleSelectedDatasetIds([' ds-a ', 'ds-b', 'ds-a'], 'ds-c'),
      ['ds-a', 'ds-b', 'ds-c'],
    );
    assert.deepEqual(
      toggleSelectedDatasetIds(['ds-a', 'ds-b', 'ds-c'], 'ds-b'),
      ['ds-a', 'ds-c'],
    );
    assert.deepEqual(toggleSelectedDatasetIds([], 'ds-a'), ['ds-a']);
  });

  it('collects document, report, and static page draft dataset ownership fields', () => {
    assert.deepEqual(
      documentDatasetIds({
        dataset_id: 'doc-a',
        datasetId: 'doc-b',
        dataset_ids: ['doc-c', 'doc-a'],
        datasetIds: ['doc-d'],
      }),
      ['doc-a', 'doc-b', 'doc-c', 'doc-d'],
    );
    assert.deepEqual(
      reportRecordDatasetIds({
        dataset_id: 'report-a',
        datasetId: 'report-b',
        dataset: { id: 'report-c' },
        plan: { dataset_id: 'report-d', datasetId: 'report-e' },
        dataset_ids: ['report-f'],
        datasetIds: ['report-a', 'report-g'],
      }),
      ['report-a', 'report-b', 'report-c', 'report-d', 'report-e', 'report-f', 'report-g'],
    );
    assert.deepEqual(
      staticPageDraftDatasetIds({
        matchedDatasetIds: ['draft-a'],
        matched_dataset_ids: ['draft-b'],
        matchedDatasetId: 'draft-c',
        matched_dataset_id: 'draft-d',
        datasetId: 'draft-e',
        dataset_id: 'draft-f',
        dataSnapshot: { datasetId: 'draft-g', dataset_id: 'draft-h' },
        source: { datasetId: 'draft-i', dataset_id: 'draft-j' },
        source_refs: { dataset_id: 'draft-k' },
        sourceRefs: { datasetId: 'draft-l' },
      }),
      ['draft-a', 'draft-b', 'draft-c', 'draft-d', 'draft-e', 'draft-f', 'draft-g', 'draft-h', 'draft-i', 'draft-j', 'draft-k', 'draft-l'],
    );
  });

  it('builds document selection updates only when ownership exists', () => {
    assert.deepEqual(
      documentDatasetSelectionUpdate({
        dataset_id: 'doc-a',
        datasetId: 'doc-b',
        dataset_ids: ['doc-c', 'doc-a'],
      }),
      {
        selectedDatasetId: 'doc-a',
        selectedDatasetIds: ['doc-a', 'doc-b', 'doc-c'],
      },
    );
    assert.equal(documentDatasetSelectionUpdate({ id: 'orphan-doc' }), null);
    assert.equal(documentDatasetSelectionUpdate(null), null);
  });

  it('uses document membership ids before selected dataset fallback ids', () => {
    assert.deepEqual(
      documentMembershipCurrentDatasetIds(
        { dataset_ids: ['doc-a', 'doc-b'] },
        ['fallback-a'],
      ),
      ['doc-a', 'doc-b'],
    );
    assert.deepEqual(
      documentMembershipCurrentDatasetIds(
        { id: 'orphan-doc' },
        [' fallback-a ', 'fallback-b', 'fallback-a'],
      ),
      ['fallback-a', 'fallback-b'],
    );
    assert.deepEqual(documentMembershipCurrentDatasetIds(null, []), []);
  });

  it('extracts document membership response dataset ids with existing fallback priority', () => {
    assert.deepEqual(
      documentMembershipResponseDatasetIds({
        dataset_ids: ['top-a', ' top-b ', 'top-a'],
        datasetIds: ['camel-skip'],
        document: { dataset_ids: ['doc-skip'] },
      }),
      ['top-a', 'top-b'],
    );
    assert.deepEqual(
      documentMembershipResponseDatasetIds({
        datasetIds: ['camel-a'],
        document: { dataset_ids: ['doc-skip'] },
      }),
      ['camel-a'],
    );
    assert.deepEqual(
      documentMembershipResponseDatasetIds({
        document: { dataset_ids: ['doc-a'], datasetIds: ['doc-skip'] },
      }),
      ['doc-a'],
    );
    assert.deepEqual(
      documentMembershipResponseDatasetIds({
        document: { datasetIds: ['doc-camel-a'] },
      }),
      ['doc-camel-a'],
    );
    assert.deepEqual(
      documentMembershipResponseDatasetIds({
        dataset_ids: [],
        datasetIds: ['camel-skip'],
      }),
      [],
    );
  });

  it('filters records by selected dataset ids with default and custom owner resolvers', () => {
    const reports = [
      { id: 'skip', dataset_id: 'other' },
      { id: 'match-direct', dataset_id: 'ds-a' },
      { id: 'match-array', dataset_ids: ['ds-b'] },
    ];
    assert.deepEqual(
      filterRecordsByDatasetIds(reports, ['ds-a', 'ds-b']).map((item) => item.id),
      ['match-direct', 'match-array'],
    );
    assert.deepEqual(filterRecordsByDatasetIds(reports, []), []);

    const drafts = [
      { id: 'draft-skip', matchedDatasetIds: ['other'] },
      { id: 'draft-match', matchedDatasetIds: ['ds-c'] },
    ];
    assert.deepEqual(
      filterRecordsByDatasetIds(drafts, 'ds-c', staticPageDraftDatasetIds).map((item) => item.id),
      ['draft-match'],
    );
  });

  it('sorts datasets by Chinese title fallback and records by descending date', () => {
    assert.deepEqual(
      sortDatasets([
        { key: 'z-key' },
        { title: '北京' },
        { title: '上海' },
      ]).map((item) => item.title || item.key),
      ['北京', '上海', 'z-key'],
    );

    assert.deepEqual(
      sortByDateDesc([
        { id: 'old', updated_at: '2026-01-01T00:00:00Z' },
        { id: 'empty' },
        { id: 'new', updated_at: '2026-02-01T00:00:00Z' },
      ], 'updated_at').map((item) => item.id),
      ['new', 'old', 'empty'],
    );
  });
});
