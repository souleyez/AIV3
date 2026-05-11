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
