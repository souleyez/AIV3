'use client';

import { useEffect, useMemo, useState } from 'react';
import {
  buildStaticPageImagePayload,
  buildStaticPageImagePromptText,
} from '../../lib/static-page-draft';

const STATIC_PAGE_QUEUE_MESSAGE = '资源正在排队，可以联系商务开通高级用户跳过等待。';

function promptConfirmStatus(draft = {}) {
  const previewStale = draft.previewContract?.status === 'stale' || draft.imageJob?.status === 'stale';
  const jobStatus = previewStale ? 'stale' : (draft.imageJob?.status || draft.previewContract?.status || 'idle');
  if (['queued', 'running'].includes(jobStatus)) return '可视化正在生成中';
  if (jobStatus === 'preview_ready') return '可视化已返回，正在继续生成页面';
  if (jobStatus === 'failed') return '上次作图失败，可以修改文案后重试';
  if (jobStatus === 'stale') return '规划已更新，需要重新请求作图';
  return '确认后会提交当前文字给作图队列';
}

export default function StaticPageImagePromptConfirm({
  draft,
  onStartDraft,
  onRequestPreview,
  onBackToChat,
  compact = false,
  busy = false,
}) {
  const defaultPrompt = useMemo(() => (
    draft ? buildStaticPageImagePromptText(draft) : ''
  ), [draft]);
  const [promptText, setPromptText] = useState(defaultPrompt);

  useEffect(() => {
    setPromptText(defaultPrompt);
  }, [defaultPrompt]);

  if (!draft) {
    return (
      <div className={`static-page-prompt-confirm empty ${compact ? 'compact' : ''}`.trim()}>
        <div className="static-page-prompt-head">
          <div>
            <span>生图文案</span>
            <strong>还没有静态页规划</strong>
          </div>
          {onBackToChat ? (
            <button type="button" className="ghost-btn compact-action-btn" onClick={onBackToChat}>
              返回
            </button>
          ) : null}
        </div>
        <p className="static-page-prompt-note">先生成一版规划，随后这里只会显示提交给作图的文字信息。</p>
        <button type="button" className="primary-btn" onClick={() => onStartDraft?.({ oneClick: false })}>
          创建生图文案
        </button>
      </div>
    );
  }

  const previewStale = draft.previewContract?.status === 'stale' || draft.imageJob?.status === 'stale';
  const jobStatus = previewStale ? 'stale' : (draft.imageJob?.status || draft.previewContract?.status || 'idle');
  const submitting = busy || ['queued', 'running'].includes(jobStatus);
  const cleanPromptText = promptText.trim() || defaultPrompt;

  function handleSubmit() {
    const prompt = cleanPromptText;
    onRequestPreview?.({
      type: 'queue_image_job',
      prompt,
      promptOnly: true,
      queueMessage: STATIC_PAGE_QUEUE_MESSAGE,
      imagePromptPayload: buildStaticPageImagePayload(draft, {
        oneClick: Boolean(draft.source?.oneClick),
        promptText: prompt,
        promptOnly: true,
      }),
    });
  }

  return (
    <div className={`static-page-prompt-confirm ${compact ? 'compact' : ''}`.trim()}>
      <div className="static-page-prompt-head">
        <div>
          <span>提交给作图的文字</span>
          <strong>{draft.objective || draft.title || '静态页视觉稿'}</strong>
        </div>
        {onBackToChat ? (
          <button type="button" className="ghost-btn compact-action-btn" onClick={onBackToChat}>
            返回
          </button>
        ) : null}
      </div>

      <label className="static-page-prompt-field">
        <span>用户确认后，将按下方文字请求 GPT-Image2 / Cloudflare Codex 作图</span>
        <textarea
          value={promptText}
          onChange={(event) => setPromptText(event.target.value)}
          spellCheck={false}
          rows={compact ? 14 : 18}
        />
      </label>

      <div className="static-page-prompt-actions">
        <span className="static-page-prompt-status">{promptConfirmStatus(draft)}</span>
        <button
          type="button"
          className="primary-btn"
          onClick={handleSubmit}
          disabled={submitting || !cleanPromptText}
        >
          {submitting ? '作图队列中' : '确认文案并请求作图'}
        </button>
      </div>
    </div>
  );
}
