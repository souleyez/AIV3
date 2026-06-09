const CODEX_CUSTOMER_ARTIFACT_TYPE = 'codex_customer_artifact_bundle';
const CODEX_CUSTOMER_ARTIFACT_MANIFEST_TYPE = 'codex_customer_artifacts';
const CODEX_CUSTOMER_RESULT_SUMMARY_SCHEMA = 'v3.customer_codex_result_summary';
const CUSTOMER_CODEX_CAPABILITIES = new Set([
  'customer_complex_request',
  'customer_artifact_request',
  'generated_static_page_edit',
  'generated_static_page_publish',
  'v3_product_change_request',
]);
const CUSTOMER_CODEX_ROUTES = new Set(CUSTOMER_CODEX_CAPABILITIES);
const CUSTOMER_CODEX_ARTIFACT_CAPABILITIES = new Set([
  'customer_artifact_request',
  'generated_static_page_edit',
  'generated_static_page_publish',
]);
const CUSTOMER_CODEX_ARTIFACT_ROUTES = new Set(CUSTOMER_CODEX_ARTIFACT_CAPABILITIES);
const TERMINAL_CUSTOMER_CODEX_TASK_STATUSES = new Set([
  'completed',
  'failed',
  'cancelled',
  'rejected',
  'blocked',
]);

const CODEX_TASK_EVENT_NAMES = new Set([
  'assistant_run.codex_sidecar_queued',
  'assistant_run.codex_sidecar_preflight_rejected',
  'assistant_run.codex_sidecar_scope_blocked',
  'codex_host_task.exec_heartbeat',
  'codex_host_task.cloudflare_heartbeat',
  'codex_host_task.poll_retry',
  'codex_host_task.exec_fallback_started',
  'codex_host_task.dry_run_completed',
  'codex_host_task.plan_only_completed',
  'codex_host_task.exec_completed',
  'codex_host_task.completed',
  'codex_host_task.exec_failed',
  'codex_host_task.cancelled',
  'assistant_run.customer_artifact_request_artifacts_ready',
  'assistant_run.generated_static_page_edit_artifacts_ready',
]);

export function promptMayUseCustomerCodex(prompt) {
  return /^cc(?:$|[\s:：,，.。;；-])/i.test(String(prompt || '').trimStart());
}

function safeString(value, maxLength = 160) {
  return String(value || '').trim().replace(/\s+/g, ' ').slice(0, maxLength);
}

function resultTextLooksSensitive(value) {
  const lower = String(value || '').toLowerCase();
  return [
    'authorization',
    'bearer ',
    'api_key',
    'apikey',
    'access_token',
    'refresh_token',
    'private_key',
    'secret',
    'cookie',
    'database_url',
    'mysql://',
    'postgres://',
    'postgresql://',
    'mongodb://',
    '[redacted-log-line]',
    '/users/',
    '/srv/aiv3/repo',
    '/srv/aiv3/shared',
    '/private/var/',
    '\\.codex',
    '/.codex',
    '.env',
    'sk-',
  ].some((needle) => lower.includes(needle)) || lower.includes(':\\');
}

function safeResultText(value, maxLength = 180) {
  const text = safeString(value, maxLength);
  return text && !resultTextLooksSensitive(text) ? text : '';
}

function firstSafeResultString(source = {}, keys = [], maxLength = 180) {
  for (const key of keys) {
    const text = safeResultText(source[key], maxLength);
    if (text) return text;
  }
  return '';
}

function safeResultArray(source = {}, keys = [], maxItems = 4, maxLength = 180) {
  for (const key of keys) {
    const value = source[key];
    if (!Array.isArray(value)) continue;
    const items = value
      .map((item) => {
        if (typeof item === 'string') return safeResultText(item, maxLength);
        if (item && typeof item === 'object') {
          return firstSafeResultString(item, ['title', 'summary', 'message', 'action', 'text', 'description'], maxLength);
        }
        return '';
      })
      .filter(Boolean)
      .slice(0, maxItems);
    if (items.length) return items;
  }
  return [];
}

export function safeCodexCustomerArtifactPath(path) {
  const value = String(path || '').trim();
  if (
    !value
    || value.startsWith('/')
    || value.startsWith('~')
    || value.startsWith('\\\\')
    || value.includes('\\')
    || value.includes(':')
    || value.includes('\0')
  ) {
    return '';
  }
  const segments = value.split('/');
  if (segments.some((segment) => !segment || segment === '.' || segment === '..')) {
    return '';
  }
  const lower = value.toLowerCase();
  if (
    lower.includes('/srv/aiv3/repo')
    || lower.includes('/users/')
    || lower.includes('node_modules')
    || lower.includes('.git')
    || lower.includes('.env')
    || lower.endsWith('.env')
    || lower.includes('secret')
    || lower.includes('credential')
    || lower.includes('access_token')
    || lower.includes('refresh_token')
    || lower.includes('api_key')
    || lower.includes('apikey')
    || lower.includes('private_key')
    || lower.includes('password')
    || lower.includes('secrets')
    || lower.endsWith('.pem')
    || lower.endsWith('.key')
    || lower.endsWith('.p12')
    || lower.endsWith('.pfx')
    || lower.includes('id_rsa')
    || lower.includes('id_dsa')
    || lower.includes('id_ecdsa')
    || lower.includes('id_ed25519')
    || lower.includes('authorized_keys')
  ) {
    return '';
  }
  return value;
}

export function safeCodexCustomerArtifactPublicUrl(url) {
  const value = String(url || '').trim();
  if (
    !value
    || value.includes('\0')
    || value.includes('/generated-artifacts/pending-')
    || value.includes('/generated-artifacts/pending/')
    || value.endsWith('/generated-artifacts/pending')
  ) {
    return '';
  }
  if (value.startsWith('/generated-artifacts/')) {
    return value;
  }
  try {
    const parsed = new URL(value);
    const currentOrigin = typeof window !== 'undefined' ? window.location.origin : '';
    const allowedOrigin = parsed.origin === 'https://v3.elepcloud.com'
      || (currentOrigin && parsed.origin === currentOrigin);
    return allowedOrigin && parsed.pathname.startsWith('/generated-artifacts/')
      ? value
      : '';
  } catch {
    return '';
  }
}

function firstArray(...values) {
  return values.find((value) => Array.isArray(value)) || [];
}

function firstObject(...values) {
  return values.find((value) => value && typeof value === 'object' && !Array.isArray(value)) || {};
}

function safeCodexCapability(value) {
  const capability = safeString(value, 80);
  return CUSTOMER_CODEX_CAPABILITIES.has(capability) ? capability : '';
}

function safeCodexRoute(value, fallback = '') {
  const route = safeString(value, 80);
  if (CUSTOMER_CODEX_ROUTES.has(route)) return route;
  return CUSTOMER_CODEX_ROUTES.has(fallback) ? fallback : '';
}

function safeCodexArtifactCapability(value) {
  const capability = safeString(value, 80);
  return CUSTOMER_CODEX_ARTIFACT_CAPABILITIES.has(capability) ? capability : '';
}

function safeCodexArtifactRoute(value, fallback = '') {
  const route = safeString(value, 80);
  if (CUSTOMER_CODEX_ARTIFACT_ROUTES.has(route)) return route;
  return CUSTOMER_CODEX_ARTIFACT_ROUTES.has(fallback) ? fallback : '';
}

export function isTerminalCodexCustomerTaskStatus(status) {
  return TERMINAL_CUSTOMER_CODEX_TASK_STATUSES.has(safeString(status, 40).toLowerCase());
}

function safeWorkflowExecutionId(value) {
  const text = safeString(value, 80);
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(text)
    ? text
    : '';
}

function eventNameOf(event = {}) {
  return safeString(event.event_name || event.eventName || event.name, 120);
}

function eventPayloadOf(event = {}) {
  return firstObject(event.payload, event.data);
}

function safeCodexCustomerResultStatus(value) {
  const status = safeString(value, 40).toLowerCase();
  if (status === 'success' || status === 'succeeded' || status === 'done') return 'completed';
  if (status === 'completed' || status === 'needs_human' || status === 'failed') return status;
  return 'completed';
}

function normalizeCodexCustomerResultSummaryFromValue(value = {}, capability = '') {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const safety = firstObject(value.safety);
  if (
    value.raw_logs_exposed === true
    || value.rawLogsExposed === true
    || value.credentials_exposed === true
    || value.credentialsExposed === true
    || value.absolute_paths_exposed === true
    || value.absolutePathsExposed === true
    || value.prompt_exposed === true
    || value.promptExposed === true
    || value.secrets_exposed === true
    || value.secretsExposed === true
    || safety.raw_logs_exposed === true
    || safety.rawLogsExposed === true
    || safety.credentials_exposed === true
    || safety.credentialsExposed === true
    || safety.absolute_paths_exposed === true
    || safety.absolutePathsExposed === true
    || safety.prompt_exposed === true
    || safety.promptExposed === true
    || safety.secrets_exposed === true
    || safety.secretsExposed === true
  ) {
    return null;
  }
  if (
    value.schema
    && value.schema !== CODEX_CUSTOMER_RESULT_SUMMARY_SCHEMA
  ) {
    return null;
  }
  const summary = firstSafeResultString(value, ['summary', 'message', 'final_message', 'finalMessage', 'description'], 280);
  const findings = safeResultArray(value, ['findings', 'key_findings', 'keyFindings', 'insights'], 6, 180);
  const recommendedNextActions = safeResultArray(
    value,
    ['recommended_next_actions', 'recommendedNextActions', 'next_actions', 'nextActions', 'actions', 'recommendations'],
    5,
    180,
  );
  const warnings = safeResultArray(value, ['warnings', 'notes', 'caveats', 'risks'], 4, 160);
  if (!summary && !findings.length && !recommendedNextActions.length && !warnings.length) return null;
  return {
    schema: CODEX_CUSTOMER_RESULT_SUMMARY_SCHEMA,
    schemaVersion: 1,
    status: safeCodexCustomerResultStatus(value.status),
    title: firstSafeResultString(value, ['title', 'name', 'heading'], 80) || 'Codex 执行结果',
    summary: summary || 'Codex 已完成客户任务。',
    findings,
    recommendedNextActions,
    warnings,
    artifactIntent: value.artifact_intent === true || value.artifactIntent === true || capability !== 'customer_complex_request',
  };
}

function normalizeCodexCustomerResultSummary(payload = {}, capability = '') {
  const direct = firstObject(
    payload.customer_result_summary,
    payload.customerResultSummary,
    payload.result_summary,
    payload.resultSummary,
  );
  if (direct && Object.keys(direct).length) {
    const normalized = normalizeCodexCustomerResultSummaryFromValue(direct, capability);
    if (normalized) return normalized;
  }
  const result = firstObject(payload.result, payload.output);
  const nested = firstObject(
    result.customer_result_summary,
    result.customerResultSummary,
    result.result_summary,
    result.resultSummary,
  );
  return nested && Object.keys(nested).length
    ? normalizeCodexCustomerResultSummaryFromValue(nested, capability)
    : null;
}

function assistantRunEvents(response = {}) {
  const run = firstObject(response.run, response.assistant_run, response.assistantRun);
  return [
    ...firstArray(response.events, response.assistant_run_events, response.assistantRunEvents),
    ...firstArray(run.events, run.assistant_run_events, run.assistantRunEvents),
  ];
}

function normalizeSha256(value) {
  const text = safeString(value, 80);
  return /^[a-f0-9]{64}$/i.test(text) ? text.toLowerCase() : '';
}

function normalizeCodexCustomerArtifactFile(file = {}, fallbackPath = '') {
  const path = safeCodexCustomerArtifactPath(file.path || file.uri || fallbackPath);
  if (!path) return null;
  const publicUrl = safeCodexCustomerArtifactPublicUrl(file.public_url || file.publicUrl || file.url);
  const title = safeResultText(file.title || file.name || file.label, 120) || path;
  return {
    path,
    title,
    kind: safeString(file.kind || file.type || file.artifact_kind || 'file', 80),
    mimeType: safeString(file.mime_type || file.mimeType || file.content_type || '', 120),
    bytes: Number.isFinite(Number(file.bytes)) && Number(file.bytes) >= 0 ? Number(file.bytes) : 0,
    sha256: normalizeSha256(file.sha256),
    publicUrl,
    published: Boolean(publicUrl) && file.published !== false,
  };
}

function normalizeCodexCustomerArtifactFiles(customerArtifacts = {}, manifest = {}) {
  const refs = firstObject(manifest.refs, manifest.references);
  const refPaths = firstArray(refs.artifact_paths, refs.artifactPaths);
  const manifestFiles = firstArray(customerArtifacts.artifacts, customerArtifacts.files);
  const files = manifestFiles
    .map((file) => normalizeCodexCustomerArtifactFile(file))
    .filter(Boolean);
  refPaths.forEach((path) => {
    const safePath = safeCodexCustomerArtifactPath(path);
    if (safePath && !files.some((file) => file.path === safePath)) {
      files.push(normalizeCodexCustomerArtifactFile({ path: safePath }));
    }
  });
  return files;
}

export function normalizeCodexCustomerArtifactBundle(artifact = {}) {
  const manifest = firstObject(artifact.artifact_manifest, artifact.artifactManifest);
  const artifactType = safeString(
    artifact.artifact_type || artifact.artifactType || manifest.artifact_type || manifest.artifactType,
    80,
  );
  if (
    artifact.type !== CODEX_CUSTOMER_ARTIFACT_TYPE
    && artifactType !== CODEX_CUSTOMER_ARTIFACT_MANIFEST_TYPE
  ) {
    return null;
  }
  if (
    manifest.schema
    && (
      manifest.schema !== 'v3.output_artifact_manifest'
      || Number(manifest.schema_version || manifest.schemaVersion) !== 1
    )
  ) {
    return null;
  }
  const safety = firstObject(manifest.safety);
  if (safety.credentials_exposed === true || safety.credentialsExposed === true) return null;
  if (safety.raw_logs_exposed === true || safety.rawLogsExposed === true) return null;
  if (safety.absolute_paths_exposed === true || safety.absolutePathsExposed === true) return null;

  const customerArtifacts = firstObject(artifact.customer_artifacts, artifact.customerArtifacts);
  const files = normalizeCodexCustomerArtifactFiles(customerArtifacts, manifest);
  if (!files.length) return null;

  const refs = firstObject(manifest.refs, manifest.references);
  const workflowExecutionId = safeString(
    artifact.workflow_execution_id
      || artifact.workflowExecutionId
      || refs.workflow_execution_id
      || refs.workflowExecutionId,
    80,
  );
  const manifestPath = safeCodexCustomerArtifactPath(refs.manifest_path || refs.manifestPath || customerArtifacts.manifest_path || customerArtifacts.manifestPath);
  const primaryUrl = safeCodexCustomerArtifactPublicUrl(
    manifest.primary_url
      || manifest.primaryUrl
      || artifact.primary_url
      || artifact.primaryUrl
      || artifact.public_url
      || artifact.publicUrl
      || customerArtifacts.primary_url
      || customerArtifacts.primaryUrl
      || customerArtifacts.public_url
      || customerArtifacts.publicUrl,
  ) || files.find((file) => file.publicUrl)?.publicUrl || '';
  const published = Boolean(primaryUrl) && (
    safety.published === true
    || artifact.published === true
    || customerArtifacts.published === true
    || manifest.status === 'published'
    || artifact.status === 'published'
    || customerArtifacts.status === 'published'
  );
  const title = firstSafeResultString(
    {
      manifestTitle: manifest.title,
      customerTitle: customerArtifacts.title,
      artifactTitle: artifact.title,
    },
    ['manifestTitle', 'customerTitle', 'artifactTitle'],
    80,
  ) || (published ? 'Codex 客户产物' : 'Codex 待发布产物');
  const status = safeString(manifest.status || artifact.status || customerArtifacts.status || 'available', 40);
  const summary = firstSafeResultString(
    {
      customerSummary: customerArtifacts.summary,
      artifactSummary: artifact.summary,
    },
    ['customerSummary', 'artifactSummary'],
    140,
  ) || `${files.length} 个文件，等待 DataMax 发布校验`;
  const id = workflowExecutionId || `${title}:${files.map((file) => file.path).join('|')}`;
  const routeFromPayload = safeCodexArtifactRoute(artifact.route || customerArtifacts.route || manifest.route);
  const capability = safeCodexArtifactCapability(artifact.capability || customerArtifacts.capability) || routeFromPayload;
  const route = routeFromPayload || safeCodexArtifactRoute('', capability);

  return {
    id,
    title,
    status,
    statusLabel: published ? '已发布' : '待发布',
    summary,
    artifactType,
    artifactKind: safeString(artifact.artifact_kind || artifact.artifactKind || manifest.artifact_kind || manifest.artifactKind || 'customer_artifact_bundle', 80),
    capability,
    route,
    workflowExecutionId,
    manifestPath,
    primaryUrl: published ? primaryUrl : '',
    published,
    requiresPublishValidation: !published,
    files,
    createdAt: artifact.created_at || artifact.createdAt || '',
  };
}

function assistantRunOutputArtifacts(response = {}) {
  const run = firstObject(response.run, response.assistant_run, response.assistantRun);
  return [
    ...firstArray(response.output_artifacts, response.outputArtifacts),
    ...firstArray(run.output_artifacts, run.outputArtifacts),
  ];
}

export function normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse(response = {}) {
  const seen = new Set();
  return assistantRunOutputArtifacts(response)
    .map(normalizeCodexCustomerArtifactBundle)
    .filter(Boolean)
    .filter((bundle) => {
      const key = bundle.id || `${bundle.title}:${bundle.files.map((file) => file.path).join('|')}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
}

export function mergeCodexCustomerArtifactBundles(current = [], incoming = []) {
  const merged = [];
  const seen = new Set();
  [...incoming, ...current].forEach((bundle) => {
    if (!bundle) return;
    const key = bundle.id || `${bundle.title}:${(bundle.files || []).map((file) => file.path).join('|')}`;
    if (!key || seen.has(key)) return;
    seen.add(key);
    merged.push(bundle);
  });
  return merged.slice(0, 12);
}

function customerCodexTaskStatusForEvent(eventName, payload = {}) {
  if (eventName === 'assistant_run.codex_sidecar_queued') return 'queued';
  if (eventName === 'assistant_run.codex_sidecar_preflight_rejected') return 'rejected';
  if (eventName === 'assistant_run.codex_sidecar_scope_blocked') return 'blocked';
  if (eventName === 'codex_host_task.exec_heartbeat' || eventName === 'codex_host_task.cloudflare_heartbeat') return 'running';
  if (eventName === 'codex_host_task.poll_retry' || eventName === 'codex_host_task.exec_fallback_started') return 'waiting';
  if (eventName === 'codex_host_task.exec_failed') return 'failed';
  if (eventName === 'codex_host_task.cancelled') return 'cancelled';
  if (
    eventName === 'codex_host_task.dry_run_completed'
    || eventName === 'codex_host_task.plan_only_completed'
    || eventName === 'codex_host_task.exec_completed'
    || eventName === 'codex_host_task.completed'
    || eventName === 'assistant_run.customer_artifact_request_artifacts_ready'
    || eventName === 'assistant_run.generated_static_page_edit_artifacts_ready'
  ) {
    const status = safeString(payload.status, 40);
    return status === 'failed' || status === 'cancelled' ? status : 'completed';
  }
  return safeString(payload.status || 'active', 40);
}

function customerCodexTaskStatusLabel(status) {
  switch (status) {
    case 'queued':
      return '已排队';
    case 'running':
      return '执行中';
    case 'waiting':
      return '等待重试';
    case 'completed':
      return '已完成';
    case 'failed':
      return '失败';
    case 'cancelled':
      return '已取消';
    case 'rejected':
      return '未启用';
    case 'blocked':
      return '需人工审核';
    default:
      return '处理中';
  }
}

function customerCodexCapabilityTitle(capability) {
  switch (capability) {
    case 'customer_complex_request':
      return 'Codex 复杂任务';
    case 'customer_artifact_request':
      return 'Codex 产物任务';
    case 'generated_static_page_edit':
      return 'Codex 页面编辑';
    case 'generated_static_page_publish':
      return 'Codex 页面发布';
    case 'v3_product_change_request':
      return '需人工审核';
    default:
      return 'Codex 执行';
  }
}

function customerCodexTaskRoute(payload = {}, capability = '') {
  return safeCodexRoute(payload.route || payload.task_route || payload.taskRoute, capability);
}

function customerCodexArtifactReadyEventCapability(eventName, payload = {}) {
  if (eventName === 'assistant_run.generated_static_page_edit_artifacts_ready') {
    return 'generated_static_page_edit';
  }
  if (eventName === 'assistant_run.customer_artifact_request_artifacts_ready') {
    return safeCodexArtifactCapability(payload.capability)
      || safeCodexArtifactRoute(payload.route || payload.task_route || payload.taskRoute)
      || 'customer_artifact_request';
  }
  return '';
}

function customerCodexTaskSummary(eventName, payload = {}, status, capability, resultSummary = null) {
  const reason = safeResultText(payload.reason || payload.error || payload.failure_reason || payload.failureReason, 120);
  if (status === 'blocked' && reason === 'v3_product_change_not_customer_writable') {
    return '该请求涉及 V3 产品变更，需要平台管理员或开发人员审核。';
  }
  if (reason && (status === 'failed' || status === 'cancelled' || status === 'rejected' || status === 'blocked')) {
    return reason;
  }
  if (resultSummary?.summary && status === 'completed') {
    return resultSummary.summary;
  }
  if (eventName === 'assistant_run.codex_sidecar_queued') {
    return payload.main_answer_path_preserved === false
      ? '已转入 Codex Host 队列。'
      : '已转入 Codex Host 队列，主回答照常返回。';
  }
  if (eventName === 'assistant_run.codex_sidecar_scope_blocked') {
    return '该请求涉及 V3 产品变更，需要平台管理员或开发人员审核。';
  }
  if (eventName === 'codex_host_task.exec_heartbeat' || eventName === 'codex_host_task.cloudflare_heartbeat') {
    const elapsedMs = Number(payload.elapsed_ms ?? payload.elapsedMs);
    if (Number.isFinite(elapsedMs) && elapsedMs > 0) {
      return `Codex 已运行 ${Math.round(elapsedMs / 1000)} 秒。`;
    }
    return 'Codex 正在执行客户任务。';
  }
  if (eventName === 'codex_host_task.poll_retry') {
    return '远端 Codex 任务仍在运行，系统会继续轮询。';
  }
  if (status === 'completed') {
    return capability === 'customer_complex_request'
      ? 'Codex 已完成复杂任务处理，结果通过主回答或后续事件回传。'
      : 'Codex 已完成客户任务，产物会在校验后显示。';
  }
  return '客户 Codex 任务已进入 DataMax 受控执行链路。';
}

export function normalizeCodexCustomerTaskFromAssistantRunEvent(event = {}) {
  const eventName = eventNameOf(event);
  if (!CODEX_TASK_EVENT_NAMES.has(eventName)) return null;
  const payload = eventPayloadOf(event);
  const artifactReadyCapability = customerCodexArtifactReadyEventCapability(eventName, payload);
  const capability = artifactReadyCapability || safeCodexCapability(payload.capability);
  if (!capability) return null;
  const workflowExecutionId = safeWorkflowExecutionId(payload.workflow_execution_id || payload.workflowExecutionId);
  const status = customerCodexTaskStatusForEvent(eventName, payload);
  const statusLabel = customerCodexTaskStatusLabel(status);
  const title = customerCodexCapabilityTitle(capability);
  const route = artifactReadyCapability
    ? safeCodexArtifactRoute(payload.route || payload.task_route || payload.taskRoute, capability)
    : customerCodexTaskRoute(payload, capability);
  const resultSummary = normalizeCodexCustomerResultSummary(payload, capability);
  const createdAt = event.created_at || event.createdAt || payload.created_at || payload.createdAt || '';
  const id = workflowExecutionId || `${capability}:${eventName}:${event.sequence_no || event.sequenceNo || createdAt || 'latest'}`;
  return {
    id,
    title,
    capability,
    route,
    status,
    statusLabel,
    summary: customerCodexTaskSummary(eventName, payload, status, capability, resultSummary),
    resultSummary,
    workflowExecutionId,
    eventName,
    sequenceNo: Number(event.sequence_no ?? event.sequenceNo) || 0,
    createdAt,
    nonBlocking: payload.non_blocking !== false && payload.nonBlocking !== false,
    mainAnswerPathPreserved: payload.main_answer_path_preserved !== false && payload.mainAnswerPathPreserved !== false,
    permissionScope: safeResultText(payload.permission_scope || payload.permissionScope || '', 180),
    retryable: payload.retryable === true,
  };
}

export function normalizeCodexCustomerTasksFromAssistantRunResponse(response = {}) {
  const latestByKey = new Map();
  assistantRunEvents(response).forEach((event) => {
    const task = normalizeCodexCustomerTaskFromAssistantRunEvent(event);
    if (!task) return;
    const key = task.workflowExecutionId || task.id;
    latestByKey.set(key, task);
  });
  return Array.from(latestByKey.values()).sort((left, right) => {
    const leftSeq = Number(left.sequenceNo) || 0;
    const rightSeq = Number(right.sequenceNo) || 0;
    if (leftSeq !== rightSeq) return rightSeq - leftSeq;
    return String(right.createdAt || '').localeCompare(String(left.createdAt || ''));
  });
}

export function mergeCodexCustomerTasks(current = [], incoming = []) {
  const merged = [];
  const seen = new Set();
  [...incoming, ...current].forEach((task) => {
    if (!task) return;
    const key = task.workflowExecutionId || task.id;
    if (!key || seen.has(key)) return;
    seen.add(key);
    merged.push(task);
  });
  return merged.slice(0, 16);
}
