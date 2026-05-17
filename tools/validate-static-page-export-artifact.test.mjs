import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { validateStaticPageExportArtifact } from './validate-static-page-export-artifact.mjs';

const PACKAGE_FILES = [
  { path: 'index.html', role: 'rendered_static_page', mime: 'text/html' },
  { path: 'asset-manifest.json', role: 'renderer_manifest', mime: 'application/json' },
  { path: 'export-package.json', role: 'export_package_manifest', mime: 'application/json' },
  { path: 'data-snapshot.json', role: 'render_data_snapshot', mime: 'application/json' },
  { path: 'data-quality-report.json', role: 'module_data_quality_report', mime: 'application/json' },
  { path: 'visual-bridge.json', role: 'confirmed_visual_contract_bridge', mime: 'application/json' },
  { path: 'modules.json', role: 'editable_module_plan', mime: 'application/json' },
  { path: 'runtime-requirements.json', role: 'optional_runtime_requirements', mime: 'application/json' },
  { path: 'render-spec.json', role: 'render_contract', mime: 'application/json' },
  { path: 'README.md', role: 'human_handoff_note', mime: 'text/markdown' },
];

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2));
}

function makeArtifact(overrides = {}) {
  const artifact = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-static-page-export-artifact-'));
  const exportPackage = {
    kind: 'static-page-export-package',
    version: 1,
    status: 'rendered',
    files: PACKAGE_FILES,
    browser_delivery_contract: {
      entry: 'index.html',
      layout: 'responsive_static_html',
      remote_scripts_allowed: false,
      deterministic_chart_fallback: true,
      optional_echarts_hydration: 'safe_json_option_islands',
      mobile_viewport: 'responsive_no_horizontal_overflow_expected',
    },
  };
  const manifest = {
    renderer: 'static-page-renderer-v1',
    design_contract: { final_role: 'html_css_svg_renderer' },
    chart_runtime: {
      dataQualitySummary: {
        confirmedModules: 1,
        partialModules: 0,
        missingModules: 0,
        attentionModules: 0,
      },
      modules: [{ moduleId: 'overview', title: '总览', dataQualityStatus: 'confirmed' }],
    },
    export_package: exportPackage,
  };
  const html = [
    '<!doctype html>',
    '<html>',
    '<head><meta name="viewport" content="width=device-width, initial-scale=1"></head>',
    '<body><section class="module-grid"><section class="module">总览</section></section></body>',
    '</html>',
  ].join('');
  const files = {
    'index.html': html,
    'asset-manifest.json': manifest,
    'export-package.json': exportPackage,
    'data-snapshot.json': { source: 'unit-test' },
    'data-quality-report.json': {
      kind: 'static-page-data-quality-report',
      summary: manifest.chart_runtime.dataQualitySummary,
      modules: manifest.chart_runtime.modules,
    },
    'visual-bridge.json': { kind: 'static-page-visual-bridge', status: 'confirmed' },
    'modules.json': [{ id: 'overview', title: '总览' }],
    'runtime-requirements.json': [{ name: 'Apache ECharts', license: 'Apache-2.0', required: false }],
    'render-spec.json': { componentModel: 'dom-text-svg-chart' },
    'README.md': '打开 index.html。检查 data-quality-report.json、visual-bridge.json 和 runtime-requirements.json。\n',
    ...overrides.files,
  };
  for (const [filePath, content] of Object.entries(files)) {
    const fullPath = path.join(artifact, filePath);
    fs.mkdirSync(path.dirname(fullPath), { recursive: true });
    if (typeof content === 'string') {
      fs.writeFileSync(fullPath, content);
    } else {
      writeJson(fullPath, content);
    }
  }
  return artifact;
}

test('validateStaticPageExportArtifact accepts a complete static page artifact', () => {
  const artifact = makeArtifact();
  const report = validateStaticPageExportArtifact(artifact);

  assert.equal(report.ready, true);
  assert.equal(report.errors.length, 0);
  assert.ok(report.checks.some((check) => check.name === 'all declared package files exist'));
});

test('validateStaticPageExportArtifact rejects missing declared files', () => {
  const artifact = makeArtifact();
  fs.unlinkSync(path.join(artifact, 'data-quality-report.json'));

  const report = validateStaticPageExportArtifact(artifact);

  assert.equal(report.ready, false);
  assert.ok(report.errors.some((error) => error.code === 'declared_handoff_file_missing'));
});

test('validateStaticPageExportArtifact rejects remote scripts in index html', () => {
  const artifact = makeArtifact({
    files: {
      'index.html': '<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><section class="module-grid"></section><script src="https://example.com/app.js"></script></body></html>',
    },
  });

  const report = validateStaticPageExportArtifact(artifact);

  assert.equal(report.ready, false);
  assert.ok(report.errors.some((error) => error.code === 'index_html_contract_invalid'));
});

test('validateStaticPageExportArtifact rejects unsafe declared paths', () => {
  const artifact = makeArtifact();
  const manifestPath = path.join(artifact, 'asset-manifest.json');
  const exportPackagePath = path.join(artifact, 'export-package.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const exportPackage = JSON.parse(fs.readFileSync(exportPackagePath, 'utf8'));
  exportPackage.files.push({ path: '../secret.txt', role: 'unsafe', mime: 'text/plain' });
  manifest.export_package = exportPackage;
  writeJson(manifestPath, manifest);
  writeJson(exportPackagePath, exportPackage);

  const report = validateStaticPageExportArtifact(artifact);

  assert.equal(report.ready, false);
  assert.ok(report.errors.some((error) => error.code === 'unsafe_declared_package_path'));
});

test('validateStaticPageExportArtifact rejects export package file list drift', () => {
  const artifact = makeArtifact();
  const exportPackagePath = path.join(artifact, 'export-package.json');
  const exportPackage = JSON.parse(fs.readFileSync(exportPackagePath, 'utf8'));
  exportPackage.files = [exportPackage.files[1], exportPackage.files[0], ...exportPackage.files.slice(2)];
  writeJson(exportPackagePath, exportPackage);

  const report = validateStaticPageExportArtifact(artifact);

  assert.equal(report.ready, false);
  assert.ok(report.errors.some((error) => error.code === 'export_package_manifest_mismatch'));
});

test('validateStaticPageExportArtifact rejects browser delivery contract drift', () => {
  const artifact = makeArtifact();
  const exportPackagePath = path.join(artifact, 'export-package.json');
  const exportPackage = JSON.parse(fs.readFileSync(exportPackagePath, 'utf8'));
  exportPackage.browser_delivery_contract.layout = 'unexpected_layout';
  writeJson(exportPackagePath, exportPackage);

  const report = validateStaticPageExportArtifact(artifact);

  assert.equal(report.ready, false);
  assert.ok(report.errors.some((error) => error.code === 'browser_delivery_contract_invalid'));
});
