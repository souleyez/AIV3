import test from 'node:test';
import assert from 'node:assert/strict';
import {
  LOCAL_ACCOUNT_EMAIL_STORAGE_KEY,
  LOCAL_SECRET_BINDING_IDS_STORAGE_KEY,
  LOCAL_SECRET_VALUE_STORAGE_KEY,
  clearLocalSecretState,
  readLocalAccountEmail,
  readLocalSecretBindingIds,
  readLocalSecretBindingIdsHeader,
  readLocalSecretValue,
  writeLocalAccountEmail,
  writeLocalSecretState,
} from './local-account-state.js';

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

test('secret binding id cache normalizes header and list values', () => {
  withWindow(createStorage({ [LOCAL_SECRET_BINDING_IDS_STORAGE_KEY]: ' id-1, ,id-2,, id-1 ' }), () => {
    assert.equal(readLocalSecretBindingIdsHeader(), 'id-1,id-2,id-1');
    assert.deepEqual(readLocalSecretBindingIds(), ['id-1', 'id-2', 'id-1']);
  });
});

test('secret binding id reads return empty values without usable browser storage', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  if (previous) {
    delete globalThis.window;
  }
  try {
    assert.equal(readLocalSecretBindingIdsHeader(), '');
    assert.deepEqual(readLocalSecretBindingIds(), []);
    assert.equal(readLocalSecretValue(), '');
    assert.equal(readLocalAccountEmail(), '');
  } finally {
    if (previous) {
      Object.defineProperty(globalThis, 'window', previous);
    }
  }

  withWindow({
    getItem() {
      throw new Error('blocked');
    },
  }, () => {
    assert.equal(readLocalSecretBindingIdsHeader(), '');
    assert.deepEqual(readLocalSecretBindingIds(), []);
    assert.equal(readLocalSecretValue(), '');
    assert.equal(readLocalAccountEmail(), '');
  });
});

test('writeLocalSecretState stores unique binding ids and optional local key value', () => {
  withWindow(createStorage(), (storage) => {
    writeLocalSecretState('local-key-placeholder', ['id-1', '', 'id-2', 'id-1']);
    assert.equal(storage.snapshot()[LOCAL_SECRET_BINDING_IDS_STORAGE_KEY], 'id-1,id-2');
    assert.equal(storage.snapshot()[LOCAL_SECRET_VALUE_STORAGE_KEY], 'local-key-placeholder');
  });

  withWindow(createStorage({ [LOCAL_SECRET_VALUE_STORAGE_KEY]: 'existing-key' }), (storage) => {
    writeLocalSecretState('', ['id-3']);
    assert.equal(storage.snapshot()[LOCAL_SECRET_BINDING_IDS_STORAGE_KEY], 'id-3');
    assert.equal(storage.snapshot()[LOCAL_SECRET_VALUE_STORAGE_KEY], 'existing-key');
  });
});

test('clearLocalSecretState removes local key and binding ids', () => {
  withWindow(createStorage({
    [LOCAL_SECRET_BINDING_IDS_STORAGE_KEY]: 'id-1',
    [LOCAL_SECRET_VALUE_STORAGE_KEY]: 'local-key-placeholder',
  }), (storage) => {
    clearLocalSecretState();
    assert.equal(storage.snapshot()[LOCAL_SECRET_BINDING_IDS_STORAGE_KEY], undefined);
    assert.equal(storage.snapshot()[LOCAL_SECRET_VALUE_STORAGE_KEY], undefined);
  });
});

test('account email cache normalizes, clears, and tolerates storage failures', () => {
  withWindow(createStorage(), (storage) => {
    writeLocalAccountEmail(' User@Example.COM ');
    assert.equal(storage.snapshot()[LOCAL_ACCOUNT_EMAIL_STORAGE_KEY], 'user@example.com');
    assert.equal(readLocalAccountEmail(), 'user@example.com');
    writeLocalAccountEmail('');
    assert.equal(storage.snapshot()[LOCAL_ACCOUNT_EMAIL_STORAGE_KEY], undefined);
  });

  withWindow({
    getItem() {
      throw new Error('blocked');
    },
    setItem() {
      throw new Error('blocked');
    },
    removeItem() {
      throw new Error('blocked');
    },
  }, () => {
    assert.equal(readLocalAccountEmail(), '');
    assert.doesNotThrow(() => writeLocalAccountEmail('user@example.com'));
  });
});
