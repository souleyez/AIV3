import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  normalizeReportShelfDefaultValue,
  reportShelfDefaultTargetDatasetIds,
  staticPageDraftArtifactKey,
  staticPageDraftBaselineStatus,
  staticPageDraftHasAnyReportShelfDefault,
  staticPageDraftIsDefaultForDatasetIds,
  staticPageDraftReportShelfDefaults,
  withStaticPageDraftReportShelfDefaults,
} from './static-page-report-shelf.js';

describe('static page report shelf helpers', () => {
  it('reads artifact key and baseline status from supported draft fields', () => {
    assert.equal(
      staticPageDraftArtifactKey({
        source_refs: { artifact_stability: { dataset_artifact_key: ' template:dashboard|x ' } },
        datasetArtifactKey: 'fallback',
      }),
      'template:dashboard|x',
    );
    assert.equal(staticPageDraftArtifactKey({ datasetArtifactKey: 'plain-key' }), 'plain-key');
    assert.equal(
      staticPageDraftBaselineStatus({
        draftPayload: { artifactStability: { baselineStatus: ' ACCEPTED ' } },
      }),
      'accepted',
    );
    assert.equal(staticPageDraftBaselineStatus({ finalPage: { baseline_status: 'Retired' } }), 'retired');
  });

  it('normalizes accepted and rejected report shelf default values', () => {
    assert.equal(normalizeReportShelfDefaultValue(true), true);
    assert.equal(normalizeReportShelfDefaultValue(false), false);
    assert.equal(normalizeReportShelfDefaultValue(' accepted '), true);
    assert.equal(normalizeReportShelfDefaultValue('enabled'), true);
    assert.equal(normalizeReportShelfDefaultValue('not_default'), false);
    assert.equal(normalizeReportShelfDefaultValue('disabled'), false);
    assert.equal(normalizeReportShelfDefaultValue('unknown'), null);
    assert.equal(normalizeReportShelfDefaultValue(1), null);
  });

  it('merges report shelf defaults from every compatibility field in existing order', () => {
    const draft = {
      source_refs: {
        artifact_stability: {
          report_shelf_defaults: { ' ds-a ': 'accepted', 'ds-b': 'disabled' },
        },
      },
      artifactStability: {
        reportShelfDefaults: { 'ds-b': 'enabled', 'ds-c': 'unknown' },
      },
      report_shelf_defaults: {
        'ds-a': 'false',
        'ds-d': true,
      },
    };

    assert.deepEqual(staticPageDraftReportShelfDefaults(draft), {
      'ds-a': false,
      'ds-b': true,
      'ds-d': true,
    });
  });

  it('resolves default eligibility from explicit overrides, related datasets, and retired status', () => {
    const draft = {
      matchedDatasetIds: ['ds-a', 'ds-b'],
      reportShelfDefaults: { 'ds-a': false, 'ds-c': true },
    };
    assert.equal(staticPageDraftIsDefaultForDatasetIds(draft, 'ds-a'), false);
    assert.equal(staticPageDraftIsDefaultForDatasetIds(draft, 'ds-b'), true);
    assert.equal(staticPageDraftIsDefaultForDatasetIds(draft, 'ds-c'), true);
    assert.equal(staticPageDraftIsDefaultForDatasetIds(draft, 'ds-x'), false);

    const retiredDraft = {
      matchedDatasetIds: ['ds-a'],
      finalPage: { baselineStatus: 'retired' },
    };
    assert.equal(staticPageDraftIsDefaultForDatasetIds(retiredDraft, 'ds-a'), false);
  });

  it('detects any usable report shelf default while respecting retired and disabled drafts', () => {
    assert.equal(staticPageDraftHasAnyReportShelfDefault(null), false);
    assert.equal(staticPageDraftHasAnyReportShelfDefault({ reportShelfDefaults: { 'ds-a': true } }), true);
    assert.equal(staticPageDraftHasAnyReportShelfDefault({ matchedDatasetIds: ['ds-a'] }), true);
    assert.equal(staticPageDraftHasAnyReportShelfDefault({
      matchedDatasetIds: ['ds-a'],
      reportShelfDefaults: { 'ds-a': false },
    }), false);
    assert.equal(staticPageDraftHasAnyReportShelfDefault({
      matchedDatasetIds: ['ds-a'],
      finalPage: { baselineStatus: 'retired' },
    }), false);
  });

  it('uses selected dataset ids for shelf actions and falls back to draft datasets', () => {
    const draft = { matchedDatasetIds: ['ds-a', 'ds-b'] };
    assert.deepEqual(reportShelfDefaultTargetDatasetIds(draft, [' ds-c ', 'ds-c']), ['ds-c']);
    assert.deepEqual(reportShelfDefaultTargetDatasetIds(draft, []), ['ds-a', 'ds-b']);
  });

  it('writes report shelf defaults without mutating the original draft', () => {
    const draft = {
      id: 'draft-1',
      source_refs: {
        artifact_stability: {
          report_shelf_defaults: { 'ds-a': false },
        },
      },
    };
    const next = withStaticPageDraftReportShelfDefaults(draft, ['ds-a', 'ds-b'], true);
    assert.notEqual(next, draft);
    assert.deepEqual(draft.source_refs.artifact_stability.report_shelf_defaults, { 'ds-a': false });
    assert.deepEqual(next.reportShelfDefaults, { 'ds-a': true, 'ds-b': true });
    assert.deepEqual(next.report_shelf_defaults, { 'ds-a': true, 'ds-b': true });
    assert.equal(next.source_refs.artifact_stability.baseline_status, 'accepted');
    assert.equal(next.sourceRefs.artifactStability.baselineStatus, 'accepted');
    assert.match(next.source_refs.artifact_stability.report_shelf_defaults_updated_at, /^\d{4}-\d{2}-\d{2}T/);

    assert.equal(withStaticPageDraftReportShelfDefaults(draft, [], true), draft);
    assert.equal(withStaticPageDraftReportShelfDefaults(null, ['ds-a'], true), null);
  });
});
