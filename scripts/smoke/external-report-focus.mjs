#!/usr/bin/env node

import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/external-report-focus-smoke';
const PRIMARY_XINBAI_TEMPLATE_ID = 'xinbai-functional-modular-template-20260604';
const PRIMARY_PUBLIC_URL = `https://v3.elepcloud.com/generated-artifacts/database-static-pages/${PRIMARY_XINBAI_TEMPLATE_ID}/index.html`;

const REPORT_CASES = [
  { id: 'take_high', prompt: '取高', expectedFocus: '取高机会' },
  { id: 'business_status', prompt: '经营状况', expectedFocus: '经营总览' },
  { id: 'business_health', prompt: '经营健康度', expectedFocus: '经营总览' },
  { id: 'risk_identification', prompt: '风险识别', expectedFocus: '风险店铺' },
  {
    id: 'sales_gap_boost',
    prompt: '销售缺口统计一下，哪些门店需要助推？',
    expectedFocus: '取高机会',
  },
  { id: 'boost_stores', prompt: '哪些门店需要助推', expectedFocus: '取高机会' },
  { id: 'operations_report', prompt: '统计经营报表', expectedFocus: null },
];

const ORDINARY_GUARD_CASES = [
  { id: 'take_high_meaning', prompt: '取高是什么意思？' },
  { id: 'risk_system_resume', prompt: '风险识别系统有哪些项目经历？' },
  { id: 'unrelated_general_question', prompt: '与新百经营数据无关的普通知识问答' },
  { id: 'business_risk_meaning', prompt: '经营风险是什么意思？' },
];

function parseArgs(argv) {
  const args = {
    selfTest: false,
    pretty: false,
    outputDir: process.env.EXTERNAL_REPORT_FOCUS_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
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
  node scripts/smoke/external-report-focus.mjs --self-test

Runs a deterministic offline regression corpus for Xinbai report trigger/focus
routing. It does not call DataMax and does not publish artifacts.
`);
}

function compactText(value) {
  return String(value || '').replace(/\s+/g, '');
}

function containsAny(text, values) {
  return values.some((value) => text.includes(value));
}

function promptRequestsReport(prompt) {
  const compact = compactText(prompt);
  if (!compact) {
    return false;
  }
  if (
    containsAny(compact, ['什么', '怎么', '如何', '为什么', '是否', '能不能', '可不可以', '吗', '介绍', '说明', '含义', '口径', '问题', '原因'])
    || (containsAny(compact, ['哪些', '有哪些']) && containsAny(compact, ['问题', '口径', '原因', '含义', '意思', '是什么', '项目经历']))
  ) {
    return false;
  }
  if (
    [
      '取高',
      '经营状况',
      '经营情况',
      '经营状态',
      '经营健康度',
      '整体经营情况',
      '整体经营状况',
      '销售缺口',
      '销售额缺口',
      '风险识别',
      '经营风险',
      '经营总览',
      '助推门店',
      '门店助推',
    ].includes(compact)
  ) {
    return true;
  }
  const hasRiskIdentificationModule = !compact.includes('风险识别系统')
    && compact.includes('风险识别')
    && containsAny(compact, ['经营', '门店', '店铺', '品牌', '销售', '租金', '取高', '报表', '看板', '统计', '汇总', '排行', '排名', '新百', '新世界']);
  const hasBusinessModule = hasRiskIdentificationModule || containsAny(compact, [
    '取高',
    '取高机会',
    '高分成',
    '经营总览',
    '经营状况',
    '经营情况',
    '整体经营',
    '经营风险',
    '经营健康',
    '经营健康度',
    '销售趋势',
    '销售额缺口',
    '销售缺口',
    '助推',
    '需要助推',
    '需助推',
    '助推门店',
    '销售统计',
    '门店统计',
    '品牌统计',
    '经营统计',
    '经营报表',
    '经营月报',
    '新百经营',
  ]);
  if (!hasBusinessModule) {
    return false;
  }
  return containsAny(compact, [
    '看看',
    '看',
    '查看',
    '哪些',
    '列',
    '列出',
    '统计',
    '汇总',
    '排行',
    '排名',
    '报表',
    '月报',
    '生成',
    '制作',
    '取高',
    '经营状况',
    '经营情况',
    '销售缺口',
    '销售额缺口',
    '需要助推',
    '需助推',
    '助推',
  ]);
}

function focusForPrompt(prompt) {
  const compact = compactText(prompt);
  if (containsAny(compact, ['取高', '高分成', '提成', '分成线', '缺口', '机会', '助推', '需助推', '需要助推', '租金'])) {
    return '取高机会';
  }
  if (containsAny(compact, ['低活跃', '低销售', '客流下降', '客流降低', '异常'])) {
    return '低活跃';
  }
  if (containsAny(compact, ['风险店铺', '风险门店', '风险品牌', '风险提示', '风险', '预警', '高风险'])) {
    return '风险店铺';
  }
  if (containsAny(compact, ['合同面积', '门店面积', '坪效', '客流统计', '客流数据', '经营健康', '健康度', '评分', '销售趋势', '月度销售趋势', '收入趋势', '计划完成'])) {
    return '经营总览';
  }
  if (containsAny(compact, ['明细', '品牌', '品牌店', '店铺客户', '客户名单', '合同'])) {
    return '品牌明细';
  }
  if (containsAny(compact, ['品类', '业态', '类别', '结构', '占比'])) {
    return '品类业态';
  }
  if (containsAny(compact, ['经营总览', '经营状况', '经营情况', '经营状态', '总览'])) {
    return '经营总览';
  }
  return null;
}

function publicUrlWithFocus(focus) {
  if (!focus) {
    return PRIMARY_PUBLIC_URL;
  }
  const url = new URL(PRIMARY_PUBLIC_URL);
  url.searchParams.set('focus', focus);
  return url.toString();
}

function buildReport() {
  const reportCases = REPORT_CASES.map((item) => {
    const triggered = promptRequestsReport(item.prompt);
    const focus = triggered ? focusForPrompt(item.prompt) : null;
    const publicUrl = triggered ? publicUrlWithFocus(focus) : null;
    const artifactLinks = publicUrl ? [publicUrl] : [];
    return {
      ...item,
      triggered,
      focus,
      publicUrl,
      artifactLinkCount: artifactLinks.length,
      primaryTemplateUsed: Boolean(publicUrl?.includes(PRIMARY_XINBAI_TEMPLATE_ID)),
      fallbackTemplateUsed: /fallback|prewarm|smoke/i.test(publicUrl || ''),
    };
  });
  const ordinaryCases = ORDINARY_GUARD_CASES.map((item) => ({
    ...item,
    triggered: promptRequestsReport(item.prompt),
    focus: null,
    artifactLinkCount: 0,
  }));
  return {
    reportType: 'external_report_focus_self_test',
    generatedAt: new Date().toISOString(),
    primaryTemplateId: PRIMARY_XINBAI_TEMPLATE_ID,
    reportCaseCount: reportCases.length,
    ordinaryGuardCaseCount: ordinaryCases.length,
    reportCases,
    ordinaryCases,
    safety: {
      dataMaxCalled: false,
      publicArtifactPublished: false,
      productionMutation: false,
    },
  };
}

function assertReport(report) {
  for (const item of report.reportCases) {
    assert.equal(item.triggered, true, `${item.id} should trigger report workflow`);
    assert.equal(item.focus, item.expectedFocus, `${item.id} focus`);
    assert.equal(item.artifactLinkCount, 1, `${item.id} should expose one report link`);
    assert.equal(item.primaryTemplateUsed, true, `${item.id} should use primary template`);
    assert.equal(item.fallbackTemplateUsed, false, `${item.id} should avoid fallback/prewarm/smoke template`);
  }
  for (const item of report.ordinaryCases) {
    assert.equal(item.triggered, false, `${item.id} should remain ordinary QA`);
    assert.equal(item.artifactLinkCount, 0, `${item.id} should not expose a report link`);
  }
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.selfTest) {
    printHelp();
    throw new Error('--self-test is required for the deterministic focus smoke');
  }
  const report = buildReport();
  assertReport(report);
  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const reportPath = join(outputDir, `external-report-focus-self-test-${makeRunId()}.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, args.pretty ? 2 : 0)}\n`, 'utf8');
  console.log(
    `OK external report focus self-test: reportCases=${report.reportCaseCount} ordinaryGuards=${report.ordinaryGuardCaseCount} report=${reportPath}`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
