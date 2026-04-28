'use client';

import { useEffect, useState } from 'react';
import { STATIC_PAGE_VISUALIZATION_TYPES } from '../../lib/static-page-draft';

function stopControlPropagation(event) {
  event.stopPropagation();
}

function visualizationLabel(type) {
  return STATIC_PAGE_VISUALIZATION_TYPES.find((item) => item.type === type)?.label || type;
}

export default function StaticPageModuleCard({ module, onApplyOperation, compact = false }) {
  const [draft, setDraft] = useState({
    title: module.title || '',
    content: module.content || '',
    dataLabel: module.dataBinding?.label || '',
    visualizationType: module.visualization?.type || 'text-insight',
  });

  useEffect(() => {
    setDraft({
      title: module.title || '',
      content: module.content || '',
      dataLabel: module.dataBinding?.label || '',
      visualizationType: module.visualization?.type || 'text-insight',
    });
  }, [module.id, module.title, module.content, module.dataBinding?.label, module.visualization?.type]);

  const dirty = draft.title !== (module.title || '')
    || draft.content !== (module.content || '')
    || draft.dataLabel !== (module.dataBinding?.label || '')
    || draft.visualizationType !== (module.visualization?.type || 'text-insight');

  function applyModuleEdit() {
    if (!dirty) return;
    onApplyOperation?.({
      type: 'update_module',
      targetModuleId: module.id,
      patch: {
        title: draft.title.trim() || module.title || '未命名模块',
        content: draft.content.trim() || module.content || '补充这个模块要表达的内容。',
        dataBinding: {
          ...(module.dataBinding || {}),
          label: draft.dataLabel.trim() || '数据绑定待确认',
        },
        visualization: {
          ...(module.visualization || {}),
          type: draft.visualizationType,
          label: visualizationLabel(draft.visualizationType),
        },
      },
    });
  }

  return (
    <article className={`static-page-module-card${compact ? ' compact' : ''}`}>
      <div className="static-page-module-drag-handle" title="拖动模块">
        <span>{module.role}</span>
        <strong>{module.title}</strong>
      </div>
      <p>{module.content}</p>
      <div className="static-page-module-meta">
        <span>数据：{module.dataBinding?.label || '未绑定'}</span>
        <span>图表：{module.visualization?.label || module.visualization?.type || '未选择'}</span>
      </div>
      <div className="static-page-module-layout">
        x{module.layout?.x ?? 0} y{module.layout?.y ?? 0} · {module.layout?.w ?? 1}x{module.layout?.h ?? 1}
      </div>
      {onApplyOperation ? (
        <details className="static-page-module-editor" onPointerDown={stopControlPropagation}>
          <summary>微调模块</summary>
          <div className="static-page-module-editor-grid">
            <label>
              <span>标题</span>
              <input
                value={draft.title}
                onChange={(event) => setDraft((current) => ({ ...current, title: event.target.value }))}
                onBlur={applyModuleEdit}
              />
            </label>
            <label>
              <span>数据</span>
              <input
                value={draft.dataLabel}
                onChange={(event) => setDraft((current) => ({ ...current, dataLabel: event.target.value }))}
                onBlur={applyModuleEdit}
              />
            </label>
            <label className="wide">
              <span>内容</span>
              <textarea
                value={draft.content}
                rows={compact ? 2 : 3}
                onChange={(event) => setDraft((current) => ({ ...current, content: event.target.value }))}
                onBlur={applyModuleEdit}
              />
            </label>
            <label>
              <span>图表</span>
              <select
                value={draft.visualizationType}
                onChange={(event) => setDraft((current) => ({ ...current, visualizationType: event.target.value }))}
                onBlur={applyModuleEdit}
              >
                {STATIC_PAGE_VISUALIZATION_TYPES.map((item) => (
                  <option key={item.type} value={item.type}>{item.label}</option>
                ))}
              </select>
            </label>
            <button type="button" className="ghost-btn compact-action-btn" onClick={applyModuleEdit} disabled={!dirty}>
              应用调整
            </button>
          </div>
        </details>
      ) : null}
    </article>
  );
}
