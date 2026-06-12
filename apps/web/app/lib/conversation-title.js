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
