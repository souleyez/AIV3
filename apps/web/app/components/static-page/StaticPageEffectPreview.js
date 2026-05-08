'use client';

const JOB_LABELS = {
  idle: '待生成',
  queued: '资源排队中',
  running: '生成中',
  preview_ready: '效果图待确认',
  failed: '生成失败',
  confirmed: '效果图已确认',
};

const DEFAULT_QUEUE_MESSAGE = '资源正在排队，可以联系商务开通高级用户跳过等待。';

function modulePreviewWidth(module) {
  return `${Math.max(32, Math.min(100, Number(module.width || 4) * 8))}%`;
}

function isRenderablePreviewAsset(assetKey) {
  const value = String(assetKey || '').trim();
  return /^https?:\/\//.test(value)
    || value.startsWith('data:image/')
    || value.startsWith('blob:')
    || value.startsWith('/api/')
    || value.startsWith('/_next/');
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
  const previewAssetKey = preview?.assetKey || '';
  const canRenderPreviewImage = isRenderablePreviewAsset(previewAssetKey);
  const isBackendJob = Boolean(imageJob.id) && !String(imageJob.id).startsWith('mock-image-job-');
  const confirmed = draft?.status === 'effect_confirmed'
    || draft?.status === 'rendering'
    || Boolean(draft?.finalPage);
  const hasPreview = jobStatus === 'preview_ready' || draft?.status === 'effect_confirmed';
  const queueMessage = imageJob.queueMessage || preview?.queueMessage || DEFAULT_QUEUE_MESSAGE;
  const failureMessage = jobStatus === 'failed'
    ? (imageJob.queueMessage || '效果图生成失败，可以重新生成。')
    : '';
  const previewStale = draft?.previewContract?.status === 'stale';

  function queuePreview() {
    onApplyOperation?.({
      type: 'queue_image_job',
      queuePosition: 2,
      queueMessage: DEFAULT_QUEUE_MESSAGE,
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

      {jobStatus === 'failed' ? (
        <div className="static-page-failure-card">
          <strong>效果图生成失败</strong>
          <span>{failureMessage}</span>
        </div>
      ) : null}

      {previewStale ? (
        <div className="static-page-stale-card">
          <strong>规划已经改过</strong>
          <span>上一张效果图和最终静态页已失效，需要重新生成后再确认。</span>
        </div>
      ) : null}

      {hasPreview && preview ? (
        canRenderPreviewImage ? (
          <figure className="static-page-preview-image-card">
            <img src={previewAssetKey} alt={preview.title || '静态页效果图'} loading="lazy" />
            <figcaption>
              <span>{preview.title || '静态页效果图'}</span>
              <strong>{preview.subtitle || '由远程生图队列生成，等待客户确认'}</strong>
            </figcaption>
          </figure>
        ) : (
          <div className="static-page-preview-card">
            <span>{preview.title}</span>
            <strong>{preview.subtitle}</strong>
            <div className="static-page-preview-lines">
              {(preview.modules || []).map((module) => (
                <i key={module.id} style={{ width: modulePreviewWidth(module) }} title={module.title} />
              ))}
            </div>
          </div>
        )
      ) : (
        <div className="static-page-preview-placeholder">
          <strong>先生成一张效果图给客户确认</strong>
          <p>后端会通过 Codex 远程队列生成真实图片，等待中仍可继续修改规划。</p>
        </div>
      )}

      <div className="static-page-effect-actions">
        {jobStatus === 'idle' || jobStatus === 'failed' ? (
          <button type="button" className="primary-btn compact-action-btn" onClick={queuePreview}>
            {jobStatus === 'failed' ? '重新生成效果图' : '生成效果图'}
          </button>
        ) : null}
        {(jobStatus === 'queued' || jobStatus === 'running') && !isBackendJob ? (
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
