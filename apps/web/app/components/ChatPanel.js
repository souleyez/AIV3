'use client';

import { formatDateTime, formatRelativeTime, truncateText } from '../lib/formatters';

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
  confirmation_required: '需要 2 选 1',
  confirmed: '已进入报告服务',
};

function renderParagraphs(content) {
  const parts = String(content || '')
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

function ReportEntryGate({ reportEntry, busy, onResolve }) {
  if (reportEntry?.state !== 'confirmation_required') {
    return null;
  }

  return (
    <div className="report-entry-gate">
      <div className="report-entry-head">
        <strong>检测到报告入口分流</strong>
        <span>这条会话已经给出 host-side `2 选 1` gate。</span>
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

export default function ChatPanel({
  dataset,
  session,
  messages,
  messageLoading,
  input,
  onInputChange,
  onSubmit,
  submitting,
  reportEntryBusy,
  onResolveReportEntry,
}) {
  const reportEntry = session?.session_manifest_view?.report_entry || null;

  return (
    <section className="chat-panel card">
      <div className="panel-header chat-header">
        <div>
          <h3>{session ? session.title : dataset ? `${dataset.title} · 新问答` : '选择数据集后开始'}</h3>
          <p>
            {session
              ? `会话 ${truncateText(session.id, 16)} · 最后更新 ${formatRelativeTime(session.updated_at)}`
              : dataset
                ? `当前选择 ${dataset.title}，发送问题会在这个数据集下启动新的 chat_session workflow。`
                : '左侧先选数据集，右侧会自动加载这个数据集下的历史会话和发布结果。'}
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
          </div>
        ) : null}
      </div>

      <ReportEntryGate
        reportEntry={reportEntry}
        busy={reportEntryBusy}
        onResolve={onResolveReportEntry}
      />

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
            <h4>从当前数据集发起新会话</h4>
            <p>
              第一版 UI 先对接 V3 现有 host surface：左侧切数据集，底部输入问题，系统会创建新会话并持续轮询消息回写。
            </p>
          </div>
        )}
      </div>

      <div className="chat-composer-wrap">
        <div className="composer-note">
          现阶段暂时没有“在旧会话里追加一轮消息”的独立接口，所以每次发送都会新建会话；历史内容仍会保留在右侧列表里。
        </div>
        <div className="chat-input-row">
          <textarea
            value={input}
            onChange={(event) => onInputChange(event.target.value)}
            placeholder={dataset ? `围绕 ${dataset.title} 提问，系统会在这个数据集下创建一条新会话` : '先在左侧选择数据集'}
            disabled={!dataset || submitting}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault();
                if (dataset && !submitting) {
                  onSubmit();
                }
              }
            }}
          />
          <button className="primary-btn send-btn" type="button" onClick={onSubmit} disabled={!dataset || submitting}>
            {submitting ? '启动中...' : '发起会话'}
          </button>
        </div>
      </div>
    </section>
  );
}
