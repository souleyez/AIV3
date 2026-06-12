import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';

import {
  buildReportRenderHtmlArtifact,
  buildStaticPagePublishedHtmlArtifact,
  firstReportAssetPath,
  isReportRenderHtmlArtifact,
  mergeHtmlArtifacts,
  replaceReportRenderHtmlArtifacts,
  reportAssetKind,
  safeHtmlArtifactIdSegment,
  staticPageRenderedUrlFromDraft,
  staticPageSafePreviewPath,
} from './html-artifact-utils.js';

describe('HTML artifact helpers', () => {
  afterEach(() => {
    delete globalThis.window;
  });

  it('extracts rendered URLs from static page final page compatibility fields', () => {
    assert.equal(staticPageRenderedUrlFromDraft({ finalPage: { publicUrl: '/generated-artifacts/a/index.html' } }), '/generated-artifacts/a/index.html');
    assert.equal(staticPageRenderedUrlFromDraft({ generated_artifact_url: '/generated-artifacts/b/index.html' }), '/generated-artifacts/b/index.html');
    assert.equal(staticPageRenderedUrlFromDraft({ assetManifest: { generatedArtifactUrl: '/generated-artifacts/c/index.html' } }), '/generated-artifacts/c/index.html');
  });

  it('allows only safe preview paths and same-origin absolute URLs when window is available', () => {
    assert.equal(staticPageSafePreviewPath('/generated-artifacts/a/index.html?x=1#top'), '/generated-artifacts/a/index.html?x=1#top');
    assert.equal(staticPageSafePreviewPath('/v1/static-page-render-outputs/out-1/preview'), '/v1/static-page-render-outputs/out-1/preview');
    assert.equal(staticPageSafePreviewPath('/v1/external/channels/ch-1/static-page-renders/render-1/preview?view=mobile'), '/v1/external/channels/ch-1/static-page-renders/render-1/preview?view=mobile');
    assert.equal(staticPageSafePreviewPath('/admin'), '');
    assert.equal(staticPageSafePreviewPath('javascript:alert(1)'), '');

    globalThis.window = { location: { origin: 'https://v3.elepcloud.com' } };
    assert.equal(staticPageSafePreviewPath('https://v3.elepcloud.com/generated-artifacts/a/index.html'), '/generated-artifacts/a/index.html');
    assert.equal(staticPageSafePreviewPath('https://other.example/generated-artifacts/a/index.html'), '');
  });

  it('normalizes safe HTML artifact id segments', () => {
    assert.equal(safeHtmlArtifactIdSegment('  abc/中文 ? id  '), 'abc-id');
    assert.equal(safeHtmlArtifactIdSegment('', 'fallback'), 'fallback');
    assert.equal(safeHtmlArtifactIdSegment('x'.repeat(120)).length, 96);
  });

  it('builds static page published HTML artifacts from rendered drafts only', () => {
    assert.equal(buildStaticPagePublishedHtmlArtifact({ finalPage: { status: 'queued' } }), null);
    assert.equal(buildStaticPagePublishedHtmlArtifact({ finalPage: { status: 'rendered', publicUrl: '/admin' } }), null);

    const artifact = buildStaticPagePublishedHtmlArtifact({
      id: 'local-1',
      backendDraftId: 'backend-1',
      assistantRunId: 'run-1',
      backendUpdatedAt: '2026-06-01T00:00:00Z',
      modelSummary: '已生成。',
      finalPage: {
        status: 'rendered',
        publicUrl: '/generated-artifacts/page/index.html',
        renderOutputId: 'render-1',
        renderer: 'renderer-x',
        assetManifest: {
          reportTitle: '经营月报',
          dataUrl: '/generated-artifacts/page/table-data.csv',
          dataSnapshotUrl: '/generated-artifacts/page/data.json',
        },
      },
    });

    assert.equal(artifact.kind, 'html_artifact');
    assert.equal(artifact.id, 'html-static-page-published-backend-1-render-1');
    assert.equal(artifact.title, '经营月报 · 成品');
    assert.equal(artifact.ownerScope.id, 'backend-1');
    assert.deepEqual(artifact.dataRefs.map((item) => item.kind), ['local_static_page_draft', 'static_page_draft', 'static_page_render_output']);
    assert.equal(artifact.provenance.producer, 'renderer-x');
    assert.equal(artifact.payload.previewPath, '/generated-artifacts/page/index.html');
    assert.equal(artifact.payload.dataPath, '/generated-artifacts/page/table-data.csv');
    assert.equal(artifact.payload.snapshotPath, '/generated-artifacts/page/data.json');
    assert.equal(artifact.payload.summary, '已生成。');
  });

  it('builds report render HTML artifacts with asset kind and warning semantics', () => {
    assert.equal(buildReportRenderHtmlArtifact(null, { id: 'plan-1' }), null);
    assert.equal(buildReportRenderHtmlArtifact({ status: 'rendered' }, { id: 'plan-1' }), null);
    assert.equal(firstReportAssetPath({ path: ' report.html ' }), 'report.html');
    assert.equal(firstReportAssetPath({ assets: [{ path: '' }, { path: 'nested.pdf' }] }), 'nested.pdf');
    assert.equal(reportAssetKind({ path: 'report.html' }), 'html');
    assert.equal(reportAssetKind({ path: 'data.unknown' }), 'asset');

    const rendered = buildReportRenderHtmlArtifact({
      id: 'output-1',
      plan_id: 'plan-1',
      execution_id: 'exec-1',
      status: 'rendered',
      surface: 'mobile',
      created_at: '2026-06-02T00:00:00Z',
      asset_manifest: { path: 'report.html' },
    }, { id: 'fallback-plan', title: '经营报表', objective: '看经营' });
    assert.equal(rendered.id, 'html-report-render-output-1');
    assert.equal(rendered.payload.publishable, true);
    assert.equal(rendered.payload.assetKind, 'html');
    assert.equal(rendered.payload.reportPlanId, 'plan-1');
    assert.deepEqual(rendered.payload.warnings, []);

    const failed = buildReportRenderHtmlArtifact({
      id: 'output-2',
      status: 'failed',
      asset_manifest: {},
    }, { id: 'plan-2' });
    assert.equal(failed.payload.publishable, false);
    assert.equal(failed.payload.warnings[0].title, '渲染失败');
  });

  it('merges and replaces HTML artifacts by id while sorting newest first', () => {
    const reportOld = { id: 'report-old', templateId: 'report_render_summary', createdAt: '2026-01-01T00:00:00Z' };
    const staticPage = { id: 'static-1', templateId: 'static_page_published_preview', createdAt: '2026-03-01T00:00:00Z' };
    const duplicate = { id: 'static-1', templateId: 'ignored', createdAt: '2026-04-01T00:00:00Z' };
    const reportNew = { id: 'report-new', templateId: 'report_render_summary', createdAt: '2026-02-01T00:00:00Z' };

    assert.equal(isReportRenderHtmlArtifact(reportOld), true);
    assert.deepEqual(mergeHtmlArtifacts([reportOld, staticPage], [duplicate]).map((item) => item.id), ['static-1', 'report-old']);
    assert.deepEqual(
      replaceReportRenderHtmlArtifacts([reportOld, staticPage], [reportNew]).map((item) => item.id),
      ['static-1', 'report-new'],
    );
  });
});
