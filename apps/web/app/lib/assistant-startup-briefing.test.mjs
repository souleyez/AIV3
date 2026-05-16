import test from 'node:test';
import assert from 'node:assert/strict';
import { buildAssistantStartupBriefing, formatStartupBriefingForModel } from './assistant-startup-briefing.js';

test('startup briefing summarizes visible datasets and system capability', () => {
  const briefing = buildAssistantStartupBriefing({
    datasets: [
      {
        id: 'dataset-1',
        key: 'orders',
        title: '订单数据',
        lifecycle: 'active',
        document_count: 3,
        estimated_word_count: 1200,
        parse_status_summary: 'completed:3',
        materialHints: ['tabular'],
        updated_at: '2026-04-26T10:00:00.000Z',
      },
      {
        id: 'dataset-2',
        key: 'support',
        title: '客服知识库',
        lifecycle: 'ingesting',
        documentCount: 2,
        word_count: 800,
        updated_at: '2026-04-25T10:00:00.000Z',
      },
    ],
    reportPlans: [{ id: 'plan-1' }],
    publishedReports: [{ id: 'report-1' }],
    selectedDataset: { title: '订单数据' },
  });

  assert.equal(briefing.visibleDatasetCount, 2);
  assert.equal(briefing.visibleDocumentCount, 5);
  assert.equal(briefing.estimatedWordCount, 2000);
  assert.equal(briefing.reportPlanCount, 1);
  assert.equal(briefing.publishedReportCount, 1);
  assert.equal(briefing.staticPageDraftCount, 0);
  assert.equal(briefing.selectedScopeLabel, '订单数据');
  assert.equal(briefing.briefingVersion, 3);
  assert.equal(briefing.datasetBriefs.length, 2);
  assert.equal(briefing.datasetBriefs[0].parseStatusSummary, 'completed:3');
  assert.deepEqual(briefing.datasetBriefs[0].materialHints, ['tabular']);
  assert.ok(briefing.capabilities.includes('media_detail'));
  assert.ok(briefing.capabilities.includes('video_url_resolve'));
  assert.ok(briefing.capabilities.includes('video_ppt_extract'));
  assert.ok(briefing.capabilities.includes('continuous_execution'));
  assert.match(briefing.modelAwarenessPolicy.identity, /AI Data Platform V3/);
  assert.match(briefing.modelAwarenessPolicy.additiveContextRule, /不是能力限制/);
  assert.match(briefing.modelAwarenessPolicy.unavailableEvidenceRule, /当前不可见\/未供料/);
  assert.match(briefing.modelAwarenessPolicy.externalSearchPolicy.modelRule, /不要声称已联网搜索/);
  assert.match(briefing.supplyEvidencePolicy.citableEvidenceRule, /supplied_items/);
  assert.match(briefing.supplyEvidencePolicy.citableEvidenceRule, /observation/);
  assert.match(briefing.supplyEvidencePolicy.planningOnlyRule, /scope candidates/);
  assert.match(briefing.supplyEvidencePolicy.planningOnlyRule, /startup briefing/);
  assert.match(briefing.supplyEvidencePolicy.detailTargetRule, /detail_targets/);
  assert.match(briefing.productCapabilities.staticPage, /静态页规划/);
  assert.match(briefing.productCapabilities.staticPage, /index\.html/);
  assert.match(briefing.productCapabilities.staticPage, /ZIP 交付包/);
  assert.match(briefing.productCapabilities.media, /partial/);
  assert.match(briefing.productCapabilities.media, /视频 URL/);
  assert.match(briefing.productCapabilities.media, /PPT\/原文提取/);
  assert.match(briefing.productCapabilities.media, /video_slides\.md/);
  assert.match(briefing.productCapabilities.media, /登录态、扫码、Cookie/);
  assert.ok(briefing.mediaExtractionPolicy.supportedSources.includes('上传视频文件'));
  assert.ok(briefing.mediaExtractionPolicy.supportedSources.includes('直接视频 URL'));
  assert.ok(briefing.mediaExtractionPolicy.unsupportedSources.includes('扫码登录'));
  assert.ok(briefing.mediaExtractionPolicy.unsupportedSources.includes('Cookie/Session 复用'));
  assert.deepEqual(briefing.mediaExtractionPolicy.modelRequestActions, [
    'media.resolve_video_url',
    'media.extract_ppt_transcript',
  ]);
  assert.deepEqual(briefing.mediaExtractionPolicy.controlledPipeline, [
    'media.resolve_video_url',
    'media.register_video_asset',
    'media.extract_ppt_transcript',
  ]);
  assert.ok(briefing.mediaExtractionPolicy.outputArtifacts.includes('subtitle_page_map'));
  assert.ok(briefing.mediaExtractionPolicy.outputArtifacts.includes('video_slides_markdown'));
  assert.ok(briefing.mediaExtractionPolicy.outputArtifacts.includes('video_slides.md'));
  assert.ok(briefing.mediaExtractionPolicy.outputArtifacts.includes('final_deliverables_manifest'));
  assert.ok(briefing.mediaExtractionPolicy.outputArtifacts.includes('published_version_history'));
  assert.ok(briefing.mediaExtractionPolicy.outputArtifacts.includes('extraction_artifacts_manifest'));
  assert.match(briefing.mediaExtractionPolicy.accessRule, /不要声称已访问视频/);
  assert.match(briefing.productCapabilities.continuousExecution, /受控动作/);
  assert.match(briefing.latestActivity, /订单数据/);
  assert.match(briefing.productTruth, /智能数据工作台/);
});

test('formatted briefing tells model when no dataset is selected', () => {
  const briefing = buildAssistantStartupBriefing();
  const formatted = formatStartupBriefingForModel(briefing);

  assert.match(formatted, /当前未选数据集/);
  assert.match(formatted, /普通模型聊天/);
  assert.match(formatted, /AI Data Platform V3/);
  assert.match(formatted, /V3 上下文是附加能力/);
  assert.match(formatted, /当前不可见\/未供料/);
  assert.match(formatted, /不要声称已联网搜索/);
  assert.match(formatted, /只有 V3 supplied_items、observation/);
  assert.match(formatted, /scope candidates、dataset briefs、startup briefing、detail_targets/);
  assert.match(formatted, /不是引用依据/);
  assert.match(formatted, /detail_targets 代表建议深读目标/);
  assert.match(formatted, /创建报表/);
  assert.match(formatted, /媒体细节/);
  assert.match(formatted, /视频转 PPT\/原文提取/);
  assert.match(formatted, /媒体提取边界/);
  assert.match(formatted, /模型请求入口：media\.resolve_video_url、media\.extract_ppt_transcript/);
  assert.match(formatted, /后台受控链路：media\.resolve_video_url -> media\.register_video_asset -> media\.extract_ppt_transcript/);
  assert.match(formatted, /扫码登录/);
  assert.match(formatted, /Cookie\/Session 复用/);
  assert.match(formatted, /不要声称已访问视频/);
  assert.match(formatted, /subtitle_page_map/);
  assert.match(formatted, /video_slides_markdown/);
  assert.match(formatted, /video_slides\.md/);
  assert.match(formatted, /final_deliverables_manifest/);
  assert.match(formatted, /published_version_history/);
  assert.match(formatted, /extraction_artifacts_manifest/);
  assert.match(formatted, /规划\/渲染\/修改静态页/);
  assert.match(formatted, /导出静态页 ZIP 交付包/);
  assert.match(formatted, /连续提出检索/);
  assert.match(formatted, /静态页草稿\/成品 0 个/);
  assert.match(formatted, /不能编造数据/);
  assert.match(formatted, /当前可见库为空/);
});

test('formatted briefing includes compact dataset material metadata without content', () => {
  const briefing = buildAssistantStartupBriefing({
    datasets: [{
      id: 'dataset-media',
      key: 'media',
      title: '会议录音',
      lifecycle: 'active',
      documents: [
        { id: 'doc-1', parseStatus: 'completed', content: '正文不能进入简报' },
        { id: 'doc-2', parseStatus: 'partial', content: '转写原文也不能进入简报' },
      ],
      description: '音频、视频、转写和关键帧 OCR',
    }],
  });
  const formatted = formatStartupBriefingForModel(briefing);

  assert.equal(briefing.visibleDocumentCount, 2);
  assert.equal(briefing.datasetBriefs[0].documentCount, 2);
  assert.equal(briefing.datasetBriefs[0].parseStatusSummary, 'completed:1，partial:1');
  assert.ok(briefing.datasetBriefs[0].materialHints.includes('audio_video'));
  assert.match(formatted, /会议录音\(2文档\/active\/解析:completed:1，partial:1\/audio_video/);
  assert.doesNotMatch(formatted, /正文不能进入简报/);
  assert.doesNotMatch(formatted, /转写原文/);
});

test('startup briefing prefers recent upload or classification activity', () => {
  const briefing = buildAssistantStartupBriefing({
    datasets: [
      {
        id: 'dataset-1',
        key: 'orders',
        title: '订单数据',
        lifecycle: 'active',
        updated_at: '2026-04-25T10:00:00.000Z',
      },
    ],
    activityEvents: [
      {
        summary: '上传分类：客服投诉明细.xlsx -> 客服',
        created_at: '2026-04-26T10:00:00.000Z',
      },
    ],
  });

  assert.match(briefing.latestActivity, /客服投诉明细/);
});

test('startup briefing includes compact static page workspace context', () => {
  const briefing = buildAssistantStartupBriefing({
    activeStaticPageDraft: {
      id: 'local-draft-1',
      backendDraftId: 'draft-backend-1',
      objective: '客户经营分析静态页',
      status: 'rendered',
      styleDirection: 'data-command',
      previewContract: { status: 'confirmed' },
      finalPage: { status: 'rendered' },
      modules: [
        {
          id: 'trend',
          title: '趋势',
          content: '不应该完整依赖 content 进入工作区摘要',
          visualization: { chartRuntime: 'echarts' },
        },
        {
          id: 'risk',
          title: '风险',
          visualization: { chartRuntime: 'deterministic' },
        },
      ],
    },
    staticPageDrafts: [
      {
        id: 'local-draft-1',
        objective: '客户经营分析静态页',
        status: 'rendered',
        modules: [{ id: 'trend' }],
      },
    ],
  });
  const formatted = formatStartupBriefingForModel(briefing);

  assert.equal(briefing.staticPageDraftCount, 1);
  assert.equal(briefing.staticPageWorkspace.activeDraftId, 'draft-backend-1');
  assert.equal(briefing.staticPageWorkspace.activeModuleCount, 2);
  assert.equal(briefing.staticPageWorkspace.activeEchartsModuleCount, 1);
  assert.equal(briefing.staticPageWorkspace.canExportFinal, true);
  assert.match(formatted, /当前静态页：客户经营分析静态页/);
  assert.match(formatted, /ECharts 1 个/);
  assert.match(formatted, /已可导出 index\.html/);
  assert.doesNotMatch(formatted, /不应该完整依赖 content/);
});

test('startup briefing marks stale static page preview as non-exportable', () => {
  const briefing = buildAssistantStartupBriefing({
    activeStaticPageDraft: {
      id: 'local-draft-stale',
      objective: '已修改的客户静态页',
      status: 'planning',
      previewContract: { status: 'stale' },
      finalPage: { status: 'rendered' },
      modules: [{ id: 'hero' }],
    },
    staticPageDrafts: [
      {
        id: 'local-draft-stale',
        objective: '已修改的客户静态页',
        previewContract: { status: 'stale' },
        finalPage: { status: 'rendered' },
        modules: [{ id: 'hero' }],
      },
    ],
  });
  const formatted = formatStartupBriefingForModel(briefing);

  assert.equal(briefing.staticPageWorkspace.previewStale, true);
  assert.equal(briefing.staticPageWorkspace.canExportFinal, false);
  assert.equal(briefing.staticPageWorkspace.latestDrafts[0].status, 'stale');
  assert.match(formatted, /旧效果图和最终页不能继续复用/);
  assert.doesNotMatch(formatted, /已可导出 index\.html/);
});
