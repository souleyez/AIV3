export function buildApiError(payload, fallbackMessage = '请求失败', status = null) {
  const message = apiPayloadMessage(payload, fallbackMessage);
  const error = new Error(message);
  error.name = 'ApiError';
  error.status = Number.isFinite(Number(status)) ? Number(status) : null;
  if (payload && typeof payload === 'object') {
    error.code = payload.code || payload.payload?.code || '';
    error.details = payload.details || payload.payload?.details || null;
    error.payload = payload;
  } else {
    error.code = '';
    error.details = null;
    error.payload = payload;
  }
  return error;
}

export function apiErrorMessage(error, fallbackMessage = '请求失败') {
  if (!error) return fallbackMessage;
  if (error instanceof Error && error.message) return error.message;
  return apiPayloadMessage(error, fallbackMessage);
}

export function staticPagePreviewGateErrorMessage(error, fallbackMessage = '请求失败') {
  const message = apiErrorMessage(error, fallbackMessage);
  if (!STATIC_PAGE_DATA_QUALITY_GATE_CODES.has(apiErrorCode(error))) {
    return message;
  }

  const details = apiErrorDetails(error);
  const modules = details?.attentionModules || details?.attention_modules || [];
  if (!Array.isArray(modules) || !modules.length) {
    return message;
  }

  const labels = modules.slice(0, 3).map(previewGateModuleLabel).filter(Boolean).join('、');
  if (!labels || message.includes(labels)) {
    return message;
  }
  return `${message} 需处理模块：${labels}。`;
}

export function assistantRunFailureMessage(error, fallbackMessage = '本轮没有生成回复') {
  const code = apiErrorCode(error);
  const status = Number(error?.status || error?.payload?.status || 0);
  const message = apiErrorMessage(error, fallbackMessage);

  if (ASSISTANT_RUN_PROVIDER_FAILURE_CODES.has(code)) {
    return `模型供应商调用失败，本轮没有生成回复。错误：${message}。`;
  }
  if (ASSISTANT_RUN_STATIC_PAGE_ACTION_CODES.has(code)) {
    return `静态页动作链被后端拒绝，本轮没有完成页面生成。错误：${message}。`;
  }
  if (status === 502 || status === 503 || status === 504 || code === 'assistant_run_join_failed') {
    return `DataMax 服务可能正在重启或网关暂不可达，本轮没有生成回复。错误：${message}。`;
  }
  return `AssistantRun 执行失败，本轮没有生成回复。错误：${message}。`;
}

export function assistantRunErrorRunId(error) {
  const details = apiErrorDetails(error);
  return details?.assistant_run_id || details?.assistantRunId || '';
}

const STATIC_PAGE_DATA_QUALITY_GATE_CODES = new Set([
  'static_page_preview_data_quality_gate',
  'static_page_final_render_data_quality_gate',
]);

const ASSISTANT_RUN_PROVIDER_FAILURE_CODES = new Set([
  'assistant_run_provider_failed',
  'assistant_run_continue_provider_failed',
]);

const ASSISTANT_RUN_STATIC_PAGE_ACTION_CODES = new Set([
  'active_assistant_run_required',
  'current_static_page_draft_required',
  'current_static_page_draft_run_mismatch',
  'static_page_preview_not_confirmed',
  'static_page_preview_stale',
  'static_page_image_job_mismatch',
]);

function apiPayloadMessage(payload, fallbackMessage) {
  if (typeof payload === 'string') return payload || fallbackMessage;
  if (!payload || typeof payload !== 'object') return fallbackMessage;
  return payload.message || payload.payload?.message || payload.error || fallbackMessage;
}

function apiErrorCode(error) {
  return error?.code || error?.payload?.code || error?.payload?.payload?.code || '';
}

function apiErrorDetails(error) {
  return error?.details || error?.payload?.details || error?.payload?.payload?.details || null;
}

function previewGateModuleLabel(module = {}) {
  const title = module.title || module.moduleTitle || module.moduleId || module.module_id || '未命名模块';
  const marker = module.chartDataFit || module.chart_data_fit || module.bindingQualityStatus || module.binding_quality_status || module.status || '';
  return marker ? `${title}（${marker}）` : String(title);
}
