import { createLocalMessage } from './local-chat-sessions.js';

export const SUPPRESSED_UI_NOTICE_TEXT = '已发送，助手正在后台处理；你可以继续输入。';

export function buildUiNoticeDescriptor(kind, content) {
  const text = String(content || '').trim();
  if (!text || text === SUPPRESSED_UI_NOTICE_TEXT) {
    return null;
  }
  const displayContent = kind === 'error' ? `提示：${text}` : text;
  return {
    displayContent,
    stableKey: `ui-notice:${kind}:${displayContent.replace(/\s+/g, ' ')}`,
    tone: kind,
  };
}

export function buildUiNoticeLocalMessage(descriptor, options = {}) {
  if (!descriptor?.stableKey || !descriptor?.displayContent) {
    return null;
  }
  const messageFactory = typeof options.messageFactory === 'function'
    ? options.messageFactory
    : createLocalMessage;
  return {
    ...messageFactory('assistant', descriptor.displayContent),
    metadata: {
      source: 'ui_notice',
      tone: descriptor.tone,
      key: descriptor.stableKey,
    },
  };
}
