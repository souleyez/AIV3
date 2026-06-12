import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  firstArtifactUrlFromObject,
  publishedReportForPlan,
  reportPlanIdFromPublished,
  reportTemplateCandidateId,
  reportTemplateTitle,
  reportTemplateUrl,
} from './report-template-utils.js';

describe('report template helpers', () => {
  it('matches published reports by all supported plan id fields', () => {
    assert.equal(reportPlanIdFromPublished({ report_plan_id: ' plan-a ' }), 'plan-a');
    assert.equal(reportPlanIdFromPublished({ reportPlanId: 'plan-b' }), 'plan-b');
    assert.equal(reportPlanIdFromPublished({ plan_id: 'plan-c' }), 'plan-c');
    assert.equal(reportPlanIdFromPublished({ planId: 'plan-d' }), 'plan-d');

    const match = publishedReportForPlan({ id: 'plan-b' }, [
      { id: 'skip', report_plan_id: 'plan-a' },
      { id: 'hit', reportPlanId: 'plan-b' },
    ]);
    assert.equal(match?.id, 'hit');
    assert.equal(publishedReportForPlan({}, [{ reportPlanId: 'plan-b' }]), null);
  });

  it('finds the first artifact URL across nested report containers', () => {
    assert.equal(
      firstArtifactUrlFromObject({
        current_version: {
          asset_manifest: {
            generated_artifact_url: '/generated-artifacts/report/index.html',
          },
        },
      }),
      '/generated-artifacts/report/index.html',
    );
    assert.equal(
      firstArtifactUrlFromObject({
        report: {
          links: [
            { href: '' },
            { url: 'https://v3.elepcloud.com/generated-artifacts/report/index.html' },
          ],
        },
      }),
      'https://v3.elepcloud.com/generated-artifacts/report/index.html',
    );
    assert.equal(
      firstArtifactUrlFromObject({
        artifactLinks: ['', '/generated-artifacts/from-links/index.html'],
      }),
      '/generated-artifacts/from-links/index.html',
    );
    assert.equal(firstArtifactUrlFromObject({ url: 'mailto:test@example.com' }), '');
  });

  it('keeps existing report template title priority', () => {
    assert.equal(
      reportTemplateTitle({
        detail: { current_version: { asset_manifest: { report_title: 'detail title' } } },
        published: { title: 'published title' },
        plan: { title: 'plan title' },
      }),
      'detail title',
    );
    assert.equal(
      reportTemplateTitle({
        plan: { objective: 'plan objective' },
        draft: { finalPage: { assetManifest: { reportTitle: 'draft manifest title' } } },
      }),
      'plan objective',
    );
    assert.equal(
      reportTemplateTitle({
        draft: { finalPage: { assetManifest: { displayTitle: 'draft display title' } } },
      }),
      'draft display title',
    );
    assert.equal(reportTemplateTitle({}, 'fallback title'), 'fallback title');
  });

  it('keeps existing candidate id fallback order', () => {
    assert.equal(reportTemplateCandidateId({ plan: { id: 'plan-id' } }), 'plan-id');
    assert.equal(reportTemplateCandidateId({ published: { report_id: 'report-id' } }), 'report-id');
    assert.equal(reportTemplateCandidateId({ draft: { backendDraftId: 'draft-id' } }), 'draft-id');
    assert.equal(reportTemplateCandidateId({ plan: { title: 'title fallback' } }), 'title fallback');
  });

  it('chooses template URL from detail, published, plan, then draft', () => {
    assert.equal(
      reportTemplateUrl({
        detail: { publicUrl: '/generated-artifacts/detail/index.html' },
        published: { publicUrl: '/generated-artifacts/published/index.html' },
        plan: { publicUrl: '/generated-artifacts/plan/index.html' },
        draft: { finalPage: { publicUrl: '/generated-artifacts/draft/index.html' } },
      }),
      '/generated-artifacts/detail/index.html',
    );
    assert.equal(
      reportTemplateUrl({
        published: { publicUrl: '/generated-artifacts/published/index.html' },
        draft: { finalPage: { publicUrl: '/generated-artifacts/draft/index.html' } },
      }),
      '/generated-artifacts/published/index.html',
    );
    assert.equal(
      reportTemplateUrl({
        staticPageDraft: { finalPage: { publicUrl: '/generated-artifacts/draft/index.html' } },
      }),
      '/generated-artifacts/draft/index.html',
    );
  });
});
