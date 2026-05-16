const ASSISTANT_RUN_PROGRESS_LIMIT = 8;
const ASSISTANT_RUN_TRACE_LIMIT = 6;

export function limitAssistantRunText(value, maxLength = 80) {
  const text = String(value || '').trim();
  if (!text) {
    return '';
  }
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

function assistantRunDiagnosticText(value, maxLength = 80) {
  if (value === null || value === undefined) {
    return '';
  }
  if (['string', 'number', 'boolean'].includes(typeof value)) {
    return limitAssistantRunText(value, maxLength);
  }
  if (typeof value !== 'object') {
    return '';
  }
  const candidate = value.status
    || value.comparison_status
    || value.kind
    || value.reason
    || value.blocked_by;
  return ['string', 'number', 'boolean'].includes(typeof candidate)
    ? limitAssistantRunText(candidate, maxLength)
    : '';
}

export function sanitizeAssistantRunTrailStep(step) {
  if (!step || typeof step !== 'object') {
    return null;
  }
  const label = limitAssistantRunText(step.label || step.react_action || step.status, 44);
  if (!label) {
    return null;
  }
  return {
    label,
    status: limitAssistantRunText(step.status || 'completed', 24),
    message: limitAssistantRunText(step.safe_message || step.message || step.hint || '', 72),
    suppliedCount: Number.isFinite(Number(step.supplied_count)) ? Number(step.supplied_count) : null,
    detailTargetCount: Number.isFinite(Number(step.detail_target_count)) ? Number(step.detail_target_count) : null,
    returnedCount: Number.isFinite(Number(step.returned_count ?? step.item_count)) ? Number(step.returned_count ?? step.item_count) : null,
    deniedCount: Number.isFinite(Number(step.denied_count)) ? Number(step.denied_count) : null,
    stepCount: Number.isFinite(Number(step.step_count)) ? Number(step.step_count) : null,
    reactStep: Number.isFinite(Number(step.react_step)) ? Number(step.react_step) : null,
  };
}

export function sanitizeAssistantRunTraceStep(step) {
  if (!step || typeof step !== 'object') {
    return null;
  }
  const actionType = limitAssistantRunText(step.action_type || 'react_action', 40);
  if (!actionType) {
    return null;
  }
  return {
    actionType,
    status: limitAssistantRunText(step.status || 'unknown', 24),
    message: limitAssistantRunText(step.safe_message || '', 72),
    deniedCount: Number.isFinite(Number(step.denied_count)) ? Number(step.denied_count) : 0,
    returnedCount: Number.isFinite(Number(step.returned_count)) ? Number(step.returned_count) : 0,
    detailTargetCount: Number.isFinite(Number(step.detail_target_count)) ? Number(step.detail_target_count) : 0,
    durationMs: Number.isFinite(Number(step.duration_ms)) ? Number(step.duration_ms) : null,
  };
}

export function assistantRunCodexReadinessFromDiagnostics(diagnostics) {
  const promotionGate = diagnostics?.codex_executor?.promotion_gate || {};
  const readinessChecks = promotionGate?.readiness_checks || {};
  if (!readinessChecks || typeof readinessChecks !== 'object' || !Object.keys(readinessChecks).length) {
    return null;
  }
  const labels = {
    shadow_gate: 'Shadow',
    host_validation: 'Host',
    model_gateway: 'Model',
  };
  const checks = ['shadow_gate', 'host_validation', 'model_gateway']
    .map((key) => {
      const check = readinessChecks[key] || {};
      const status = limitAssistantRunText(check.status || 'unknown', 28);
      return {
        key,
        label: labels[key],
        ready: check.ready === true,
        status,
      };
    });
  return {
    status: limitAssistantRunText(promotionGate.status || 'unknown', 36),
    blockedBy: assistantRunDiagnosticText(promotionGate.blocked_by, 36),
    nextStep: assistantRunDiagnosticText(promotionGate.next_step, 54),
    allReady: readinessChecks.all_ready === true,
    checks,
  };
}

export function assistantRunProviderUsageFromDiagnostics(diagnostics) {
  const providerUsage = diagnostics?.provider_usage || {};
  const summary = providerUsage.summary || {};
  const recentEvents = Array.isArray(providerUsage.recent_events)
    ? providerUsage.recent_events
    : [];
  const requestCount = Number(summary.request_count);
  const failedRequestCount = Number(summary.failed_request_count);
  const inputTokens = Number(summary.input_tokens);
  const outputTokens = Number(summary.output_tokens);
  const totalTokens = Number(summary.total_tokens);
  const latestEvent = [...recentEvents]
    .reverse()
    .find((event) => event && typeof event === 'object') || null;
  const safeRequestCount = Number.isFinite(requestCount) ? requestCount : 0;

  if (safeRequestCount <= 0 && !latestEvent) {
    return null;
  }

  return {
    requestCount: safeRequestCount,
    failedRequestCount: Number.isFinite(failedRequestCount) ? failedRequestCount : 0,
    inputTokens: Number.isFinite(inputTokens) ? inputTokens : 0,
    outputTokens: Number.isFinite(outputTokens) ? outputTokens : 0,
    totalTokens: Number.isFinite(totalTokens) ? totalTokens : 0,
    lastRequestId: limitAssistantRunText(summary.last_request_id || latestEvent?.request_id || '', 32),
    lastProvider: limitAssistantRunText(latestEvent?.provider || '', 28),
    lastModel: limitAssistantRunText(latestEvent?.model || '', 36),
    lastStatus: limitAssistantRunText(latestEvent?.status || '', 24),
  };
}

export function buildAssistantRunProgress(response, continued = false) {
  const run = response?.run || {};
  const runtime = response?.runtime || run.runtime || {};
  const trail = Array.isArray(response?.execution_trail)
    ? response.execution_trail
    : Array.isArray(run.execution_trail)
      ? run.execution_trail
      : [];
  const trace = Array.isArray(runtime?.react_trace?.steps) ? runtime.react_trace.steps : [];
  const steps = trail
    .map(sanitizeAssistantRunTrailStep)
    .filter(Boolean)
    .slice(-ASSISTANT_RUN_PROGRESS_LIMIT);
  const traceSteps = trace
    .map(sanitizeAssistantRunTraceStep)
    .filter(Boolean)
    .slice(-ASSISTANT_RUN_TRACE_LIMIT);
  const codexReadiness = assistantRunCodexReadinessFromDiagnostics(response?.diagnostics);
  const providerUsage = assistantRunProviderUsageFromDiagnostics(response?.diagnostics);

  if (!steps.length && !traceSteps.length && !codexReadiness && !providerUsage) {
    return null;
  }
  return {
    runId: response?.assistant_run_id || run.id || '',
    continued,
    steps,
    traceSteps,
    codexReadiness,
    providerUsage,
  };
}
