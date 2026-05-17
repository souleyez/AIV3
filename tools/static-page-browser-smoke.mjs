#!/usr/bin/env node

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const args = parseArgs(process.argv.slice(2));
const htmlPath = requiredPath(args.html, '--html');
const manifestPath = requiredPath(args.manifest, '--manifest');
const screenshotDir = requiredPath(args.screenshotDir, '--screenshot-dir');
const reportJsonPath = requiredPath(args.reportJson, '--report-json');
const reportMdPath = requiredPath(args.reportMd, '--report-md');
const chromeBin = args.chromeBin || findChromeExecutable();

await main();

async function main() {
  fs.mkdirSync(screenshotDir, { recursive: true });
  fs.mkdirSync(path.dirname(reportJsonPath), { recursive: true });
  fs.mkdirSync(path.dirname(reportMdPath), { recursive: true });

  if (!chromeBin) {
    writeReport({
      smoke: 'static-page-browser',
      ready: false,
      skipped: true,
      reason: 'chrome_not_found',
      checks: [{ name: 'Chrome executable is available', status: 'failed', details: 'Set CHROME_BIN to run the browser smoke.' }],
    });
    process.exit(1);
  }

  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const sourceHtml = fs.readFileSync(htmlPath, 'utf8');
  const viewports = [
    { name: 'desktop', width: 1280, height: 900 },
    { name: 'mobile', width: 390, height: 844 },
  ];
  const results = [];

  for (const viewport of viewports) {
    const instrumentedHtmlPath = path.join(screenshotDir, `${viewport.name}-instrumented.html`);
    fs.writeFileSync(instrumentedHtmlPath, instrumentHtml(sourceHtml, viewport.name));
    const screenshotPath = path.join(screenshotDir, `${viewport.name}.png`);
    const dump = await runChromeForViewport({
      chromeBin,
      htmlPath: instrumentedHtmlPath,
      screenshotPath,
      viewport,
    });
    results.push({
      viewport,
      inspection: parseInspectionFromDump(dump.stdout),
      screenshot: {
        path: screenshotPath,
        bytes: fs.statSync(screenshotPath).size,
        dimensions: pngDimensions(fs.readFileSync(screenshotPath)),
      },
      chromeStderr: dump.stderr.trim().slice(-800),
    });
  }

  const report = buildReport({
    chromeBin,
    htmlPath,
    manifestPath,
    screenshotDir,
    manifest,
    results,
  });
  writeReport(report);
  if (!report.ready) {
    process.exit(1);
  }
}

function parseArgs(rawArgs) {
  const parsed = {};
  for (let index = 0; index < rawArgs.length; index += 1) {
    const item = rawArgs[index];
    if (!item.startsWith('--')) continue;
    const key = item.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    parsed[key] = rawArgs[index + 1];
    index += 1;
  }
  return parsed;
}

function requiredPath(value, label) {
  if (!value) {
    throw new Error(`${label} is required`);
  }
  return path.resolve(value);
}

function findChromeExecutable() {
  const candidates = [];
  if (process.env.CHROME_BIN) candidates.push(process.env.CHROME_BIN);
  if (process.platform === 'win32') {
    candidates.push(
      path.join(process.env.LOCALAPPDATA || '', 'ms-playwright'),
      path.join(process.env.PROGRAMFILES || '', 'Google', 'Chrome', 'Application', 'chrome.exe'),
      path.join(process.env['PROGRAMFILES(X86)'] || '', 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
    );
  } else {
    const user = process.env.USER || 'soulzyn';
    candidates.push(
      `/mnt/c/Users/${user}/AppData/Local/ms-playwright`,
      '/mnt/c/Program Files/Google/Chrome/Application/chrome.exe',
      '/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
      '/usr/bin/google-chrome',
      '/usr/bin/chromium',
      '/usr/bin/chromium-browser',
    );
  }
  for (const candidate of candidates.filter(Boolean)) {
    const resolved = resolveChromeCandidate(candidate);
    if (resolved) return resolved;
  }
  return '';
}

function resolveChromeCandidate(candidate) {
  if (!candidate) return '';
  if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return candidate;
  if (!fs.existsSync(candidate) || !fs.statSync(candidate).isDirectory()) return '';
  const stack = [candidate];
  while (stack.length) {
    const current = stack.shift();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name);
      if (entry.isFile() && ['chrome.exe', 'chrome', 'chromium', 'msedge.exe'].includes(entry.name)) {
        return fullPath;
      }
      if (entry.isDirectory() && stack.length < 100) {
        stack.push(fullPath);
      }
    }
  }
  return '';
}

function runChromeForViewport({ chromeBin, htmlPath, screenshotPath, viewport }) {
  const executableLooksWindows = chromeExecutableLooksWindows(chromeBin);
  const chromeArgs = [
    '--headless=new',
    '--disable-gpu',
    '--disable-dev-shm-usage',
    '--no-first-run',
    '--no-default-browser-check',
    '--run-all-compositor-stages-before-draw',
    '--virtual-time-budget=1000',
    '--force-device-scale-factor=1',
    `--window-size=${viewport.width},${viewport.height}`,
    `--screenshot=${pathForChromeArg(screenshotPath, executableLooksWindows)}`,
    '--dump-dom',
    fileUrlForChrome(htmlPath, executableLooksWindows),
  ];
  if (!executableLooksWindows && process.platform !== 'win32') {
    chromeArgs.unshift('--no-sandbox');
  }
  return new Promise((resolve, reject) => {
    const child = spawn(chromeBin, chromeArgs, { stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error('Chrome headless dump timed out'));
    }, 20000);
    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString('utf8');
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString('utf8');
    });
    child.once('error', (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once('exit', (code) => {
      clearTimeout(timer);
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        reject(new Error(`Chrome exited with ${code}: ${stderr}`));
      }
    });
  });
}

function chromeExecutableLooksWindows(executablePath) {
  return /\.exe$/i.test(executablePath) || executablePath.startsWith('/mnt/c/');
}

function pathForChromeArg(filePath, executableLooksWindows) {
  const normalized = path.resolve(filePath).replace(/\\/g, '/');
  if (executableLooksWindows && normalized.startsWith('/mnt/')) {
    const match = normalized.match(/^\/mnt\/([a-z])\/(.*)$/i);
    if (match) {
      return `${match[1].toUpperCase()}:\\${match[2].replace(/\//g, '\\')}`;
    }
  }
  return path.resolve(filePath);
}

function fileUrlForChrome(filePath, executableLooksWindows) {
  const normalized = path.resolve(filePath).replace(/\\/g, '/');
  if (executableLooksWindows && normalized.startsWith('/mnt/')) {
    const match = normalized.match(/^\/mnt\/([a-z])\/(.*)$/i);
    if (match) {
      return `file:///${match[1].toUpperCase()}:/${match[2]}`;
    }
  }
  return pathToFileURL(filePath).href;
}

function instrumentHtml(sourceHtml, viewportName) {
  const script = `<script>
(() => {
  const inspect = ${inspectPage.toString()};
  const write = () => {
    const node = document.createElement('script');
    node.type = 'application/json';
    node.id = 'v3-browser-smoke-result';
    node.textContent = JSON.stringify(inspect(${JSON.stringify(viewportName)}));
    document.body.appendChild(node);
  };
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => setTimeout(write, 50), { once: true });
  } else {
    setTimeout(write, 50);
  }
})();
</script>`;
  return sourceHtml.includes('</body>')
    ? sourceHtml.replace('</body>', `${script}</body>`)
    : `${sourceHtml}${script}`;
}

function inspectPage(viewportName) {
  function rectFor(element) {
    const rect = element.getBoundingClientRect();
    return {
      left: Math.round(rect.left),
      top: Math.round(rect.top),
      right: Math.round(rect.right),
      bottom: Math.round(rect.bottom),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    };
  }
  function visible(element) {
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return rect.width > 2 && rect.height > 2 && style.visibility !== 'hidden' && style.display !== 'none';
  }
  const modules = Array.from(document.querySelectorAll('.module'));
  const moduleRects = modules.map(rectFor);
  const overlappingModules = [];
  for (let left = 0; left < moduleRects.length; left += 1) {
    for (let right = left + 1; right < moduleRects.length; right += 1) {
      const a = moduleRects[left];
      const b = moduleRects[right];
      const overlapWidth = Math.max(0, Math.min(a.right, b.right) - Math.max(a.left, b.left));
      const overlapHeight = Math.max(0, Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top));
      if (overlapWidth * overlapHeight > 16) {
        overlappingModules.push([left, right]);
      }
    }
  }
  const textOverflow = Array.from(document.querySelectorAll('h1,h2,p,small,span,strong,b,em,td'))
    .filter((element) => visible(element) && element.scrollWidth > element.clientWidth + 2)
    .map((element) => ({
      tag: element.tagName.toLowerCase(),
      text: (element.textContent || '').trim().slice(0, 80),
      clientWidth: element.clientWidth,
      scrollWidth: element.scrollWidth,
    }));
  const textBoxOverflow = Array.from(document.querySelectorAll('h1,h2,p,small,span,strong,b,em,td'))
    .filter((element) => {
      if (!visible(element)) return false;
      const container = element.closest('.cover,.module,.chart,.kpi-card') || element.parentElement;
      if (!container) return false;
      const rect = element.getBoundingClientRect();
      const containerRect = container.getBoundingClientRect();
      return rect.left < containerRect.left - 2 || rect.right > containerRect.right + 2;
    })
    .map((element) => ({
      tag: element.tagName.toLowerCase(),
      text: (element.textContent || '').trim().slice(0, 80),
      rect: rectFor(element),
      container: rectFor(element.closest('.cover,.module,.chart,.kpi-card') || element.parentElement),
    }));
  return {
    viewportName,
    title: document.title,
    bodyTextLength: document.body.innerText.length,
    moduleCount: modules.length,
    visibleModuleCount: modules.filter(visible).length,
    svgCount: document.querySelectorAll('svg.chart-svg').length,
    echartsOptionCount: document.querySelectorAll('.static-page-echarts-option').length,
    hydrationTargetCount: document.querySelectorAll('.echarts-hydration-target').length,
    remoteScripts: Array.from(document.scripts).map((scriptElement) => scriptElement.src).filter(Boolean),
    missingDataText: /数据待确认|等待确认图表数据/.test(document.body.innerText),
    horizontalOverflow: document.documentElement.scrollWidth > window.innerWidth + 2,
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: window.innerWidth,
    moduleRects,
    overlappingModules,
    textOverflow,
    textBoxOverflow,
  };
}

function parseInspectionFromDump(dump) {
  const match = dump.match(/<script[^>]*id="v3-browser-smoke-result"[^>]*>([\s\S]*?)<\/script>/);
  if (!match) {
    throw new Error('Browser smoke result was not found in dumped DOM');
  }
  return JSON.parse(decodeHtmlEntities(match[1]));
}

function decodeHtmlEntities(value) {
  return value
    .replace(/&quot;/g, '"')
    .replace(/&#x27;/g, "'")
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');
}

function pngDimensions(bytes) {
  if (bytes.length < 24 || bytes.toString('ascii', 1, 4) !== 'PNG') {
    return null;
  }
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
  };
}

function buildReport({ chromeBin, htmlPath, manifestPath, screenshotDir, manifest, results }) {
  const checks = [];
  const add = (name, passed, details) => checks.push({ name, status: passed ? 'passed' : 'failed', details });
  const desktop = results.find((result) => result.viewport.name === 'desktop');
  const mobile = results.find((result) => result.viewport.name === 'mobile');
  for (const result of results) {
    const { name, width, height } = result.viewport;
    const data = result.inspection;
    add(`${name} page opens with expected modules`, data.moduleCount === 3 && data.visibleModuleCount === 3 && data.bodyTextLength > 120, `${data.visibleModuleCount}/${data.moduleCount} modules visible at ${width}x${height}.`);
    add(`${name} charts render as visible HTML/SVG`, data.svgCount >= 2 && data.echartsOptionCount === 1 && data.hydrationTargetCount === 1, `svg=${data.svgCount}, echartsJson=${data.echartsOptionCount}, hydrationTargets=${data.hydrationTargetCount}.`);
    add(`${name} has no missing-data placeholder`, data.missingDataText === false, 'Confirmed fixture must not render data-missing labels.');
    add(`${name} has no horizontal overflow`, data.horizontalOverflow === false, `documentWidth=${data.documentWidth}, viewportWidth=${data.viewportWidth}.`);
    add(`${name} modules do not overlap`, data.overlappingModules.length === 0, `${data.overlappingModules.length} overlapping module pairs.`);
    add(`${name} text does not overflow containers`, data.textOverflow.length === 0 && data.textBoxOverflow.length === 0, `${data.textOverflow.length} overflowing text nodes; ${data.textBoxOverflow.length} clipped text boxes.`);
    add(`${name} screenshot is non-empty`, result.screenshot.bytes > 12000 && result.screenshot.dimensions?.width === width && result.screenshot.dimensions?.height === height, `${result.screenshot.bytes} bytes at ${result.screenshot.dimensions?.width || 0}x${result.screenshot.dimensions?.height || 0}.`);
  }
  add('HTML uses no remote scripts', results.every((result) => result.inspection.remoteScripts.length === 0), 'Static-page artifacts should not load remote scripts.');
  add('manifest remains delivery-ready', manifest.chart_runtime?.dataQualitySummary?.attentionModules === 0 && manifest.export_package?.status === 'rendered', 'Manifest attentionModules must stay zero and export package must be rendered.');
  add(
    'manifest preserves browser delivery contract',
    manifest.export_package?.browser_delivery_contract?.entry === 'index.html'
      && manifest.export_package?.browser_delivery_contract?.remote_scripts_allowed === false
      && manifest.export_package?.browser_delivery_contract?.deterministic_chart_fallback === true
      && manifest.export_package?.browser_delivery_contract?.optional_echarts_hydration === 'safe_json_option_islands'
      && manifest.export_package?.browser_delivery_contract?.mobile_viewport === 'responsive_no_horizontal_overflow_expected',
    'Export package must preserve direct-browser delivery, no remote scripts, deterministic chart fallback, and optional safe ECharts hydration.',
  );
  return {
    smoke: 'static-page-browser',
    ready: checks.every((check) => check.status === 'passed'),
    skipped: false,
    chrome: chromeBin,
    html: htmlPath,
    manifest: manifestPath,
    screenshot_dir: screenshotDir,
    viewports: { desktop, mobile },
    checks,
  };
}

function writeReport(report) {
  fs.writeFileSync(reportJsonPath, JSON.stringify(report, null, 2));
  const lines = [
    '# Static Page Browser Smoke',
    '',
    `- Status: ${report.ready ? 'passed' : report.skipped ? 'skipped' : 'failed'}`,
    report.chrome ? `- Chrome: ${report.chrome}` : '',
    report.html ? `- HTML: ${report.html}` : '',
    report.screenshot_dir ? `- Screenshots: ${report.screenshot_dir}` : '',
    '',
    '## Checks',
    '',
    ...report.checks.map((check) => `- ${check.status}: \`${check.name}\` - ${check.details}`),
    '',
  ].filter((line) => line !== '').join('\n');
  fs.writeFileSync(reportMdPath, lines);
}
