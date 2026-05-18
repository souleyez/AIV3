#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderHtmlArtifactDocument } from '../apps/web/app/lib/html-artifact-manifest.js';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const DEFAULT_MANIFEST = 'docs/smokes/new-world-ioa-static-page-handoff.json';
const DEFAULT_HTML_OUTPUT = 'target/html-artifacts/new-world-ioa-static-page-handoff.html';
const DEFAULT_MANIFEST_OUTPUT = 'target/html-artifacts/new-world-ioa-static-page-handoff.json';

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith('--')) {
      continue;
    }
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith('--')) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

function resolveFromRoot(value) {
  return path.resolve(ROOT, value);
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function ensureParentDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

export function renderStaticPagePlanningSmoke({
  manifestPath = resolveFromRoot(DEFAULT_MANIFEST),
  htmlOutputPath = resolveFromRoot(DEFAULT_HTML_OUTPUT),
  manifestOutputPath = resolveFromRoot(DEFAULT_MANIFEST_OUTPUT),
  check = false,
} = {}) {
  const manifest = readJson(manifestPath);
  const rendered = renderHtmlArtifactDocument(manifest);
  if (rendered.rejected) {
    throw new Error(`HTML artifact manifest rejected: ${rendered.reason}`);
  }
  if (rendered.sandbox !== '') {
    throw new Error(`Expected read-only sandbox, got ${rendered.sandbox || '<empty>'}`);
  }

  const manifestText = stableJson(manifest);
  if (check) {
    const existingHtml = fs.existsSync(htmlOutputPath) ? fs.readFileSync(htmlOutputPath, 'utf8') : '';
    const existingManifest = fs.existsSync(manifestOutputPath) ? fs.readFileSync(manifestOutputPath, 'utf8') : '';
    if (existingHtml !== rendered.html || existingManifest !== manifestText) {
      throw new Error('Static-page planning smoke HTML is out of date; run npm run build:static-page-planning-smoke-html');
    }
  } else {
    ensureParentDir(htmlOutputPath);
    ensureParentDir(manifestOutputPath);
    fs.writeFileSync(htmlOutputPath, rendered.html, 'utf8');
    fs.writeFileSync(manifestOutputPath, manifestText, 'utf8');
  }

  return {
    manifestPath,
    htmlOutputPath,
    manifestOutputPath,
    bytes: rendered.html.length,
    sandbox: rendered.sandbox,
  };
}

if (fileURLToPath(import.meta.url) === path.resolve(process.argv[1] || '')) {
  const args = parseArgs(process.argv.slice(2));
  const result = renderStaticPagePlanningSmoke({
    manifestPath: resolveFromRoot(args.manifest || DEFAULT_MANIFEST),
    htmlOutputPath: resolveFromRoot(args.output || DEFAULT_HTML_OUTPUT),
    manifestOutputPath: resolveFromRoot(args['manifest-output'] || DEFAULT_MANIFEST_OUTPUT),
    check: Boolean(args.check),
  });
  console.log(JSON.stringify(result, null, 2));
}
