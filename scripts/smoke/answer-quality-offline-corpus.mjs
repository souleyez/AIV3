#!/usr/bin/env node

import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/answer-quality-offline-corpus-smoke';

const REQUIRED_COVERAGE_TAGS = [
  'deng_person',
  'one_character_pdf',
  'doc_deng_person',
  'resume_company_stats',
  'resume_multidimensional_ranking',
  'resume_project_experience_14_docs',
  'attendance_absence_workhour_date_format',
  'elderly_turning_medicine_handoff',
  'xinbai_high_risk_operations_gap',
  'temporary_resume_attachment_scope',
];

const REQUIRED_LABELS = [
  'weak_insufficient_evidence_answer',
  'irrelevant_retrieval_supply',
  'missing_report_artifact',
  'missing_report_link',
  'duplicate_report_link',
  'temporary_attachment_not_in_scope',
  'third_party_remote_fallback',
];

function parseArgs(argv) {
  const args = {
    selfTest: false,
    pretty: false,
    outputDir: process.env.ANSWER_QUALITY_OFFLINE_CORPUS_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
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
  node scripts/smoke/answer-quality-offline-corpus.mjs --self-test

This deterministic smoke classifies local answer-quality fixtures only. It does
not call DataMax, does not enqueue Codex tasks, and does not affect customer
answers. Reports contain case ids, labels, counts, and redaction flags only.
`);
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function buildFixtures() {
  return [
    {
      caseId: 'deng_person_grounded',
      coverageTags: ['deng_person'],
      question: '邓工是谁',
      answer: '邓工是工程负责人，负责项目实施和现场协调。',
      evidence: { relevantSupplyCount: 2, irrelevantSupplyCount: 0 },
      expectedLabels: [],
    },
    {
      caseId: 'one_character_pdf_honest_parse_limit',
      coverageTags: ['one_character_pdf'],
      question: '这份一字 PDF 讲了什么',
      answer: '这份文档可提取文字过短，无法形成可靠结论。',
      evidence: { relevantSupplyCount: 0, parseQuality: 'low_text_coverage' },
      expectedLabels: [],
    },
    {
      caseId: 'doc_deng_person_weak_insufficient',
      coverageTags: ['doc_deng_person'],
      question: 'doc 里邓工是谁',
      answer: '资料不足，无法判断邓工是谁。',
      evidence: { relevantSupplyCount: 1, irrelevantSupplyCount: 0 },
      expectedLabels: ['weak_insufficient_evidence_answer'],
    },
    {
      caseId: 'resume_company_stats_weak_insufficient',
      coverageTags: ['resume_company_stats'],
      question: '统计这些简历里的公司名出现次数',
      answer: '目前没有足够资料统计公司名。',
      evidence: { relevantSupplyCount: 14, irrelevantSupplyCount: 0 },
      expectedLabels: ['weak_insufficient_evidence_answer'],
    },
    {
      caseId: 'resume_multidimensional_ranking_missing_report',
      coverageTags: ['resume_multidimensional_ranking'],
      question: '按经验、公司、项目维度给简历排序并出表',
      answer: '已完成排序。',
      evidence: { relevantSupplyCount: 14 },
      intent: { reportExpected: true },
      artifacts: { expected: true, links: [] },
      expectedLabels: ['missing_report_artifact'],
    },
    {
      caseId: 'resume_project_experience_14_docs_irrelevant_supply',
      coverageTags: ['resume_project_experience_14_docs'],
      question: '汇总 14 份简历的项目经历',
      answer: '以下是项目经历汇总。',
      evidence: { relevantSupplyCount: 0, irrelevantSupplyCount: 6, expectedRelevantSupplyCount: 14 },
      expectedLabels: ['irrelevant_retrieval_supply'],
    },
    {
      caseId: 'attendance_absence_workhour_date_format_weak',
      coverageTags: ['attendance_absence_workhour_date_format'],
      question: '汇总考勤缺勤、工时长短，并统一日期格式',
      answer: '没有足够上下文判断考勤情况。',
      evidence: { relevantSupplyCount: 3, irrelevantSupplyCount: 0 },
      expectedLabels: ['weak_insufficient_evidence_answer'],
    },
    {
      caseId: 'elderly_turning_medicine_handoff_grounded',
      coverageTags: ['elderly_turning_medicine_handoff'],
      question: '养老护理翻身、发药核对、交接班怎么做',
      answer: '翻身需记录体位和时间，发药核对人药床号，交接班记录重点风险。',
      evidence: { relevantSupplyCount: 3, irrelevantSupplyCount: 0 },
      expectedLabels: [],
    },
    {
      caseId: 'elderly_medicine_irrelevant_supply',
      coverageTags: ['elderly_turning_medicine_handoff'],
      question: '发药核对流程有哪些关键点',
      answer: '发药前需要确认。',
      evidence: { relevantSupplyCount: 0, irrelevantSupplyCount: 4, expectedRelevantSupplyCount: 1 },
      expectedLabels: ['irrelevant_retrieval_supply'],
    },
    {
      caseId: 'xinbai_high_missing_report_link',
      coverageTags: ['xinbai_high_risk_operations_gap'],
      question: '新百取高机会生成经营报表',
      answer: '报表已生成，请查看结果。',
      evidence: { relevantSupplyCount: 5 },
      intent: { reportExpected: true },
      artifacts: { expected: true, links: [], generated: true },
      expectedLabels: ['missing_report_link'],
    },
    {
      caseId: 'xinbai_risk_duplicate_report_link',
      coverageTags: ['xinbai_high_risk_operations_gap'],
      question: '新百风险和经营状况报表',
      answer: '报表链接如下。',
      evidence: { relevantSupplyCount: 5 },
      intent: { reportExpected: true },
      artifacts: {
        expected: true,
        links: [
          'https://v3.example/reports/risk',
          'https://v3.example/reports/risk',
        ],
        generated: true,
      },
      expectedLabels: ['duplicate_report_link'],
    },
    {
      caseId: 'temporary_resume_attachment_not_scoped',
      coverageTags: ['temporary_resume_attachment_scope'],
      question: '把刚临时上传的简历也加入问答范围',
      answer: '当前只看到历史简历。',
      evidence: { relevantSupplyCount: 1 },
      attachments: { expectedCount: 1, scopedCount: 0 },
      expectedLabels: ['temporary_attachment_not_in_scope'],
    },
    {
      caseId: 'third_party_remote_fallback_observed',
      coverageTags: ['xinbai_high_risk_operations_gap'],
      question: '新百销售缺口和助推建议',
      answer: '第三方远程接口不可用，已使用本地兜底。',
      evidence: { relevantSupplyCount: 2 },
      provider: { fallbackUsed: true, failureCount: 2 },
      expectedLabels: ['third_party_remote_fallback'],
    },
  ];
}

function includesInsufficientLanguage(answer) {
  return /资料不足|没有足够|无法判断|不能判断|无法形成可靠结论|没有足够上下文/.test(answer);
}

function classifyFixture(fixture) {
  const labels = new Set();
  const evidence = fixture.evidence || {};
  const artifacts = fixture.artifacts || {};
  const attachments = fixture.attachments || {};
  const provider = fixture.provider || {};
  const artifactLinks = Array.isArray(artifacts.links) ? artifacts.links : [];
  const uniqueArtifactLinks = new Set(artifactLinks);

  if (includesInsufficientLanguage(fixture.answer) && Number(evidence.relevantSupplyCount || 0) > 0) {
    labels.add('weak_insufficient_evidence_answer');
  }
  if (
    Number(evidence.expectedRelevantSupplyCount || 0) > 0
    && Number(evidence.relevantSupplyCount || 0) === 0
    && Number(evidence.irrelevantSupplyCount || 0) > 0
  ) {
    labels.add('irrelevant_retrieval_supply');
  }
  if (fixture.intent?.reportExpected && artifacts.expected && !artifacts.generated && artifactLinks.length === 0) {
    labels.add('missing_report_artifact');
  }
  if (fixture.intent?.reportExpected && artifacts.generated && artifactLinks.length === 0) {
    labels.add('missing_report_link');
  }
  if (artifactLinks.length > 1 && uniqueArtifactLinks.size < artifactLinks.length) {
    labels.add('duplicate_report_link');
  }
  if (Number(attachments.expectedCount || 0) > Number(attachments.scopedCount || 0)) {
    labels.add('temporary_attachment_not_in_scope');
  }
  if (provider.fallbackUsed || Number(provider.failureCount || 0) >= 2) {
    labels.add('third_party_remote_fallback');
  }
  return [...labels].sort();
}

function assertSelfTest(report, fixtures) {
  for (const fixture of fixtures) {
    const actual = report.cases.find((item) => item.caseId === fixture.caseId);
    assert(actual, `missing case result ${fixture.caseId}`);
    assert.deepEqual(actual.labels, [...fixture.expectedLabels].sort(), fixture.caseId);
  }
  for (const tag of REQUIRED_COVERAGE_TAGS) {
    assert(report.coverageTags.includes(tag), `missing coverage tag ${tag}`);
  }
  for (const label of REQUIRED_LABELS) {
    assert(report.labelCounts[label] > 0, `missing label ${label}`);
  }
}

function buildReport(fixtures) {
  const cases = fixtures.map((fixture) => {
    const labels = classifyFixture(fixture);
    return {
      caseId: fixture.caseId,
      coverageTags: fixture.coverageTags,
      labels,
      labelCount: labels.length,
    };
  });
  const labelCounts = {};
  for (const item of cases) {
    for (const label of item.labels) {
      labelCounts[label] = (labelCounts[label] || 0) + 1;
    }
  }
  const coverageTags = [...new Set(cases.flatMap((item) => item.coverageTags))].sort();
  return {
    reportType: 'answer_quality_offline_corpus_self_test',
    generatedAt: new Date().toISOString(),
    result: 'passed',
    caseCount: cases.length,
    labeledCaseCount: cases.filter((item) => item.labels.length > 0).length,
    coverageTags,
    labelCounts,
    cases,
    runtime: {
      liveHardGateEnabled: false,
      liveAutofixEnqueueAttempted: false,
      customerResponseBlocked: false,
    },
    redaction: {
      rawPromptIncluded: false,
      rawAnswerIncluded: false,
      rawEvidenceIncluded: false,
      rawCustomerPayloadIncluded: false,
      credentialIncluded: false,
    },
  };
}

async function writeReport(outputDir, report, pretty = false) {
  await mkdir(outputDir, { recursive: true });
  const filename = `answer-quality-offline-corpus-self-test-${makeRunId()}.json`;
  const path = join(outputDir, filename);
  await writeFile(path, JSON.stringify(report, null, pretty ? 2 : 0));
  return path;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.selfTest) {
    printHelp();
    throw new Error('--self-test is required for this deterministic smoke');
  }
  const fixtures = buildFixtures();
  const report = buildReport(fixtures);
  assertSelfTest(report, fixtures);
  const reportPath = await writeReport(args.outputDir, report, args.pretty);
  console.log(
    `OK answer quality offline corpus self-test: cases=${report.caseCount} labels=${Object.keys(report.labelCounts).length} report=${reportPath}`,
  );
}

main().catch((error) => {
  console.error(error?.message || error);
  process.exit(1);
});
