import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  artifactTaskStatusLabel,
  buildArtifactTaskCards,
  normalizeArtifactTaskStatus,
} from './artifact-task-cards.js';
import { buildFashionDesignAssetImportShelfTask } from './asset-library-view-model.js';

describe('artifact task cards', () => {
  it('normalizes running drafts, published HTML artifacts, running Codex tasks, and finished Codex artifacts', () => {
    const cards = buildArtifactTaskCards({
      staticPageDrafts: [{
        id: 'local-draft-1',
        backendDraftId: 'draft-backend-1',
        title: '门店取高报表',
        status: 'rendering',
        backendUpdatedAt: '2026-06-17T08:00:00Z',
        modelSummary: '正在生成静态页。',
        finalPage: {
          status: 'queued',
          renderOutputId: 'render-1',
          assetManifest: {
            reportTitle: '门店取高报表',
            workflow: { executionId: '11111111-1111-4111-8111-111111111111' },
          },
        },
      }],
      htmlArtifacts: [{
        kind: 'html_artifact',
        version: 1,
        id: 'html-artifact-1',
        title: '门店取高报表 · 成品',
        sourceType: 'static_page',
        templateId: 'static_page_published_preview',
        interactionMode: 'read_only',
        ownerScope: { type: 'static_page_draft', id: 'draft-backend-1' },
        dataRefs: [{ kind: 'static_page_render_output', id: 'render-1', label: 'Render Output' }],
        provenance: { producer: 'renderer', reason: 'published static page preview' },
        createdAt: '2026-06-17T08:02:00Z',
        payload: {
          status: 'rendered',
          previewPath: '/generated-artifacts/xinbai/index.html',
          renderOutputId: 'render-1',
          summary: '页面已生成。',
        },
      }, {
        kind: 'html_artifact',
        version: 1,
        id: 'html-static-page-handoff-draft-1',
        title: '门店取高报表 · 交接',
        sourceType: 'static_page',
        templateId: 'static_page_planning_handoff',
        payload: { status: 'planned' },
      }, {
        kind: 'html_artifact',
        version: 1,
        id: 'html-static-page-quality-draft-1',
        title: '门店取高报表 · 数据质量报告',
        sourceType: 'static_page',
        templateId: 'static_page_data_quality_report',
        payload: { status: 'failed' },
      }, {
        kind: 'html_artifact',
        version: 1,
        id: 'html-artifact-codex-exec-1',
        title: 'Codex Host 执行报告',
        sourceType: 'codex_host',
        templateId: 'codex_execution_report',
        payload: { status: 'completed' },
      }],
      codexCustomerTasks: [{
        id: 'task-1',
        title: 'Codex 页面发布',
        status: 'running',
        statusLabel: '执行中',
        route: 'generated_static_page_publish',
        workflowExecutionId: '22222222-2222-4222-8222-222222222222',
        createdAt: '2026-06-17T08:01:00Z',
      }, {
        id: 'task-v3-change',
        title: '需人工审核',
        status: 'blocked',
        capability: 'v3_product_change_request',
        route: 'v3_product_change_request',
        createdAt: '2026-06-17T08:01:30Z',
      }],
      codexCustomerArtifacts: [{
        id: 'artifact-bundle-1',
        title: 'Codex 发布产物',
        summary: '2 个文件',
        workflowExecutionId: '22222222-2222-4222-8222-222222222222',
        primaryUrl: '/generated-artifacts/codex/index.html',
        published: true,
        createdAt: '2026-06-17T08:03:00Z',
        files: [
          { path: 'index.html', title: 'index.html', kind: 'html', publicUrl: '/generated-artifacts/codex/index.html' },
          { path: 'report.md', title: 'report.md', kind: 'report_markdown', publicUrl: '/generated-artifacts/codex/report.md' },
        ],
      }],
      clientArtifacts: [{
        artifact_id: 'v3ca_1',
        title: '客户端上传静态页',
        status: 'published',
        task_id: 'client-task-1',
        dataset_ids: ['dataset-1'],
        asset_library_ids: ['asset-1'],
        files: [
          {
            file_index: 0,
            filename: 'index.html',
            content_type: 'text/html',
            role: 'primary_html',
            size_bytes: 1200,
            sha256: 'abc',
            download_url: '/v1/client-artifacts/v3ca_1/files/0',
            preview_url: '/v1/client-artifacts/v3ca_1/files/0/preview',
            public_url: '/generated-artifacts/client-artifacts/v3ca_1/html-0/index.html',
          },
          {
            file_index: 1,
            filename: 'report.md',
            content_type: 'text/markdown',
            role: 'source_summary',
            size_bytes: 120,
            sha256: 'def',
            download_url: '/v1/client-artifacts/v3ca_1/files/1',
          },
        ],
        manifest: {
          title: '客户端上传静态页',
          task_id: 'client-task-1',
        },
        created_at: '2026-06-17T08:04:00Z',
      }],
    });

    assert.equal(cards.length, 3);
    assert.deepEqual(cards.map((card) => card.kind), ['v3_client_artifact', 'codex_artifact', 'html_artifact']);
    assert.equal(cards.some((card) => /交接|数据质量|执行报告|人工审核/.test(card.title)), false);

    const staticPageCard = cards.find((card) => card.sourceRefs.some((ref) => ref.id === 'draft-backend-1'));
    assert.ok(staticPageCard);
    assert.equal(staticPageCard.status, 'published');
    assert.equal(staticPageCard.statusLabel, '已发布');
    assert.equal(staticPageCard.raw.staticPageDraft.backendDraftId, 'draft-backend-1');
    assert.equal(staticPageCard.raw.htmlArtifact.id, 'html-artifact-1');
    assert.ok(staticPageCard.files.some((file) => file.label === 'index.html'));
    assert.ok(staticPageCard.files.some((file) => file.label === 'report.ppt'));
    assert.ok(staticPageCard.files.some((file) => file.label === 'report.md'));
    assert.ok(staticPageCard.files.some((file) => file.label === 'table-data.csv'));
    assert.ok(staticPageCard.primaryFileId);
    assert.equal(staticPageCard.canOpen, true);
    assert.equal(staticPageCard.canEdit, true);

    const codexCard = cards.find((card) => card.sourceRefs.some((ref) => ref.id === '22222222-2222-4222-8222-222222222222'));
    assert.ok(codexCard);
    assert.equal(codexCard.status, 'published');
    assert.equal(codexCard.raw.codexCustomerTask.id, 'task-1');
    assert.equal(codexCard.raw.codexCustomerArtifact.id, 'artifact-bundle-1');
    assert.ok(codexCard.files.some((file) => file.label === '打开产物'));
    assert.equal(
      codexCard.files.filter((file) => file.url === '/generated-artifacts/codex/index.html').length,
      1,
    );

    const clientArtifactCard = cards.find((card) => card.raw.clientArtifact?.artifact_id === 'v3ca_1');
    assert.ok(clientArtifactCard);
    assert.equal(clientArtifactCard.status, 'published');
    assert.equal(clientArtifactCard.phase, 'V3 已发布');
    assert.ok(clientArtifactCard.detail.stages.some((stage) => stage.key === 'publish' && stage.status === 'completed'));
    assert.equal(clientArtifactCard.canOpen, true);
    assert.ok(clientArtifactCard.files.some((file) => file.label === 'index.html'));
    assert.ok(clientArtifactCard.files.some((file) => file.url === '/generated-artifacts/client-artifacts/v3ca_1/html-0/index.html'));
    assert.ok(clientArtifactCard.files.some((file) => file.kind === 'client_html' && file.canOpen));
    assert.equal(
      clientArtifactCard.primaryFileId,
      clientArtifactCard.files.find((file) => file.kind === 'client_html')?.id,
    );
    assert.ok(clientArtifactCard.sourceRefs.some((ref) => ref.kind === 'dataset' && ref.id === 'dataset-1'));
  });

  it('covers canonical task statuses and common provider aliases', () => {
    assert.equal(normalizeArtifactTaskStatus('queued'), 'queued');
    assert.equal(normalizeArtifactTaskStatus('rendering'), 'running');
    assert.equal(normalizeArtifactTaskStatus('poll_retry'), 'retrying');
    assert.equal(normalizeArtifactTaskStatus('blocked'), 'needs_review');
    assert.equal(normalizeArtifactTaskStatus('published'), 'published');
    assert.equal(normalizeArtifactTaskStatus('rendered'), 'completed');
    assert.equal(normalizeArtifactTaskStatus('error'), 'failed');
    assert.equal(normalizeArtifactTaskStatus('canceled'), 'cancelled');

    assert.equal(artifactTaskStatusLabel('retrying'), '重试中');
  });

  it('keeps report plans and published reports selectable without duplicating plan-owned reports', () => {
    const cards = buildArtifactTaskCards({
      reportPlans: [{
        id: 'plan-1',
        objective: '生成经营分析报表',
        status: 'draft',
        created_at: '2026-06-17T08:00:00Z',
      }],
      publishedReports: [{
        id: 'published-1',
        report_plan_id: 'plan-1',
        title: '经营分析报表',
        public_url: '/generated-artifacts/report/index.html',
        created_at: '2026-06-17T08:10:00Z',
      }, {
        id: 'published-2',
        title: '独立报表',
        public_url: '/generated-artifacts/other/index.html',
      }],
    });

    assert.equal(cards.length, 2);
    const planCard = cards.find((card) => card.raw.reportPlan?.id === 'plan-1');
    assert.ok(planCard);
    assert.equal(planCard.status, 'published');
    assert.equal(planCard.raw.publishedReport.id, 'published-1');

    const publishedOnlyCard = cards.find((card) => card.raw.publishedReport?.id === 'published-2');
    assert.ok(publishedOnlyCard);
    assert.equal(publishedOnlyCard.kind, 'published_report');
  });

  it('does not surface archived static page drafts even if the payload still has a rendered URL', () => {
    const cards = buildArtifactTaskCards({
      staticPageDrafts: [{
        id: 'archived-static-page',
        backendStatus: 'archived',
        status: 'rendered',
        title: '静态页：历史归档报表',
        finalPage: {
          status: 'rendered',
          htmlPreviewUrl: '/generated-artifacts/archived/index.html',
        },
      }],
    });

    assert.deepEqual(cards, []);
  });

  it('derives concise business names for artifact task cards', () => {
    const cards = buildArtifactTaskCards({
      staticPageDrafts: [{
        id: 'draft-fixed-commission',
        title: '静态页：生成固定提成取高报表',
        status: 'rendered',
        finalPage: {
          status: 'rendered',
          htmlPreviewUrl: '/generated-artifacts/fixed-commission/index.html',
          assetManifest: {
            title: '门店取高报表 · 成品',
          },
        },
      }, {
        id: 'draft-health',
        title: '静态页：经营健康度总览 识别大区、小区、分店的整体经营表现，包括收入、面积、租金等',
        status: 'rendering',
      }, {
        id: 'draft-generic',
        title: '随便生成一个报表我看看 [已引用模板：资料1.docx]',
        objective: '快速生成一页数据可视化报告，展示关键指标、趋势、结构和可核查证据。',
        prompt: '固定提成取高风险识别，可以从哪些品牌已触发、哪些即将触发、哪些差距较大三个层级来做。',
        status: 'queued',
      }],
      htmlArtifacts: [{
        kind: 'html_artifact',
        version: 1,
        id: 'html-fixed-commission',
        title: '固定提成取高报表 · 成品',
        sourceType: 'static_page',
        templateId: 'static_page_published_preview',
        ownerScope: { type: 'static_page_draft', id: 'draft-fixed-commission' },
        payload: {
          status: 'rendered',
          previewPath: '/generated-artifacts/fixed-commission/index.html',
        },
      }],
    });

    assert.ok(cards.some((card) => card.title === '静态页：固定提成取高报表'));
    assert.ok(cards.some((card) => card.title === '静态页：经营健康度总览'));
    assert.ok(cards.some((card) => card.title === '静态页：固定提成取高风险识别'));
    assert.equal(cards.some((card) => /随便|我看看|成品|生成固定/.test(card.title)), false);
  });

  it('surfaces one redacted asset-import shelf card for duplicate scope tasks', () => {
    const assetLibrary = {
      id: 'library-private-001',
      name: '服装设计资产库',
    };
    const assets = [
      {
        id: 'asset-a',
        title: 'spring-dress.png',
        object_key: 'objects/private/spring-dress.png',
      },
      {
        id: 'asset-b',
        title: 'workwear.webp',
        source_id: 'https://example.com/workwear.webp',
      },
    ];
    const buildTask = (items) => buildFashionDesignAssetImportShelfTask({
      assetLibrary,
      scope: {
        summary: {
          asset_library: assetLibrary,
          dataset_ids: ['dataset-private-001'],
          asset_count: 2,
          asset_parse_status_counts: { pending: 2 },
          asset_parse_run_count: 2,
          assets: items,
        },
      },
    });

    const cards = buildArtifactTaskCards({
      codexCustomerTasks: [buildTask(assets), buildTask([...assets].reverse())],
    });

    assert.equal(cards.length, 1);
    assert.equal(cards[0].kind, 'codex_task');
    assert.equal(cards[0].title, '任务：图库任务：服装设计资产库');
    assert.equal(cards[0].status, 'queued');
    assert.equal(cards[0].detail.resultItems.some((item) => item.text.startsWith('字段账本：')), true);

    const serialized = JSON.stringify(cards[0]);
    assert.equal(serialized.includes('objects/private'), false);
    assert.equal(serialized.includes('https://example.com'), false);
    assert.equal(/object[_-]?key/i.test(serialized), false);
  });
});
