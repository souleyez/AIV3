#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import {
  datasetUnderstandingForceConfig,
  layoutDatasetUnderstandingGraph,
} from '../apps/web/app/lib/dataset-understanding-graph-layout.js';

const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const urlIndex = args.indexOf('--url');
const url = urlIndex >= 0 ? args[urlIndex + 1] : '';
const requireBrowser = args.includes('--require-browser');

function assertIncludes(source, text, label) {
  assert.ok(source.includes(text), `${label}: missing ${JSON.stringify(text)}`);
}

function graphFixture() {
  const nodes = [{ id: 'dataset:fixture', kind: 'dataset', symbolSize: 72 }];
  for (let objectIndex = 0; objectIndex < 9; objectIndex += 1) {
    const objectId = `object:${String(objectIndex).padStart(2, '0')}`;
    nodes.push({ id: objectId, kind: 'object', entityType: 'object', symbolSize: 36 });
    for (let fieldIndex = 0; fieldIndex < 10; fieldIndex += 1) {
      nodes.push({
        id: `field:${objectIndex}:${String(fieldIndex).padStart(2, '0')}`,
        kind: 'field',
        entityType: 'field',
        objectId,
        businessScore: 100 - fieldIndex,
      });
    }
  }
  return nodes;
}

async function staticFixtureSmoke() {
  const componentPath = resolve(rootDir, 'apps/web/app/components/DatasetUnderstandingGraph.js');
  const cssPath = resolve(rootDir, 'apps/web/app/globals.css');
  const [component, css] = await Promise.all([
    readFile(componentPath, 'utf8'),
    readFile(cssPath, 'utf8'),
  ]);

  const startedAt = performance.now();
  const laidOut = layoutDatasetUnderstandingGraph(graphFixture());
  const elapsedMs = performance.now() - startedAt;
  const root = laidOut.find((node) => node.kind === 'dataset');

  assert.equal(laidOut.length, 100, 'fixture should exercise a 100-node graph');
  assert.deepEqual(
    { x: root?.x, y: root?.y, fixed: root?.fixed, symbolSize: root?.symbolSize },
    { x: 0, y: 0, fixed: true, symbolSize: 42 },
    'dataset root should be a fixed 42px node at the origin',
  );
  assert.ok(elapsedMs < 100, `100-node fixture layout took ${elapsedMs.toFixed(2)}ms`);
  assert.equal(datasetUnderstandingForceConfig(121).layoutAnimation, false);

  assert.equal((component.match(/echarts\.init\(/g) || []).length, 1, 'ECharts must have one init call site');
  assertIncludes(component, 'echarts.getInstanceByDom(chartRef.current)', 'single-instance lifecycle');
  assertIncludes(component, "replaceMerge: ['series']", 'incremental series update');
  assertIncludes(component, 'lazyUpdate: true', 'lazy ECharts update');
  assertIncludes(component, 'new ResizeObserver(scheduleChartResize)', 'resize observer');
  assertIncludes(component, 'window.requestAnimationFrame', 'resize throttling');
  assertIncludes(component, "event.key === 'Escape'", 'focus-mode Escape exit');
  assertIncludes(component, 'dataset.echartsInitCount', 'browser lifecycle instrumentation');

  assertIncludes(css, 'grid-template-columns: minmax(0, 1fr) 300px;', 'desktop inspector width');
  assertIncludes(css, 'min-height: clamp(680px, 72vh, 820px);', 'desktop canvas height');
  assertIncludes(css, '@container dataset-understanding (max-width: 1080px)', 'container breakpoint');
  assertIncludes(css, 'height: 380px;', 'phone canvas height');
  assertIncludes(css, '.dataset-understanding-panel.focus-mode {', 'full-screen focus mode');
  assertIncludes(css, 'position: fixed;', 'full-screen focus positioning');

  return {
    status: 'passed',
    mode: 'static-fixture',
    nodeCount: laidOut.length,
    layoutMs: Number(elapsedMs.toFixed(2)),
    centerNodePx: root.symbolSize,
    desktopCanvasCss: 'clamp(680px, 72vh, 820px)',
    echartsInitCallSites: 1,
  };
}

async function loadPlaywright() {
  try {
    return await import('playwright');
  } catch {
    return null;
  }
}

async function liveBrowserSmoke(targetUrl) {
  const playwright = await loadPlaywright();
  if (!playwright?.chromium) {
    if (requireBrowser) throw new Error('playwright is unavailable; omit --require-browser for fixture-only validation');
    return { status: 'skipped', mode: 'browser', reason: 'playwright unavailable' };
  }

  const browser = await playwright.chromium.launch({ headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    await page.goto(targetUrl, { waitUntil: 'networkidle' });
    const chart = page.locator('.dataset-understanding-chart[data-echarts-ready="true"]');
    await chart.waitFor({ state: 'visible', timeout: 15_000 });

    const before = await chart.evaluate((element) => ({
      instanceId: element.dataset.echartsInstanceId,
      initCount: Number(element.dataset.echartsInitCount || 0),
      updateCount: Number(element.dataset.echartsUpdateCount || 0),
    }));
    assert.ok(before.instanceId, 'chart should expose its ECharts instance id');
    assert.equal(before.initCount, 1, 'chart should initialize exactly once');

    const action = page.locator('.dataset-understanding-local-toolbar button').filter({ hasText: '展开' });
    if (await action.count()) {
      await action.first().click();
      await page.waitForFunction(
        (previous) => Number(document.querySelector('.dataset-understanding-chart')?.dataset.echartsUpdateCount || 0) > previous,
        before.updateCount,
      );
    }

    const after = await chart.evaluate((element) => ({
      instanceId: element.dataset.echartsInstanceId,
      initCount: Number(element.dataset.echartsInitCount || 0),
      updateCount: Number(element.dataset.echartsUpdateCount || 0),
    }));
    assert.equal(after.instanceId, before.instanceId, 'filter/density updates must retain the ECharts instance');
    assert.equal(after.initCount, 1, 'filter/density updates must not reinitialize ECharts');

    const desktop = await page.locator('.dataset-understanding-chart-shell').evaluate((element) => ({
      height: element.getBoundingClientRect().height,
      inspectorWidth: document.querySelector('.dataset-understanding-inspector')?.getBoundingClientRect().width || 0,
    }));
    assert.ok(desktop.height >= 719 && desktop.height <= 821, `desktop chart height was ${desktop.height}px`);
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

    return { status: 'passed', mode: 'browser', before, after, desktop, phone };
  } finally {
    await browser.close();
  }
}

const staticResult = await staticFixtureSmoke();
const browserResult = url ? await liveBrowserSmoke(url) : {
  status: 'skipped',
  mode: 'browser',
  reason: 'no --url supplied; static fixture needs no credentials',
};

console.log(JSON.stringify({ static: staticResult, browser: browserResult }, null, 2));
