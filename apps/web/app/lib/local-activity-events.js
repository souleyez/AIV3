export const LOCAL_ACTIVITY_EVENTS_STORAGE_KEY = 'aidp-v3-local-activity-events';

const DEFAULT_MAX_ACTIVITY_EVENTS = 20;

export function readLocalActivityEvents(options = {}) {
  if (typeof window === 'undefined') {
    return [];
  }
  const maxEvents = Number(options.maxEvents) || DEFAULT_MAX_ACTIVITY_EVENTS;
  try {
    const raw = window.localStorage.getItem(LOCAL_ACTIVITY_EVENTS_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.slice(0, maxEvents) : [];
  } catch {
    return [];
  }
}

export function writeLocalActivityEvents(events, options = {}) {
  if (typeof window === 'undefined') {
    return;
  }
  const maxEvents = Number(options.maxEvents) || DEFAULT_MAX_ACTIVITY_EVENTS;
  try {
    const normalizedEvents = Array.isArray(events) ? events : [];
    window.localStorage.setItem(
      LOCAL_ACTIVITY_EVENTS_STORAGE_KEY,
      JSON.stringify(normalizedEvents.slice(0, maxEvents)),
    );
  } catch {
    // Activity cache is only a local briefing hint; ignore write failures.
  }
}

export function buildLocalUserStatementMemoryPayload(options = {}) {
  const message = options.message || null;
  const summary = String(message?.content || '').trim();
  if (!summary) {
    return null;
  }
  return {
    local_thread_id: String(options.localThreadId || ''),
    role: 'user',
    item_kind: 'user_statement',
    summary,
    source_message_refs: [message.id].filter(Boolean),
    artifact_refs: [],
    metadata: {
      source: 'browser_local_chat',
      assistant_run_id: options.assistantRunId || null,
    },
  };
}
