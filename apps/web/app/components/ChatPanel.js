'use client';

import { useEffect, useRef } from 'react';
import { formatDateTime, formatRelativeTime, formatSnakeCaseLabel, truncateText } from '../lib/formatters';
import HtmlArtifactViewer from './artifacts/HtmlArtifactViewer';
import StaticPageAssistantNotice from './static-page/StaticPageAssistantNotice';
import StaticPagePlanningPanel from './static-page/StaticPagePlanningPanel';

const SERVICE_LANE_LABELS = {
  material_service: '资料服务',
  report_service: '报告服务',
};

const CONTINUATION_LABELS = {
  idle: '可继续',
  needs_user_confirmation: '等待确认',
  ready_for_host_action: '待宿主动作',
  in_progress: '进行中',
  completed: '已完成',
};

const REPORT_ENTRY_LABELS = {
  not_applicable: '保持资料服务',
  confirmation_required: '待确认报告入口',
  confirmed: '已进入报告服务',
};

const STATIC_PAGE_PRIMARY_ACTION_LABEL = '效果图——生成页面';

const RUNTIME_PHASES = [
  {
    key: 'provider',
    title: 'Provider',
    field: 'provider_status',
    labels: {
      pending: '请求中',
      responded: '已响应',
      failed: '失败',
    },
  },
  {
    key: 'stream',
    title: 'Stream',
    field: 'stream_status',
    labels: {
      not_requested: '未请求',
      pending: '进行中',
      completed: '已完成',
      failed: '失败',
    },
  },
  {
    key: 'tool-loop',
    title: 'Tool loop',
    field: 'tool_loop_status',
    labels: {
      not_requested: '未请求',
      pending: '等待工具',
      completed: '已收口',
      failed: '失败',
    },
  },
  {
    key: 'artifact',
    title: 'Artifact',
    field: 'artifact_commit_status',
    labels: {
      not_ready: '未就绪',
      pending: '待落库',
      failed: '失败',
      completed: '已落库',
    },
  },
];

function renderParagraphs(content) {
  const displayContent = stripThinkingBlocks(content);
  const parts = displayContent
    .split(/\n{2,}/)
    .map((part) => part.trim())
    .filter(Boolean);

  if (!parts.length) {
    return <p className="message-paragraph">暂无正文。</p>;
  }

  return parts.map((part, index) => (
    <p className="message-paragraph" key={`${part.slice(0, 24)}-${index}`}>
      {part}
    </p>
  ));
}

function stripThinkingBlocks(content) {
  const raw = String(content || '');
  const withoutThinking = raw.replace(/<think>[\s\S]*?<\/think>/gi, '').trim();
  return withoutThinking || raw;
}

function buildMessageChips(message) {
  const chips = [];
  const modelFacing = message.model_facing;
  const evidenceCount = (message.message_manifest_view?.output?.sections || []).reduce(
    (total, section) => total + (section.retrieval_evidence_ids || []).length,
    0,
  );

  if (modelFacing?.service_lane) {
    chips.push({
      tone: 'neutral',
      label: SERVICE_LANE_LABELS[modelFacing.service_lane] || modelFacing.service_lane,
    });
  }

  if (modelFacing?.continuation_state) {
    chips.push({
      tone: 'neutral',
      label: CONTINUATION_LABELS[modelFacing.continuation_state] || modelFacing.continuation_state,
    });
  }

  if (message.llm_invocations?.length) {
    chips.push({ tone: 'blue', label: `LLM ${message.llm_invocations.length}` });
  }

  if (message.tool_executions?.length) {
    chips.push({ tone: 'blue', label: `工具 ${message.tool_executions.length}` });
  }

  if (evidenceCount) {
    chips.push({ tone: 'green', label: `证据 ${evidenceCount}` });
  }

  return chips;
}

function runtimeTone(value) {
  if (value === 'failed') return 'danger';
  if (value === 'pending' || value === 'not_ready' || value === 'retrying' || value === 'stalled') return 'warn';
  if (value === 'completed' || value === 'responded' || value === 'recovered' || value === 'ok') return 'green';
  return 'neutral';
}

function supplyQualityTone(value) {
  if (value === 'grounded' || value === 'supplied' || value === 'ready') return 'green';
  if (value === 'missing' || value === 'degraded' || value === 'failed') return 'danger';
  if (value === 'partial' || value === 'fallback' || value === 'attention') return 'warn';
  return 'neutral';
}

function hostValidationTone(summary) {
  if (!summary) return 'neutral';
  if (summary.status === 'validated' && summary.validationRequirementsMet) return 'green';
  if (summary.failedCount || summary.guardFailedCount || summary.status === 'failed') return 'danger';
  if (summary.pendingCount || summary.status === 'pending' || summary.status === 'not_run') return 'warn';
  return 'neutral';
}

function transportPolicyTone(policy) {
  if (!policy) return 'neutral';
  if (policy.downgraded || policy.realTransportFeatureGateEnabled === false || policy.realTransportPromotionReviewApproved === false) return 'warn';
  if (policy.realTransportRequested && policy.codexMutationAllowed) return 'green';
  return 'neutral';
}

function renderRuntimePhaseRail(turn) {
  if (!turn) {
    return null;
  }

  return (
    <div className="runtime-phase-rail" aria-label="运行时阶段">
      {RUNTIME_PHASES.map((phase) => {
        const value = turn[phase.field];
        const label = phase.labels[value] || formatSnakeCaseLabel(value);
        return (
          <div className={`runtime-phase ${runtimeTone(value)}`} key={phase.key}>
            <span>{phase.title}</span>
            <strong>{label}</strong>
          </div>
        );
      })}
      {turn.finish_reason ? (
        <div className="runtime-phase neutral">
          <span>Finish</span>
          <strong>{formatSnakeCaseLabel(turn.finish_reason)}</strong>
        </div>
      ) : null}
    </div>
  );
}

function RuntimeProviderFailure({ failure }) {
  if (!failure) {
    return null;
  }

  return (
    <div className="runtime-provider-failure" role="note" aria-label="provider failure">
      <strong>Provider 失败</strong>
      <span>
        {formatSnakeCaseLabel(failure.kind)}
        {failure.message ? ` · ${failure.message}` : ''}
      </span>
    </div>
  );
}

function SessionRuntimeSummary({ turn }) {
  if (!turn) {
    return null;
  }

  return (
    <div className="session-runtime-summary">
      <div className="session-runtime-head">
        <div>
          <span>最新运行时阶段</span>
          <strong>
            {formatSnakeCaseLabel(turn.status)} · {truncateText(turn.turn_id, 18)}
          </strong>
        </div>
        {turn.provider_request_id ? (
          <code>{truncateText(turn.provider_request_id, 22)}</code>
        ) : null}
      </div>
      {renderRuntimePhaseRail(turn)}
      <RuntimeProviderFailure failure={turn.provider_failure} />
    </div>
  );
}

function ReportEntryGate({ reportEntry, busy, onResolve }) {
  if (reportEntry?.state !== 'confirmation_required') {
    return null;
  }

  return (
    <div className="report-entry-gate">
      <div className="report-entry-head">
        <strong>检测到报告入口分流</strong>
        <span>模型认为可以进入报告服务；宿主只提供入口，不替模型编排正文。</span>
      </div>
      <div className="report-entry-body">
        <div className="report-entry-copy">
          <div>
            <span>建议标题</span>
            <strong>{reportEntry.suggested_title || '未给出'}</strong>
          </div>
          <div>
            <span>建议目标</span>
            <strong>{reportEntry.suggested_objective || '未给出'}</strong>
          </div>
        </div>
        <div className="report-entry-actions">
          <button type="button" className="ghost-btn" disabled={busy} onClick={() => onResolve('stay_material_service')}>
            {busy ? '处理中...' : '保持资料服务'}
          </button>
          <button type="button" className="primary-btn" disabled={busy} onClick={() => onResolve('enter_report_service')}>
            {busy ? '处理中...' : '进入报告服务'}
          </button>
        </div>
      </div>
    </div>
  );
}

function AssistantContextStrip({ dataset, selectedDatasets = [], startupBriefing, scopePlan }) {
  const candidates = Array.isArray(scopePlan?.candidates) ? scopePlan.candidates : [];
  const datasetCandidates = candidates.filter((candidate) => candidate.type === 'dataset');
  const memoryCandidate = candidates.find((candidate) => candidate.type === 'conversation_memory');
  const intentLabel = scopePlan?.intentLabel || '';
  const preferDetail = Boolean(scopePlan?.supplyStrategy?.preferDetail);
  const recommendedActions = Array.isArray(scopePlan?.supplyStrategy?.recommendedActions)
    ? scopePlan.supplyStrategy.recommendedActions
    : [];
  const mediaCandidate = datasetCandidates.find((candidate) => (
    candidate.materialHints || candidate.material_hints || []
  ).some((hint) => ['audio_video', 'transcript_possible', 'keyframe_ocr_possible'].includes(hint)));
  const selectedScope = selectedDatasets.length ? selectedDatasets : dataset ? [dataset] : [];
  const selectedScopeLabel = selectedScope.map((item) => item.title || item.key).filter(Boolean).join('、');

  return (
    <div className="assistant-context-strip">
      <div className="assistant-context-main">
        <span>{selectedScope.length ? '当前供料' : '普通聊天'}</span>
        <strong>{selectedScopeLabel || '未选数据集'}</strong>
        <p>
          可见数据集 {startupBriefing?.visibleDatasetCount || 0} 个，
          文档 {startupBriefing?.visibleDocumentCount || 0} 份。
          {scopePlan?.hint ? ` ${scopePlan.hint}` : ' 暂未命中具体供料范围。'}
        </p>
      </div>
      <div className="assistant-context-chips" aria-label="模型范围判断">
        {intentLabel ? <span className="message-chip neutral">意图 {intentLabel}</span> : null}
        {datasetCandidates.map((candidate) => {
          const candidateDocumentCount = candidate.documentCount || candidate.document_count;
          return (
            <span className="message-chip blue" key={`${candidate.type}-${candidate.id}`}>
              预选 {candidate.label}
              {candidateDocumentCount ? ` · ${candidateDocumentCount}文档` : ''}
            </span>
          );
        })}
        {mediaCandidate ? <span className="message-chip green">媒体资料</span> : null}
        {preferDetail ? <span className="message-chip green">深度供料</span> : null}
        {memoryCandidate ? <span className="message-chip neutral">参考本轮对话</span> : null}
        {recommendedActions.slice(0, 2).map((action) => (
          <span className="message-chip neutral" key={action}>
            可用 {formatRecommendedActionLabel(action)}
          </span>
        ))}
        {!datasetCandidates.length && !memoryCandidate ? <span className="message-chip neutral">不强行检索</span> : null}
      </div>
    </div>
  );
}

function formatRecommendedActionLabel(action) {
  const labels = {
    'retrieval.search': '资料检索',
    'retrieval.read_detail': '全文细读',
    'media.detail': '媒体细节',
    'static_page.plan': '静态页规划',
    'static_page.update_draft': '修改静态页',
    'report.plan': '报表规划',
    'ordinary_chat.answer': '普通回答',
  };
  return labels[action] || action;
}

function AssistantRunProgressPanel({ progress }) {
  if (
    !progress
    || (
      !progress.steps?.length
      && !progress.traceSteps?.length
      && !progress.codexReadiness
      && !progress.providerUsage
      && !progress.codexBudget
      && !progress.codexLiveness
      && !progress.codexModelGateway
      && !progress.codexHostValidation
      && !progress.codexTransportPolicy
      && !progress.supplyQuality
    )
  ) {
    return null;
  }

  const title = progress.continued ? '连续执行进度' : '本轮执行进度';
  const codexReadiness = progress.codexReadiness;
  const providerUsage = progress.providerUsage;
  const codexBudget = progress.codexBudget;
  const codexLiveness = progress.codexLiveness;
  const codexModelGateway = progress.codexModelGateway;
  const codexHostValidation = progress.codexHostValidation;
  const codexTransportPolicy = progress.codexTransportPolicy;
  const supplyQuality = progress.supplyQuality;
  return (
    <div className="assistant-run-progress-panel" aria-label="AssistantRun 安全进度">
      <div className="assistant-run-progress-head">
        <div>
          <span>{title}</span>
          <strong>{progress.runId ? truncateText(progress.runId, 18) : 'AssistantRun'}</strong>
        </div>
        {codexReadiness ? (
          <em>{codexReadiness.allReady ? 'Codex 三门已就绪' : 'Codex 三门观测中'}</em>
        ) : codexTransportPolicy ? (
          <em>Transport {codexTransportPolicy.downgraded ? '降级中' : '观测中'}</em>
        ) : codexHostValidation ? (
          <em>Host {formatSnakeCaseLabel(codexHostValidation.status || 'observed')}</em>
        ) : codexModelGateway ? (
          <em>Gateway {formatSnakeCaseLabel(codexModelGateway.status || 'observed')}</em>
        ) : supplyQuality ? (
          <em>供料 {formatSnakeCaseLabel(supplyQuality.status || 'observed')}</em>
        ) : providerUsage ? (
          <em>模型请求 {providerUsage.requestCount}</em>
        ) : codexBudget?.budgetPressure ? (
          <em>预算 {formatSnakeCaseLabel(codexBudget.budgetPressure)}</em>
        ) : codexLiveness?.status ? (
          <em>Liveness {formatSnakeCaseLabel(codexLiveness.status)}</em>
        ) : progress.traceSteps?.length ? <em>{progress.traceSteps.length} 个模型动作</em> : null}
      </div>
      {codexReadiness ? (
        <div className="assistant-run-readiness-row" aria-label="Codex promotion readiness">
          {codexReadiness.checks.map((check) => (
            <div className={`assistant-run-readiness-pill ${check.ready ? 'green' : 'warn'}`} key={check.key}>
              <span>{check.label}</span>
              <strong>{check.ready ? 'ready' : formatSnakeCaseLabel(check.status)}</strong>
            </div>
          ))}
          <div className={`assistant-run-readiness-pill ${codexReadiness.allReady ? 'green' : 'warn'}`}>
            <span>Gate</span>
            <strong>{formatSnakeCaseLabel(codexReadiness.status)}</strong>
          </div>
        </div>
      ) : null}
      {codexReadiness?.blockedBy || codexReadiness?.nextStep ? (
        <div className="assistant-run-trace-row" aria-label="Codex promotion gate detail">
          {codexReadiness.blockedBy ? (
            <span className="message-chip warn">
              阻塞 {formatSnakeCaseLabel(codexReadiness.blockedBy)}
            </span>
          ) : null}
          {codexReadiness.nextStep ? (
            <span className="message-chip neutral">
              下一步 {formatSnakeCaseLabel(codexReadiness.nextStep)}
            </span>
          ) : null}
        </div>
      ) : null}
      {codexTransportPolicy ? (
        <div className="assistant-run-trace-row" aria-label="Codex transport policy summary">
          <span className={`message-chip ${transportPolicyTone(codexTransportPolicy)}`}>
            Transport {codexTransportPolicy.downgraded ? '降级' : '观测'}
          </span>
          {codexTransportPolicy.requestedTransport ? (
            <span className="message-chip neutral">
              请求 {formatSnakeCaseLabel(codexTransportPolicy.requestedTransport)}
            </span>
          ) : null}
          {codexTransportPolicy.effectiveTransport ? (
            <span className="message-chip neutral">
              生效 {formatSnakeCaseLabel(codexTransportPolicy.effectiveTransport)}
            </span>
          ) : null}
          {codexTransportPolicy.downgradeReason ? (
            <span className="message-chip warn">
              原因 {formatSnakeCaseLabel(codexTransportPolicy.downgradeReason)}
            </span>
          ) : null}
          {codexTransportPolicy.realTransportRequested ? (
            <span className="message-chip neutral">请求真实 Transport</span>
          ) : null}
          {codexTransportPolicy.realTransportFeatureGateEnabled === false ? (
            <span className="message-chip warn">真实开关未开</span>
          ) : codexTransportPolicy.realTransportFeatureGateEnabled === true ? (
            <span className="message-chip green">真实开关已开</span>
          ) : null}
          {codexTransportPolicy.realTransportPromotionReviewApproved === false ? (
            <span className="message-chip warn">推广复核未批</span>
          ) : codexTransportPolicy.realTransportPromotionReviewApproved === true ? (
            <span className="message-chip green">推广复核已批</span>
          ) : null}
          {codexTransportPolicy.hostValidationRequired ? (
            <span className="message-chip neutral">需宿主验证</span>
          ) : null}
          {codexTransportPolicy.directExecutionAuthoritative ? (
            <span className="message-chip neutral">Direct 仍权威</span>
          ) : null}
          {codexTransportPolicy.codexMutationAllowed === false ? (
            <span className="message-chip neutral">Codex 变更关闭</span>
          ) : null}
          {codexTransportPolicy.queueAllowed === false ? (
            <span className="message-chip neutral">队列提交关闭</span>
          ) : null}
          {codexTransportPolicy.nextStep ? (
            <span className="message-chip neutral">
              下一步 {formatSnakeCaseLabel(codexTransportPolicy.nextStep)}
            </span>
          ) : null}
        </div>
      ) : null}
      {codexHostValidation ? (
        <div className="assistant-run-trace-row" aria-label="Codex host validation summary">
          <span className={`message-chip ${hostValidationTone(codexHostValidation)}`}>
            Host {codexHostValidation.status ? formatSnakeCaseLabel(codexHostValidation.status) : 'Observed'}
          </span>
          {codexHostValidation.latestHostKind ? (
            <span className="message-chip neutral">
              Host {formatSnakeCaseLabel(codexHostValidation.latestHostKind)}
            </span>
          ) : null}
          {codexHostValidation.latestProfileKind ? (
            <span className="message-chip neutral">
              Profile {formatSnakeCaseLabel(codexHostValidation.latestProfileKind)}
            </span>
          ) : null}
          {codexHostValidation.latestMode ? (
            <span className="message-chip neutral">
              Mode {formatSnakeCaseLabel(codexHostValidation.latestMode)}
            </span>
          ) : null}
          {codexHostValidation.completedCount !== null ? (
            <span className={codexHostValidation.completedCount ? 'message-chip green' : 'message-chip neutral'}>
              完成 {codexHostValidation.completedCount}
            </span>
          ) : null}
          {codexHostValidation.pendingCount ? (
            <span className="message-chip warn">待验证 {codexHostValidation.pendingCount}</span>
          ) : null}
          {codexHostValidation.failedCount ? (
            <span className="message-chip danger">失败 {codexHostValidation.failedCount}</span>
          ) : null}
          {codexHostValidation.guardFailedCount ? (
            <span className="message-chip danger">守卫失败 {codexHostValidation.guardFailedCount}</span>
          ) : null}
          {codexHostValidation.validationRequirementsMet ? (
            <span className="message-chip green">验证要求已满足</span>
          ) : null}
          {codexHostValidation.hostKindAllowed === false ? (
            <span className="message-chip danger">宿主不允许</span>
          ) : null}
          {codexHostValidation.workspaceConfigured === false ? (
            <span className="message-chip warn">任务工作区未配置</span>
          ) : null}
          {codexHostValidation.promptRedacted ? (
            <span className="message-chip green">Prompt 已脱敏</span>
          ) : null}
          {codexHostValidation.taskMemoryIsolated ? (
            <span className="message-chip green">任务记忆隔离</span>
          ) : null}
          {codexHostValidation.taskMemorySpaceConfigured ? (
            <span className="message-chip green">任务记忆空间</span>
          ) : null}
          {codexHostValidation.codexMutationAllowed === false ? (
            <span className="message-chip neutral">变更仍关闭</span>
          ) : null}
          {codexHostValidation.nextStep ? (
            <span className="message-chip neutral">
              下一步 {formatSnakeCaseLabel(codexHostValidation.nextStep)}
            </span>
          ) : null}
        </div>
      ) : null}
      {codexModelGateway ? (
        <div className="assistant-run-trace-row" aria-label="Codex model gateway summary">
          <span className={`message-chip ${codexModelGateway.readyForPromotion ? 'green' : 'neutral'}`}>
            Gateway {codexModelGateway.status ? formatSnakeCaseLabel(codexModelGateway.status) : 'Observed'}
          </span>
          {codexModelGateway.profileId ? (
            <span className="message-chip neutral">Profile {truncateText(codexModelGateway.profileId, 24)}</span>
          ) : null}
          {codexModelGateway.provider ? (
            <span className="message-chip neutral">Provider {codexModelGateway.provider}</span>
          ) : null}
          {codexModelGateway.model ? (
            <span className="message-chip neutral">Model {truncateText(codexModelGateway.model, 24)}</span>
          ) : null}
          {codexModelGateway.wireApi ? (
            <span className="message-chip neutral">Wire {formatSnakeCaseLabel(codexModelGateway.wireApi)}</span>
          ) : null}
          {codexModelGateway.capabilities?.length ? (
            <span className="message-chip green">Surface {codexModelGateway.capabilities.join(' / ')}</span>
          ) : null}
          {codexModelGateway.authConfigured !== null ? (
            <span className={`message-chip ${codexModelGateway.authConfigured ? 'green' : 'warn'}`}>
              Auth {codexModelGateway.authConfigured ? 'ready' : 'missing'}
            </span>
          ) : null}
          {codexModelGateway.realExecutionAllowed ? (
            <span className="message-chip green">Real exec allowed</span>
          ) : codexModelGateway.realExecutionBlockReason ? (
            <span className="message-chip neutral">
              Real exec {formatSnakeCaseLabel(codexModelGateway.realExecutionBlockReason)}
            </span>
          ) : null}
          {codexModelGateway.rawProviderPayloadsAllowed ? (
            <span className="message-chip danger">Raw payload enabled</span>
          ) : null}
        </div>
      ) : null}
      {supplyQuality ? (
        <div className="assistant-run-trace-row" aria-label="AssistantRun supply quality summary">
          <span className={`message-chip ${supplyQualityTone(supplyQuality.status)}`}>
            供料质量 {supplyQuality.status ? formatSnakeCaseLabel(supplyQuality.status) : 'Observed'}
          </span>
          {supplyQuality.suppliedItemCount !== null ? (
            <span className={supplyQuality.suppliedItemCount ? 'message-chip green' : 'message-chip warn'}>
              可引用 {supplyQuality.suppliedItemCount}
            </span>
          ) : null}
          {supplyQuality.indexedEvidenceCount ? (
            <span className="message-chip green">索引证据 {supplyQuality.indexedEvidenceCount}</span>
          ) : null}
          {supplyQuality.citationLocatorCount ? (
            <span className="message-chip green">来源定位 {supplyQuality.citationLocatorCount}</span>
          ) : null}
          {supplyQuality.fallbackChunkCount ? (
            <span className="message-chip warn">Fallback {supplyQuality.fallbackChunkCount}</span>
          ) : null}
          {supplyQuality.detailTargetCount ? (
            <span className="message-chip warn">建议深读 {supplyQuality.detailTargetCount}</span>
          ) : null}
          {supplyQuality.mediaContextCount ? (
            <span className="message-chip green">媒体上下文 {supplyQuality.mediaContextCount}</span>
          ) : null}
          {supplyQuality.conversationMemoryItemCount ? (
            <span className="message-chip neutral">记忆 {supplyQuality.conversationMemoryItemCount}</span>
          ) : null}
          {supplyQuality.selectedDatasetCount ? (
            <span className="message-chip neutral">数据集 {supplyQuality.selectedDatasetCount}</span>
          ) : null}
          {supplyQuality.supplyRequested === false ? (
            <span className="message-chip neutral">普通聊天未强制供料</span>
          ) : null}
          {supplyQuality.qualityFirst ? (
            <span className="message-chip neutral">质量优先</span>
          ) : null}
        </div>
      ) : null}
      {codexBudget ? (
        <div className="assistant-run-trace-row" aria-label="Codex context budget summary">
          {codexBudget.budgetPressure ? (
            <span className={`message-chip ${['attention', 'high', 'critical', 'over_limit'].includes(codexBudget.budgetPressure) ? 'warn' : 'neutral'}`}>
              预算压力 {formatSnakeCaseLabel(codexBudget.budgetPressure)}
            </span>
          ) : null}
          {codexBudget.estimatedPromptChars !== null ? (
            <span className="message-chip neutral">
              Prompt {codexBudget.estimatedPromptChars}
              {codexBudget.maxPromptChars !== null ? ` / ${codexBudget.maxPromptChars}` : ''}
            </span>
          ) : null}
          {codexBudget.itemCount !== null ? (
            <span className="message-chip neutral">上下文项 {codexBudget.itemCount}</span>
          ) : null}
          {codexBudget.trimmedItemCount ? (
            <span className="message-chip warn">上下文裁剪 {codexBudget.trimmedItemCount}</span>
          ) : null}
          {codexBudget.trimmedOutputCount ? (
            <span className="message-chip warn">工具输出裁剪 {codexBudget.trimmedOutputCount}</span>
          ) : null}
          {codexBudget.preservedEvidenceRefCount ? (
            <span className="message-chip green">证据引用保留 {codexBudget.preservedEvidenceRefCount}</span>
          ) : null}
          {codexBudget.largestOutputChars ? (
            <span className="message-chip neutral">最大输出 {codexBudget.largestOutputChars}</span>
          ) : null}
        </div>
      ) : null}
      {codexLiveness ? (
        <div className="assistant-run-trace-row" aria-label="Codex liveness retry summary">
          <span className={`message-chip ${runtimeTone(codexLiveness.status)}`}>
            Liveness {codexLiveness.status ? formatSnakeCaseLabel(codexLiveness.status) : 'Observed'}
          </span>
          {codexLiveness.eventCount ? (
            <span className="message-chip neutral">事件 {codexLiveness.eventCount}</span>
          ) : null}
          {codexLiveness.eventType ? (
            <span className="message-chip neutral">类型 {formatSnakeCaseLabel(codexLiveness.eventType)}</span>
          ) : null}
          {codexLiveness.retryCount !== null ? (
            <span className={codexLiveness.retryCount ? 'message-chip warn' : 'message-chip neutral'}>
              Retry {codexLiveness.retryCount}
            </span>
          ) : null}
          {codexLiveness.action ? (
            <span className="message-chip neutral">动作 {formatSnakeCaseLabel(codexLiveness.action)}</span>
          ) : null}
          {codexLiveness.hasNote ? (
            <span className="message-chip neutral">内部备注已隐藏</span>
          ) : null}
        </div>
      ) : null}
      {providerUsage ? (
        <div className="assistant-run-trace-row" aria-label="模型请求摘要">
          <span className={`message-chip ${providerUsage.failedRequestCount ? 'warn' : 'green'}`}>
            模型请求 {providerUsage.requestCount}
            {providerUsage.failedRequestCount ? ` · 失败 ${providerUsage.failedRequestCount}` : ''}
          </span>
          {providerUsage.totalTokens ? (
            <span className="message-chip neutral">Token {providerUsage.totalTokens}</span>
          ) : null}
          {providerUsage.lastStatus ? (
            <span className={`message-chip ${runtimeTone(providerUsage.lastStatus)}`}>
              Status {formatSnakeCaseLabel(providerUsage.lastStatus)}
            </span>
          ) : null}
          {providerUsage.lastProvider ? (
            <span className="message-chip neutral">Provider {providerUsage.lastProvider}</span>
          ) : null}
          {providerUsage.lastModel ? (
            <span className="message-chip neutral">Model {truncateText(providerUsage.lastModel, 22)}</span>
          ) : null}
          {providerUsage.lastRequestId ? (
            <span className="message-chip neutral">Req {truncateText(providerUsage.lastRequestId, 18)}</span>
          ) : null}
        </div>
      ) : null}
      {progress.steps?.length ? (
        <div className="assistant-run-progress-steps">
          {progress.steps.map((step, index) => (
            <div className={`assistant-run-progress-step ${step.status || 'completed'}`} key={`${step.label}-${index}`}>
              <strong>{step.label}</strong>
              <span>
                {step.message || formatSnakeCaseLabel(step.status)}
                {step.suppliedCount !== null ? ` · 供料 ${step.suppliedCount}` : ''}
                {step.detailTargetCount ? ` · 建议深读 ${step.detailTargetCount}` : ''}
                {step.returnedCount !== null ? ` · 返回 ${step.returnedCount}` : ''}
                {step.deniedCount ? ` · 拒绝 ${step.deniedCount}` : ''}
              </span>
            </div>
          ))}
        </div>
      ) : null}
      {progress.traceSteps?.length ? (
        <div className="assistant-run-trace-row">
          {progress.traceSteps.map((step, index) => (
            <span className={`message-chip ${runtimeTone(step.status)}`} key={`${step.actionType}-${index}`}>
              {formatSnakeCaseLabel(step.actionType)}
              {step.returnedCount ? ` · ${step.returnedCount}` : ''}
              {step.detailTargetCount ? ` · 深读 ${step.detailTargetCount}` : ''}
              {step.durationMs !== null ? ` · ${step.durationMs}ms` : ''}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function staticPageJobStatus(draft) {
  if (draft?.previewContract?.status === 'stale' || draft?.imageJob?.status === 'stale') {
    return 'stale';
  }
  return draft?.imageJob?.status || draft?.previewContract?.status || 'idle';
}

function staticPageActionState(draft) {
  const jobStatus = staticPageJobStatus(draft);
  const finalStatus = draft?.finalPage?.status || '';
  const stale = draft?.previewContract?.status === 'stale' || jobStatus === 'stale';

  if (!draft) {
    return {
      disabled: false,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: '先创建静态页规划，再进入效果图和页面生成。',
      workspaceLabel: '效果图',
    };
  }

  if (['queued', 'rendering'].includes(finalStatus)) {
    return {
      disabled: true,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: '最终静态页正在后台制作，完成后会进入右侧结果区。',
      workspaceLabel: '生成中',
    };
  }

  if (finalStatus === 'rendered' || draft.status === 'rendered') {
    return {
      disabled: false,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: '页面已生成；如需调整，回到模块编辑后重新生成效果图。',
      workspaceLabel: '效果图',
    };
  }

  if (['queued', 'running'].includes(jobStatus)) {
    const queueText = draft.imageJob?.queuePosition
      ? `当前前方约 ${draft.imageJob.queuePosition} 个任务。`
      : '正在等待远程生图资源。';
    return {
      disabled: true,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: `${draft.imageJob?.queueMessage || '资源正在排队，可以联系商务开通高级用户跳过等待。'} ${queueText}`,
      workspaceLabel: '排队中',
    };
  }

  if (jobStatus === 'preview_ready' || draft.status === 'preview_ready') {
    return {
      disabled: false,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: '效果图已回来。满意就继续生成页面；不满意回到模块编辑后再出图。',
      workspaceLabel: '生成页面',
    };
  }

  if (draft.status === 'effect_confirmed' || draft.previewContract?.status === 'confirmed') {
    return {
      disabled: false,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: '效果图已确认，下一步按这个视觉合同制作静态页。',
      workspaceLabel: '生成页面',
    };
  }

  if (jobStatus === 'failed') {
    return {
      disabled: false,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: draft.imageJob?.queueMessage || '效果图生成失败，可以重新发起。',
      workspaceLabel: '效果图',
    };
  }

  if (stale) {
    return {
      disabled: false,
      label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
      helper: '模块已经改过，上一张效果图失效，需要重新发起效果图。',
      workspaceLabel: '效果图',
    };
  }

  return {
    disabled: false,
    label: STATIC_PAGE_PRIMARY_ACTION_LABEL,
    helper: '模块编辑完成后，用这一个按钮先出效果图；效果图满意后同一个按钮继续生成页面。',
    workspaceLabel: '效果图',
  };
}

function shouldOfferStaticPageWorkspaceEntry(draft) {
  if (!draft) return false;
  const jobStatus = staticPageJobStatus(draft);
  const finalStatus = draft?.finalPage?.status || '';
  return !draft.previewImage
    && !draft.imageJob?.id
    && !finalStatus
    && (draft.status === 'planning' || jobStatus === 'idle');
}

export default function ChatPanel({
  dataset,
  selectedDatasets = [],
  session,
  messages,
  messageLoading,
  input,
  onInputChange,
  onSubmit,
  onStartNewConversation,
  submitting,
  reportEntryBusy,
  onResolveReportEntry,
  panelClassName = '',
  staticPageDraft = null,
  onStartStaticPageDraft,
  onApplyStaticPageOperation,
  onStaticPagePrimaryAction,
  staticPageActionBusy = false,
  onApplyStaticPagePrompt,
  onRetryWorkflowExecution,
  onCancelWorkflowExecution,
  onRefreshStaticPageDraft,
  onOpenStaticPageBuilder,
  onCloseStaticPageDraft,
  showStaticPageWorkspace = true,
  startupBriefing,
  scopePlan,
  assistantRunProgress,
  htmlArtifact = null,
  onCloseHtmlArtifact,
  onHtmlArtifactEvent,
  onUploadClick,
  uploadingFiles = false,
}) {
  const reportEntry = session?.session_manifest_view?.report_entry || null;
  const latestTurn = session?.session_manifest_view?.last_turn || null;
  const showingHtmlArtifactWorkspace = Boolean(htmlArtifact);
  const showingStaticPageWorkspace = showStaticPageWorkspace && Boolean(staticPageDraft);
  const staticPageAction = staticPageActionState(staticPageDraft);
  const staticPageEntryOnly = shouldOfferStaticPageWorkspaceEntry(staticPageDraft);
  const selectedScope = selectedDatasets.length ? selectedDatasets : dataset ? [dataset] : [];
  const selectedScopeLabel = selectedScope.map((item) => item.title || item.key).filter(Boolean).join('、');
  const chatMessagesRef = useRef(null);
  const chatEndRef = useRef(null);
  const staticPageNotice = staticPageDraft && !showingHtmlArtifactWorkspace && !showingStaticPageWorkspace ? (
    <StaticPageAssistantNotice
      draft={staticPageDraft}
      onOpenBuilder={onOpenStaticPageBuilder}
      simpleEntry={staticPageEntryOnly}
      actionLabel={staticPageEntryOnly ? '点此开始生成静态页' : staticPageAction.label}
      actionHelper={
        staticPageEntryOnly
          ? ''
          : staticPageAction.helper
      }
      actionDisabled={staticPageEntryOnly ? false : (staticPageAction.disabled || staticPageActionBusy)}
      onPrimaryAction={() => {
        if (staticPageEntryOnly) {
          onOpenStaticPageBuilder?.();
          return;
        }
        onStaticPagePrimaryAction?.();
      }}
      secondaryLabel={staticPageEntryOnly ? '' : '不满意，回到模块编辑'}
    />
  ) : null;

  useEffect(() => {
    if (showingHtmlArtifactWorkspace || showingStaticPageWorkspace) {
      return undefined;
    }
    const frame = window.requestAnimationFrame(() => {
      if (chatMessagesRef.current) {
        chatMessagesRef.current.scrollTop = chatMessagesRef.current.scrollHeight;
      }
      chatEndRef.current?.scrollIntoView({ block: 'end', inline: 'nearest' });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [
    messageLoading,
    messages.length,
    staticPageDraft?.id,
    staticPageDraft?.imageJob?.status,
    staticPageEntryOnly,
    showingHtmlArtifactWorkspace,
    showingStaticPageWorkspace,
  ]);

  return (
    <section className={`chat-panel card ${panelClassName}`.trim()}>
      <div className="panel-header chat-header">
        <div>
          <h3>{session ? session.title : selectedScope.length ? `普通聊天 · ${selectedScope.length} 个供料范围` : '普通聊天 · 未选数据集'}</h3>
          <p>
            {session
              ? `会话 ${truncateText(session.id, 16)} · 最后更新 ${formatRelativeTime(session.updated_at)}`
              : selectedScope.length
                ? `已选 ${selectedScopeLabel} 作为优先供料范围；不会切换对话。`
                : '可以直接提问；系统会先做范围判断，命中资料意图时再预选相关数据集。'}
          </p>
        </div>
        {session ? (
          <div className="header-pill-row">
            <span className="badge">
              {REPORT_ENTRY_LABELS[reportEntry?.state] || REPORT_ENTRY_LABELS.not_applicable}
            </span>
            {session.model_facing?.recommended_tool_key ? (
              <span className="badge badge-soft">
                推荐工具 {session.model_facing.recommended_tool_key}
              </span>
            ) : null}
            <button
              type="button"
              className="ghost-btn compact-action-btn"
              onClick={onStartNewConversation}
              disabled={submitting}
            >
              新会话
            </button>
          </div>
        ) : selectedScope.length ? (
          <div className="header-pill-row">
            <button
              type="button"
              className="ghost-btn compact-action-btn"
              onClick={onStartNewConversation}
              disabled={submitting}
            >
              新会话
            </button>
          </div>
        ) : null}
      </div>

      <ReportEntryGate
        reportEntry={reportEntry}
        busy={reportEntryBusy}
        onResolve={onResolveReportEntry}
      />

      <SessionRuntimeSummary turn={latestTurn} />

      {showingHtmlArtifactWorkspace ? (
        <div className="chat-static-page-workspace">
          <div className="chat-static-page-head">
            <div>
              <span>当前工作台</span>
              <strong>HTML 产物预览</strong>
            </div>
            <button type="button" className="ghost-btn compact-action-btn" onClick={onCloseHtmlArtifact}>
              返回聊天记录
            </button>
          </div>
          <HtmlArtifactViewer
            artifact={htmlArtifact}
            onArtifactEvent={onHtmlArtifactEvent}
          />
        </div>
      ) : showingStaticPageWorkspace ? (
        <div className="chat-static-page-workspace">
          <div className="chat-static-page-head">
            <div>
              <span>当前工作台</span>
              <strong>{staticPageDraft?.status === 'rendered' ? '静态页成品' : '静态页规划'}</strong>
            </div>
            <div className="chat-static-page-head-actions">
              <button type="button" className="ghost-btn compact-action-btn" onClick={onCloseStaticPageDraft}>
                返回聊天记录
              </button>
              <button
                type="button"
                className="primary-btn compact-action-btn"
                onClick={() => onStaticPagePrimaryAction?.()}
                disabled={staticPageAction.disabled || staticPageActionBusy}
              >
                {staticPageAction.workspaceLabel}
              </button>
            </div>
          </div>
          <StaticPagePlanningPanel
            draft={staticPageDraft}
            onStartDraft={onStartStaticPageDraft}
            onApplyOperation={onApplyStaticPageOperation}
            onApplyPrompt={onApplyStaticPagePrompt}
            onRetryWorkflow={onRetryWorkflowExecution}
            onCancelWorkflow={onCancelWorkflowExecution}
            onRefreshDraft={onRefreshStaticPageDraft}
          />
        </div>
      ) : (
        <div className="chat-messages" ref={chatMessagesRef}>
          {messageLoading ? (
          <div className="chat-empty-state loading-state">
            <span className="loading-dot"></span>
            <span className="loading-dot"></span>
            <span className="loading-dot"></span>
          </div>
        ) : (
          <>
            {messages.length ? (
              messages.map((message) => {
                const assistant = message.role === 'assistant';
                const chips = buildMessageChips(message);
                return (
                  <article className={`message ${assistant ? 'assistant' : 'user'}`} key={message.id}>
                    {assistant ? <div className="avatar">AI</div> : null}
                    <div className={`bubble ${assistant ? '' : 'user-bubble'}`}>
                      <div className="message-body">{renderParagraphs(message.content)}</div>
                      {chips.length ? (
                        <div className="message-chip-row">
                          {chips.map((chip) => (
                            <span className={`message-chip ${chip.tone}`} key={`${message.id}-${chip.label}`}>
                              {chip.label}
                            </span>
                          ))}
                        </div>
                      ) : null}
                      {assistant ? renderRuntimePhaseRail(message.message_manifest_view?.turn) : null}
                      {assistant ? (
                        <RuntimeProviderFailure failure={message.message_manifest_view?.turn?.provider_failure} />
                      ) : null}
                      <div className="message-meta">
                        {formatDateTime(message.created_at)}
                      </div>
                    </div>
                    {!assistant ? <div className="avatar user-avatar">U</div> : null}
                  </article>
                );
              })
            ) : (
              <div className="chat-empty-state">
                <h4>{selectedScope.length ? '按当前供料范围继续聊天' : '可以直接聊天'}</h4>
                <p>
                  {selectedScope.length
                    ? '输入问题仍在当前对话里进行，系统会优先从已选数据集供料，正文由模型自行回答。'
                    : '未选数据集时按普通模型聊天处理；如果问题命中资料范围，系统会在左侧预选相关数据集并优先供料。'}
                </p>
              </div>
            )}
            {staticPageNotice}
            <div ref={chatEndRef} className="chat-scroll-anchor" aria-hidden="true" />
          </>
        )}
        </div>
      )}

      <div className="chat-composer-wrap">
        <div className="composer-note">
          {session
            ? '当前输入会追加到已选会话；如需分开上下文，可点顶部对话名称切换或新建。'
            : selectedScope.length
              ? '已选数据集只作为供料范围；对话本身保持当前线程，模型可按意图检索和细读。'
              : '未选数据集时先普通聊天；系统只做供料范围判断，不替模型编排答案。'}
        </div>
        <div className="chat-input-row">
          <textarea
            value={input}
            onChange={(event) => onInputChange(event.target.value)}
            placeholder={
              selectedScope.length
                ? session
                  ? `继续追问 ${session.title}`
                  : `围绕 ${selectedScopeLabel} 提问，系统会优先供料`
                : '直接提问；系统会按意图预选资料范围'
            }
            disabled={submitting}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault();
                if (!submitting) {
                  onSubmit();
                }
              }
            }}
          />
          <div className="chat-action-stack" aria-label="对话动作">
            <button className="primary-btn send-btn" type="button" onClick={onSubmit} disabled={!input.trim() || submitting}>
              {submitting ? '发送中...' : '发送'}
            </button>
            <button
              className="ghost-btn upload-btn"
              type="button"
              onClick={onUploadClick}
              disabled={!onUploadClick || submitting}
              title={onUploadClick ? '上传文件并自动分类' : '上传分类接口待接入'}
            >
              {uploadingFiles ? '上传中...' : '上传'}
            </button>
            <button
              className="ghost-btn static-page-one-click-btn"
              type="button"
              onClick={() => onStartStaticPageDraft?.({ oneClick: false, openEditor: true })}
              disabled={submitting}
            >
              页面
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
