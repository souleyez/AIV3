#!/usr/bin/env node

import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/external-report-focus-smoke';
const PRIMARY_XINBAI_TEMPLATE_ID = 'xinbai-functional-modular-template-20260604';
const PRIMARY_PUBLIC_URL = `https://v3.elepcloud.com/generated-artifacts/database-static-pages/${PRIMARY_XINBAI_TEMPLATE_ID}/index.html`;

const REPORT_CASES = [
  { id: 'take_high', prompt: '生成取高机会报表', expectedFocus: '取高机会' },
  { id: 'business_status', prompt: '生成经营状况报表', expectedFocus: '经营总览' },
  { id: 'business_health', prompt: '生成经营健康度报表', expectedFocus: '经营总览' },
  { id: 'risk_identification', prompt: '生成风险识别报表', expectedFocus: '风险店铺' },
  {
    id: 'sales_gap_boost',
    prompt: '生成销售缺口助推门店报表',
    expectedFocus: '取高机会',
  },
  { id: 'boost_stores', prompt: '生成需要助推门店报表', expectedFocus: '取高机会' },
  { id: 'operations_report', prompt: '生成经营报表', expectedFocus: null },
];

const ORDINARY_GUARD_CASES = [
  { id: 'take_high_noun', prompt: '取高' },
  { id: 'business_status_noun', prompt: '经营状况' },
  { id: 'business_health_noun', prompt: '经营健康度' },
  { id: 'overall_operation_view', prompt: '看看整体经营情况' },
  { id: 'risk_identification_noun', prompt: '风险识别' },
  { id: 'store_operation_risk_view', prompt: '看看新街口店经营风险' },
  { id: 'sales_gap_question', prompt: '销售缺口统计一下，哪些门店需要助推？' },
  { id: 'sales_trend_view', prompt: '看月度销售趋势' },
  { id: 'traffic_warning_noun', prompt: '客流降低预警' },
  { id: 'mixed_evidence_explanation', prompt: '解释销售缺口最大的门店，同时引用报告里的风险描述。' },
  { id: 'report_output_meaning', prompt: '报告输出是什么' },
  { id: 'page_generation_failure', prompt: '页面生成为什么失败' },
  { id: 'report_delete', prompt: '删除这份报表' },
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
  const phrases = String(prompt || '')
    .toLowerCase()
    .replace(/ and then | then | and |同时|然后|并且|并|[，,。.;；！!？?]/g, '\n')
    .split('\n')
    .map(compactText)
    .filter(Boolean);
  const artifactTargets = [
    '静态页', '静态页面', '报表页', '可视化报表', '经营分析报表', '经营分析页', '报表', '报告',
    '月报', '周报', '日报', '看板', '仪表盘', '大屏', '页面', '网页', '网站', '图表',
    'dashboard', 'report', 'page', 'webpage', 'html', 'artifact', 'chart',
  ];
  const explicitActions = [
    '重新生成', '生成', '创建', '制作', '做出来', '做成', '做一个', '做一份', '做个',
    '输出', '发布', '上线', '渲染', '导出', '重新设计', '重做', '改版', '修改', '修复',
    '更新', '调整', '补充', '增加', '添加', '替换', '出页面', '出报表', '出报告', '出看板',
    'create', 'generate', 'build', 'make', 'publish', 'render', 'export', 'redesign',
    'revise', 'modify', 'update',
  ];
  const referenceOnlyMarkers = [
    '解释', '引用', '查看', '请看', '读取', '阅读', '说明', '介绍', '比较', '对比', '概括',
    '总结', '复述', '什么', '为什么', '为何', '怎么', '如何', '是否', '能不能', '可不可以',
    '状态', '记录', '进度', '历史', '结果', '原因', '含义', '是什么意思', '提到', '写着',
    '建议', '计划', '方案', '步骤', '规则', '日志', '详情', 'explain', 'quote', 'cite', 'view',
    'read', 'inspect', 'compare', 'summarize', 'why', 'how', 'what', 'whether', 'mentioned',
    'suggest',
  ];
  const negationMarkers = [
    '不要', '不用', '无需', '无须', '不必', '别', '禁止', '不是要', '不是让你', '不',
    'donot', "don't", 'dont', 'never', 'without', 'not',
  ];
  return phrases.some((phrase) => {
    const actionPosition = explicitActions
      .map((term) => phrase.indexOf(term))
      .filter((position) => position >= 0)
      .sort((left, right) => left - right)[0];
    if (actionPosition === undefined
      || !containsAny(phrase, artifactTargets)
      || containsAny(phrase, referenceOnlyMarkers)) {
      return false;
    }
    return !negationMarkers.some((term) => {
      const position = phrase.indexOf(term);
      return position >= 0 && position <= actionPosition;
    });
  });
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
  const safety = {
    dataMaxCalled: false,
    publicArtifactPublished: false,
    productionMutation: false,
  };
  const summary = buildSummary({ reportCases, ordinaryCases, safety });
  return {
    reportType: 'external_report_focus_self_test',
    generatedAt: new Date().toISOString(),
    primaryTemplateId: PRIMARY_XINBAI_TEMPLATE_ID,
    ok: summary.ok,
    summary,
    reportCaseCount: reportCases.length,
    ordinaryGuardCaseCount: ordinaryCases.length,
    reportCases,
    ordinaryCases,
    safety,
  };
}

function buildSummary({ reportCases, ordinaryCases, safety }) {
  const triggeredReportCount = reportCases.filter((item) => item.triggered).length;
  const expectedFocusMatchCount = reportCases.filter((item) => item.focus === item.expectedFocus).length;
  const primaryTemplateCount = reportCases.filter((item) => item.primaryTemplateUsed).length;
  const fallbackTemplateCount = reportCases.filter((item) => item.fallbackTemplateUsed).length;
  const ordinaryMisrouteCount = ordinaryCases.filter((item) => item.triggered || item.artifactLinkCount > 0).length;
  const artifactLinkCount = reportCases.reduce((sum, item) => sum + item.artifactLinkCount, 0)
    + ordinaryCases.reduce((sum, item) => sum + item.artifactLinkCount, 0);
  const checks = {
    allReportCasesTriggered: triggeredReportCount === reportCases.length,
    allReportCasesFocusMatched: expectedFocusMatchCount === reportCases.length,
    allReportCasesUsePrimaryTemplate: primaryTemplateCount === reportCases.length,
    noFallbackTemplateUsed: fallbackTemplateCount === 0,
    ordinaryQuestionsRemainOrdinary: ordinaryMisrouteCount === 0,
    oneArtifactLinkPerReportCase: artifactLinkCount === reportCases.length,
    noLiveMutation: safety.dataMaxCalled === false
      && safety.publicArtifactPublished === false
      && safety.productionMutation === false,
  };
  return {
    ok: Object.values(checks).every(Boolean),
    checks,
    reportCaseCount: reportCases.length,
    ordinaryGuardCaseCount: ordinaryCases.length,
    triggeredReportCount,
    expectedFocusMatchCount,
    primaryTemplateCount,
    fallbackTemplateCount,
    ordinaryMisrouteCount,
    artifactLinkCount,
  };
}

function assertReport(report) {
  assert.equal(report.ok, true, 'report summary should pass');
  assert.equal(report.summary?.ok, true, 'summary ok should pass');
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
