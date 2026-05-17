#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REQUIRED_FILE_PATHS = [
  'index.html',
  'asset-manifest.json',
  'export-package.json',
  'data-snapshot.json',
  'data-quality-report.json',
  'visual-bridge.json',
  'modules.json',
  'runtime-requirements.json',
  'render-spec.json',
  'README.md',
];

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const item = argv[index];
    if (!item.startsWith('--')) continue;
    parsed[item.slice(2)] = argv[index + 1];
    index += 1;
  }
  return parsed;
}

function safeRelativePath(filePath) {
  if (typeof filePath !== 'string' || filePath.trim().length === 0) return false;
  if (path.isAbsolute(filePath)) return false;
  const normalized = path.normalize(filePath).replaceAll('\\', '/');
  return normalized !== '..' && !normalized.startsWith('../') && !normalized.includes('/../');
}

function readText(filePath, errors, code, label) {
  try {
    return fs.readFileSync(filePath, 'utf8');
  } catch (error) {
    errors.push({ code, message: `${label} is missing or unreadable`, path: filePath, detail: error.message });
    return '';
  }
}

function readJson(filePath, errors, code, label) {
  const text = readText(filePath, errors, code, label);
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch (error) {
    errors.push({ code: `${code}_invalid_json`, message: `${label} must be valid JSON`, path: filePath, detail: error.message });
    return null;
  }
}

function addCheck(checks, errors, name, passed, details, code, errorPath = '') {
  checks.push({ name, status: passed ? 'passed' : 'failed', details });
  if (!passed) {
    errors.push({ code, message: details, path: errorPath });
  }
}

function declaredFiles(manifest, exportPackage) {
  if (Array.isArray(exportPackage?.files)) return exportPackage.files;
  if (Array.isArray(manifest?.export_package?.files)) return manifest.export_package.files;
  return [];
}

function filePaths(files) {
  if (!Array.isArray(files)) return [];
  return files.map((file) => file?.path).filter((filePath) => typeof filePath === 'string');
}

function stringListsEqual(left, right) {
  return left.length === right.length && left.every((item, index) => item === right[index]);
}

function flatObjectsEqual(left, right) {
  if (!left || !right || typeof left !== 'object' || typeof right !== 'object') return false;
  if (Array.isArray(left) || Array.isArray(right)) return false;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return stringListsEqual(leftKeys, rightKeys) && leftKeys.every((key) => left[key] === right[key]);
}

function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (!value || typeof value !== 'object') return value;
  return Object.keys(value)
    .sort()
    .reduce((acc, key) => {
      acc[key] = canonicalJson(value[key]);
      return acc;
    }, {});
}

function jsonValuesEqual(left, right) {
  return JSON.stringify(canonicalJson(left)) === JSON.stringify(canonicalJson(right));
}

function browserDeliveryContractReady(contract) {
  return contract?.entry === 'index.html'
    && contract?.remote_scripts_allowed === false
    && contract?.deterministic_chart_fallback === true
    && contract?.optional_echarts_hydration === 'safe_json_option_islands'
    && contract?.mobile_viewport === 'responsive_no_horizontal_overflow_expected';
}

export function validateStaticPageExportArtifact(artifactDir) {
  const artifact = path.resolve(artifactDir || '.');
  const errors = [];
  const checks = [];
  const exists = fs.existsSync(artifact) && fs.statSync(artifact).isDirectory();

  addCheck(checks, errors, 'artifact directory exists', exists, `artifact=${artifact}`, 'artifact_directory_missing', artifact);
  if (!exists) {
    return buildReport({ artifact, checks, errors });
  }

  const manifestPath = path.join(artifact, 'asset-manifest.json');
  const exportPackagePath = path.join(artifact, 'export-package.json');
  const htmlPath = path.join(artifact, 'index.html');
  const manifest = readJson(manifestPath, errors, 'asset_manifest_missing', 'asset-manifest.json');
  const exportPackage = readJson(exportPackagePath, errors, 'export_package_missing', 'export-package.json');
  const html = readText(htmlPath, errors, 'index_html_missing', 'index.html');
  const dataQualityReport = readJson(path.join(artifact, 'data-quality-report.json'), errors, 'data_quality_report_missing', 'data-quality-report.json');
  const dataSnapshot = readJson(path.join(artifact, 'data-snapshot.json'), errors, 'data_snapshot_missing', 'data-snapshot.json');
  const visualBridge = readJson(path.join(artifact, 'visual-bridge.json'), errors, 'visual_bridge_missing', 'visual-bridge.json');
  const modules = readJson(path.join(artifact, 'modules.json'), errors, 'modules_missing', 'modules.json');
  const runtimeRequirements = readJson(path.join(artifact, 'runtime-requirements.json'), errors, 'runtime_requirements_missing', 'runtime-requirements.json');
  const renderSpec = readJson(path.join(artifact, 'render-spec.json'), errors, 'render_spec_missing', 'render-spec.json');
  const readme = readText(path.join(artifact, 'README.md'), errors, 'readme_missing', 'README.md');
  const files = declaredFiles(manifest, exportPackage);
  const manifestFiles = manifest?.export_package?.files;
  const exportPackageFiles = exportPackage?.files;
  const manifestFilePaths = filePaths(manifestFiles);
  const exportPackageFilePaths = filePaths(exportPackageFiles);
  const manifestBrowserContract = manifest?.export_package?.browser_delivery_contract || {};
  const exportPackageBrowserContract = exportPackage?.browser_delivery_contract || {};

  addCheck(
    checks,
    errors,
    'renderer manifest is static-page renderer output',
    manifest?.renderer === 'static-page-renderer-v1' && manifest?.design_contract?.final_role === 'html_css_svg_renderer',
    'asset-manifest.json must identify the static-page renderer and final HTML role.',
    'renderer_manifest_contract_invalid',
    manifestPath,
  );
  addCheck(
    checks,
    errors,
    'export package manifest mirrors asset manifest',
    exportPackage?.kind === manifest?.export_package?.kind
      && exportPackage?.status === manifest?.export_package?.status
      && Array.isArray(manifestFiles)
      && Array.isArray(exportPackageFiles)
      && stringListsEqual(exportPackageFilePaths, manifestFilePaths),
    'export-package.json must mirror the export_package block in asset-manifest.json.',
    'export_package_manifest_mismatch',
    exportPackagePath,
  );

  const unsafePaths = files.map((file) => file?.path).filter((filePath) => !safeRelativePath(filePath));
  addCheck(
    checks,
    errors,
    'declared package paths are safe',
    unsafePaths.length === 0,
    unsafePaths.length ? `unsafe paths: ${unsafePaths.join(', ')}` : `${files.length} declared paths are relative and safe.`,
    'unsafe_declared_package_path',
    exportPackagePath,
  );

  const declaredPaths = new Set(files.map((file) => file?.path).filter(Boolean));
  const missingRequired = REQUIRED_FILE_PATHS.filter((filePath) => !declaredPaths.has(filePath));
  addCheck(
    checks,
    errors,
    'all required handoff files are declared',
    missingRequired.length === 0,
    missingRequired.length ? `missing declarations: ${missingRequired.join(', ')}` : `${REQUIRED_FILE_PATHS.length} required files are declared.`,
    'required_handoff_file_not_declared',
    exportPackagePath,
  );

  const missingDeclared = files
    .map((file) => file?.path)
    .filter((filePath) => safeRelativePath(filePath))
    .filter((filePath) => !fs.existsSync(path.join(artifact, filePath)));
  addCheck(
    checks,
    errors,
    'all declared package files exist',
    missingDeclared.length === 0,
    missingDeclared.length ? `missing files: ${missingDeclared.join(', ')}` : `${files.length} declared files exist.`,
    'declared_handoff_file_missing',
    artifact,
  );

  addCheck(
    checks,
    errors,
    'index html is browser-ready and offline-safe',
    html.startsWith('<!doctype html>')
      && html.includes('<meta name="viewport" content="width=device-width, initial-scale=1">')
      && html.includes('class="module-grid"')
      && !/<script[^>]+\bsrc=/iu.test(html)
      && !/数据待确认|等待确认图表数据/u.test(html),
    'index.html must be complete, responsive, free of remote scripts, and free of missing-data placeholders.',
    'index_html_contract_invalid',
    htmlPath,
  );

  const manifestDataQualitySummary = manifest?.chart_runtime?.dataQualitySummary || {};
  const manifestQualityModules = manifest?.chart_runtime?.modules;
  const attentionModules = manifestDataQualitySummary?.attentionModules;
  addCheck(
    checks,
    errors,
    'data quality report mirrors ready manifest',
    dataQualityReport?.kind === 'static-page-data-quality-report'
      && jsonValuesEqual(dataQualityReport?.summary, manifestDataQualitySummary)
      && attentionModules === 0
      && Array.isArray(dataQualityReport?.modules)
      && dataQualityReport.modules.length === manifestQualityModules?.length,
    'data-quality-report.json must mirror the ready module-quality summary with zero attention modules.',
    'data_quality_report_contract_invalid',
    path.join(artifact, 'data-quality-report.json'),
  );

  addCheck(
    checks,
    errors,
    'data quality module details mirror renderer manifest',
    Array.isArray(dataQualityReport?.modules)
      && Array.isArray(manifestQualityModules)
      && jsonValuesEqual(dataQualityReport.modules, manifestQualityModules),
    'data-quality-report.json modules must mirror chart_runtime.modules in asset-manifest.json.',
    'data_quality_modules_manifest_mismatch',
    path.join(artifact, 'data-quality-report.json'),
  );

  addCheck(
    checks,
    errors,
    'module plan mirrors renderer manifest',
    Array.isArray(modules)
      && Array.isArray(manifest?.modules)
      && jsonValuesEqual(modules, manifest.modules),
    'modules.json must mirror the editable module plan embedded in asset-manifest.json.',
    'modules_manifest_mismatch',
    path.join(artifact, 'modules.json'),
  );

  addCheck(
    checks,
    errors,
    'render spec mirrors renderer manifest',
    renderSpec?.componentModel === 'dom-text-svg-chart'
      && renderSpec?.responsive === true
      && manifest?.render_spec
      && jsonValuesEqual(renderSpec, manifest.render_spec),
    'render-spec.json must mirror asset-manifest.json render_spec and preserve the responsive DOM/SVG chart renderer contract.',
    'render_spec_manifest_mismatch',
    path.join(artifact, 'render-spec.json'),
  );

  addCheck(
    checks,
    errors,
    'supporting JSON files are self-contained',
    typeof dataSnapshot?.source === 'string'
      && visualBridge?.kind === 'static-page-visual-bridge'
      && Array.isArray(modules)
      && Array.isArray(runtimeRequirements)
      && runtimeRequirements.some((item) => item?.license === 'Apache-2.0')
      && typeof renderSpec === 'object'
      && renderSpec !== null,
    'data-snapshot, visual-bridge, runtime-requirements, and render-spec must be parseable without the main manifest.',
    'supporting_json_contract_invalid',
    artifact,
  );

  addCheck(
    checks,
    errors,
    'browser delivery contract is direct and offline-safe',
    browserDeliveryContractReady(manifestBrowserContract)
      && browserDeliveryContractReady(exportPackageBrowserContract)
      && flatObjectsEqual(exportPackageBrowserContract, manifestBrowserContract),
    'browser delivery contract must be valid in both manifests and match exactly.',
    'browser_delivery_contract_invalid',
    exportPackagePath,
  );

  addCheck(
    checks,
    errors,
    'handoff README points reviewers to key files',
    readme.includes('index.html')
      && readme.includes('data-quality-report.json')
      && readme.includes('visual-bridge.json')
      && readme.includes('runtime-requirements.json'),
    'README.md must summarize the direct HTML, data quality report, visual bridge, and runtime requirements.',
    'readme_contract_invalid',
    path.join(artifact, 'README.md'),
  );

  return buildReport({ artifact, checks, errors });
}

function buildReport({ artifact, checks, errors }) {
  const uniqueErrors = [];
  const seen = new Set();
  for (const error of errors.filter((item) => item?.code)) {
    const key = `${error.code}:${error.path}:${error.message}`;
    if (seen.has(key)) continue;
    seen.add(key);
    uniqueErrors.push(error);
  }
  return {
    validator: 'static-page-export-artifact',
    ready: uniqueErrors.length === 0 && checks.every((check) => check.status === 'passed'),
    artifact,
    summary: {
      checks: checks.length,
      passed: checks.filter((check) => check.status === 'passed').length,
      failed: checks.filter((check) => check.status === 'failed').length,
    },
    checks,
    errors: uniqueErrors,
  };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const artifact = args.artifact || args.dir || args.package;
  if (!artifact) {
    console.error('Usage: node tools/validate-static-page-export-artifact.mjs --artifact <artifact-dir> [--out report.json]');
    process.exit(2);
  }
  const report = validateStaticPageExportArtifact(artifact);
  const output = JSON.stringify(report, null, 2);
  if (args.out) {
    fs.mkdirSync(path.dirname(path.resolve(args.out)), { recursive: true });
    fs.writeFileSync(args.out, output);
  } else {
    process.stdout.write(`${output}\n`);
  }
  if (!report.ready) {
    process.exit(1);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main();
}
