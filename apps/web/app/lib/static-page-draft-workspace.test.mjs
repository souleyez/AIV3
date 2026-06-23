import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  findReusableReportTemplate,
  isReusableStaticPageReportDraft,
  isVisibleReportShelfStaticPageDraft,
  replaceStaticPageDraftInMap,
  shouldAnnounceStaticPageRendered,
  sortStaticPageDrafts,
  staticPageDraftAsyncSnapshot,
  staticPageDraftDiscoveryId,
  staticPageDraftIsArchived,
  staticPageDraftIsDataReportArtifact,
  staticPageDraftReadyForAutoRender,
} from './static-page-draft-workspace.js';

describe('static page draft workspace helpers', () => {
  it('normalizes async snapshot fields and rendered URLs from compatibility fields', () => {
    const snapshot = staticPageDraftAsyncSnapshot({
      status: 'preview_ready',
      imageJob: { id: 'job-1', status: 'RUNNING' },
      previewContract: { assetKey: ' preview.png ' },
      finalPage: {
        status: 'rendered',
        render_output_id: 'render-1',
        html_preview_url: '/generated-artifacts/pages/demo/index.html',
      },
    });

    assert.equal(snapshot.jobStatus, 'running');
    assert.equal(snapshot.draftStatus, 'preview_ready');
    assert.equal(snapshot.finalStatus, 'rendered');
    assert.equal(snapshot.previewAssetKey, 'preview.png');
    assert.equal(snapshot.jobId, 'job-1');
    assert.equal(snapshot.renderOutputId, 'render-1');
    assert.equal(snapshot.finalUrl, '/generated-artifacts/pages/demo/index.html');
    assert.equal(snapshot.previewReady, true);
    assert.equal(snapshot.renderInProgress, false);
    assert.equal(snapshot.rendered, true);
  });

  it('detects auto-render readiness only for backend drafts with a ready preview asset', () => {
    assert.equal(staticPageDraftReadyForAutoRender({
      backendDraftId: 'draft-1',
      imageJob: { status: 'preview_ready' },
      previewImage: { assetKey: 'preview.png' },
      finalPage: { status: 'queued' },
    }), false);

    assert.equal(staticPageDraftReadyForAutoRender({
      backendDraftId: 'draft-1',
      previewContract: { status: 'preview_ready', assetKey: 'preview.png' },
      finalPage: { status: 'not_requested' },
    }), true);

    assert.equal(staticPageDraftReadyForAutoRender({
      previewContract: { status: 'preview_ready', assetKey: 'preview.png' },
    }), false);
  });

  it('excludes archived backend drafts even when their payload still has rendered output', () => {
    const archived = {
      id: 'archived-draft',
      backendStatus: 'archived',
      status: 'rendered',
      previewContract: { status: 'preview_ready', assetKey: 'preview.png' },
      finalPage: {
        status: 'rendered',
        htmlPreviewUrl: '/generated-artifacts/archived/index.html',
      },
    };

    assert.equal(staticPageDraftIsArchived(archived), true);
    assert.equal(shouldAnnounceStaticPageRendered(archived), false);
    assert.equal(isVisibleReportShelfStaticPageDraft(archived), false);
    assert.equal(isReusableStaticPageReportDraft(archived), false);
    assert.equal(staticPageDraftReadyForAutoRender(archived), false);
  });

  it('keeps data-report artifacts visible but suppresses rendered chat announcements and reuse', () => {
    const draft = {
      dataset_artifact_key: 'template:data-report|dataset:ds-1',
      status: 'rendered',
      finalPage: {
        status: 'rendered',
        htmlPreviewUrl: '/generated-artifacts/data-report/index.html',
      },
    };

    assert.equal(staticPageDraftIsDataReportArtifact(draft), true);
    assert.equal(shouldAnnounceStaticPageRendered(draft), false);
    assert.equal(isVisibleReportShelfStaticPageDraft(draft), true);
    assert.equal(isReusableStaticPageReportDraft(draft), false);
  });

  it('detects reusable rendered report templates while excluding stale and retired drafts', () => {
    const reusable = {
      datasetArtifactKey: 'template:dashboard|dataset:ds-1',
      status: 'rendered',
      finalPage: {
        status: 'rendered',
        htmlDownloadUrl: '/generated-artifacts/dashboard/index.html',
      },
    };
    assert.equal(isReusableStaticPageReportDraft(reusable), true);

    assert.equal(isReusableStaticPageReportDraft({
      ...reusable,
      imageJob: { status: 'stale' },
    }), false);
    assert.equal(isReusableStaticPageReportDraft({
      ...reusable,
      finalPage: { ...reusable.finalPage, baselineStatus: 'retired' },
    }), false);
  });

  it('finds reusable report templates in the existing priority order', () => {
    const plan = { id: 'plan-1', currentAstVersionId: 'ast-1' };
    const published = { report_plan_id: 'plan-1', html_url: '/report.html' };
    assert.deepEqual(findReusableReportTemplate([plan], [published], []), { plan, published });

    const standalonePublished = { id: 'pub-standalone', html_url: '/standalone.html' };
    assert.deepEqual(findReusableReportTemplate([], [standalonePublished], []), {
      plan: null,
      published: standalonePublished,
    });

    const draft = {
      datasetArtifactKey: 'template:generated-static-page|dataset:ds-1',
      status: 'rendered',
      finalPage: { status: 'rendered', htmlPreviewUrl: '/page.html' },
    };
    assert.deepEqual(findReusableReportTemplate([], [], [draft]), {
      plan: null,
      published: null,
      draft,
    });

    const planned = { id: 'plan-2', status: 'planned' };
    assert.deepEqual(findReusableReportTemplate([planned], [], []), {
      plan: planned,
      published: null,
    });
  });

  it('sorts drafts by freshest backend timestamp and reads stable discovery ids', () => {
    const sorted = sortStaticPageDrafts([
      { id: 'old', updatedAt: '2026-01-01T00:00:00.000Z' },
      { id: 'new', backendUpdatedAt: '2026-03-01T00:00:00.000Z' },
      { id: 'middle', updated_at: '2026-02-01T00:00:00.000Z' },
    ]);

    assert.deepEqual(sorted.map((item) => item.id), ['new', 'middle', 'old']);
    assert.equal(staticPageDraftDiscoveryId({ source: { local_draft_id: ' local-1 ' }, id: 'fallback' }), 'local-1');
    assert.equal(staticPageDraftDiscoveryId({ source_refs: { local_draft_id: 'source-ref-1' } }), 'source-ref-1');
    assert.equal(staticPageDraftDiscoveryId({ id: 'id-1', backendDraftId: 'backend-1' }), 'id-1');
  });

  it('replaces static page drafts in a map without mutating the previous state', () => {
    const previousDraft = { id: 'local-old', title: 'Old' };
    const retainedDraft = { id: 'retained', title: 'Keep' };
    const current = {
      'local-old': previousDraft,
      retained: retainedDraft,
    };
    const replacement = { id: 'backend-new', backendDraftId: 'backend-new', title: 'New' };

    const next = replaceStaticPageDraftInMap(current, 'local-old', replacement);

    assert.deepEqual(Object.keys(next).sort(), ['backend-new', 'retained']);
    assert.equal(next['backend-new'], replacement);
    assert.equal(next.retained, retainedDraft);
    assert.deepEqual(Object.keys(current).sort(), ['local-old', 'retained']);
  });

  it('upserts a static page draft without deleting old ids when no previous id is supplied', () => {
    const current = { 'local-old': { id: 'local-old' } };
    const replacement = { id: 'backend-new' };
    const next = replaceStaticPageDraftInMap(current, null, replacement);

    assert.deepEqual(Object.keys(next).sort(), ['backend-new', 'local-old']);
    assert.equal(next['backend-new'], replacement);
  });

  it('keeps a copied draft map when the replacement draft is invalid', () => {
    const current = { existing: { id: 'existing' } };
    const next = replaceStaticPageDraftInMap(current, 'existing', null);

    assert.deepEqual(next, current);
    assert.notEqual(next, current);
  });
});
