#!/usr/bin/env node

import { readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_ARTIFACT_DIR =
  'target/database-static-pages/xinbai-functional-modular-template-20260604';
const DEFAULT_CONTRACT_PATH =
  'docs/static-page-templates/xinbai-functional-modular-template-20260604/template-contract.json';

function parseArgs(argv) {
  const args = {
    artifactDir: process.env.XINBAI_REPORT_TEMPLATE_ARTIFACT_DIR || DEFAULT_ARTIFACT_DIR,
    contractPath: process.env.XINBAI_REPORT_TEMPLATE_CONTRACT || DEFAULT_CONTRACT_PATH,
    publicUrl: process.env.XINBAI_REPORT_TEMPLATE_PUBLIC_URL || '',
    json: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--artifact-dir') {
      args.artifactDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--contract') {
      args.contractPath = requireValue(arg, next);
      index += 1;
    } else if (arg === '--public-url') {
      args.publicUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--json') {
      args.json = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage:
  npm run validate:xinbai-report-template
  npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html

Options:
  --artifact-dir <path>   Local artifact directory to validate.
  --contract <path>       Template contract JSON path.
  --public-url <url>      Optional public index.html URL; checks required sibling files.
  --json                  Print JSON summary only.
`);
}

async function readText(path) {
  return readFile(path, 'utf8');
}

async function readJson(path) {
  const text = await readText(path);
  return JSON.parse(text);
}

async function fileSize(path) {
  const info = await stat(path);
  return info.size;
}

function assertCondition(condition, message, details = {}) {
  if (!condition) {
    const error = new Error(message);
    error.details = details;
    throw error;
  }
}

function assertIncludes(text, snippet, label) {
  assertCondition(text.includes(snippet), `${label} missing required snippet`, {
    snippet,
  });
}

function normalizeIndexUrl(publicUrl) {
  const url = new URL(publicUrl);
  url.search = '';
  url.hash = '';
  if (!url.pathname.endsWith('/index.html')) {
    url.pathname = `${url.pathname.replace(/\/+$/, '')}/index.html`;
  }
  return url;
}

function siblingUrl(publicUrl, fileName) {
  const url = normalizeIndexUrl(publicUrl);
  const parts = url.pathname.split('/');
  parts.pop();
  parts.push(fileName);
  url.pathname = parts.join('/');
  return url.toString();
}

async function fetchNonEmpty(url) {
  const response = await fetch(url);
  const bytes = new Uint8Array(await response.arrayBuffer()).byteLength;
  return {
    url,
    status: response.status,
    bytes,
    ok: response.ok && bytes > 0,
  };
}

function validateManifest(manifest, contract) {
  assertCondition(manifest.title === contract.title, 'manifest title mismatch', {
    expected: contract.title,
    actual: manifest.title,
  });
  assertCondition(manifest.report_title === contract.title, 'manifest report_title mismatch', {
    expected: contract.title,
    actual: manifest.report_title,
  });
  assertCondition(manifest.kind === contract.kind, 'manifest kind mismatch', {
    expected: contract.kind,
    actual: manifest.kind,
  });
  assertCondition(manifest.is_primary_template === true, 'manifest must mark primary template');

  const features = new Set(Array.isArray(manifest.features) ? manifest.features : []);
  for (const feature of contract.requiredManifestFeatures || []) {
    assertCondition(features.has(feature), 'manifest missing required feature', { feature });
  }

  const exportsByKind = new Map(
    (Array.isArray(manifest.download_exports) ? manifest.download_exports : []).map((item) => [
      item.kind,
      item,
    ]),
  );
  for (const expected of contract.requiredManifestExports || []) {
    const actual = exportsByKind.get(expected.kind);
    assertCondition(Boolean(actual), 'manifest missing required export', { expected });
    assertCondition(actual.label === expected.label, 'manifest export label mismatch', {
      expected,
      actual,
    });
    assertCondition(actual.path === expected.path, 'manifest export path mismatch', {
      expected,
      actual,
    });
  }
}

function validateData(data, contract) {
  const dataArrays = contract.requiredDataArrays || {};
  for (const [key, minLength] of Object.entries(dataArrays)) {
    const value = data[key];
    assertCondition(Array.isArray(value), 'data missing required array', { key });
    assertCondition(value.length >= minLength, 'data array shorter than required', {
      key,
      minLength,
      actual: value.length,
    });
  }
}

async function validateLocalArtifact(args, contract) {
  const sizes = {};
  for (const fileName of contract.requiredFiles || []) {
    const path = join(args.artifactDir, fileName);
    const size = await fileSize(path);
    assertCondition(size > 0, 'required artifact file is empty', { fileName });
    sizes[fileName] = size;
  }

  const html = await readText(join(args.artifactDir, 'index.html'));
  const manifest = await readJson(join(args.artifactDir, 'manifest.json'));
  const data = await readJson(join(args.artifactDir, 'data.json'));
  const reportMarkdown = await readText(join(args.artifactDir, 'report.md'));

  for (const snippet of contract.requiredHtmlSnippets || []) {
    assertIncludes(html, snippet, 'index.html');
  }
  for (const snippet of contract.requiredMarkdownSnippets || []) {
    assertIncludes(reportMarkdown, snippet, 'report.md');
  }

  validateManifest(manifest, contract);
  validateData(data, contract);

  return {
    artifactDir: args.artifactDir,
    fileSizes: sizes,
    title: manifest.title,
    featureCount: Array.isArray(manifest.features) ? manifest.features.length : 0,
    storeCount: Array.isArray(data.storeList) ? data.storeList.length : null,
    opportunityRows: Array.isArray(data.opportunities) ? data.opportunities.length : null,
    lowActivityRows: Array.isArray(data.lowActivity) ? data.lowActivity.length : null,
  };
}

async function validatePublicArtifact(publicUrl, contract) {
  if (!publicUrl) {
    return null;
  }
  const checks = [];
  for (const fileName of contract.requiredFiles || []) {
    const url = siblingUrl(publicUrl, fileName);
    checks.push(await fetchNonEmpty(url));
  }
  for (const check of checks) {
    assertCondition(check.ok, 'public artifact file is not accessible or empty', check);
  }
  return checks;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const contract = await readJson(args.contractPath);
  assertCondition(contract.templateId === 'xinbai-functional-modular-template-20260604', 'unexpected template contract id', {
    templateId: contract.templateId,
  });

  const local = await validateLocalArtifact(args, contract);
  const remote = await validatePublicArtifact(args.publicUrl, contract);
  const summary = {
    status: 'passed',
    contract: args.contractPath,
    templateId: contract.templateId,
    local,
    remote,
  };

  if (args.json) {
    console.log(JSON.stringify(summary, null, 2));
  } else {
    console.log(`Xinbai report template validation passed: ${contract.templateId}`);
    console.log(`- local artifact: ${local.artifactDir}`);
    console.log(`- files checked: ${Object.keys(local.fileSizes).join(', ')}`);
    console.log(`- manifest features: ${local.featureCount}`);
    console.log(`- data rows: stores=${local.storeCount}, opportunities=${local.opportunityRows}, lowActivity=${local.lowActivityRows}`);
    if (remote) {
      console.log(`- public files checked: ${remote.length}`);
    }
  }
}

main().catch((error) => {
  console.error(`Xinbai report template validation failed: ${error.message}`);
  if (error.details) {
    console.error(JSON.stringify(error.details, null, 2));
  }
  process.exit(1);
});
