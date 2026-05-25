'use client';

import StaticPageImagePromptConfirm from './StaticPageImagePromptConfirm';

export default function StaticPagePlanningPanel({
  draft,
  onStartDraft,
  onRequestPreview,
  busy = false,
}) {
  return (
    <div className="static-page-planning-panel">
      <StaticPageImagePromptConfirm
        draft={draft}
        onStartDraft={onStartDraft}
        onRequestPreview={onRequestPreview}
        busy={busy}
      />
    </div>
  );
}
