'use client';

export default function StaticPageAssistantNotice({
  draft,
  onOpenBuilder,
  onPrimaryAction,
  actionLabel = '效果图——生成页面',
  actionHelper = '',
  actionDisabled = false,
  secondaryLabel = '不满意，调整生图文案',
  simpleEntry = false,
}) {
  if (!draft) {
    return null;
  }

  if (simpleEntry) {
    return (
      <div className="static-page-assistant-notice static-page-simple-entry" role="note">
        <div className="static-page-simple-copy">
          <strong>已经了解您的意图，初步规划已经完成</strong>
          <span>点此查看并确认提交给作图的文字。</span>
        </div>
        <button type="button" className="primary-btn compact-action-btn" onClick={onPrimaryAction} disabled={actionDisabled}>
          {actionLabel}
        </button>
      </div>
    );
  }

  const jobStatus = draft.previewContract?.status === 'stale' || draft.imageJob?.status === 'stale'
    ? 'stale'
    : (draft.imageJob?.status || draft.previewContract?.status || 'idle');
  const finalStatus = draft.finalPage?.status || '';
  const preview = draft.previewImage;
  const previewAssetKey = preview?.assetKey || draft.previewContract?.assetKey || '';
  const canRenderPreviewImage = isRenderablePreviewAsset(previewAssetKey);
  const modules = Array.isArray(draft.modules) ? draft.modules : [];

  return (
    <div className="static-page-assistant-notice" role="note">
      <div className="static-page-notice-copy">
        <span>静态页草稿</span>
        <strong>{draft.objective}</strong>
        <p>{draft.modelSummary}</p>
      </div>
      <div className="static-page-notice-meta">
        <span>{modules.length} 个模块</span>
        <span>{draft.styleDirection}</span>
        <span>{statusLabel(jobStatus, finalStatus, draft.status)}</span>
      </div>
      <StaticPageNoticePreview
        preview={preview}
        previewAssetKey={previewAssetKey}
        canRenderPreviewImage={canRenderPreviewImage}
        modules={modules}
        jobStatus={jobStatus}
        finalStatus={finalStatus}
      />
      <div className="static-page-notice-actions">
        <button type="button" className="primary-btn compact-action-btn" onClick={onPrimaryAction} disabled={actionDisabled}>
          {actionLabel}
        </button>
        <span>{actionHelper}</span>
      </div>
      {secondaryLabel ? (
        <a
          href="#static-page-editor"
          className="static-page-edit-link"
          onClick={(event) => {
            event.preventDefault();
            onOpenBuilder?.();
          }}
        >
          {secondaryLabel}
        </a>
      ) : null}
    </div>
  );
}

function StaticPageNoticePreview({
  preview,
  previewAssetKey,
  canRenderPreviewImage,
  modules,
  jobStatus,
  finalStatus,
}) {
  if (['queued', 'running'].includes(jobStatus)) {
    return (
      <div className="static-page-notice-queue">
        <strong>效果图资源排队中</strong>
        <span>生成完成后会自动回到主聊天区域。等待期间可联系商务开通高级用户跳过等待。</span>
      </div>
    );
  }

  if (['queued', 'rendering'].includes(finalStatus)) {
    return (
      <div className="static-page-notice-queue">
        <strong>页面正在后台制作</strong>
        <span>完成后会保存到右侧生成结果，可以继续聊天或处理其他任务。</span>
      </div>
    );
  }

  if (jobStatus === 'failed') {
    return (
      <div className="static-page-notice-queue warning">
        <strong>效果图生成失败</strong>
        <span>可以用同一个按钮重新发起效果图。</span>
      </div>
    );
  }

  if (!preview) {
    return (
      <div className="static-page-notice-queue subtle">
        <strong>等待效果图</strong>
        <span>当前只需要确认提交给作图的文字；效果图返回后会继续生成最终静态页。</span>
      </div>
    );
  }

  if (canRenderPreviewImage) {
    return (
      <figure className="static-page-notice-image">
        <img src={previewAssetKey} alt={preview.title || '静态页效果图'} loading="lazy" />
        <figcaption>
          <span>{preview.title || '静态页效果图'}</span>
          <strong>{preview.subtitle || '效果图已返回，正在继续生成页面'}</strong>
        </figcaption>
      </figure>
    );
  }

  return (
    <div className="static-page-notice-preview">
      <span>{preview.title || '效果图视觉合同'}</span>
      <strong>{preview.subtitle || '效果图已返回，正在继续生成页面'}</strong>
      <div className="static-page-notice-preview-lines">
        {(preview.modules || modules).slice(0, 8).map((module) => (
          <i key={module.id || module.title} style={{ width: modulePreviewWidth(module) }} title={module.title} />
        ))}
      </div>
    </div>
  );
}

function isRenderablePreviewAsset(assetKey) {
  const value = String(assetKey || '').trim();
  return /^https?:\/\//.test(value)
    || value.startsWith('data:image/')
    || value.startsWith('blob:')
    || value.startsWith('/api/')
    || value.startsWith('/_next/');
}

function modulePreviewWidth(module) {
  const width = module?.width || module?.layout?.w || 4;
  return `${Math.max(32, Math.min(100, Number(width) * 8))}%`;
}

function statusLabel(jobStatus, finalStatus, draftStatus) {
  if (finalStatus === 'rendered' || draftStatus === 'rendered') return '页面已生成';
  if (['queued', 'rendering'].includes(finalStatus)) return '页面制作中';
  if (jobStatus === 'preview_ready') return '效果图已生成';
  if (jobStatus === 'confirmed' || draftStatus === 'effect_confirmed') return '效果图已确认';
  if (jobStatus === 'queued') return '效果图排队中';
  if (jobStatus === 'running') return '效果图生成中';
  if (jobStatus === 'stale') return '规划已变更';
  if (jobStatus === 'failed') return '效果图失败';
  return '规划中';
}
