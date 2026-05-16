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

function assistantRunDiagnosticNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function assistantRunDiagnosticField(source, fieldNames) {
  if (!source || typeof source !== 'object') {
    return undefined;
  }
  return fieldNames
    .map((fieldName) => source[fieldName])
    .find((value) => value !== undefined && value !== null);
}

function assistantRunDiagnosticCount(source, fieldNames) {
  return assistantRunDiagnosticNumber(assistantRunDiagnosticField(source, fieldNames));
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
    ? providerUsage.recent_events.filter((event) => event && typeof event === 'object')
    : [];
  const requestCount = Number(summary.request_count);
  const failedRequestCount = Number(summary.failed_request_count);
  const inputTokens = Number(summary.input_tokens);
  const outputTokens = Number(summary.output_tokens);
  const totalTokens = Number(summary.total_tokens);
  const latestEvent = recentEvents.at(-1) || null;
  const safeRequestCount = Number.isFinite(requestCount) ? requestCount : recentEvents.length;
  const safeFailedRequestCount = Number.isFinite(failedRequestCount)
    ? failedRequestCount
    : recentEvents.filter((event) => event.status === 'failed').length;
  const inferredInputTokens = recentEvents.reduce((total, event) => total + (Number(event.input_tokens) || 0), 0);
  const inferredOutputTokens = recentEvents.reduce((total, event) => total + (Number(event.output_tokens) || 0), 0);
  const inferredTotalTokens = recentEvents.reduce((total, event) => total + (Number(event.total_tokens) || 0), 0);

  if (safeRequestCount <= 0 && !latestEvent) {
    return null;
  }

  return {
    requestCount: safeRequestCount,
    failedRequestCount: safeFailedRequestCount,
    inputTokens: Number.isFinite(inputTokens) ? inputTokens : inferredInputTokens,
    outputTokens: Number.isFinite(outputTokens) ? outputTokens : inferredOutputTokens,
    totalTokens: Number.isFinite(totalTokens) ? totalTokens : inferredTotalTokens,
    lastRequestId: limitAssistantRunText(summary.last_request_id || latestEvent?.request_id || '', 32),
    lastProvider: limitAssistantRunText(latestEvent?.provider || '', 28),
    lastModel: limitAssistantRunText(latestEvent?.model || '', 36),
    lastStatus: limitAssistantRunText(latestEvent?.status || '', 24),
  };
}

export function assistantRunCodexBudgetFromDiagnostics(diagnostics) {
  const latest = diagnostics?.codex_executor?.latest || {};
  const contextBudget = latest.context_budget || diagnostics?.codex_executor?.context_budget || {};
  const shimObservability = latest.provider_shim_observability
    || diagnostics?.codex_executor?.provider_shim_observability
    || {};
  const shimContextBudget = shimObservability.context_budget_report || {};
  const toolOutputBudget = shimObservability.tool_output_budget || {};
  const budgetPressure = limitAssistantRunText(
    contextBudget.budget_pressure || shimContextBudget.budget_pressure || '',
    28,
  );
  const estimatedPromptChars = assistantRunDiagnosticNumber(
    contextBudget.estimated_prompt_chars ?? shimContextBudget.estimated_prompt_chars,
  );
  const maxPromptChars = assistantRunDiagnosticNumber(
    contextBudget.max_prompt_chars ?? shimContextBudget.max_prompt_chars,
  );
  const itemCount = assistantRunDiagnosticNumber(
    contextBudget.item_count ?? shimContextBudget.item_count,
  );
  const trimmedItemCount = assistantRunDiagnosticNumber(
    contextBudget.trimmed_item_count ?? shimContextBudget.trimmed_item_count,
  );
  const trimmedOutputCount = assistantRunDiagnosticNumber(toolOutputBudget.trimmed_output_count);
  const preservedEvidenceRefCount = assistantRunDiagnosticNumber(
    toolOutputBudget.preserved_evidence_ref_count,
  );
  const largestOutputChars = assistantRunDiagnosticNumber(toolOutputBudget.largest_output_chars);

  if (
    !budgetPressure
    && estimatedPromptChars === null
    && maxPromptChars === null
    && itemCount === null
    && trimmedItemCount === null
    && trimmedOutputCount === null
    && preservedEvidenceRefCount === null
    && largestOutputChars === null
  ) {
    return null;
  }

  return {
    budgetPressure,
    estimatedPromptChars,
    maxPromptChars,
    itemCount,
    trimmedItemCount,
    trimmedOutputCount,
    preservedEvidenceRefCount,
    largestOutputChars,
  };
}

export function assistantRunCodexLivenessFromDiagnostics(diagnostics) {
  const latest = diagnostics?.codex_executor?.latest || {};
  const shimObservability = latest.provider_shim_observability
    || diagnostics?.codex_executor?.provider_shim_observability
    || {};
  const livenessEvents = shimObservability.liveness_events || latest.liveness_events || {};
  const latestEvent = livenessEvents.latest && typeof livenessEvents.latest === 'object'
    ? livenessEvents.latest
    : {};
  const eventCount = assistantRunDiagnosticNumber(livenessEvents.event_count);
  const retryCount = assistantRunDiagnosticNumber(latestEvent.retry_count);
  const eventType = limitAssistantRunText(latestEvent.event_type || '', 40);
  const status = limitAssistantRunText(latestEvent.status || '', 28);
  const action = limitAssistantRunText(latestEvent.action || '', 32);
  const occurredAt = limitAssistantRunText(latestEvent.occurred_at || '', 40);
  const hasNote = latestEvent.has_note === true;

  if (
    (eventCount === null || eventCount <= 0)
    && retryCount === null
    && !eventType
    && !status
    && !action
    && !occurredAt
    && !hasNote
  ) {
    return null;
  }

  return {
    eventCount: eventCount ?? 0,
    eventType,
    status,
    retryCount,
    action,
    occurredAt,
    hasNote,
  };
}

export function assistantRunCodexModelGatewayFromDiagnostics(diagnostics) {
  const latestGateway = diagnostics?.codex_executor?.latest?.model_gateway;
  const fallbackGateway = diagnostics?.codex_executor?.model_gateway;
  const gateway = latestGateway && typeof latestGateway === 'object'
    ? latestGateway
    : fallbackGateway && typeof fallbackGateway === 'object'
      ? fallbackGateway
      : {};
  const gate = diagnostics?.codex_executor?.model_gateway_gate || {};
  const hasGateway = Object.keys(gateway).length > 0;
  const hasGate = gate && typeof gate === 'object' && Object.keys(gate).length > 0;
  if (!hasGateway && !hasGate) {
    return null;
  }

  const selectedModel = gateway.selected_model && typeof gateway.selected_model === 'object'
    ? gateway.selected_model
    : {};
  const codexSurface = gateway.codex_surface && typeof gateway.codex_surface === 'object'
    ? gateway.codex_surface
    : gate.codex_surface && typeof gate.codex_surface === 'object'
      ? gate.codex_surface
      : {};
  const capabilityManifest = gateway.capability_manifest && typeof gateway.capability_manifest === 'object'
    ? gateway.capability_manifest
    : {};
  const profileAvailable = gate.profile_available ?? gateway.profile_available;
  const authConfigured = typeof gateway.auth_configured === 'boolean'
    ? gateway.auth_configured
    : null;
  const codexCompatible = codexSurface.codex_compatible === true
    || capabilityManifest.codex_compatible === true;
  const jsonActionsSupported = codexSurface.json_actions_supported === true
    || capabilityManifest.json_mode === true;
  const toolCallsSupported = codexSurface.tool_calls_supported === true
    || capabilityManifest.tool_calling === true;
  const capabilities = [
    codexCompatible ? 'codex' : '',
    jsonActionsSupported ? 'json' : '',
    toolCallsSupported ? 'tools' : '',
  ].filter(Boolean);

  return {
    status: limitAssistantRunText(gate.status || gateway.profile_status || '', 32),
    readyForPromotion: gate.ready_for_promotion_review === true,
    lane: limitAssistantRunText(gateway.lane || gate.lane || '', 36),
    profileAvailable: profileAvailable === true,
    profileId: limitAssistantRunText(gateway.profile_id || gate.profile_id || '', 36),
    provider: limitAssistantRunText(selectedModel.provider || gateway.provider_id || '', 28),
    model: limitAssistantRunText(selectedModel.model || gateway.model_id || '', 36),
    wireApi: limitAssistantRunText(gateway.wire_api || codexSurface.wire_api || '', 32),
    authConfigured,
    codexCompatible,
    jsonActionsSupported,
    toolCallsSupported,
    capabilities,
    realExecutionAllowed: gateway.codex_real_execution_allowed === true,
    realExecutionBlockReason: limitAssistantRunText(codexSurface.real_execution_block_reason || '', 36),
    secretsRedacted: gateway.secrets_redacted !== false,
    rawProviderPayloadsAllowed: gateway.raw_provider_payloads_allowed === true,
  };
}

export function assistantRunSupplyQualityFromDiagnostics(diagnostics, evidenceState = null) {
  const latest = diagnostics?.codex_executor?.latest || {};
  const latestTrailSupply = Array.isArray(latest.execution_trail)
    ? latest.execution_trail
      .map((step) => step?.supply_quality)
      .find((supplyQuality) => supplyQuality && typeof supplyQuality === 'object')
    : null;
  const source = latest.supply_quality && typeof latest.supply_quality === 'object'
    ? latest.supply_quality
    : diagnostics?.codex_executor?.supply_quality && typeof diagnostics.codex_executor.supply_quality === 'object'
      ? diagnostics.codex_executor.supply_quality
      : latestTrailSupply
        ? latestTrailSupply
        : evidenceState?.supply_quality && typeof evidenceState.supply_quality === 'object'
          ? evidenceState.supply_quality
          : {};
  const contextBudget = latest.context_budget || diagnostics?.codex_executor?.context_budget || {};
  const status = limitAssistantRunText(source.status || evidenceState?.status || '', 28);
  const suppliedItemCount = assistantRunDiagnosticCount(source, ['suppliedItemCount', 'supplied_item_count'])
    ?? assistantRunDiagnosticCount(contextBudget, ['evidence_item_count']);
  const selectedDatasetCount = assistantRunDiagnosticCount(source, ['selectedDatasetCount', 'selected_dataset_count'])
    ?? assistantRunDiagnosticCount(contextBudget, ['selected_dataset_count']);
  const conversationMemoryItemCount = assistantRunDiagnosticCount(source, ['conversationMemoryItemCount', 'conversation_memory_item_count'])
    ?? assistantRunDiagnosticCount(contextBudget, ['hidden_memory_item_count']);
  const indexedEvidenceCount = assistantRunDiagnosticCount(source, ['indexedEvidenceCount', 'indexed_evidence_count']);
  const fallbackChunkCount = assistantRunDiagnosticCount(source, ['fallbackChunkCount', 'fallback_chunk_count']);
  const citationLocatorCount = assistantRunDiagnosticCount(source, ['citationLocatorCount', 'citation_locator_count']);
  const mediaContextCount = assistantRunDiagnosticCount(source, ['mediaContextCount', 'media_context_count']);
  const detailTargetCount = assistantRunDiagnosticCount(source, ['detailTargetCount', 'detail_target_count']);
  const limit = assistantRunDiagnosticCount(source, ['limit']);
  const supplyRequested = typeof source.supplyRequested === 'boolean'
    ? source.supplyRequested
    : typeof source.supply_requested === 'boolean'
      ? source.supply_requested
      : null;
  const qualityFirst = typeof source.qualityFirst === 'boolean'
    ? source.qualityFirst
    : typeof source.quality_first === 'boolean'
      ? source.quality_first
      : null;

  if (
    !status
    && suppliedItemCount === null
    && selectedDatasetCount === null
    && conversationMemoryItemCount === null
    && indexedEvidenceCount === null
    && fallbackChunkCount === null
    && citationLocatorCount === null
    && mediaContextCount === null
    && detailTargetCount === null
    && limit === null
    && supplyRequested === null
    && qualityFirst === null
  ) {
    return null;
  }

  return {
    status,
    suppliedItemCount,
    selectedDatasetCount,
    conversationMemoryItemCount,
    indexedEvidenceCount,
    fallbackChunkCount,
    citationLocatorCount,
    mediaContextCount,
    detailTargetCount,
    limit,
    supplyRequested,
    qualityFirst,
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
  const diagnostics = response?.diagnostics || run.diagnostics || {};
  const evidenceState = response?.evidence_state || run.evidence_state || null;
  const codexReadiness = assistantRunCodexReadinessFromDiagnostics(diagnostics);
  const providerUsage = assistantRunProviderUsageFromDiagnostics(diagnostics);
  const codexBudget = assistantRunCodexBudgetFromDiagnostics(diagnostics);
  const codexLiveness = assistantRunCodexLivenessFromDiagnostics(diagnostics);
  const codexModelGateway = assistantRunCodexModelGatewayFromDiagnostics(diagnostics);
  const supplyQuality = assistantRunSupplyQualityFromDiagnostics(diagnostics, evidenceState);

  if (
    !steps.length
    && !traceSteps.length
    && !codexReadiness
    && !providerUsage
    && !codexBudget
    && !codexLiveness
    && !codexModelGateway
    && !supplyQuality
  ) {
    return null;
  }
  return {
    runId: response?.assistant_run_id || run.id || '',
    continued,
    steps,
    traceSteps,
    codexReadiness,
    providerUsage,
    codexBudget,
    codexLiveness,
    codexModelGateway,
    supplyQuality,
  };
}
