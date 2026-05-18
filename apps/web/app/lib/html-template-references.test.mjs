import test from 'node:test';
import assert from 'node:assert/strict';
import {
  getHtmlTemplateReference,
  getStaticPageTemplateReference,
  inferStaticPageTemplateReferenceId,
  listHtmlTemplateReferences,
  normalizeTemplateDesignReferences,
  staticPageTemplateReferenceDraftSeed,
  templateReferenceProviderPromptPolicy,
} from './html-template-references.js';

test('lists only enabled static page html-anything references by default', () => {
  const references = listHtmlTemplateReferences({ surface: 'static_page' });
  const ids = references.map((reference) => reference.id);

  assert.deepEqual(ids, ['data-report', 'dashboard', 'docs-page']);
  assert.equal(references.every((reference) => reference.status === 'enabled'), true);
  assert.equal(references.every((reference) => reference.designReference.source === 'html-anything'), true);
});

test('infers enabled static page references from user intent', () => {
  assert.equal(inferStaticPageTemplateReferenceId('做一个运营监控看板'), 'dashboard');
  assert.equal(inferStaticPageTemplateReferenceId('整理接口交接文档和验收步骤'), 'docs-page');
  assert.equal(inferStaticPageTemplateReferenceId('生成经营分析报告和 KPI 图表'), 'data-report');
  assert.equal(inferStaticPageTemplateReferenceId('随便聊聊'), '');
});

test('keeps paused deck and video references out of normal selection', () => {
  assert.equal(getHtmlTemplateReference('deck-swiss-international'), null);
  assert.equal(getStaticPageTemplateReference('video-hyperframes'), null);

  const paused = getHtmlTemplateReference('deck-swiss-international', { includePaused: true });
  assert.equal(paused.status, 'paused');
  assert.equal(paused.designReference.quickOutput, false);
  assert.match(paused.designReference.pauseReason, /frozen/);
});

test('builds a safe static page draft seed without raw html', () => {
  const seed = staticPageTemplateReferenceDraftSeed('data-report');

  assert.equal(seed.templateId, 'data-report');
  assert.equal(seed.styleDirection, 'data-command');
  assert.equal(seed.designReference.importPolicy, 'metadata_and_constraints_only');
  assert.equal(seed.modules.length, 5);
  assert.equal(seed.modules.some((module) => module.visualization.type === 'bar-chart'), true);
  assert.doesNotMatch(JSON.stringify(seed), /<html|<script|https?:\/\//i);
});

test('normalizes stored design references to a small serializable contract', () => {
  const normalized = normalizeTemplateDesignReferences([
    {
      template_id: 'DATA-REPORT',
      label: '数据报告',
      prompt_hints: ['use evidence', '', null],
      guardrails: ['no raw html'],
      extra: { ignored: true },
    },
    { template_id: '../bad' },
  ]);

  assert.equal(normalized.length, 1);
  assert.equal(normalized[0].templateId, 'data-report');
  assert.deepEqual(normalized[0].promptHints, ['use evidence']);
  assert.deepEqual(normalized[0].guardrails, ['no raw html']);
  assert.equal(Object.hasOwn(normalized[0], 'extra'), false);
});

test('provider policy forbids raw html and blocks paused surfaces', () => {
  const enabled = templateReferenceProviderPromptPolicy('dashboard');
  const paused = templateReferenceProviderPromptPolicy('video-hyperframes');

  assert.equal(enabled.providerOutput, 'structured_static_page_draft_json');
  assert.equal(enabled.forbiddenOutput.includes('raw_html'), true);
  assert.equal(paused.providerOutput, 'not_allowed_for_current_v3_track');
});
