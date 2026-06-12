import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { buildAutoDatasetIdentity } from './dataset-identity.js';

describe('dataset identity helpers', () => {
  it('builds deterministic auto dataset identity from count, time, and random source', () => {
    const now = new Date('2026-06-12T02:03:00.000Z');
    const random = () => 0.123456789;
    const titleTime = now.toLocaleString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });

    assert.deepEqual(buildAutoDatasetIdentity(3, { now, random }), {
      key: `dataset-${now.getTime().toString(36)}-${random().toString(36).slice(2, 7)}`,
      title: `新数据集 4 · ${titleTime}`,
    });
  });

  it('defaults the next dataset number to one', () => {
    const now = new Date('2026-01-02T03:04:00.000Z');
    const identity = buildAutoDatasetIdentity(undefined, { now, random: () => 0.5 });

    assert.equal(identity.key, `dataset-${now.getTime().toString(36)}-${(0.5).toString(36).slice(2, 7)}`);
    assert.match(identity.title, /^新数据集 1 · /);
  });
});
