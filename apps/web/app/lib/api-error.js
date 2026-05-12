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
  if (apiErrorCode(error) !== 'static_page_preview_data_quality_gate') {
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
