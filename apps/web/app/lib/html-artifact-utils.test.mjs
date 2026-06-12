import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';

import {
  buildReportRenderHtmlArtifact,
  buildStaticPagePlanningHtmlArtifact,
  buildStaticPagePublishedHtmlArtifact,
  compactStaticPageMissingEvidence,
  compactStaticPageStructureSignals,
  compactStaticPageTemplateReference,
  findPublishedStaticPageArtifactForDraft,
  findStaticPageDraftByAnyId,
  firstReportAssetPath,
  htmlArtifactOwnerScope,
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

  it('finds static page drafts by local or backend id', () => {
    const localDraft = { id: 'local-1', backendDraftId: 'backend-1' };
    const mappedDraft = { id: 'mapped-local', backendDraftId: 'mapped-backend' };

    assert.equal(
      findStaticPageDraftByAnyId('backend-1', { 'mapped-local': mappedDraft }, [localDraft]),
      localDraft,
    );
    assert.equal(
      findStaticPageDraftByAnyId('mapped-local', { 'mapped-local': mappedDraft }, [localDraft]),
      mappedDraft,
    );
    assert.equal(findStaticPageDraftByAnyId('', {}, [localDraft]), null);
    assert.equal(findStaticPageDraftByAnyId('missing', {}, null), null);
  });

  it('finds published static page artifacts by owner scope and template id', () => {
    const draft = { id: 'local-1', backendDraftId: 'backend-1' };
    const wrongTemplate = {
      id: 'artifact-wrong-template',
      templateId: 'static_page_planning_handoff',
      ownerScope: { type: 'static_page_draft', id: 'backend-1' },
    };
    const wrongOwner = {
      id: 'artifact-wrong-owner',
      templateId: 'static_page_published_preview',
      ownerScope: { type: 'report_render_output', id: 'backend-1' },
    };
    const matchedBySnakeCase = {
      id: 'artifact-match',
      template_id: 'static_page_published_preview',
      owner_scope: { type: 'static_page_draft', id: 'backend-1' },
    };

    assert.deepEqual(htmlArtifactOwnerScope(matchedBySnakeCase), { type: 'static_page_draft', id: 'backend-1' });
    assert.equal(
      findPublishedStaticPageArtifactForDraft(draft, [wrongTemplate, wrongOwner, matchedBySnakeCase]),
      matchedBySnakeCase,
    );
    assert.equal(findPublishedStaticPageArtifactForDraft(null, [matchedBySnakeCase]), null);
    assert.equal(findPublishedStaticPageArtifactForDraft(draft, null), null);
  });

  it('compacts static page planning references, missing evidence, and structure signals', () => {
    const draft = {
      designReferences: [{
        source: 'html-anything',
        template_id: 'data-report',
        name: '数据报表',
        import_policy: 'style_only',
        style_direction: 'data-command',
        design_intent: '经营分析',
        prompt_hints: ['a', 'b', 'c', 'd', 'e', 'f', 'g'],
        provider_policy: {
          forbidden_output: ['x', 'y', 'z', '1', '2', '3', '4', '5', '6'],
        },
      }],
      source: {
        missing_evidence: {
          status: 'partial',
          items: [
            { code: 'area_missing', message: '缺少面积', recommended_action: '上传合同', detail_target_count: 3 },
          ],
        },
      },
      dataSnapshot: {
        structure_signals: {
          section_title_hints: [' 概览 ', '风险'],
          field_candidates: [{
            source_id: 'evidence',
            field_path: 'stores.risk',
            label: '风险门店',
            kind: 'dimension',
            section_title_hints: ['风险', '明细'],
          }],
          bound_modules: [{
            module_id: 'risk',
            title: '风险提示',
            field_path: 'stores.risk',
            binding_quality_status: 'confirmed',
          }],
        },
      },
    };

    const templateReference = compactStaticPageTemplateReference(draft);
    assert.equal(templateReference.templateId, 'data-report');
    assert.equal(templateReference.label, '数据报表');
    assert.equal(templateReference.importPolicy, 'style_only');
    assert.equal(templateReference.promptHints.length, 6);
    assert.equal(templateReference.forbiddenOutput.length, 8);

    const missingEvidence = compactStaticPageMissingEvidence(draft);
    assert.equal(missingEvidence.status, 'partial');
    assert.deepEqual(missingEvidence.items[0], {
      code: 'area_missing',
      message: '缺少面积',
      recommendedAction: '上传合同',
      detailTargetCount: 3,
    });

    const structureSignals = compactStaticPageStructureSignals(draft);
    assert.equal(structureSignals.status, 'available');
    assert.equal(structureSignals.policy, 'source_structure_only_no_body_no_sample_rows');
    assert.deepEqual(structureSignals.sectionTitleHints, ['概览', '风险', '明细']);
    assert.equal(structureSignals.fieldCandidates[0].fieldPath, 'stores.risk');
    assert.equal(structureSignals.boundModules[0].bindingQualityStatus, 'confirmed');
  });

  it('builds static page planning handoff artifacts without exposing raw rows', () => {
    const artifact = buildStaticPagePlanningHtmlArtifact({
      id: 'local-1',
      backendDraftId: 'backend-1',
      datasetId: 'dataset-1',
      sessionId: 'session-1',
      assistantRunId: 'run-1',
      objective: '新百经营分析',
      backendUpdatedAt: '2026-06-03T00:00:00Z',
      templateEvidenceSummary: '模板参考已收到',
      templateReference: { id: 'dashboard', label: '看板模板' },
      previewContract: {
        status: 'preview_ready',
        assetKey: 'preview.png',
        draftFingerprint: 'design-1234',
      },
      imageJob: {
        id: 'job-1',
        status: 'preview_ready',
      },
      renderSpec: {
        componentModel: 'dom-text-svg-chart',
      },
      finalPage: {
        status: 'not_requested',
      },
      modules: [{
        id: 'hero',
        title: '核心判断',
        content: '展示重点',
        dataBinding: { label: '汇总指标' },
        visualizationType: 'headline',
        layout: { x: 0, y: 0, w: 12, h: 2 },
        dataQualityStatus: 'confirmed',
      }],
    });

    assert.equal(artifact.id, 'html-static-page-handoff-backend-1');
    assert.equal(artifact.title, '新百经营分析 · 交接');
    assert.equal(artifact.ownerScope.id, 'backend-1');
    assert.deepEqual(artifact.dataRefs.map((item) => item.kind), ['dataset', 'chat_session', 'static_page_draft']);
    assert.equal(artifact.provenance.sourceRunId, 'run-1');
    assert.equal(artifact.createdAt, '2026-06-03T00:00:00Z');
    assert.equal(artifact.payload.visualBridge.status, 'preview_ready');
    assert.equal(artifact.payload.visualBridge.providerLane, 'gpt-image-2-cloudflare-queue');
    assert.equal(artifact.payload.modules[0].dataBinding, '汇总指标');
    assert.equal(JSON.stringify(artifact).includes('rawRows'), false);
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
