'use client';

import { formatDateTime, formatRelativeTime, truncateText } from '../lib/formatters';

function SectionHeader({ title, subtitle }) {
  return (
    <div className="insight-section-head">
      <h4>{title}</h4>
      <span>{subtitle}</span>
    </div>
  );
}

function EmptySection({ text }) {
  return <div className="insight-empty">{text}</div>;
}

export default function InsightPanel({
  dataset,
  sessions,
  selectedSessionId,
  outputs,
  reportPlans,
  publishedReports,
  onSelectSession,
}) {
  return (
    <aside className="insight-panel">
      <section className="card insight-card">
        <SectionHeader
          title="会话"
          subtitle={dataset ? `${dataset.title} 下的最近会话` : '选择数据集后展示'}
        />
        <div className="insight-list">
          {sessions.length ? (
            sessions.map((session) => {
              const active = session.id === selectedSessionId;
              const reportEntryState = session.session_manifest_view?.report_entry?.state;
              return (
                <button
                  key={session.id}
                  type="button"
                  className={`insight-item ${active ? 'active' : ''}`}
                  onClick={() => onSelectSession(session.id)}
                >
                  <div className="insight-item-head">
                    <strong>{session.title}</strong>
                    <span>{formatRelativeTime(session.updated_at)}</span>
                  </div>
                  <p>{truncateText(session.latest_assistant_message?.content || session.session_manifest_view?.initial_prompt || '', 88)}</p>
                  <div className="insight-meta-row">
                    <span>{reportEntryState || 'not_applicable'}</span>
                    <span>{session.model_facing?.recommended_tool_key || 'chat_session'}</span>
                  </div>
                </button>
              );
            })
          ) : (
            <EmptySection text="当前数据集还没有会话。" />
          )}
        </div>
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="资料输出"
          subtitle="数据集级 material output 回看"
        />
        <div className="insight-list">
          {outputs.length ? (
            outputs.map((output) => (
              <article className="insight-item static" key={output.id}>
                <div className="insight-item-head">
                  <strong>{truncateText(output.prompt, 28)}</strong>
                  <span>{formatRelativeTime(output.created_at)}</span>
                </div>
                <p>{truncateText(output.output_text, 110) || '等待 worker 写回输出。'}</p>
                <div className="insight-meta-row">
                  <span>证据 {output.retrieval_evidence_ids?.length || 0}</span>
                  <span>工具 {output.tool_executions?.length || 0}</span>
                </div>
              </article>
            ))
          ) : (
            <EmptySection text="当前数据集还没有资料输出。" />
          )}
        </div>
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="报告计划"
          subtitle="已进入报告服务的 plan 聚合"
        />
        <div className="insight-list">
          {reportPlans.length ? (
            reportPlans.map((plan) => (
              <article className="insight-item static" key={plan.id}>
                <div className="insight-item-head">
                  <strong>{plan.title}</strong>
                  <span>{plan.status}</span>
                </div>
                <p>{truncateText(plan.objective, 110)}</p>
                <div className="insight-meta-row">
                  <span>{plan.model_facing?.recommended_tool_key || 'report.plan'}</span>
                  <span>{plan.current_ast_version_id ? '已有 AST 版本' : '尚无 AST 版本'}</span>
                </div>
              </article>
            ))
          ) : (
            <EmptySection text="当前数据集下还没有 report plan。" />
          )}
        </div>
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="已发布"
          subtitle="published reports 聚合视图"
        />
        <div className="insight-list">
          {publishedReports.length ? (
            publishedReports.map((report) => (
              <article className="insight-item static" key={report.id}>
                <div className="insight-item-head">
                  <strong>{report.slug}</strong>
                  <span>{formatDateTime(report.updated_at)}</span>
                </div>
                <p>plan_id: {truncateText(report.plan_id, 24)}</p>
                <div className="insight-meta-row">
                  <span>report_id {truncateText(report.id, 12)}</span>
                  <span>{report.current_version_id ? '有当前版本' : '尚未设当前版本'}</span>
                </div>
              </article>
            ))
          ) : (
            <EmptySection text="这个数据集下还没有 published report。" />
          )}
        </div>
      </section>
    </aside>
  );
}
