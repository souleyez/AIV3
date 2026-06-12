export const LOCAL_THREAD_ID_STORAGE_KEY = 'aidp-v3-local-thread-id';
export const LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY = 'aidp-v3-local-assistant-run-id';

export function createLocalThreadId() {
  return globalThis.crypto?.randomUUID?.() || `thread-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function readLocalThreadId() {
  if (typeof window === 'undefined') {
    return 'server-render-thread';
  }
  try {
    const existing = window.localStorage.getItem(LOCAL_THREAD_ID_STORAGE_KEY);
    if (existing) {
      return existing;
    }
    const next = createLocalThreadId();
    window.localStorage.setItem(LOCAL_THREAD_ID_STORAGE_KEY, next);
    return next;
  } catch {
    return 'browser-thread-unavailable';
  }
}

export function writeLocalThreadId(threadId) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(LOCAL_THREAD_ID_STORAGE_KEY, threadId);
  } catch {
    // The browser cache is a convenience; AssistantRun can still use the current in-memory thread.
  }
}

export function readLocalAssistantRunId() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    return window.localStorage.getItem(LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY) || '';
  } catch {
    return '';
  }
}

export function writeLocalAssistantRunId(runId) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    if (runId) {
      window.localStorage.setItem(LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY, runId);
    } else {
      window.localStorage.removeItem(LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY);
    }
  } catch {
    // AssistantRun id is a convenience cache; chat still works without it.
  }
}
