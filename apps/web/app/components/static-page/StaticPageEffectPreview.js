'use client';

const JOB_LABELS = {
  idle: '待生成',
  queued: '资源排队中',
  running: '生成中',
  preview_ready: '效果图待确认',
  failed: '生成失败',
};

function modulePreviewWidth(module) {
  return `${Math.max(32, Math.min(100, Number(module.width || 4) * 8))}%`;
}

export default function StaticPageEffectPreview({
  draft,
  onApplyOperation,
  onContinueEditing,
  compact = false,
}) {
  const imageJob = draft?.imageJob || {};
  const jobStatus = imageJob.status || 'idle';
  const preview = draft?.previewImage;
  const confirmed = draft?.status === 'effect_confirmed'
    || draft?.status === 'rendering'
    || Boolean(draft?.finalPage);
  const hasPreview = jobStatus === 'preview_ready' || draft?.status === 'effect_confirmed';
  const queueMessage = imageJob.queueMessage || preview?.queueMessage || '资源正在排队，可以联系商务开通高级用户跳过等待。';

  function queuePreview() {
    onApplyOperation?.({
      type: 'queue_image_job',
      queuePosition: 2,
      queueMessage,
    });
  }

  function finishMockPreview() {
    onApplyOperation?.({ type: 'mark_preview_ready' });
  }

  function confirmPreview() {
    onApplyOperation?.({ type: 'confirm_preview' });
  }

  return (
    <section className={`static-page-effect-preview${compact ? ' compact' : ''}`}>
      <div className="static-page-effect-head">
        <div>
          <span>效果图</span>
          <strong>{JOB_LABELS[jobStatus] || jobStatus}</strong>
        </div>
        {confirmed ? <em>已确认</em> : null}
      </div>

      {jobStatus === 'queued' || jobStatus === 'running' ? (
        <div className="static-page-queue-card">
          <strong>{queueMessage}</strong>
          <span>{imageJob.queuePosition ? `当前前方约 ${imageJob.queuePosition} 个任务` : '正在等待生成资源'}</span>
        </div>
      ) : null}

      {hasPreview && preview ? (
        <div className="static-page-preview-card">
          <span>{preview.title}</span>
          <strong>{preview.subtitle}</strong>
          <div className="static-page-preview-lines">
            {preview.modules.map((module) => (
              <i key={module.id} style={{ width: modulePreviewWidth(module) }} title={module.title} />
            ))}
          </div>
        </div>
      ) : (
        <div className="static-page-preview-placeholder">
          <strong>先生成一张效果图给客户确认</strong>
          <p>这里先用确定性模拟图占位，后续会替换成 Cloudflare 队列返回的真实图片。</p>
        </div>
      )}

      <div className="static-page-effect-actions">
        {jobStatus === 'idle' || jobStatus === 'failed' ? (
          <button type="button" className="primary-btn compact-action-btn" onClick={queuePreview}>
            生成效果图
          </button>
        ) : null}
        {jobStatus === 'queued' || jobStatus === 'running' ? (
          <button type="button" className="primary-btn compact-action-btn" onClick={finishMockPreview}>
            查看模拟效果图
          </button>
        ) : null}
        {hasPreview ? (
          <button type="button" className="primary-btn compact-action-btn" onClick={confirmPreview} disabled={confirmed}>
            {confirmed ? '效果图已确认' : '确认效果图'}
          </button>
        ) : null}
        {hasPreview ? (
          <button type="button" className="ghost-btn compact-action-btn" onClick={queuePreview}>
            重新生成
          </button>
        ) : null}
        <button type="button" className="ghost-btn compact-action-btn" onClick={onContinueEditing}>
          继续修改规划
        </button>
      </div>
    </section>
  );
}
