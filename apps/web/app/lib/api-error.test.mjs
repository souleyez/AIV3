import test from 'node:test';
import assert from 'node:assert/strict';
import {
  assistantRunFailureMessage,
  apiErrorMessage,
  buildApiError,
  staticPagePreviewGateErrorMessage,
} from './api-error.js';

test('buildApiError preserves structured backend error details', () => {
  const error = buildApiError({
    code: 'static_page_preview_data_quality_gate',
    message: '当前静态页还有 1 个模块的数据绑定未达到效果图生成要求。',
    details: {
      attentionModuleCount: 1,
      attentionModules: [{
        moduleId: 'trend',
        title: '订单趋势',
        chartDataFit: 'needs_sample_rows',
      }],
    },
  }, '请求失败', 400);

  assert.equal(error.name, 'ApiError');
  assert.equal(error.status, 400);
  assert.equal(error.code, 'static_page_preview_data_quality_gate');
  assert.equal(error.details.attentionModuleCount, 1);
  assert.match(apiErrorMessage(error), /数据绑定未达到效果图生成要求/);
});

test('staticPagePreviewGateErrorMessage can append module details when backend message is terse', () => {
  const error = buildApiError({
    code: 'static_page_preview_data_quality_gate',
    message: '效果图未入队。',
    details: {
      attentionModules: [{
        moduleId: 'trend',
        title: '订单趋势',
        chartDataFit: 'needs_sample_rows',
      }],
    },
  }, '请求失败', 400);

  assert.match(staticPagePreviewGateErrorMessage(error), /订单趋势/);
  assert.match(staticPagePreviewGateErrorMessage(error), /needs_sample_rows/);
});

test('staticPagePreviewGateErrorMessage handles final render data-quality gates', () => {
  const error = buildApiError({
    code: 'static_page_final_render_data_quality_gate',
    message: '最终页生成已拦截。',
    details: {
      attention_modules: [{
        module_id: 'risk',
        title: '风险提示',
        binding_quality_status: 'matched_field_candidate',
      }],
    },
  }, '请求失败', 400);

  assert.match(staticPagePreviewGateErrorMessage(error), /风险提示/);
  assert.match(staticPagePreviewGateErrorMessage(error), /matched_field_candidate/);
});

test('assistantRunFailureMessage separates provider, static page action, and restart failures', () => {
  const provider = buildApiError({
    code: 'assistant_run_provider_failed',
    message: 'upstream returned HTTP 500',
  }, '请求失败', 500);
  assert.match(assistantRunFailureMessage(provider), /模型供应商调用失败/);

  const staticPage = buildApiError({
    code: 'active_assistant_run_required',
    message: 'active assistant run is required',
  }, '请求失败', 400);
  assert.match(assistantRunFailureMessage(staticPage), /静态页动作链被后端拒绝/);

  const restart = buildApiError('Bad gateway', 'Bad gateway', 502);
  assert.match(assistantRunFailureMessage(restart), /服务可能正在重启/);
});
