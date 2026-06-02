import test from 'node:test';
import assert from 'node:assert/strict';
import { isExternalObservabilityPath } from './platform-api.js';

test('isExternalObservabilityPath protects conversation list and lazy timeline detail', () => {
  assert.equal(isExternalObservabilityPath(['external', 'conversation-tests']), true);
  assert.equal(
    isExternalObservabilityPath([
      'external',
      'conversation-tests',
      '0f02e551-c95b-4a45-b809-d5364bcefd88',
      'timeline',
    ]),
    true,
  );
  assert.equal(isExternalObservabilityPath(['external', 'integrations']), false);
});
