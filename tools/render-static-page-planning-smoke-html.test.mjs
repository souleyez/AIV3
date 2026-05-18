import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { renderStaticPagePlanningSmoke } from './render-static-page-planning-smoke-html.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const MANIFEST = path.join(ROOT, 'docs/smokes/new-world-ioa-static-page-handoff.json');
const HTML_OUTPUT = path.join(ROOT, 'target/html-artifacts/new-world-ioa-static-page-handoff.test.html');
const MANIFEST_OUTPUT = path.join(ROOT, 'target/html-artifacts/new-world-ioa-static-page-handoff.test.json');

test('renders the New World IOA static-page planning smoke artifact', () => {
  const result = renderStaticPagePlanningSmoke({
    manifestPath: MANIFEST,
    htmlOutputPath: HTML_OUTPUT,
    manifestOutputPath: MANIFEST_OUTPUT,
  });

  assert.equal(result.sandbox, '');
  assert.ok(result.bytes > 1000);

  const html = fs.readFileSync(HTML_OUTPUT, 'utf8');
  const source = JSON.parse(fs.readFileSync(MANIFEST_OUTPUT, 'utf8'));

  assert.equal(source.templateId, 'static_page_planning_handoff');
  assert.match(html, /新世界 IOA/);
  assert.match(html, /模板参考/);
  assert.match(html, /证据状态/);
  assert.match(html, /源结构/);
  assert.match(html, /source_structure_only_no_body_no_sample_rows/);
  assert.match(html, /视觉链路/);
  assert.match(html, /模块规划/);

  const checked = renderStaticPagePlanningSmoke({
    manifestPath: MANIFEST,
    htmlOutputPath: HTML_OUTPUT,
    manifestOutputPath: MANIFEST_OUTPUT,
    check: true,
  });
  assert.equal(checked.bytes, result.bytes);
});
