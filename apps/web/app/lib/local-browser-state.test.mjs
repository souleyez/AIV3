import test from 'node:test';
import assert from 'node:assert/strict';
import {
  LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY,
  LOCAL_THREAD_ID_STORAGE_KEY,
  createLocalThreadId,
  readLocalAssistantRunId,
  readLocalThreadId,
  writeLocalAssistantRunId,
  writeLocalThreadId,
} from './local-browser-state.js';

function createStorage(initial = {}) {
  const store = new Map(Object.entries(initial).map(([key, value]) => [key, String(value)]));
  return {
    getItem(key) {
      return store.has(key) ? store.get(key) : null;
    },
    setItem(key, value) {
      store.set(key, String(value));
    },
    removeItem(key) {
      store.delete(key);
    },
    snapshot() {
      return Object.fromEntries(store.entries());
    },
  };
}

function withWindow(localStorage, callback) {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: { localStorage },
  });
  try {
    return callback(localStorage);
  } finally {
    if (previous) {
      Object.defineProperty(globalThis, 'window', previous);
    } else {
      delete globalThis.window;
    }
  }
}

test('local browser state returns safe fallbacks without window', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  if (previous) {
    delete globalThis.window;
  }
  try {
    assert.equal(readLocalThreadId(), 'server-render-thread');
    assert.equal(readLocalAssistantRunId(), '');
    assert.doesNotThrow(() => writeLocalThreadId('thread-1'));
    assert.doesNotThrow(() => writeLocalAssistantRunId('run-1'));
  } finally {
    if (previous) {
      Object.defineProperty(globalThis, 'window', previous);
    }
  }
});

test('readLocalThreadId returns an existing stored thread id', () => {
  withWindow(createStorage({ [LOCAL_THREAD_ID_STORAGE_KEY]: 'thread-existing' }), () => {
    assert.equal(readLocalThreadId(), 'thread-existing');
  });
});

test('readLocalThreadId creates and persists a thread id when missing', () => {
  withWindow(createStorage(), (storage) => {
    const threadId = readLocalThreadId();
    assert.equal(typeof threadId, 'string');
    assert.notEqual(threadId, '');
    assert.equal(storage.snapshot()[LOCAL_THREAD_ID_STORAGE_KEY], threadId);
  });
});

test('writeLocalThreadId stores the active thread id', () => {
  withWindow(createStorage(), (storage) => {
    writeLocalThreadId('thread-next');
    assert.equal(storage.snapshot()[LOCAL_THREAD_ID_STORAGE_KEY], 'thread-next');
  });
});

test('local thread storage tolerates browser storage failures', () => {
  const throwingStorage = {
    getItem() {
      throw new Error('blocked');
    },
    setItem() {
      throw new Error('blocked');
    },
  };
  withWindow(throwingStorage, () => {
    assert.equal(readLocalThreadId(), 'browser-thread-unavailable');
    assert.doesNotThrow(() => writeLocalThreadId('thread-ignored'));
  });
});

test('assistant run id reads, writes, and clears the browser cache', () => {
  withWindow(createStorage(), (storage) => {
    assert.equal(readLocalAssistantRunId(), '');
    writeLocalAssistantRunId('run-1');
    assert.equal(readLocalAssistantRunId(), 'run-1');
    assert.equal(storage.snapshot()[LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY], 'run-1');
    writeLocalAssistantRunId('');
    assert.equal(readLocalAssistantRunId(), '');
    assert.equal(storage.snapshot()[LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY], undefined);
  });
});

test('createLocalThreadId always returns a non-empty string', () => {
  assert.equal(typeof createLocalThreadId(), 'string');
  assert.notEqual(createLocalThreadId(), '');
});
