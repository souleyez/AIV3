export function compactConversationSummary(value, maxLength = 28) {
  const text = String(value || '').replace(/\s+/g, ' ').trim();
  if (!text) {
    return '新对话';
  }
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

export function formatConversationTitleTime(value = new Date()) {
  const date = value instanceof Date ? value : new Date(value);
  const safeDate = Number.isNaN(date.getTime()) ? new Date() : date;
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(safeDate).replace(/\//g, '-');
}

export function buildDefaultConversationTitle(prompt, startedAt = new Date()) {
  return `${formatConversationTitleTime(startedAt)} · ${compactConversationSummary(prompt)}`;
}

export function buildCurrentConversationTitle(options = {}) {
  if (options.selectedSession) {
    return options.selectedSession.title || '当前对话';
  }
  const draftTitle = String(options.draftSessionTitle || '').trim();
  if (draftTitle) {
    return draftTitle;
  }
  const messages = Array.isArray(options.messages) ? options.messages : [];
  const firstUserMessage = messages.find((message) => message?.role === 'user')?.content || '';
  return buildDefaultConversationTitle(
    options.input || firstUserMessage || '新对话',
    options.startedAt,
  );
}
