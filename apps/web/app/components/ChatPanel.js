'use client';

import { formatDateTime, formatRelativeTime, formatSnakeCaseLabel, truncateText } from '../lib/formatters';
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
  if (value === 'pending' || value === 'not_ready') return 'warn';
  if (value === 'completed' || value === 'responded') return 'green';
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

function AssistantContextStrip({ dataset, startupBriefing, scopePlan }) {
  const candidates = Array.isArray(scopePlan?.candidates) ? scopePlan.candidates : [];
  const datasetCandidates = candidates.filter((candidate) => candidate.type === 'dataset');
  const memoryCandidate = candidates.find((candidate) => candidate.type === 'conversation_memory');
  const intentLabel = scopePlan?.intentLabel || '';
  const preferDetail = Boolean(scopePlan?.supplyStrategy?.preferDetail);
  const mediaCandidate = datasetCandidates.find((candidate) => (
    candidate.materialHints || candidate.material_hints || []
  ).some((hint) => ['audio_video', 'transcript_possible', 'keyframe_ocr_possible'].includes(hint)));

  return (
    <div className="assistant-context-strip">
      <div className="assistant-context-main">
        <span>{dataset ? '当前供料' : '普通聊天'}</span>
        <strong>{dataset?.title || '未选数据集'}</strong>
        <p>
          可见数据集 {startupBriefing?.visibleDatasetCount || 0} 个，
          文档 {startupBriefing?.visibleDocumentCount || 0} 份。
          {scopePlan?.hint ? ` ${scopePlan.hint}` : ' 暂未命中具体供料范围。'}
        </p>
      </div>
      <div className="assistant-context-chips" aria-label="模型范围判断">
        {intentLabel ? <span className="message-chip neutral">意图 {intentLabel}</span> : null}
        {datasetCandidates.map((candidate) => (
          <span className="message-chip blue" key={`${candidate.type}-${candidate.id}`}>
            预选 {candidate.label}
            {candidate.documentCount ? ` · ${candidate.documentCount}文档` : ''}
          </span>
        ))}
        {mediaCandidate ? <span className="message-chip green">媒体资料</span> : null}
        {preferDetail ? <span className="message-chip green">深度供料</span> : null}
        {memoryCandidate ? <span className="message-chip neutral">参考本轮对话</span> : null}
        {!datasetCandidates.length && !memoryCandidate ? <span className="message-chip neutral">不强行检索</span> : null}
      </div>
    </div>
  );
}

function AssistantRunProgressPanel({ progress }) {
  if (!progress || (!progress.steps?.length && !progress.traceSteps?.length)) {
    return null;
  }

  const title = progress.continued ? '连续执行进度' : '本轮执行进度';
  return (
    <div className="assistant-run-progress-panel" aria-label="AssistantRun 安全进度">
      <div className="assistant-run-progress-head">
        <div>
          <span>{title}</span>
          <strong>{progress.runId ? truncateText(progress.runId, 18) : 'AssistantRun'}</strong>
        </div>
        {progress.traceSteps?.length ? <em>{progress.traceSteps.length} 个模型动作</em> : null}
      </div>
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

export default function ChatPanel({
  dataset,
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
  onRetryWorkflowExecution,
  onCancelWorkflowExecution,
  onRefreshStaticPageDraft,
  onOpenStaticPageBuilder,
  onCloseStaticPageDraft,
  showStaticPageWorkspace = true,
  startupBriefing,
  scopePlan,
  assistantRunProgress,
  onUploadClick,
  uploadingFiles = false,
}) {
  const reportEntry = session?.session_manifest_view?.report_entry || null;
  const latestTurn = session?.session_manifest_view?.last_turn || null;
  const showingStaticPageWorkspace = showStaticPageWorkspace && Boolean(staticPageDraft);

  return (
    <section className={`chat-panel card ${panelClassName}`.trim()}>
      <div className="panel-header chat-header">
        <div>
          <h3>{session ? session.title : dataset ? `${dataset.title} · 新问答` : '普通聊天 · 未选数据集'}</h3>
          <p>
            {session
              ? `会话 ${truncateText(session.id, 16)} · 最后更新 ${formatRelativeTime(session.updated_at)}`
              : dataset
                ? `当前选择 ${dataset.title}，发送问题会在这个数据集下启动新的 chat_session workflow。`
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
        ) : dataset ? (
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

      {!showingStaticPageWorkspace ? (
        <StaticPageAssistantNotice
          draft={staticPageDraft}
          onOpenBuilder={onOpenStaticPageBuilder}
          onOneClick={() => onStartStaticPageDraft?.({ oneClick: true })}
        />
      ) : null}

      <AssistantContextStrip
        dataset={dataset}
        startupBriefing={startupBriefing}
        scopePlan={scopePlan}
      />

      <AssistantRunProgressPanel progress={assistantRunProgress} />

      {showingStaticPageWorkspace ? (
        <div className="chat-static-page-workspace">
          <div className="chat-static-page-head">
            <div>
              <span>当前工作台</span>
              <strong>{staticPageDraft?.status === 'rendered' ? '静态页成品' : '静态页规划'}</strong>
            </div>
            <button type="button" className="ghost-btn compact-action-btn" onClick={onCloseStaticPageDraft}>
              返回聊天记录
            </button>
          </div>
          <StaticPagePlanningPanel
            draft={staticPageDraft}
            onStartDraft={onStartStaticPageDraft}
            onApplyOperation={onApplyStaticPageOperation}
            onRetryWorkflow={onRetryWorkflowExecution}
            onCancelWorkflow={onCancelWorkflowExecution}
            onRefreshDraft={onRefreshStaticPageDraft}
          />
        </div>
      ) : (
        <div className="chat-messages">
          {messageLoading ? (
          <div className="chat-empty-state loading-state">
            <span className="loading-dot"></span>
            <span className="loading-dot"></span>
            <span className="loading-dot"></span>
          </div>
        ) : messages.length ? (
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
            <h4>{dataset ? '从当前数据集发起新会话' : '可以直接聊天'}</h4>
            <p>
              {dataset
                ? '输入问题会创建独立 chat_session；选中右侧历史会话后，底部输入会追加到该会话的新一轮。'
                : '未选数据集时按普通模型聊天处理；如果问题命中资料范围，系统会在左侧预选相关数据集并优先供料。'}
            </p>
          </div>
          )}
        </div>
      )}

      <div className="chat-composer-wrap">
        <div className="composer-note">
          {session
            ? '当前输入会追加到已选会话；如需分开上下文，点右上角“新会话”后再发送。'
            : dataset
              ? '当前输入会在所选数据集下创建新会话；右侧可随时切回历史会话继续追问。'
              : '未选数据集时先普通聊天；系统只做供料范围判断，不替模型编排答案。'}
        </div>
        <div className="chat-input-row">
          <button
            className="ghost-btn upload-btn"
            type="button"
            onClick={onUploadClick}
            disabled={!onUploadClick || submitting}
            title={onUploadClick ? '上传文件并自动分类' : '上传分类接口待接入'}
          >
            {uploadingFiles ? '上传中...' : '上传'}
          </button>
          <textarea
            value={input}
            onChange={(event) => onInputChange(event.target.value)}
            placeholder={
              dataset
                ? session
                  ? `继续追问 ${session.title}`
                  : `围绕 ${dataset.title} 提问，系统会在这个数据集下创建一条新会话`
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
          <button className="primary-btn send-btn" type="button" onClick={onSubmit} disabled={!input.trim() || submitting}>
            {submitting ? '提交中...' : session ? '追加一轮' : dataset ? '发起会话' : '发送'}
          </button>
          <button
            className="ghost-btn static-page-one-click-btn"
            type="button"
            onClick={() => onStartStaticPageDraft?.({ oneClick: true })}
            disabled={submitting}
          >
            一键生成静态页
          </button>
        </div>
      </div>
    </section>
  );
}
