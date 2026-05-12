import assert from 'node:assert/strict';
import test from 'node:test';
import {
  isAllowedHtmlArtifactMessage,
  normalizeHtmlArtifactManifest,
  renderHtmlArtifactDocument,
} from './html-artifact-manifest.js';

function baseManifest(overrides = {}) {
  return {
    kind: 'html_artifact',
    version: 1,
    id: 'artifact-1',
    title: '执行报告',
    sourceType: 'codex_host',
    templateId: 'codex_execution_report',
    interactionMode: 'read_only',
    ownerScope: { type: 'assistant_run', id: 'run-1' },
    dataRefs: [{ kind: 'workflow_execution', id: 'workflow-1', label: 'Codex Host task' }],
    provenance: { producer: 'codex-host-agent', reason: 'plan_only report', sourceRunId: 'run-1' },
    payload: {
      mode: 'plan_only',
      status: 'planned',
      capability: 'inspect_project',
      workspaceLabel: 'codex-host-task-test',
      summary: '只生成安全计划，不执行本机 Codex。',
      steps: [{ title: '检查仓库', detail: '读取目录和计划。' }],
      risks: [{ title: '未真机执行', detail: '等待跳板机验证。' }],
    },
    ...overrides,
  };
}

test('normalizes a safe html artifact manifest', () => {
  const result = normalizeHtmlArtifactManifest(baseManifest());

  assert.equal(result.rejected, false);
  assert.equal(result.manifest.kind, 'html_artifact');
  assert.equal(result.manifest.templateId, 'codex_execution_report');
  assert.equal(result.manifest.templateLabel, 'Codex 执行报告');
  assert.equal(result.manifest.ownerScope.id, 'run-1');
  assert.equal(result.manifest.dataRefs[0].kind, 'workflow_execution');
});

test('rejects unsupported templates and interaction modes', () => {
  assert.equal(normalizeHtmlArtifactManifest(baseManifest({ templateId: 'raw_html_app' })).rejected, true);
  assert.match(
    normalizeHtmlArtifactManifest(baseManifest({ interactionMode: 'allow_everything' })).reason,
    /unsupported interaction mode/,
  );
});

test('rejects script remote urls and secret-like payloads', () => {
  const script = normalizeHtmlArtifactManifest(baseManifest({
    payload: { summary: '<script>alert(1)</script>' },
  }));
  const remoteUrl = normalizeHtmlArtifactManifest(baseManifest({
    payload: { summary: 'see https://example.com/asset.js' },
  }));
  const secret = normalizeHtmlArtifactManifest(baseManifest({
    payload: { summary: 'Authorization: Bearer abc' },
  }));

  assert.equal(script.rejected, true);
  assert.match(script.reason, /script/);
  assert.equal(remoteUrl.rejected, true);
  assert.match(remoteUrl.reason, /https/);
  assert.equal(secret.rejected, true);
  assert.match(secret.reason, /authorization|bearer/i);
});

test('renders read-only html in a locked sandbox', () => {
  const result = renderHtmlArtifactDocument(baseManifest());

  assert.equal(result.rejected, false);
  assert.equal(result.sandbox, '');
  assert.match(result.html, /Content-Security-Policy/);
  assert.match(result.html, /script-src 'none'/);
  assert.match(result.html, /只生成安全计划/);
  assert.doesNotMatch(result.html, /allow-same-origin/);
});

test('interactive artifacts only allow matching safe structured postMessage payloads', () => {
  const manifest = baseManifest({
    interactionMode: 'json_patch',
    payload: {
      ...baseManifest().payload,
      operations: [{ op: 'replace', path: '/modules/0/title', value: '新标题' }],
    },
  });
  const rendered = renderHtmlArtifactDocument(manifest);

  assert.equal(rendered.sandbox, 'allow-scripts');
  assert.match(rendered.html, /html_artifact\.patch/);
  assert.equal(isAllowedHtmlArtifactMessage({
    source: 'v3-html-artifact',
    artifactId: 'artifact-1',
    type: 'html_artifact.patch',
    payload: { operations: [{ op: 'replace', path: '/modules/0/title', value: '新标题' }] },
  }, manifest), true);
  assert.equal(isAllowedHtmlArtifactMessage({
    source: 'v3-html-artifact',
    artifactId: 'artifact-1',
    type: 'html_artifact.patch',
    payload: { action: 'submit' },
  }, manifest), false);
  assert.equal(isAllowedHtmlArtifactMessage({
    source: 'v3-html-artifact',
    artifactId: 'artifact-1',
    type: 'html_artifact.action_intent',
    payload: { action: 'submit' },
  }, manifest), false);
  assert.equal(isAllowedHtmlArtifactMessage({
    source: 'v3-html-artifact',
    artifactId: 'artifact-1',
    type: 'html_artifact.patch',
    payload: { action: '<script>alert(1)</script>' },
  }, manifest), false);

  const intentManifest = baseManifest({ interactionMode: 'action_intent' });
  const intentRendered = renderHtmlArtifactDocument(intentManifest);
  assert.match(intentRendered.html, /data-artifact-intent/);
  assert.equal(isAllowedHtmlArtifactMessage({
    source: 'v3-html-artifact',
    artifactId: 'artifact-1',
    type: 'html_artifact.action_intent',
    payload: { action: 'apply_static_page_intent', prompt: '把风险模块缩小一点' },
  }, intentManifest), true);
});

test('renders static page planning and code review templates', () => {
  const staticPage = renderHtmlArtifactDocument(baseManifest({
    id: 'static-page-handoff',
    sourceType: 'static_page',
    templateId: 'static_page_planning_handoff',
    title: '客户经营页规划',
    payload: {
      objective: '展示客户经营状态。',
      visualBridge: {
        providerLane: 'gpt-image-2-cloudflare-queue',
        status: 'stale',
        imageJobId: 'image-job-1',
        queuePosition: 2,
        previewAssetKey: 'static-page-previews/image-job-1.json',
        staleReason: '规划已变更',
        draftFingerprint: 'design-abc123',
        finalRenderStatus: 'not_requested',
        renderModel: 'dom-text-svg-chart',
        rule: '效果图只锁定视觉方向；最终 HTML 由 renderer 生成。',
      },
      modules: [{
        id: 'risk',
        title: '风险模块',
        content: '延期订单',
        dataBinding: 'orders.delay_rate',
        visualizationType: 'bar',
        layout: { x: 0, y: 2, w: 4, h: 3 },
        dataQuality: 'partial',
      }],
    },
  }));
  const quality = renderHtmlArtifactDocument(baseManifest({
    id: 'quality-report',
    sourceType: 'static_page',
    templateId: 'static_page_data_quality_report',
    title: '客户经营页数据质量',
    payload: {
      summary: {
        confirmedModules: 1,
        partialModules: 1,
        missingModules: 0,
        attentionModules: 1,
      },
      modules: [{
        moduleId: 'trend',
        title: '趋势模块',
        dataQualityStatus: 'partial',
        chartRuntime: 'echarts',
        sampleDataRows: 3,
        fallback: true,
        recommendedAction: '补齐完整月份数据',
      }],
    },
  }));
  const review = renderHtmlArtifactDocument(baseManifest({
    id: 'review-summary',
    sourceType: 'code_review',
    templateId: 'code_review_summary',
    title: '代码审查',
    payload: {
      summary: '发现一个问题。',
      findings: [{ severity: 'P1', title: '权限缺口', file: 'src/api.rs', line: 42, detail: '缺少 owner check。' }],
    },
  }));

  assert.match(staticPage.html, /风险模块/);
  assert.match(staticPage.html, /视觉链路/);
  assert.match(staticPage.html, /gpt-image-2-cloudflare-queue/);
  assert.match(staticPage.html, /规划已变更/);
  assert.match(staticPage.html, /design-abc123/);
  assert.match(staticPage.html, /效果图只锁定视觉方向/);
  assert.match(staticPage.html, /orders.delay_rate/);
  assert.match(quality.html, /数据质量汇总/);
  assert.match(quality.html, /趋势模块/);
  assert.match(quality.html, /最终页使用静态回退/);
  assert.match(review.html, /权限缺口/);
  assert.match(review.html, /src\/api.rs:L42/);
});

test('renders report render summary template', () => {
  const report = renderHtmlArtifactDocument(baseManifest({
    id: 'report-render-summary',
    sourceType: 'report',
    templateId: 'report_render_summary',
    title: '季度经营报告 · 渲染摘要',
    payload: {
      reportTitle: '季度经营报告',
      objective: '汇总订单、客服和风险信号。',
      surface: 'pc',
      status: 'rendered',
      publishable: true,
      assetKind: 'html',
      assetPath: 'reports/quarterly/pc.html',
      reportPlanId: 'plan-1',
      reportRenderOutputId: 'output-1',
      workflowExecutionId: 'workflow-1',
      astVersionId: 'ast-1',
      modelFacing: {
        recommendedToolKey: 'report.publish',
      },
      serviceHandoff: {
        reportEntryState: 'confirmed',
      },
      warnings: [{
        title: '发布前检查',
        detail: '确认报告资产路径可访问。',
      }],
    },
  }));

  assert.equal(report.rejected, false);
  assert.match(report.html, /报告渲染摘要/);
  assert.match(report.html, /季度经营报告/);
  assert.match(report.html, /reports\/quarterly\/pc\.html/);
  assert.match(report.html, /report\.publish/);
  assert.match(report.html, /发布前检查/);
});

test('renders video extraction summary template', () => {
  const video = renderHtmlArtifactDocument(baseManifest({
    id: 'video-extraction-summary',
    sourceType: 'video_extraction',
    templateId: 'video_extraction_summary',
    title: '课程视频 · 视频提取摘要',
    ownerScope: { type: 'document', id: 'doc-1' },
    payload: {
      document: {
        id: 'doc-1',
        datasetId: 'dataset-1',
        title: '课程视频',
        contentType: 'video/mp4',
        lifecycle: 'extracted',
      },
      mediaKind: 'video',
      parseStatus: 'transcribed',
      evidenceStatus: 'available',
      summary: {
        transcriptSegmentCount: 1,
        sceneCount: 1,
        keyframeOcrSnippetCount: 1,
      },
      transcriptSegments: [{
        startSeconds: 1,
        endSeconds: 3,
        text: '第一页介绍系统目标',
        source: 'MEDIA_TRANSCRIBE_BIN',
      }],
      scenes: [{
        startSeconds: 1,
        endSeconds: 8,
        summary: '标题页',
        source: 'MEDIA_SCENE_BIN',
      }],
      keyframeOcrSnippets: [{
        timestampSeconds: 2,
        text: 'AI 数据智能助手',
        source: 'MEDIA_KEYFRAME_OCR_BIN',
      }],
      missing: [],
      providerEvidence: [{
        provider: 'minimax',
        capability: 'media_understanding',
        status: 'ready',
        supported: true,
        detail: '已启用视频解析',
      }],
      generatedArtifacts: {
        status: 'completed',
        files: [{
          artifactKind: 'transcript_text',
          title: '课程视频 - transcript_text',
          format: 'text/plain',
          path: 'generated_artifacts/transcript.txt',
        }, {
          artifactKind: 'ppt_outline',
          title: '课程视频 - ppt_outline',
          format: 'text/markdown',
          path: 'generated_artifacts/ppt_outline.md',
        }],
      },
      deliverableStatus: {
        state: 'evidence_artifacts_ready',
        warningCount: 1,
        warnings: [{
          code: 'missing_transcript_alignment',
          severity: 'medium',
          message: 'Speaker notes cannot be aligned yet.',
        }],
      },
      completionFollowUp: {
        kind: 'video_extraction_completion_follow_up',
        status: 'evidence_artifacts_ready',
        readyFileKinds: ['transcript_text', 'ppt_outline'],
        nextActions: [
          'open_video_extraction_summary',
          'retry_frame_extraction',
          'provide_local_media_file_or_parsed_frames',
          'retry_generated_artifact_writer',
          'attach_or_parse_transcript_evidence',
          'generate_contact_sheet_from_raw_frames',
          'review_contact_sheet_or_promote_rectangle_extraction',
          'fill_ppt_keep_list_template',
          'review_low_confidence_transcript',
          'rerun_or_review_subtitle_ocr',
          'rerun_or_refresh_video_parse',
          'check_video_provider_configuration',
          'complete_keep_list_or_review_missing_inputs',
        ],
        modelFollowUp: {
          required: true,
          instruction: 'Use this structured completion status to notify the user.',
        },
        userNotification: {
          kind: 'video_extraction_status_notification',
          channel: 'assistant_run_artifact_status',
          scope: 'status_only',
          userVisible: true,
          severity: 'warning',
          title: '视频/PPT 提取需要复核',
          message: '后台任务已更新视频提取摘要，但仍有缺失证据或待复核交付项。',
          primaryNextAction: 'open_video_extraction_summary',
          noHostComposedAnswer: true,
        },
        noHostComposedAnswer: true,
      },
      completionAudit: {
        kind: 'video_extraction_completion_audit',
        version: 1,
        stateTransition: {
          workflowTask: 'extract_video_ppt',
          outputStatus: 'completed',
          deliverableState: 'evidence_artifacts_ready',
        },
        sourceResolution: {
          sourceType: 'public_page_resolvable_video',
          assetState: 'remote_registered',
          contentType: 'video/mp4',
          sourceUrlPresent: true,
          sourceUrlRedacted: true,
          sourcePageUrlPresent: true,
          sourcePageUrlRedacted: true,
        },
        warningCodes: ['missing_transcript_alignment', 'provider_failure'],
        warningCount: 2,
        providerFailureCount: 1,
        providerFailures: [{
          provider: 'minimax',
          capability: 'native_video_understanding',
          status: 'unsupported',
          supported: false,
        }],
        redaction: {
          rawUrlsIncluded: false,
          privatePathsIncluded: false,
          cookiesIncluded: false,
          providerKeysIncluded: false,
          rawProviderPayloadsIncluded: false,
        },
      },
      note: '缺失项不会被补造。',
    },
  }));

  assert.equal(video.rejected, false);
  assert.match(video.html, /视频提取摘要/);
  assert.match(video.html, /第一页介绍系统目标/);
  assert.match(video.html, /AI 数据智能助手/);
  assert.match(video.html, /标题页/);
  assert.match(video.html, /缺失项不会被补造/);
  assert.match(video.html, /minimax/);
  assert.match(video.html, /生成文件/);
  assert.match(video.html, /transcript\.txt/);
  assert.doesNotMatch(video.html, /generated_artifacts\/transcript\.txt/);
  assert.match(video.html, /ppt_outline/);
  assert.match(video.html, /质量提示/);
  assert.match(video.html, /Speaker notes cannot be aligned yet/);
  assert.match(video.html, /审计摘要/);
  assert.match(video.html, /状态流转/);
  assert.match(video.html, /completed -&gt; evidence_artifacts_ready/);
  assert.match(video.html, /来源解析/);
  assert.match(video.html, /public_page_resolvable_video/);
  assert.match(video.html, /url_redacted=yes/);
  assert.match(video.html, /provider_failures=1/);
  assert.match(video.html, /native_video_understanding/);
  assert.match(video.html, /raw_urls=no/);
  assert.match(video.html, /provider_keys=no/);
  assert.match(video.html, /完成状态/);
  assert.match(video.html, /后续提醒/);
  assert.match(video.html, /model follow-up required/);
  assert.match(video.html, /用户通知/);
  assert.match(video.html, /视频\/PPT 提取需要复核/);
  assert.match(video.html, /后台任务已更新视频提取摘要/);
  assert.match(video.html, /status_only/);
  assert.match(video.html, /PPT 大纲/);
  assert.match(video.html, /重试原始帧提取/);
  assert.match(video.html, /提供本地视频或已解析帧/);
  assert.match(video.html, /重试生成文件写入/);
  assert.match(video.html, /补充或解析转写证据/);
  assert.match(video.html, /基于原始帧生成联系表/);
  assert.match(video.html, /复核联系表或补齐矩形提取/);
  assert.match(video.html, /填写 PPT 保留页清单/);
  assert.match(video.html, /复核低置信度转写片段/);
  assert.match(video.html, /重跑或复核字幕 OCR/);
  assert.match(video.html, /重跑或刷新视频解析/);
  assert.match(video.html, /检查视频解析提供方配置/);
  assert.match(video.html, /完成选页或补齐缺失输入/);
  assert.match(video.html, /Use this structured completion status/);
  assert.doesNotMatch(video.html, /private\.example/);
  assert.doesNotMatch(video.html, /secret-token/);
  assert.doesNotMatch(video.html, /secret-cookie/);
});

test('renders legacy video login handoff as unsupported source guidance', () => {
  const video = renderHtmlArtifactDocument(baseManifest({
    id: 'video-source-limited',
    sourceType: 'video_extraction',
    templateId: 'wechat_video_login_handoff',
    title: '视频来源受限',
    payload: {
      sourcePlatform: '微信视频号',
      shortCode: 'ActLMg4yTD',
      status: 'login_required',
      loginMethod: '扫码登录',
      qrStatus: 'pending_executor',
      summary: '扫码登录交接',
      fallback: '录屏保存视频',
      targetArtifact: '视频 PPT 提取',
    },
  }));

  assert.equal(video.rejected, false);
  assert.match(video.html, /视频来源受限/);
  assert.match(video.html, /不执行扫码登录、Cookie、登录态页面获取或录屏绕过/);
  assert.match(video.html, /请上传视频文件/);
  assert.match(video.html, /视频 PPT 提取/);
  assert.doesNotMatch(video.html, /二维码/);
  assert.doesNotMatch(video.html, /扫码登录交接/);
  assert.doesNotMatch(video.html, /录屏保存视频/);
});
