#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { createRequire } from 'node:module';

import {
  layoutCrossDatasetUnderstandingGraph,
} from '../apps/web/app/lib/dataset-understanding-graph.js';
import {
  datasetUnderstandingForceConfig,
  layoutDatasetUnderstandingGraph,
} from '../apps/web/app/lib/dataset-understanding-graph-layout.js';
import { applyDatasetUnderstandingGraphBudget } from '../apps/web/app/lib/dataset-understanding-graph-budget.js';

const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const valueOptions = new Set([
  '--url',
  '--dataset-id',
  '--dataset-title',
  '--performance-runs',
  '--api-url',
  '--api-body-file',
  '--auth-base-url',
]);
const flagOptions = new Set([
  '--require-browser',
  '--require-performance',
  '--measure-fps',
  '--expect-feature-off',
  '--expect-cross-enabled',
  '--auth-from-env',
]);

function validateArgs(argv) {
  const seen = new Set();
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (valueOptions.has(argument)) {
      if (seen.has(argument)) throw new Error(`${argument} may only be supplied once`);
      const value = argv[index + 1];
      if (!value || value.startsWith('--')) throw new Error(`${argument} requires a value`);
      seen.add(argument);
      index += 1;
    } else if (flagOptions.has(argument)) {
      if (seen.has(argument)) throw new Error(`${argument} may only be supplied once`);
      seen.add(argument);
    } else {
      throw new Error(`unknown argument: ${argument}`);
    }
  }
}

validateArgs(args);

function optionValue(name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] || '' : '';
}

const url = optionValue('--url');
const requireBrowser = args.includes('--require-browser');
const requirePerformance = args.includes('--require-performance');
const measureFps = args.includes('--measure-fps');
const expectFeatureOff = args.includes('--expect-feature-off');
const expectCrossEnabled = args.includes('--expect-cross-enabled');
const performanceRunsInput = Number(optionValue('--performance-runs') || 0);
const performanceRuns = Number.isInteger(performanceRunsInput) && performanceRunsInput >= 5
  ? performanceRunsInput
  : 0;
const apiUrl = optionValue('--api-url');
const apiBodyFile = optionValue('--api-body-file');
const datasetId = optionValue('--dataset-id');
const datasetTitle = optionValue('--dataset-title');
const authFromEnv = args.includes('--auth-from-env');
const authBaseUrl = optionValue('--auth-base-url');

if (datasetTitle && !datasetId) throw new Error('--dataset-title requires --dataset-id');
if (datasetId && !url) throw new Error('--dataset-id requires --url');
if (expectFeatureOff && expectCrossEnabled) throw new Error('--expect-feature-off and --expect-cross-enabled are mutually exclusive');
if (expectCrossEnabled && !datasetId) throw new Error('--expect-cross-enabled requires --dataset-id');
if (authBaseUrl && !authFromEnv) throw new Error('--auth-base-url requires --auth-from-env');
if (authFromEnv && !url) throw new Error('--auth-from-env requires --url');
if (optionValue('--performance-runs') && performanceRuns === 0) {
  throw new Error('--performance-runs must be an integer of at least 5');
}

function percentile(values, percentileValue) {
  assert.ok(values.length > 0, 'percentile requires at least one sample');
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.max(0, Math.ceil((percentileValue / 100) * sorted.length) - 1);
  return sorted[index];
}

function rounded(value) {
  return Number(value.toFixed(2));
}

function assertIncludes(source, text, label) {
  assert.ok(source.includes(text), `${label}: missing ${JSON.stringify(text)}`);
}

function graphFixture() {
  const nodes = [{ id: 'dataset:fixture', kind: 'dataset', symbolSize: 72 }];
  const links = [];
  for (let objectIndex = 0; objectIndex < 9; objectIndex += 1) {
    const objectId = `object:${String(objectIndex).padStart(2, '0')}`;
    nodes.push({ id: objectId, kind: 'object', entityType: 'object', symbolSize: 36 });
    links.push({
      id: `root:${objectId}`,
      source: 'dataset:fixture',
      target: objectId,
      type: 'observed',
      structural: true,
      rootRelation: true,
    });
  }
  for (let fieldIndex = 0; fieldIndex < 155; fieldIndex += 1) {
    const objectIndex = fieldIndex % 9;
    const objectId = `object:${String(objectIndex).padStart(2, '0')}`;
    const fieldId = `field:${String(fieldIndex).padStart(3, '0')}`;
    nodes.push({
      id: fieldId,
      kind: 'field',
      entityType: 'field',
      objectId,
      businessScore: 1000 - fieldIndex,
    });
    links.push({
      id: `membership:${fieldId}`,
      source: objectId,
      target: fieldId,
      type: 'observed',
      structural: true,
      rootRelation: false,
    });
  }
  for (let index = 0; index < 120; index += 1) {
    links.push({
      id: `semantic:${String(index).padStart(3, '0')}`,
      source: `field:${String(index).padStart(3, '0')}`,
      target: `field:${String((index + 17) % 155).padStart(3, '0')}`,
      type: index % 4 === 0 ? 'confirmed' : 'inferred',
      structural: false,
    });
  }
  return { nodes, links };
}

async function staticFixtureSmoke() {
  const componentPath = resolve(rootDir, 'apps/web/app/components/DatasetUnderstandingGraph.js');
  const cssPath = resolve(rootDir, 'apps/web/app/globals.css');
  const graphModelPath = resolve(rootDir, 'apps/web/app/lib/dataset-understanding-graph.js');
  const [component, css, graphModel] = await Promise.all([
    readFile(componentPath, 'utf8'),
    readFile(cssPath, 'utf8'),
    readFile(graphModelPath, 'utf8'),
  ]);

  const fixture = graphFixture();
  const standard = applyDatasetUnderstandingGraphBudget(fixture, { density: 'standard' });
  const expanded = applyDatasetUnderstandingGraphBudget(fixture, { density: 'expanded' });
  const layoutSamples = [];
  const projectionSamples = [];
  let laidOut = [];
  for (let index = 0; index < 30; index += 1) {
    let startedAt = performance.now();
    laidOut = layoutDatasetUnderstandingGraph(standard.nodes);
    layoutSamples.push(performance.now() - startedAt);
    startedAt = performance.now();
    applyDatasetUnderstandingGraphBudget(fixture, {
      density: index % 2 === 0 ? 'standard' : 'expanded',
      selectedNodeId: `field:${String(index).padStart(3, '0')}`,
    });
    projectionSamples.push(performance.now() - startedAt);
  }
  const layoutP95Ms = percentile(layoutSamples, 95);
  const projectionP95Ms = percentile(projectionSamples, 95);
  const root = laidOut.find((node) => node.kind === 'dataset');
  const fullLayout = layoutDatasetUnderstandingGraph(fixture.nodes);
  const fullLayoutById = new Map(fullLayout.map((node) => [node.id, node]));
  const expandedLayout = expanded.nodes.map((node) => fullLayoutById.get(node.id)).filter(Boolean);
  const crossNodes = [
    { id: 'dataset:left', kind: 'dataset', datasetRefs: ['left'], rootDataset: true },
    { id: 'dataset:right', kind: 'dataset', datasetRefs: ['right'] },
    ...Array.from({ length: 24 }, (_, index) => ({
      id: `shared:${String(index).padStart(2, '0')}`,
      kind: 'field',
      shared: true,
      datasetRefs: ['left', 'right'],
      symbolSize: 24,
    })),
  ];
  const crossClusters = [{ id: 'left' }, { id: 'right' }];
  const crossLayout = layoutCrossDatasetUnderstandingGraph(crossNodes, crossClusters);
  const reversedCrossLayout = new Map(
    layoutCrossDatasetUnderstandingGraph([...crossNodes].reverse(), [...crossClusters].reverse())
      .map((node) => [node.id, { x: node.x, y: node.y }]),
  );
  const sharedNodes = crossLayout.filter((node) => node.shared);
  const sharedDistances = sharedNodes.flatMap((node, index) => (
    sharedNodes.slice(index + 1).map((other) => Math.hypot(node.x - other.x, node.y - other.y))
  ));
  const minimumSharedDistancePx = Math.min(...sharedDistances);

  assert.equal(standard.nodes.length, 100, 'standard fixture should exercise exactly 100 nodes');
  assert.ok(standard.links.length <= 180, 'standard fixture should stay within 180 edges');
  assert.equal(expanded.nodes.length, 160, 'expanded fixture should exercise exactly 160 nodes');
  assert.ok(expanded.links.length <= 240, 'expanded fixture should stay within 240 edges');
  assert.equal(expandedLayout.length, expanded.nodes.length, 'expanded nodes must all reuse the full-model coordinate map');
  assert.ok(expanded.links.every((link) => (
    expanded.nodes.some((node) => node.id === link.source)
      && expanded.nodes.some((node) => node.id === link.target)
  )), 'expanded edges must retain valid endpoints');
  assert.deepEqual(
    { x: root?.x, y: root?.y, fixed: root?.fixed, symbolSize: root?.symbolSize },
    { x: 0, y: 0, fixed: true, symbolSize: 42 },
    'dataset root should be a fixed 42px node at the origin',
  );
  assert.ok(layoutP95Ms < 100, `100-node fixture layout p95 took ${layoutP95Ms.toFixed(2)}ms`);
  assert.ok(projectionP95Ms < 100, `selection/budget projection p95 took ${projectionP95Ms.toFixed(2)}ms`);
  assert.equal(datasetUnderstandingForceConfig(121).layoutAnimation, false);
  assert.ok(minimumSharedDistancePx >= 48, `cross shared-node spacing was ${minimumSharedDistancePx.toFixed(2)}px`);
  assert.ok(crossLayout.every((node) => {
    const reversed = reversedCrossLayout.get(node.id);
    return reversed?.x === node.x && reversed?.y === node.y;
  }), 'cross layout coordinates must not depend on input order');

  assert.equal((component.match(/echarts\.init\(/g) || []).length, 1, 'ECharts must have one init call site');
  assertIncludes(component, 'echarts.getInstanceByDom(chartRef.current)', 'single-instance lifecycle');
  assertIncludes(component, "replaceMerge: ['series']", 'incremental series update');
  assertIncludes(component, 'lazyUpdate: true', 'lazy ECharts update');
  assertIncludes(component, "layout: staticLayout ? 'none' : 'force'", 'large and cross graph deterministic layout');
  assertIncludes(component, 'const layoutInputNodes = model.nodes;', 'filter-independent layout universe');
  assertIncludes(component, 'staticLayoutExtentAnchors(layoutBaseNodes)', 'stable static layout extent anchors');
  assertIncludes(component, 'new ResizeObserver(scheduleChartResize)', 'resize observer');
  assertIncludes(component, 'window.requestAnimationFrame', 'resize throttling');
  assertIncludes(component, "event.key === 'Escape'", 'focus-mode Escape exit');
  assertIncludes(component, 'dataset.echartsInitCount', 'browser lifecycle instrumentation');
  assertIncludes(component, "chart.on('finished', handleFinished)", 'render-finished instrumentation');
  assertIncludes(component, 'dataset.echartsRenderedRevision', 'render revision instrumentation');
  assertIncludes(component, 'crossGraphAvailable ? (', 'feature-off cross-graph hiding');
  assertIncludes(graphModel, 'const rootId = `dataset:${datasetId}`;', 'single-graph root dataset identity');

  assertIncludes(css, 'grid-template-columns: minmax(0, 1fr) 300px;', 'desktop inspector width');
  assertIncludes(css, 'min-height: clamp(680px, 72vh, 820px);', 'desktop canvas height');
  assertIncludes(css, '@container dataset-understanding (max-width: 1080px)', 'container breakpoint');
  assertIncludes(css, 'height: 380px;', 'phone canvas height');
  assertIncludes(css, '.dataset-understanding-panel.focus-mode {', 'full-screen focus mode');
  assertIncludes(css, 'position: fixed;', 'full-screen focus positioning');

  const benchmarkCanvasPx = Math.min(820, Math.max(680, 0.72 * 1000));
  assert.ok(benchmarkCanvasPx >= 720, '1440x1000 CSS benchmark should allocate at least 720px');

  return {
    status: 'passed',
    mode: 'static-fixture',
    nodeCount: standard.nodes.length,
    edgeCount: standard.links.length,
    expandedNodeCount: expanded.nodes.length,
    expandedEdgeCount: expanded.links.length,
    layoutP95Ms: rounded(layoutP95Ms),
    selectionProjectionP95Ms: rounded(projectionP95Ms),
    centerNodePx: root.symbolSize,
    crossSharedMinimumDistancePx: rounded(minimumSharedDistancePx),
    desktopCanvasCss: 'clamp(680px, 72vh, 820px)',
    desktopBenchmark: { viewport: '1440x1000', canvasPx: benchmarkCanvasPx, method: 'css-expression' },
    echartsInitCallSites: 1,
    performance: {
      layoutAlgorithm: {
        status: 'measured',
        scope: 'offline deterministic layout only',
        samples: layoutSamples.length,
        p95Ms: rounded(layoutP95Ms),
      },
      selectionProjectionAlgorithm: {
        status: 'measured',
        scope: 'offline budget/filter projection only; not browser interaction',
        samples: projectionSamples.length,
        p95Ms: rounded(projectionP95Ms),
      },
      cachedApi: { status: 'skipped', reason: 'requires --url, --api-url, --api-body-file and --performance-runs >=5' },
      firstInteractive: { status: 'skipped', reason: 'requires live browser samples' },
      firstExpansion: { status: 'skipped', reason: 'requires live browser samples' },
      selectionFilterInteraction: { status: 'skipped', reason: 'requires live browser samples' },
      dragZoomFps: { status: 'skipped', reason: 'requires --measure-fps against a live chart' },
    },
  };
}

async function loadPlaywright() {
  try {
    return await import('playwright');
  } catch {
    try {
      return createRequire(import.meta.url)('playwright');
    } catch {
      return null;
    }
  }
}

function sessionCookiePair(value) {
  const match = String(value || '').match(/(?:^|[;,]\s*)(aidp_v3_session=[^;,\s]+)/i);
  return match?.[1] || '';
}

function contextCookie(cookiePair, targetUrl) {
  const separator = cookiePair.indexOf('=');
  assert.ok(separator > 0, 'session cookie was malformed');
  return {
    name: cookiePair.slice(0, separator),
    value: cookiePair.slice(separator + 1),
    url: new URL(targetUrl).origin,
  };
}

function authenticationEnvironment() {
  return {
    cookie: process.env.DATASET_GRAPH_SMOKE_SESSION_COOKIE
      || process.env.V3_AGENT_TERMINAL_SMOKE_SESSION_COOKIE
      || '',
    email: process.env.DATASET_GRAPH_SMOKE_AUTH_EMAIL
      || process.env.V3_AGENT_TERMINAL_SMOKE_AUTH_EMAIL
      || '',
    localKey: process.env.DATASET_GRAPH_SMOKE_LOCAL_KEY
      || process.env.V3_AGENT_TERMINAL_SMOKE_LOCAL_KEY
      || '',
  };
}

async function prepareAuthentication(targetUrl) {
  if (!authFromEnv) {
    return {
      cookie: '',
      createdSession: false,
      baseUrl: '',
      summary: { status: 'skipped', method: 'none', reason: 'pass --auth-from-env to use environment credentials' },
    };
  }
  const credentials = authenticationEnvironment();
  const existingCookie = sessionCookiePair(credentials.cookie);
  if (existingCookie) {
    return {
      cookie: existingCookie,
      createdSession: false,
      baseUrl: '',
      summary: { status: 'authenticated', method: 'environment-session-cookie', loginStatus: null, logoutStatus: 'not-applicable' },
    };
  }
  if (!credentials.email.trim() || !credentials.localKey.trim()) {
    throw new Error('--auth-from-env requires a session cookie or the configured smoke email/local-key environment pair');
  }
  const baseUrl = new URL(authBaseUrl || targetUrl).origin;
  const response = await fetch(new URL('/v1/auth/key/login', baseUrl), {
    method: 'POST',
    headers: {
      accept: 'application/json',
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      email: credentials.email.trim().toLowerCase(),
      local_key: credentials.localKey.trim(),
      device_fingerprint: `dataset-graph-browser-smoke-${Date.now()}`,
    }),
  });
  if (!response.ok) throw new Error(`smoke local-key login failed with HTTP ${response.status}`);
  const setCookieValues = typeof response.headers.getSetCookie === 'function'
    ? response.headers.getSetCookie()
    : [response.headers.get('set-cookie') || ''];
  const cookie = setCookieValues.map(sessionCookiePair).find(Boolean) || '';
  if (!cookie) throw new Error('smoke local-key login did not return the expected session cookie');
  return {
    cookie,
    createdSession: true,
    baseUrl,
    summary: { status: 'authenticated', method: 'environment-local-key-login', loginStatus: response.status, logoutStatus: 'pending' },
  };
}

async function revokeCreatedSession(authentication) {
  if (!authentication.createdSession || !authentication.cookie || !authentication.baseUrl) return;
  try {
    const response = await fetch(new URL('/v1/auth/logout', authentication.baseUrl), {
      method: 'POST',
      headers: { cookie: authentication.cookie, accept: 'application/json' },
    });
    authentication.summary.logoutStatus = response.status;
  } catch {
    authentication.summary.logoutStatus = 'request-failed';
  }
}

async function loadApiBody() {
  if (!apiUrl && !apiBodyFile) return null;
  if (!apiUrl || !apiBodyFile) {
    throw new Error('--api-url and --api-body-file must be supplied together');
  }
  return JSON.parse(await readFile(resolve(apiBodyFile), 'utf8'));
}

async function selectAndVerifyTargetDataset(page, targetDatasetId, expectedDatasetTitle = '') {
  const catalogDataset = await page.evaluate(async (requestedId) => {
    const response = await fetch('/api/v3/datasets', {
      method: 'GET',
      credentials: 'include',
      headers: { accept: 'application/json' },
    });
    if (!response.ok) return { status: response.status, dataset: null };
    const payload = await response.json();
    const datasets = Array.isArray(payload) ? payload : [];
    const dataset = datasets.find((item) => String(item?.id || '') === requestedId);
    return {
      status: response.status,
      dataset: dataset ? {
        id: String(dataset.id || ''),
        title: String(dataset.title || ''),
        key: String(dataset.key || ''),
      } : null,
    };
  }, targetDatasetId);
  assert.equal(catalogDataset.status, 200, `dataset catalog returned HTTP ${catalogDataset.status}`);
  assert.ok(catalogDataset.dataset, `target dataset ${targetDatasetId} was not present in the visible catalog`);
  const target = catalogDataset.dataset;
  assert.equal(target.id, targetDatasetId, 'catalog dataset id did not match --dataset-id');
  if (expectedDatasetTitle) {
    assert.equal(target.title, expectedDatasetTitle, 'catalog dataset title did not match --dataset-title');
  }

  const items = page.locator('.dataset-list .dataset-item:not(.ordinary-chat-item)');
  await items.first().waitFor({ state: 'visible', timeout: 15_000 });
  const matchingIndexes = [];
  for (let index = 0; index < await items.count(); index += 1) {
    const item = items.nth(index);
    const title = (await item.locator('.dataset-item-title').innerText())
      .replace(/\s*已选\s*$/u, '')
      .trim();
    const meta = (await item.locator('.dataset-item-meta').innerText()).trim();
    if (title === target.title && (!target.key || meta.startsWith(`${target.key} ·`))) {
      matchingIndexes.push(index);
    }
  }
  assert.equal(matchingIndexes.length, 1, 'visible catalog must contain exactly one item matching the target id/title/key');
  const item = items.nth(matchingIndexes[0]);
  const clearSelection = page.locator('.dataset-list .ordinary-chat-item');
  assert.equal(await clearSelection.count(), 1, 'catalog smoke requires the normal dataset-selection mode');
  await clearSelection.click();
  await page.waitForFunction(() => !document.querySelector('.dataset-list .dataset-item:not(.ordinary-chat-item).active'));

  const understandingResponse = page.waitForResponse((response) => {
    const path = new URL(response.url()).pathname;
    return path === `/api/v3/datasets/${encodeURIComponent(targetDatasetId)}/understanding`
      && [200, 304].includes(response.status());
  }, { timeout: 15_000 });
  const selectionStartedAt = await page.evaluate(() => performance.now());
  await item.click();
  await understandingResponse;
  const understandingResponseAt = await page.evaluate(() => performance.now());

  await page.waitForFunction(
    (title) => document.querySelector('.dataset-understanding-head h3')?.textContent?.trim() === title,
    target.title,
    { timeout: 15_000 },
  );

  const understanding = await page.evaluate(async (requestedId) => {
    const response = await fetch(`/api/v3/datasets/${encodeURIComponent(requestedId)}/understanding`, {
      method: 'GET',
      credentials: 'include',
      headers: { accept: 'application/json' },
    });
    const payload = response.ok ? await response.json() : null;
    return {
      status: response.status,
      datasetId: String(payload?.dataset?.id || ''),
      datasetTitle: String(payload?.dataset?.title || ''),
      snapshotStatus: String(payload?.status || ''),
    };
  }, targetDatasetId);
  assert.equal(understanding.status, 200, `target understanding endpoint returned HTTP ${understanding.status}`);
  assert.equal(understanding.datasetId, targetDatasetId, 'understanding graph root dataset id did not match --dataset-id');
  assert.equal(understanding.datasetTitle, target.title, 'understanding graph root dataset title did not match the visible catalog');

  const chart = page.locator('.dataset-understanding-chart[data-echarts-ready="true"]');
  await chart.waitFor({ state: 'visible', timeout: 15_000 });
  const renderState = await readRenderedChartState(chart);
  const firstInteractiveMs = renderState.finishedAt - selectionStartedAt;
  assert.equal(renderState.nodeCount, 100, `standard graph rendered ${renderState.nodeCount} nodes instead of 100`);
  assert.ok(renderState.edgeCount <= 180, `standard graph rendered ${renderState.edgeCount} edges`);
  const renderedTitle = (await page.locator('.dataset-understanding-head h3').innerText()).trim();
  const chartLabel = await chart.getAttribute('aria-label');
  assert.equal(renderedTitle, target.title, 'rendered graph title did not match the target dataset');
  assert.ok(chartLabel?.startsWith(target.title), 'rendered chart accessible label did not match the target dataset');
  return {
    id: target.id,
    title: target.title,
    expectedGraphRootId: `dataset:${target.id}`,
    understandingDatasetId: understanding.datasetId,
    understandingDatasetTitle: understanding.datasetTitle,
    snapshotStatus: understanding.snapshotStatus,
    firstInteractiveMs: rounded(firstInteractiveMs),
    understandingResponseMs: rounded(understandingResponseAt - selectionStartedAt),
    responseToFinishedMs: rounded(renderState.finishedAt - understandingResponseAt),
    renderState,
  };
}

async function openDatasetDirectory(page) {
  const datasetPageButton = page.getByRole('button', { name: '数据集', exact: true });
  assert.equal(await datasetPageButton.count(), 1, 'main toolbar must expose one dataset page button');
  if (!await datasetPageButton.evaluate((element) => element.classList.contains('active'))) {
    await datasetPageButton.click();
  }
  await page.locator('.dataset-directory-workspace .dataset-understanding-panel').waitFor({
    state: 'visible',
    timeout: 15_000,
  });
}

async function navigateToReadyChart(page, targetUrl) {
  const startedAt = performance.now();
  await page.goto(targetUrl, { waitUntil: 'domcontentloaded' });
  await openDatasetDirectory(page);
  const pageReadyMs = performance.now() - startedAt;
  const targetDataset = datasetId
    ? await selectAndVerifyTargetDataset(page, datasetId, datasetTitle)
    : null;
  const chart = page.locator('.dataset-understanding-chart[data-echarts-ready="true"]');
  await chart.waitFor({ state: 'visible', timeout: 15_000 });
  return { chart, targetDataset, pageReadyMs: rounded(pageReadyMs) };
}

async function readRenderedChartState(chart) {
  return chart.evaluate((element) => ({
    requestedRevision: Number(element.dataset.echartsRequestedRevision || 0),
    renderedRevision: Number(element.dataset.echartsRenderedRevision || 0),
    nodeCount: Number(element.dataset.echartsNodeCount || 0),
    edgeCount: Number(element.dataset.echartsEdgeCount || 0),
    layout: String(element.dataset.echartsLayout || ''),
    finishedAt: Number(element.dataset.echartsFinishedAt || 0),
  }));
}

async function readScreenPositionSnapshot(chart) {
  const positions = await chart.evaluate((element) => (
    element.__datasetGraphScreenPositionSnapshot?.() || []
  ));
  return new Map(positions.map((position) => [position.id, position]));
}

function screenPositionDrift(beforePositions, afterPositions) {
  const common = [...beforePositions.entries()].filter(([id]) => afterPositions.has(id));
  const maximumPx = Math.max(0, ...common.map(([id, before]) => {
    const after = afterPositions.get(id);
    return Math.hypot(before.x - after.x, before.y - after.y);
  }));
  return { commonCount: common.length, maximumPx };
}

async function waitForChartIdle(page, chart) {
  await page.waitForFunction(() => {
    const element = document.querySelector('.dataset-understanding-chart');
    const requested = Number(element?.dataset.echartsRequestedRevision || 0);
    const rendered = Number(element?.dataset.echartsRenderedRevision || 0);
    return element?.dataset.echartsReady === 'true' && requested > 0 && rendered === requested;
  }, null, { timeout: 10_000 });
  return readRenderedChartState(chart);
}

async function clickAndWaitForChartRender(page, chart, control) {
  const idle = await waitForChartIdle(page, chart);
  const baselineRevision = idle.requestedRevision;
  const startedAt = await chart.evaluate(() => performance.now());
  await control.click();
  await page.waitForFunction(
    ({ previousRevision, actionStartedAt }) => {
      const element = document.querySelector('.dataset-understanding-chart');
      const requested = Number(element?.dataset.echartsRequestedRevision || 0);
      const rendered = Number(element?.dataset.echartsRenderedRevision || 0);
      const finishedAt = Number(element?.dataset.echartsFinishedAt || 0);
      return element?.dataset.echartsReady === 'true'
        && requested > previousRevision
        && rendered === requested
        && finishedAt >= actionStartedAt;
    },
    { previousRevision: baselineRevision, actionStartedAt: startedAt },
    { timeout: 10_000 },
  );
  const state = await readRenderedChartState(chart);
  return {
    elapsedMs: state.finishedAt - startedAt,
    state,
  };
}

async function measureDensityInteraction(page, chart, samples) {
  if (samples < 5) {
    const skipped = { status: 'skipped', reason: 'requires --performance-runs >=5' };
    return { firstExpansion: skipped, selectionFilterInteraction: skipped };
  }
  const expanded = page.getByRole('button', { name: '展开', exact: true });
  const allRelations = page.locator('.dataset-understanding-relation-filter button').filter({ hasText: /^全部关系/u });
  const confirmedRelations = page.locator('.dataset-understanding-relation-filter button').filter({ hasText: /^已确认/u });
  if (await expanded.count() === 0 || await allRelations.count() === 0 || await confirmedRelations.count() === 0) {
    const skipped = { status: 'skipped', reason: 'density or relation controls were not present on the live page' };
    return { firstExpansion: skipped, selectionFilterInteraction: skipped };
  }
  const standardPositions = await readScreenPositionSnapshot(chart);
  const firstExpanded = await clickAndWaitForChartRender(page, chart, expanded);
  const expandedPositions = await readScreenPositionSnapshot(chart);
  const expansionDrift = screenPositionDrift(standardPositions, expandedPositions);
  assert.ok(firstExpanded.state.nodeCount > 120, `expanded graph rendered only ${firstExpanded.state.nodeCount} nodes`);
  assert.ok(firstExpanded.state.nodeCount <= 160, `expanded graph rendered ${firstExpanded.state.nodeCount} nodes`);
  assert.ok(firstExpanded.state.edgeCount <= 240, `expanded graph rendered ${firstExpanded.state.edgeCount} edges`);
  assert.equal(firstExpanded.state.layout, 'none', 'expanded graph did not use deterministic static layout');
  assert.ok(firstExpanded.elapsedMs <= 1500, `first expanded render took ${firstExpanded.elapsedMs.toFixed(2)}ms`);
  assert.ok(expansionDrift.commonCount > 0, 'standard-to-expanded screen-position sample was empty');
  assert.ok(expansionDrift.maximumPx <= 1, `standard-to-expanded screen drift was ${expansionDrift.maximumPx.toFixed(2)}px`);
  await clickAndWaitForChartRender(page, chart, confirmedRelations.first());
  await clickAndWaitForChartRender(page, chart, allRelations.first());
  const baselinePositions = await readScreenPositionSnapshot(chart);
  const elapsed = [];
  for (let index = 0; index < samples; index += 1) {
    const sample = await clickAndWaitForChartRender(
      page,
      chart,
      index % 2 === 0 ? confirmedRelations.first() : allRelations.first(),
    );
    elapsed.push(sample.elapsedMs);
  }
  if (samples % 2 === 1) await clickAndWaitForChartRender(page, chart, allRelations.first());
  const finalPositions = await readScreenPositionSnapshot(chart);
  const filterDrift = screenPositionDrift(baselinePositions, finalPositions);
  assert.ok(filterDrift.commonCount > 0, 'screen-position stability sample was empty');
  assert.ok(filterDrift.maximumPx <= 1, `static graph screen drift was ${filterDrift.maximumPx.toFixed(2)}px`);
  const p95Ms = percentile(elapsed, 95);
  assert.ok(
    p95Ms <= 100,
    `live selection/filter p95 was ${p95Ms.toFixed(2)}ms (samples=${elapsed.map(rounded).join(',')})`,
  );
  return {
    firstExpansion: {
      status: 'measured',
      method: 'first expanded-density click to ECharts finished revision',
      elapsedMs: rounded(firstExpanded.elapsedMs),
      thresholdMs: 1500,
      nodeCount: firstExpanded.state.nodeCount,
      edgeCount: firstExpanded.state.edgeCount,
      layout: firstExpanded.state.layout,
      stableNodeCount: expansionDrift.commonCount,
      maximumScreenDriftPx: rounded(expansionDrift.maximumPx),
      maximumScreenDriftThresholdPx: 1,
    },
    selectionFilterInteraction: {
      status: 'measured',
      method: 'expanded graph relation-filter toggle to ECharts finished revision',
      samples: elapsed.length,
      p95Ms: rounded(p95Ms),
      thresholdMs: 100,
      stableNodeCount: filterDrift.commonCount,
      maximumScreenDriftPx: rounded(filterDrift.maximumPx),
      maximumScreenDriftThresholdPx: 1,
    },
  };
}

async function measureLiveDragZoomFps(chart) {
  if (!measureFps) {
    return { status: 'skipped', reason: 'pass --measure-fps to dispatch live wheel interactions' };
  }
  const measurement = await chart.evaluate(async (element) => {
    const durationMs = 1_200;
    let frameCount = 0;
    let maximumLongTaskMs = 0;
    let observer = null;
    if (typeof PerformanceObserver === 'function') {
      try {
        observer = new PerformanceObserver((list) => {
          list.getEntries().forEach((entry) => {
            maximumLongTaskMs = Math.max(maximumLongTaskMs, entry.duration);
          });
        });
        observer.observe({ entryTypes: ['longtask'] });
      } catch {
        observer = null;
      }
    }
    const startedAt = performance.now();
    const bounds = element.getBoundingClientRect();
    const centerX = bounds.left + (bounds.width / 2);
    const centerY = bounds.top + (bounds.height / 2);
    element.dispatchEvent(new MouseEvent('mousedown', {
      bubbles: true,
      cancelable: true,
      button: 0,
      buttons: 1,
      clientX: centerX,
      clientY: centerY,
    }));
    await new Promise((resolveMeasurement) => {
      const tick = (timestamp) => {
        frameCount += 1;
        const dragOffset = ((frameCount % 24) - 12) * 1.5;
        element.dispatchEvent(new MouseEvent('mousemove', {
          bubbles: true,
          cancelable: true,
          button: 0,
          buttons: 1,
          clientX: centerX + dragOffset,
          clientY: centerY + (dragOffset / 3),
        }));
        element.dispatchEvent(new WheelEvent('wheel', {
          bubbles: true,
          cancelable: true,
          clientX: centerX,
          clientY: centerY,
          deltaY: frameCount % 2 === 0 ? 2 : -2,
        }));
        if (timestamp - startedAt >= durationMs) resolveMeasurement();
        else requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
    element.dispatchEvent(new MouseEvent('mouseup', {
      bubbles: true,
      cancelable: true,
      button: 0,
      buttons: 0,
      clientX: centerX,
      clientY: centerY,
    }));
    observer?.disconnect();
    const elapsedMs = performance.now() - startedAt;
    return {
      frameCount,
      elapsedMs,
      fps: frameCount / (elapsedMs / 1000),
      maximumLongTaskMs,
      longTaskObserverAvailable: observer !== null,
    };
  });
  assert.ok(measurement.fps >= 45, `live drag/zoom frame rate was ${measurement.fps.toFixed(2)}fps`);
  if (measurement.longTaskObserverAvailable) {
    assert.ok(measurement.maximumLongTaskMs <= 200, `live interaction long task was ${measurement.maximumLongTaskMs.toFixed(2)}ms`);
  }
  return {
    status: 'measured',
    method: 'requestAnimationFrame while dispatching drag and wheel interactions on the live ECharts canvas',
    frameCount: measurement.frameCount,
    elapsedMs: rounded(measurement.elapsedMs),
    fps: rounded(measurement.fps),
    minimumFps: 45,
    maximumLongTaskMs: rounded(measurement.maximumLongTaskMs),
    longTaskObserverAvailable: measurement.longTaskObserverAvailable,
  };
}

async function measureCachedApi(page, requestBody) {
  if (!requestBody || performanceRuns < 5) {
    return {
      status: 'skipped',
      reason: 'requires --api-url, --api-body-file and --performance-runs >=5',
    };
  }
  const samples = await page.evaluate(async ({ targetApiUrl, body, runs }) => {
    const request = async (etag = '') => {
      const headers = { 'content-type': 'application/json' };
      if (etag) headers['if-none-match'] = etag;
      const startedAt = performance.now();
      const response = await fetch(targetApiUrl, {
        method: 'POST',
        credentials: 'include',
        headers,
        body: JSON.stringify(body),
      });
      const responseText = await response.text();
      return {
        status: response.status,
        etag: response.headers.get('etag') || '',
        elapsedMs: performance.now() - startedAt,
        bytes: new TextEncoder().encode(responseText).length,
      };
    };
    const first = await request();
    const cached = [];
    for (let index = 0; index < runs; index += 1) cached.push(await request(first.etag));
    return { first, cached };
  }, { targetApiUrl: apiUrl, body: requestBody, runs: performanceRuns });

  assert.equal(samples.first.status, 200, `initial graph API status was ${samples.first.status}`);
  assert.ok(samples.first.etag, 'initial graph API response did not expose an ETag');
  assert.ok(samples.first.bytes < 2_000_000, `initial graph API response was ${samples.first.bytes} bytes`);
  assert.ok(samples.cached.every((sample) => sample.status === 304), 'cached graph API requests must all return 304');
  const p95Ms = percentile(samples.cached.map((sample) => sample.elapsedMs), 95);
  assert.ok(p95Ms < 500, `cached graph API p95 was ${p95Ms.toFixed(2)}ms`);
  return {
    status: 'measured',
    method: 'same-session POST with If-None-Match; response bodies are not reported',
    samples: samples.cached.length,
    p95Ms: rounded(p95Ms),
    thresholdMs: 500,
    firstStatus: samples.first.status,
    cachedStatus: 304,
    responseBytes: samples.first.bytes,
  };
}

async function verifyCrossEnabledUi(page, chart, targetDatasetId) {
  if (!expectCrossEnabled) {
    return { status: 'skipped', reason: 'pass --expect-cross-enabled during a cross-graph canary' };
  }
  await waitForChartIdle(page, chart);
  const before = await chart.evaluate((element) => ({
    instanceId: element.dataset.echartsInstanceId,
    initCount: Number(element.dataset.echartsInitCount || 0),
    requestedRevision: Number(element.dataset.echartsRequestedRevision || 0),
    renderedRevision: Number(element.dataset.echartsRenderedRevision || 0),
  }));
  const crossButton = page.getByRole('button', { name: '跨数据集', exact: true });
  assert.equal(await crossButton.count(), 1, 'cross-dataset mode must be visible when the feature is enabled');
  const uiRequest = page.waitForResponse((response) => (
    new URL(response.url()).pathname === '/api/v3/dataset-semantic-graphs/query'
      && [200, 304].includes(response.status())
  ), { timeout: 15_000 });
  const actionStartedAt = await chart.evaluate(() => performance.now());
  await crossButton.click();
  await uiRequest;
  await page.locator('.dataset-understanding-snapshot-state.ready').waitFor({ state: 'visible', timeout: 15_000 });
  await page.waitForFunction(() => (
    document.querySelector('.dataset-understanding-eyebrow')?.textContent?.includes('跨数据集语义图')
  ));
  await page.waitForFunction(
    ({ previousRevision, startedAt }) => {
      const element = document.querySelector('.dataset-understanding-chart');
      const requested = Number(element?.dataset.echartsRequestedRevision || 0);
      const rendered = Number(element?.dataset.echartsRenderedRevision || 0);
      const finishedAt = Number(element?.dataset.echartsFinishedAt || 0);
      return element?.dataset.echartsReady === 'true'
        && requested > previousRevision
        && rendered === requested
        && finishedAt >= startedAt;
    },
    { previousRevision: before.requestedRevision, startedAt: actionStartedAt },
    { timeout: 10_000 },
  );

  const api = await page.evaluate(async (rootDatasetId) => {
    const response = await fetch('/api/v3/dataset-semantic-graphs/query', {
      method: 'POST',
      credentials: 'include',
      headers: { accept: 'application/json', 'content-type': 'application/json' },
      body: JSON.stringify({
        root_dataset_id: rootDatasetId,
        dataset_ids: [rootDatasetId],
        auto_neighbors: 3,
        max_nodes: 160,
        max_edges: 240,
        depth: 1,
      }),
    });
    const payload = response.ok ? await response.json() : null;
    const datasets = Array.isArray(payload?.datasets) ? payload.datasets : [];
    const nodes = Array.isArray(payload?.nodes) ? payload.nodes : [];
    const edges = Array.isArray(payload?.edges) ? payload.edges : [];
    return {
      status: response.status,
      rootDatasetId: String(payload?.root_dataset_id || ''),
      crossLinksStatus: String(payload?.cross_links_status || ''),
      datasetCount: datasets.length,
      sharedNodeCount: nodes.filter((node) => (
        String(node?.id || '').startsWith('shared:')
          && Array.isArray(node?.dataset_refs)
          && node.dataset_refs.length >= 2
      )).length,
      firstSharedLabel: String(nodes.find((node) => (
        String(node?.id || '').startsWith('shared:')
          && Array.isArray(node?.dataset_refs)
          && node.dataset_refs.length >= 2
      ))?.display_label || ''),
      reliableCrossEdgeCount: edges.filter((edge) => (
        edge?.cross_dataset === true
          && ['confirmed', 'observed'].includes(String(edge?.evidence_class || ''))
      )).length,
    };
  }, targetDatasetId);
  assert.equal(api.status, 200, `cross-graph API returned HTTP ${api.status}`);
  assert.equal(api.rootDatasetId, targetDatasetId, 'cross-graph root dataset did not match --dataset-id');
  assert.equal(api.crossLinksStatus, 'ready', 'cross-graph API was not ready');
  assert.ok(api.datasetCount >= 2, `cross-graph API returned only ${api.datasetCount} visible dataset`);
  assert.ok(
    api.sharedNodeCount > 0 || api.reliableCrossEdgeCount > 0,
    'cross-graph API had neither an exact shared node nor a reliable cross edge',
  );

  const selection = page.locator('.dataset-understanding-cross-selection');
  const clusterToolbar = page.locator('.dataset-understanding-cluster-toolbar');
  await selection.waitFor({ state: 'visible' });
  await clusterToolbar.waitFor({ state: 'visible' });
  const clusterCount = Math.max(0, (await clusterToolbar.locator('button').count()) - 1);
  assert.ok(clusterCount >= 2, `cross-graph UI rendered only ${clusterCount} dataset cluster`);

  const metrics = await page.locator('.dataset-understanding-metrics').evaluate((element) => {
    const value = (label) => {
      const item = [...element.querySelectorAll('span')]
        .find((candidate) => candidate.querySelector('small')?.textContent?.trim() === label);
      return Number(item?.querySelector('strong')?.textContent || 0);
    };
    return { sharedNodes: value('共享节点'), reliableNeighbors: value('可靠邻居') };
  });
  if (api.sharedNodeCount > 0) {
    assert.ok(metrics.sharedNodes > 0, 'cross-graph shared-node metric was not visible');
    const ledger = page.locator('.dataset-understanding-ledger');
    await ledger.locator('summary').click();
    const sharedLabelVisible = await ledger.locator('section').first().locator('strong').evaluateAll(
      (elements, label) => elements.some((element) => element.textContent?.trim() === label),
      api.firstSharedLabel,
    );
    assert.equal(sharedLabelVisible, true, 'exact shared node was not visible in the graph evidence ledger');
  } else {
    assert.ok(metrics.reliableNeighbors > 0, 'reliable cross-edge evidence was not visible in the UI metrics');
  }

  const after = await chart.evaluate((element) => ({
    instanceId: element.dataset.echartsInstanceId,
    initCount: Number(element.dataset.echartsInitCount || 0),
    renderedRevision: Number(element.dataset.echartsRenderedRevision || 0),
    nodeCount: Number(element.dataset.echartsNodeCount || 0),
    edgeCount: Number(element.dataset.echartsEdgeCount || 0),
    layout: String(element.dataset.echartsLayout || ''),
  }));
  assert.equal(after.instanceId, before.instanceId, 'cross-mode switch must retain the ECharts instance');
  assert.equal(after.initCount, 1, 'cross-mode switch must not reinitialize ECharts');
  assert.equal(after.layout, 'none', 'cross-mode graph must use its deterministic clustered layout');
  assert.ok(after.nodeCount <= 160, `cross-mode graph rendered ${after.nodeCount} nodes`);
  assert.ok(after.edgeCount <= 240, `cross-mode graph rendered ${after.edgeCount} edges`);
  return {
    status: 'measured',
    crossLinksStatus: api.crossLinksStatus,
    datasetCount: api.datasetCount,
    clusterCount,
    exactSharedNodeCount: api.sharedNodeCount,
    reliableCrossEdgeCount: api.reliableCrossEdgeCount,
    sharedNodeVisible: api.sharedNodeCount > 0,
    reliableEvidenceVisible: api.sharedNodeCount === 0 && metrics.reliableNeighbors > 0,
    retainedEchartsInstance: true,
  };
}

async function liveBrowserSmoke(targetUrl) {
  const playwright = await loadPlaywright();
  if (!playwright?.chromium) {
    if (requireBrowser) throw new Error('playwright is unavailable; omit --require-browser for fixture-only validation');
    return { status: 'skipped', mode: 'browser', reason: 'playwright unavailable' };
  }

  const authentication = await prepareAuthentication(targetUrl);
  let browser = null;
  let context = null;
  try {
    const browserExecutable = String(process.env.DATASET_GRAPH_SMOKE_BROWSER_EXECUTABLE || '').trim();
    browser = await playwright.chromium.launch({
      headless: true,
      ...(browserExecutable ? { executablePath: browserExecutable } : {}),
    });
    context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
    if (authentication.cookie) {
      await context.addCookies([contextCookie(authentication.cookie, targetUrl)]);
    }
    const requestBody = await loadApiBody();
    let page = await context.newPage();
    const sampleCount = performanceRuns || 1;
    let navigation = await navigateToReadyChart(page, targetUrl);
    const navigationSamples = navigation.targetDataset?.firstInteractiveMs
      ? [navigation.targetDataset.firstInteractiveMs]
      : [];
    const navigationBreakdowns = navigation.targetDataset ? [navigation.targetDataset] : [];
    const pageReadySamples = [navigation.pageReadyMs];
    for (let index = 1; index < sampleCount; index += 1) {
      const previousPage = page;
      page = await context.newPage();
      navigation = await navigateToReadyChart(page, targetUrl);
      if (navigation.targetDataset?.firstInteractiveMs) {
        navigationSamples.push(navigation.targetDataset.firstInteractiveMs);
      }
      if (navigation.targetDataset) navigationBreakdowns.push(navigation.targetDataset);
      pageReadySamples.push(navigation.pageReadyMs);
      await previousPage.close();
    }
    const chart = navigation.chart;

    const before = await chart.evaluate((element) => ({
      instanceId: element.dataset.echartsInstanceId,
      initCount: Number(element.dataset.echartsInitCount || 0),
      updateCount: Number(element.dataset.echartsUpdateCount || 0),
      renderedRevision: Number(element.dataset.echartsRenderedRevision || 0),
      nodeCount: Number(element.dataset.echartsNodeCount || 0),
      edgeCount: Number(element.dataset.echartsEdgeCount || 0),
      layout: String(element.dataset.echartsLayout || ''),
    }));
    assert.ok(before.instanceId, 'chart should expose its ECharts instance id');
    assert.equal(before.initCount, 1, 'chart should initialize exactly once');

    let featureOff = { status: 'skipped', reason: 'pass --expect-feature-off during a feature-off deployment' };
    if (expectFeatureOff) {
      const currentDatasetButton = page.getByRole('button', { name: '当前数据集', exact: true });
      const crossDatasetButton = page.getByRole('button', { name: '跨数据集', exact: true });
      assert.equal(await currentDatasetButton.count(), 1, 'single-dataset graph mode must remain visible while cross graph is off');
      assert.equal(await crossDatasetButton.count(), 0, 'cross-dataset graph mode must be hidden while the feature is off');
      assert.equal(await page.locator('.dataset-understanding-cross-selection').count(), 0, 'cross-dataset selection must be absent while the feature is off');
      assert.equal(await page.locator('.dataset-understanding-cluster-toolbar').count(), 0, 'cross-dataset cluster controls must be absent while the feature is off');
      featureOff = {
        status: 'measured',
        currentDatasetModeVisible: true,
        crossDatasetModeVisible: false,
        crossDatasetSelectionVisible: false,
      };
    }

    const densityPerformance = await measureDensityInteraction(page, chart, performanceRuns);
    const firstExpansionPerformance = densityPerformance.firstExpansion;
    const selectionPerformance = densityPerformance.selectionFilterInteraction;
    if (selectionPerformance.status === 'skipped') {
      const action = page.locator('.dataset-understanding-local-toolbar button').filter({ hasText: '展开' });
      if (await action.count()) {
        await clickAndWaitForChartRender(page, chart, action.first());
      }
    }
    const crossEnabled = await verifyCrossEnabledUi(page, chart, datasetId);

    const after = await chart.evaluate((element) => ({
      instanceId: element.dataset.echartsInstanceId,
      initCount: Number(element.dataset.echartsInitCount || 0),
      updateCount: Number(element.dataset.echartsUpdateCount || 0),
      renderedRevision: Number(element.dataset.echartsRenderedRevision || 0),
      nodeCount: Number(element.dataset.echartsNodeCount || 0),
      edgeCount: Number(element.dataset.echartsEdgeCount || 0),
      layout: String(element.dataset.echartsLayout || ''),
    }));
    assert.equal(after.instanceId, before.instanceId, 'filter/density updates must retain the ECharts instance');
    assert.equal(after.initCount, 1, 'filter/density updates must not reinitialize ECharts');

    const desktop = await page.locator('.dataset-understanding-chart-shell').evaluate((element) => ({
      height: element.getBoundingClientRect().height,
      inspectorWidth: document.querySelector('.dataset-understanding-inspector')?.getBoundingClientRect().width || 0,
    }));
    assert.ok(Math.round(desktop.height) >= 720 && desktop.height <= 821, `desktop chart height was ${desktop.height}px`);
    assert.ok(desktop.inspectorWidth >= 295 && desktop.inspectorWidth <= 305, `inspector width was ${desktop.inspectorWidth}px`);

    const focusToggle = page.locator('.dataset-understanding-focus-toggle');
    await focusToggle.click();
    await page.locator('.dataset-understanding-panel.focus-mode').waitFor({ state: 'visible' });
    await page.keyboard.press('Escape');
    await page.waitForFunction(() => !document.querySelector('.dataset-understanding-panel.focus-mode'));

    await page.setViewportSize({ width: 390, height: 844 });
    const phone = await page.locator('.dataset-understanding-panel').evaluate((element) => ({
      chartHeight: element.querySelector('.dataset-understanding-chart')?.getBoundingClientRect().height || 0,
      horizontalOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
    }));
    assert.ok(phone.chartHeight >= 379 && phone.chartHeight <= 381, `phone chart height was ${phone.chartHeight}px`);
    assert.equal(phone.horizontalOverflow, false, 'phone viewport should not overflow horizontally');

    await page.setViewportSize({ width: 1440, height: 1000 });
    await chart.waitFor({ state: 'visible' });

    const firstInteractive = performanceRuns && navigationSamples.length
      ? {
          status: 'measured',
          method: 'dataset selection click to ECharts finished revision',
          samples: navigationSamples.length,
          p95Ms: rounded(percentile(navigationSamples, 95)),
          thresholdMs: 1500,
          pageReadyP95Ms: rounded(percentile(pageReadySamples, 95)),
          pageReadyScope: 'diagnostic only; excluded from dataset chart SLO',
        }
      : { status: 'skipped', reason: 'requires --performance-runs >=5 and --dataset-id' };
    if (firstInteractive.status === 'measured') {
      assert.ok(
        firstInteractive.p95Ms <= 1500,
        `first-interactive p95 was ${firstInteractive.p95Ms.toFixed(2)}ms (`
          + `total=${navigationSamples.map(rounded).join(',')}; `
          + `response=${navigationBreakdowns.map((sample) => sample.understandingResponseMs).join(',')}; `
          + `render=${navigationBreakdowns.map((sample) => sample.responseToFinishedMs).join(',')})`,
      );
    }
    const dragZoomPerformance = await measureLiveDragZoomFps(chart);
    const cachedApiPerformance = await measureCachedApi(page, requestBody);

    return {
      status: 'passed',
      mode: 'browser',
      before,
      after,
      desktop,
      phone,
      featureOff,
      crossEnabled,
      authentication: authentication.summary,
      targetDataset: navigation.targetDataset || { status: 'skipped', reason: 'no --dataset-id supplied' },
      performance: {
        cachedApi: cachedApiPerformance,
        firstInteractive,
        firstExpansion: firstExpansionPerformance,
        selectionFilterInteraction: selectionPerformance,
        dragZoomFps: dragZoomPerformance,
      },
    };
  } finally {
    try {
      await context?.close();
    } finally {
      try {
        await browser?.close();
      } finally {
        await revokeCreatedSession(authentication);
      }
    }
  }
}

const staticResult = await staticFixtureSmoke();
const browserResult = url ? await liveBrowserSmoke(url) : {
  status: 'skipped',
  mode: 'browser',
  reason: 'no --url supplied; static fixture needs no credentials',
};

if (requireBrowser) {
  assert.equal(browserResult.status, 'passed', '--require-browser needs --url and an available Playwright Chromium');
}
if (expectFeatureOff) {
  assert.equal(browserResult.featureOff?.status, 'measured', '--expect-feature-off needs a live browser URL');
}
if (expectCrossEnabled) {
  assert.equal(browserResult.crossEnabled?.status, 'measured', '--expect-cross-enabled needs a live browser URL');
}

if (requirePerformance) {
  assert.equal(browserResult.status, 'passed', '--require-performance needs a live browser run');
  for (const [name, metric] of Object.entries(browserResult.performance || {})) {
    assert.equal(metric.status, 'measured', `${name} was ${metric.status || 'missing'}; supply all live performance options`);
  }
}

console.log(JSON.stringify({ static: staticResult, browser: browserResult }, null, 2));
