'use client';

export default function StaticPageModuleCard({ module }) {
  return (
    <article className="static-page-module-card">
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
    </article>
  );
}
