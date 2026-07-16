import assert from 'node:assert/strict';
import test from 'node:test';

import {
  canonicalDocumentTitle,
  classifyGraphLabel,
  projectFallbackLabel,
} from './dataset-understanding-label-quality.js';

test('classifyGraphLabel rejects numeric, row, SQL and technical noise', () => {
  const cases = [
    ['2026', 'numeric'],
    ['2026 0101 999999', 'numeric'],
    ['1001\t示例门店\t200001\t123456.78', 'row'],
    ['select date_add(day, 1, dt) from sample_table', 'sql'],
    ['/*示例日均值*/', 'sql'],
    ['application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', 'technical'],
    ['paragraph_aware_noun_terms_v1', 'technical'],
    ['00000000-1111-4222-8333-444444444444', 'technical'],
    ['aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'technical'],
    ['C:\\internal\\example\\source.xlsx', 'technical'],
  ];

  cases.forEach(([label, expectedClass]) => {
    const result = classifyGraphLabel(label);
    assert.equal(result.quality_class, expectedClass, label);
    assert.equal(result.main_canvas_allowed, false, label);
    assert.ok(result.normalized_key);
    assert.ok(result.reason);
  });
});

test('classifyGraphLabel distinguishes trusted Chinese phrases and document titles', () => {
  const business = classifyGraphLabel('合同预警');
  const chineseDocument = classifyGraphLabel('客户清单.pdf', { kind: 'document' });
  const technicalDocument = classifyGraphLabel('technical_report_alpha.xlsx', { kind: 'document' });

  assert.equal(business.quality_class, 'business');
  assert.equal(business.main_canvas_allowed, true);
  assert.equal(chineseDocument.quality_class, 'document');
  assert.equal(chineseDocument.main_canvas_allowed, true);
  assert.equal(technicalDocument.quality_class, 'document');
  assert.equal(technicalDocument.main_canvas_allowed, false);
});

test('canonicalDocumentTitle merges common extensions without changing the title identity', () => {
  assert.equal(canonicalDocumentTitle('示例经营分析场景.xlsx'), canonicalDocumentTitle('示例经营分析场景'));
  assert.equal(canonicalDocumentTitle('示例经营分析场景.ZIP'), canonicalDocumentTitle('示例经营分析场景.pdf'));
  assert.equal(canonicalDocumentTitle('  示例 经营-分析  '), canonicalDocumentTitle('示例经营分析'));
});

test('projectFallbackLabel extracts only evidenced Chinese text and keeps technical originals in detail', () => {
  const mixedDocument = projectFallbackLabel('Data_Buddy_AI经营分析5个重点场景.xlsx', { kind: 'document' });
  const chineseDocument = projectFallbackLabel('客户清单.pdf', { kind: 'document' });
  const technicalDocument = projectFallbackLabel('technical_report_alpha.xlsx', { kind: 'document' });
  const technicalHint = projectFallbackLabel('paragraph_aware_noun_terms_v1', { kind: 'knowledge' });
  const rawRow = projectFallbackLabel('1001\t示例门店\t200001\t123456.78', { kind: 'knowledge' });

  assert.equal(mixedDocument.display_label, '经营分析重点场景');
  assert.equal(mixedDocument.main_canvas_allowed, true);
  assert.match(mixedDocument.detail_label, /Data_Buddy_AI经营分析5个重点场景\.xlsx/);
  assert.equal(chineseDocument.display_label, '客户清单');
  assert.equal(chineseDocument.normalized_key, canonicalDocumentTitle('客户清单'));
  assert.match(chineseDocument.detail_label, /客户清单\.pdf/);
  assert.equal(technicalDocument.display_label, '待解释资料');
  assert.equal(technicalDocument.main_canvas_allowed, false);
  assert.match(technicalDocument.detail_label, /technical_report_alpha\.xlsx/);
  assert.equal(technicalHint.display_label, '待解释线索');
  assert.match(technicalHint.detail_label, /paragraph_aware_noun_terms_v1/);
  assert.equal(rawRow.display_label, '待解释线索');
  assert.doesNotMatch(rawRow.detail_label, /1001|200001|123456/);
});
