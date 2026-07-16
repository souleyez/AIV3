import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { loadCatalogInStages } from './catalog-bootstrap.js';

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('catalog bootstrap', () => {
  it('publishes datasets before slower secondary catalog requests finish', async () => {
    const documents = deferred();
    const datasetsReady = deferred();
    const requestedEndpoints = [];
    const fetchJson = (endpoint) => {
      requestedEndpoints.push(endpoint);
      if (endpoint === '/api/v3/datasets') return Promise.resolve([{ id: 'dataset-1' }]);
      if (endpoint === '/api/v3/documents') return documents.promise;
      return Promise.resolve([]);
    };

    const loadPromise = loadCatalogInStages(fetchJson, (datasetItems) => {
      datasetsReady.resolve(datasetItems);
    });

    assert.deepEqual(await datasetsReady.promise, [{ id: 'dataset-1' }]);
    assert.equal(requestedEndpoints[0], '/api/v3/datasets');
    const pending = Symbol('pending');
    assert.equal(await Promise.race([loadPromise, Promise.resolve(pending)]), pending);

    documents.resolve([{ id: 'document-1' }]);
    assert.deepEqual(await loadPromise, {
      planItems: [],
      reportItems: [],
      documentItems: [{ id: 'document-1' }],
    });
  });

  it('keeps datasets available when a secondary catalog request fails', async () => {
    const failure = new Error('documents unavailable');
    const never = deferred();
    let readyDatasets = null;
    const fetchJson = (endpoint) => {
      if (endpoint === '/api/v3/datasets') return Promise.resolve([{ id: 'dataset-1' }]);
      if (endpoint === '/api/v3/documents') return Promise.reject(failure);
      if (endpoint === '/api/v3/report-plans') return never.promise;
      return Promise.resolve([]);
    };

    await assert.rejects(
      () => loadCatalogInStages(fetchJson, (datasetItems) => {
        readyDatasets = datasetItems;
      }),
      failure,
    );
    assert.deepEqual(readyDatasets, [{ id: 'dataset-1' }]);
  });
});
