export const LOCAL_CHAT_SESSION_OPTION_PREFIX = 'local-chat:';
export const LOCAL_CHAT_SESSIONS_STORAGE_KEY = 'aidp-v3-local-chat-sessions';

const DEFAULT_MAX_LOCAL_SESSIONS = 20;
const DEFAULT_MAX_LOCAL_MESSAGES = 40;

function safeText(value, maxLength = 200) {
  return String(value || '').replace(/\s+/g, ' ').trim().slice(0, maxLength);
}

function safeDate(value, fallback = new Date().toISOString()) {
  const date = new Date(value || fallback);
  return Number.isFinite(date.getTime()) ? date.toISOString() : fallback;
}

export function localChatSessionOptionId(threadId) {
  return `${LOCAL_CHAT_SESSION_OPTION_PREFIX}${String(threadId || '').trim()}`;
}

export function isLocalChatSessionOptionId(value) {
  return String(value || '').startsWith(LOCAL_CHAT_SESSION_OPTION_PREFIX);
}

export function localThreadIdFromSessionOptionId(value) {
  if (!isLocalChatSessionOptionId(value)) {
    return '';
  }
  return String(value).slice(LOCAL_CHAT_SESSION_OPTION_PREFIX.length).trim();
}

export function normalizeLocalChatMessage(message) {
  if (!message || typeof message !== 'object') {
    return null;
  }
  const role = safeText(message.role, 40);
  const content = String(message.content || '').trim();
  if (!role || !content) {
    return null;
  }
  return {
    ...message,
    id: safeText(message.id, 120) || `local-message-${Date.now()}`,
    role,
    content,
    created_at: safeDate(message.created_at || message.createdAt),
  };
}

export function normalizeLocalChatSession(session, options = {}) {
  if (!session || typeof session !== 'object') {
    return null;
  }
  const maxMessages = Number(options.maxMessages) || DEFAULT_MAX_LOCAL_MESSAGES;
  const id = safeText(session.id || session.thread_id || session.threadId, 160);
  if (!id) {
    return null;
  }
  const messages = (Array.isArray(session.messages) ? session.messages : [])
    .map(normalizeLocalChatMessage)
    .filter(Boolean)
    .slice(-maxMessages);
  const title = safeText(session.title, 80)
    || safeText(messages.find((message) => message.role === 'user')?.content, 36)
    || '本地对话';
  const now = new Date().toISOString();
  const startedAt = safeDate(session.started_at || session.startedAt, now);
  const updatedAt = safeDate(session.updated_at || session.updatedAt, startedAt);
  const assistantRunId = safeText(session.assistant_run_id || session.assistantRunId, 160);
  if (!messages.length && !assistantRunId) {
    return null;
  }
  return {
    id,
    title,
    messages,
    startedAt,
    updatedAt,
    assistantRunId,
  };
}

export function normalizeLocalChatSessions(sessions, options = {}) {
  const maxSessions = Number(options.maxSessions) || DEFAULT_MAX_LOCAL_SESSIONS;
  const seen = new Set();
  return (Array.isArray(sessions) ? sessions : [])
    .map((session) => normalizeLocalChatSession(session, options))
    .filter(Boolean)
    .sort((left, right) => new Date(right.updatedAt).getTime() - new Date(left.updatedAt).getTime())
    .filter((session) => {
      if (seen.has(session.id)) {
        return false;
      }
      seen.add(session.id);
      return true;
    })
    .slice(0, maxSessions);
}

export function upsertLocalChatSession(sessions, session, options = {}) {
  const normalized = normalizeLocalChatSession(session, options);
  if (!normalized) {
    return normalizeLocalChatSessions(sessions, options);
  }
  const withoutCurrent = (Array.isArray(sessions) ? sessions : [])
    .filter((item) => item?.id !== normalized.id);
  return normalizeLocalChatSessions([normalized, ...withoutCurrent], options);
}

export function shouldPersistLocalChatSession({ messages = [], assistantRunId = '', title = '' } = {}) {
  if (String(assistantRunId || '').trim()) {
    return true;
  }
  if (String(title || '').trim() && Array.isArray(messages) && messages.length) {
    return true;
  }
  return Array.isArray(messages)
    && messages.some((message) => String(message?.role || '').trim() === 'user'
      && String(message?.content || '').trim());
}

export function readLocalChatSessions() {
  if (typeof window === 'undefined') {
    return [];
  }
  try {
    const raw = window.localStorage.getItem(LOCAL_CHAT_SESSIONS_STORAGE_KEY);
    return normalizeLocalChatSessions(raw ? JSON.parse(raw) : []);
  } catch {
    return [];
  }
}

export function writeLocalChatSessions(sessions) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(
      LOCAL_CHAT_SESSIONS_STORAGE_KEY,
      JSON.stringify(normalizeLocalChatSessions(sessions)),
    );
  } catch {
    // The local conversation index is a convenience cache; the active chat still works in memory.
  }
}
