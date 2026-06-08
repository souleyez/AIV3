import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { planAssistantScope, selectPlannerDatasetId } from './scope-planner.js';

const videoPptTriggerCases = loadVideoPptTriggerCases();

const datasets = [
  {
    id: 'dataset-orders',
    key: 'orders',
    title: '订单',
    description: '销售、营收、发货和复购数据',
    lifecycle: 'active',
    document_count: 12,
    estimated_word_count: 3600,
    parse_status_summary: 'completed:12',
  },
  {
    id: 'dataset-support',
    key: 'support',
    title: '客服',
    description: '客服、工单、投诉和满意度记录',
    lifecycle: 'ingesting',
    documentCount: 4,
    wordCount: 900,
  },
];

function loadVideoPptTriggerCases() {
  const fixture = JSON.parse(readFileSync(
    new URL('../../../../fixtures/video-ppt-trigger-classifier/trigger-cases.json', import.meta.url),
    'utf8',
  ));
  assert.equal(fixture.schema, 'v3.video_ppt_trigger_classifier_fixture.v1');
  assert.ok(Array.isArray(fixture.positive));
  assert.ok(Array.isArray(fixture.negative));
  return fixture;
}

test('scope planner keeps user selected dataset as highest priority', () => {
  const plan = planAssistantScope({
    prompt: '分析一下客服投诉趋势',
    datasets,
    selectedDatasetId: 'dataset-orders',
  });

  assert.equal(plan.candidates[0].id, 'dataset-orders');
  assert.equal(plan.candidates[0].source, 'user_selected');
  assert.equal(selectPlannerDatasetId(plan), 'dataset-orders');
});

test('scope planner auto-selects matching visible dataset when none is selected', () => {
  const plan = planAssistantScope({
    prompt: '最近订单营收和复购怎么样',
    datasets,
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-orders');
  assert.match(plan.hint, /订单/);
  assert.match(plan.hint, /12文档/);
  assert.equal(plan.intent, 'data_question');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'standard');
  assert.equal(plan.supplyStrategy.candidatePolicy, 'selected_or_inferred_visible_datasets_only');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['retrieval.search']);
  assert.equal(plan.candidates[0].documentCount, 12);
  assert.equal(plan.candidates[0].estimatedWordCount, 3600);
  assert.equal(plan.candidates[0].parseStatusSummary, 'completed:12');
});

test('scope planner auto-selects visible dataset from document title hints', () => {
  const plan = planAssistantScope({
    prompt: '固定资产怎么操作',
    datasets: [
      {
        id: 'dataset-ioa',
        key: 'xinshijie-ioa',
        title: '新世界 IOA 问答测试集',
        document_title_hints: ['用户手册3-固定资产.docx', 'IOA系统Q&A.pdf'],
        document_count: 6,
      },
    ],
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-ioa');
  assert.equal(plan.candidates[0].source, 'scope_planner');
  assert.equal(plan.candidates[0].documentCount, 6);
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'standard');
});

test('scope planner auto-selects visible dataset from document understanding hints', () => {
  const plan = planAssistantScope({
    prompt: '查找供应商确认这个主题在哪个库里',
    datasets: [
      {
        id: 'dataset-contracts',
        key: 'contracts',
        title: '合同资料',
        lifecycle: 'active',
        noun_term_hints: ['订单延期风险', '供应商确认'],
        section_title_hints: ['履约概览'],
        document_count: 3,
      },
    ],
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-contracts');
  assert.equal(plan.candidates[0].source, 'scope_planner');
  assert.deepEqual(plan.candidates[0].nounTermHints, ['订单延期风险', '供应商确认']);
  assert.deepEqual(plan.candidates[0].sectionTitleHints, ['履约概览']);
});

test('scope planner auto-selects visible dataset from visible document titles', () => {
  const plan = planAssistantScope({
    prompt: '固定资产怎么操作',
    datasets: [
      {
        id: 'dataset-ioa',
        key: 'xinshijie-ioa',
        title: '新世界 IOA 问答测试集',
        lifecycle: 'active',
      },
    ],
    documents: [
      {
        id: 'doc-fixed-assets',
        dataset_id: 'dataset-ioa',
        title: '用户手册3-固定资产.docx',
      },
    ],
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-ioa');
  assert.equal(plan.candidates[0].source, 'scope_planner');
  assert.equal(plan.candidates[0].documentCount, 1);
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'standard');
});

test('scope planner treats resume company questions as broad entity scans', () => {
  const plan = planAssistantScope({
    prompt: '简历数据集里提到了多少个公司名',
    datasets: [
      {
        id: 'dataset-resumes',
        key: 'resumes',
        title: '简历',
        description: '候选人履历、工作经历和任职公司',
        document_count: 25,
      },
    ],
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-resumes');
  assert.equal(plan.intent, 'data_question');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'detail_first');
  assert.equal(plan.supplyStrategy.coveragePolicy, 'document_entity_scan');
  assert.equal(plan.supplyStrategy.preferDetail, true);
  assert.equal(plan.supplyStrategy.contextBudgetPolicy, 'quality_first_token_tolerant');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, [
    'retrieval.search',
    'retrieval.read_detail',
    'retrieval.scan_documents',
  ]);
});

test('scope planner keeps unrelated no-dataset chat as ordinary model chat', () => {
  const plan = planAssistantScope({
    prompt: '帮我写一句开场白',
    datasets,
    conversationMemory: [{ role: 'user', content: '刚才讨论了订单风险' }],
  });

  assert.equal(plan.intent, 'ordinary_chat');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.length, 0);
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.equal(plan.supplyStrategy.preferDetail, false);
  assert.equal(plan.supplyStrategy.historyPolicy, 'intent_gated');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['ordinary_chat.answer']);
});

test('scope planner recommends direct video PPT extraction without forcing dataset retrieval', () => {
  const plan = planAssistantScope({
    prompt: 'https://example.com/talk.mp4 帮我提取视频里的PPT和原文',
    datasets,
  });

  assert.equal(plan.intent, 'data_question');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.length, 0);
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.equal(plan.supplyStrategy.preferDetail, false);
  assert.equal(plan.supplyStrategy.candidatePolicy, 'ordinary_chat_without_forced_dataset');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['media.resolve_video_url', 'media.extract_ppt_transcript']);
});

test('scope planner recommends public video page resolution before PPT extraction', () => {
  const plan = planAssistantScope({
    prompt: 'https://example.com/course-page 帮我提取这个公开视频里的PPT和字幕',
    datasets,
  });

  assert.equal(plan.intent, 'data_question');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.length, 0);
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.equal(plan.supplyStrategy.candidatePolicy, 'ordinary_chat_without_forced_dataset');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['media.resolve_video_url', 'media.extract_ppt_transcript']);
});

test('scope planner treats mkv and avi video names as PPT extraction triggers', () => {
  const mkvPlan = planAssistantScope({
    prompt: 'https://cdn.example.com/lesson.mkv 帮我提取这个视频里的PPT',
    datasets,
  });
  const aviPlan = planAssistantScope({
    prompt: '培训回放.avi 里面的幻灯片提取一下',
    datasets,
  });

  assert.equal(mkvPlan.intent, 'data_question');
  assert.equal(mkvPlan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.deepEqual(mkvPlan.supplyStrategy.recommendedActions, ['media.resolve_video_url', 'media.extract_ppt_transcript']);
  assert.deepEqual(aviPlan.supplyStrategy.recommendedActions, ['media.resolve_video_url', 'media.extract_ppt_transcript']);
});

test('scope planner recommends uploaded video extraction without public URL resolution', () => {
  const plan = planAssistantScope({
    prompt: '我刚上传了一个视频，帮我提取里面的PPT和原文',
    datasets,
  });

  assert.equal(plan.intent, 'data_question');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.length, 0);
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.equal(plan.supplyStrategy.preferDetail, false);
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['media.extract_ppt_transcript']);
});

test('scope planner does not use PPT extraction for transcript-only video requests', () => {
  const plan = planAssistantScope({
    prompt: '我刚上传了一个视频，帮我提取字幕和转写原文',
    datasets,
  });

  assert.equal(plan.intent, 'data_question');
  assert.equal(plan.supplyStrategy.recommendedActions.includes('media.extract_ppt_transcript'), false);
});

test('scope planner rejects shared negative video PPT trigger fixture', () => {
  for (const triggerCase of videoPptTriggerCases.negative) {
    const plan = planAssistantScope({ prompt: triggerCase.prompt, datasets });
    assert.equal(
      plan.supplyStrategy.recommendedActions.includes('media.extract_ppt_transcript'),
      false,
      triggerCase.id,
    );
  }
});

test('scope planner follows shared positive video PPT trigger fixture', () => {
  for (const triggerCase of videoPptTriggerCases.positive) {
    const plan = planAssistantScope({ prompt: triggerCase.prompt, datasets });

    assert.deepEqual(
      plan.supplyStrategy.recommendedActions,
      triggerCase.expected_scope_actions,
      triggerCase.id,
    );
  }
});

test('scope planner auto-selects media dataset for audio and video prompts', () => {
  const plan = planAssistantScope({
    prompt: '这段录音讲了什么，帮我提炼重点',
    datasets: [
      ...datasets,
      {
        id: 'dataset-media',
        key: 'meeting-media',
        title: '会议录音',
        description: '上传的音频、视频、转写和关键帧 OCR 资料',
      },
    ],
  });

  assert.equal(selectPlannerDatasetId(plan), 'dataset-media');
  assert.equal(plan.intent, 'data_question');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'detail_first');
  assert.equal(plan.supplyStrategy.preferDetail, true);
  assert.equal(plan.supplyStrategy.contextBudgetPolicy, 'quality_first_token_tolerant');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['retrieval.search', 'retrieval.read_detail', 'media.detail']);
  assert.deepEqual(
    plan.candidates.find((candidate) => candidate.id === 'dataset-media')?.materialHints,
    ['audio_video', 'transcript_possible', 'keyframe_ocr_possible'],
  );
  assert.match(plan.hint, /会议录音/);
});

test('scope planner can include conversation memory without forcing dataset retrieval', () => {
  const plan = planAssistantScope({
    prompt: '继续按刚才那版改一下',
    datasets,
    conversationMemory: [{ role: 'user', content: '做一个静态页草稿' }],
  });

  assert.equal(plan.candidates.some((candidate) => candidate.type === 'conversation_memory'), true);
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.supplyStrategy.historyPolicy, 'intent_gated_selected');
  assert.equal(plan.supplyStrategy.contextBudgetPolicy, 'quality_first_token_tolerant');
});

test('scope planner marks static page intent as detail-first when data is available', () => {
  const plan = planAssistantScope({
    prompt: '基于订单做一页静态页经营分析',
    datasets,
  });

  assert.equal(plan.intent, 'static_page');
  assert.equal(plan.intentLabel, '静态页规划');
  assert.equal(selectPlannerDatasetId(plan), 'dataset-orders');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'detail_first');
  assert.equal(plan.supplyStrategy.preferDetail, true);
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['retrieval.search', 'retrieval.read_detail', 'static_page.plan']);
  assert.match(plan.hint, /意图：静态页规划/);
});

test('scope planner keeps no-dataset static page request as ordinary supply', () => {
  const plan = planAssistantScope({
    prompt: '帮我先规划一页静态页',
    datasets: [],
  });

  assert.equal(plan.intent, 'static_page');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['static_page.plan']);
  assert.equal(plan.supplyStrategy.noFakeData, true);
});

test('scope planner treats active static page edits as artifact context', () => {
  const plan = planAssistantScope({
    prompt: '把标题改短，图表换成柱状图',
    datasets,
    activeStaticPageDraft: {
      backendDraftId: 'static-draft-1',
      id: 'local-static-draft-1',
      objective: '订单经营分析页',
      status: 'planning',
      styleDirection: 'data-command',
      previewContract: { status: 'stale' },
      finalPage: null,
      modules: [{ id: 'hero' }, { id: 'trend' }],
    },
  });

  const draftCandidate = plan.candidates.find((candidate) => candidate.type === 'static_page_draft');
  assert.equal(plan.intent, 'static_page');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(plan.candidates.some((candidate) => candidate.type === 'static_page_draft'), true);
  assert.equal(draftCandidate?.id, 'static-draft-1');
  assert.equal(draftCandidate?.moduleCount, 2);
  assert.equal(draftCandidate?.previewStatus, 'stale');
  assert.equal(draftCandidate?.previewStale, true);
  assert.equal(plan.supplyStrategy.currentArtifactPolicy, 'active_static_page_draft');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'not_requested');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['static_page.update_draft']);
  assert.match(plan.hint, /当前静态页：订单经营分析页\(规划已变更\)/);
});

test('scope planner reuses draft dataset when active static page data quality needs repair', () => {
  const plan = planAssistantScope({
    prompt: '继续刚才那版，把趋势数据补扎实再出效果图',
    datasets,
    activeStaticPageDraft: {
      backendDraftId: 'static-draft-quality',
      objective: '订单经营分析页',
      status: 'planning',
      modules: [{ id: 'trend' }, { id: 'summary' }],
      dataSnapshot: {
        selectedDatasetId: 'dataset-orders',
        moduleBindings: [
          {
            moduleId: 'trend',
            title: '趋势模块',
            visualizationType: 'line-chart',
            bindingQualityStatus: 'matched_field_candidate',
            chartDataFit: 'needs_sample_rows',
            bindingQuality: { sampleRows: 0 },
          },
          {
            moduleId: 'summary',
            title: '总结模块',
            visualizationType: 'text-insight',
            bindingQualityStatus: 'confirmed',
            chartDataFit: 'not_required',
          },
        ],
      },
    },
  });

  const datasetCandidate = plan.candidates.find((candidate) => candidate.type === 'dataset');
  const draftCandidate = plan.candidates.find((candidate) => candidate.type === 'static_page_draft');
  assert.equal(plan.intent, 'static_page');
  assert.equal(selectPlannerDatasetId(plan), 'dataset-orders');
  assert.equal(datasetCandidate?.source, 'active_artifact_data_quality');
  assert.equal(draftCandidate?.dataQualityStatus, 'attention_required');
  assert.equal(draftCandidate?.dataQualityAttentionCount, 1);
  assert.equal(draftCandidate?.dataQualityBindingCount, 2);
  assert.equal(draftCandidate?.dataQualityAttentionModules[0].title, '趋势模块');
  assert.equal(plan.supplyStrategy.retrievalPolicy, 'detail_first');
  assert.equal(plan.supplyStrategy.preferDetail, true);
  assert.equal(plan.supplyStrategy.contextBudgetPolicy, 'quality_first_token_tolerant');
  assert.equal(plan.supplyStrategy.artifactDataQualityPolicy, 'repair_before_preview_or_render');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, [
    'retrieval.search',
    'retrieval.read_detail',
    'static_page.update_draft',
  ]);
  assert.match(plan.hint, /需修复数据1/);
});

test('scope planner treats vague follow-up as static page edit when draft is active', () => {
  const plan = planAssistantScope({
    prompt: '继续刚才那版改一下',
    datasets,
    activeStaticPageDraft: {
      backendDraftId: 'static-draft-1',
      objective: '订单经营分析页',
      status: 'planning',
      modules: [{ id: 'hero' }],
    },
  });

  const draftCandidate = plan.candidates.find((candidate) => candidate.type === 'static_page_draft');
  assert.equal(plan.intent, 'static_page');
  assert.equal(selectPlannerDatasetId(plan), '');
  assert.equal(draftCandidate?.id, 'static-draft-1');
  assert.equal(draftCandidate?.confidence, 'high');
  assert.equal(plan.supplyStrategy.currentArtifactPolicy, 'active_static_page_draft');
  assert.deepEqual(plan.supplyStrategy.recommendedActions, ['static_page.update_draft']);
});
