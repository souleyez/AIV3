'use client';

import { STATIC_PAGE_STYLE_DIRECTIONS } from '../../lib/static-page-draft';

const STYLE_HINTS = {
  'decision-brief': {
    eyebrow: '结论先行',
    sample: '核心判断 · 关键指标 · 风险动作',
  },
  'client-delivery': {
    eyebrow: '交付清晰',
    sample: '背景说明 · 数据证据 · 推进建议',
  },
  'data-command': {
    eyebrow: '指标密集',
    sample: 'KPI · 趋势 · 构成 · 预警',
  },
};

export default function StaticPageStyleDirectionPicker({
  value,
  onChange,
  compact = false,
}) {
  return (
    <div className={`static-page-style-picker${compact ? ' compact' : ''}`}>
      <div className="static-page-style-picker-head">
        <span>风格确认</span>
        <strong>选择最终静态页方向</strong>
      </div>
      <div className="static-page-style-options">
        {STATIC_PAGE_STYLE_DIRECTIONS.map((style) => {
          const active = style.key === value;
          const hint = STYLE_HINTS[style.key] || {};
          return (
            <button
              type="button"
              key={style.key}
              className={`static-page-style-option${active ? ' active' : ''}`}
              onClick={() => onChange?.(style.key)}
            >
              <span>{hint.eyebrow || '页面风格'}</span>
              <strong>{style.label}</strong>
              <p>{style.description}</p>
              <em>{hint.sample}</em>
            </button>
          );
        })}
      </div>
    </div>
  );
}
