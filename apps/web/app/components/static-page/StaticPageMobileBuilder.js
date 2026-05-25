'use client';

import StaticPageImagePromptConfirm from './StaticPageImagePromptConfirm';

export default function StaticPageMobileBuilder({
  draft,
  onStartDraft,
  onRequestPreview,
  onBackToChat,
  busy = false,
}) {
  return (
    <section className="static-page-mobile-builder">
      <StaticPageImagePromptConfirm
        draft={draft}
        onStartDraft={onStartDraft}
        onRequestPreview={onRequestPreview}
        onBackToChat={onBackToChat}
        compact
        busy={busy}
      />
    </section>
  );
}
