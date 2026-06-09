#!/usr/bin/env node

import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_FIXTURE_PATH = 'fixtures/newbai-customer-answer/cases.jsonl';
const DEFAULT_OUTPUT_DIR = 'target/newbai-customer-answer-smoke';

const INTERNAL_LEAK_PATTERNS = [
  'evidence_state',
  'retrieval_evidence_id',
  '供料证据',
  'Prompt:',
  'raw observation',
  'provider_payload',
  'tool_call',
  'runtime manifest',
];

const REPORT_FALSE_CLAIM_PATTERNS = [
  /已联网搜索/,
  /我(刚刚)?查(了|阅了)?网页/,
  /(已|已经).*(生成|创建|发布).*(报表|页面|链接)/,
  /报表链接如下/,
  /页面链接如下/,
  /正在处理/,
  /稍后(输出|生成|给出|返回)/,
];

const TEMPLATE_SIDE_EFFECT_PATTERNS = [
  /HTML\s*兜底/i,
  /重新生成\s*\d*个?页面/,
  /重新生成新的可视化/,
  /新建(页面|可视化|模板)/,
  /生成新的(页面|可视化|模板)/,
];

const TEMPLATE_REUSE_PATTERNS = [
  /现有模板/,
  /已有模板/,
  /沿用/,
  /复用/,
  /当前模板/,
  /原模板/,
  /已有页面/,
  /现有页面/,
];

function parseArgs(argv) {
  const args = {
    selfTest: false,
    pretty: false,
    requireReady: parseBoolean(process.env.NEWBAI_CUSTOMER_ANSWER_REQUIRE_READY),
    fixturePath: process.env.NEWBAI_CUSTOMER_ANSWER_FIXTURE || DEFAULT_FIXTURE_PATH,
    resultsJsonl: process.env.NEWBAI_CUSTOMER_ANSWER_RESULTS_JSONL || null,
    outputDir: process.env.NEWBAI_CUSTOMER_ANSWER_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--require-ready') {
      args.requireReady = true;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--fixture') {
      args.fixturePath = requireValue(arg, next);
      index += 1;
    } else if (arg === '--results-jsonl') {
      args.resultsJsonl = requireValue(arg, next);
      index += 1;
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
  node scripts/smoke/newbai-customer-answer.mjs --self-test
  node scripts/smoke/newbai-customer-answer.mjs --results-jsonl target/live-results.jsonl
  node scripts/smoke/newbai-customer-answer.mjs --results-jsonl target/live-results.jsonl --require-ready

This deterministic smoke evaluates NewBai customer-facing answer quality from
local fixture data or supplied result JSONL. It does not call DataMax, does not
call a model provider, does not enqueue report rendering, and does not publish
static pages.
`);
}

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y', 'on'].includes(String(value || '').trim().toLowerCase());
}

function compactText(value) {
  return String(value || '').toLowerCase().replace(/\s+/g, '');
}

function plainIncludes(haystack, needle) {
  return compactText(haystack).includes(compactText(needle));
}

function patternMatches(text, pattern) {
  if (pattern instanceof RegExp) {
    return pattern.test(String(text || ''));
  }
  if (typeof pattern === 'object' && pattern?.regex) {
    return new RegExp(pattern.regex, pattern.flags || '').test(String(text || ''));
  }
  return plainIncludes(text, pattern);
}

function patternLabel(pattern) {
  if (pattern instanceof RegExp) {
    return pattern.toString();
  }
  if (typeof pattern === 'object' && pattern?.regex) {
    return `/${pattern.regex}/${pattern.flags || ''}`;
  }
  return String(pattern);
}

async function readJsonl(path) {
  const raw = await readFile(path, 'utf8');
  return raw
    .split(/\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((line) => !line.startsWith('#'))
    .map((line, index) => {
      try {
        return JSON.parse(line);
      } catch (error) {
        throw new Error(`${path}:${index + 1}: invalid JSONL: ${error.message}`);
      }
    });
}

function resultCaseId(result) {
  return result.case_id || result.caseId || result.id;
}

function evidenceCorpus(result) {
  const evidence = Array.isArray(result.evidence) ? result.evidence : [];
  return evidence
    .map((item) => [item.type, item.source, item.source_locator, item.title, item.text, item.summary]
      .filter(Boolean)
      .join(' '))
    .join('\n');
}

function hasTemplateReuseLanguage(answer) {
  return TEMPLATE_REUSE_PATTERNS.some((pattern) => pattern.test(String(answer || '')));
}

function hasReportedSideEffect(result) {
  const sideEffects = result.side_effects || result.sideEffects || {};
  const artifacts = Array.isArray(result.artifacts) ? result.artifacts : [];
  return Boolean(
    sideEffects.static_page_published
      || sideEffects.staticPagePublished
      || sideEffects.report_generation_enqueued
      || sideEffects.reportGenerationEnqueued
      || sideEffects.new_template_generated
      || sideEffects.newTemplateGenerated
      || sideEffects.html_fallback_used
      || sideEffects.htmlFallbackUsed
      || artifacts.some((artifact) => artifact?.generatedNewPage || artifact?.htmlFallbackUsed),
  );
}

function evidenceTypeSet(result) {
  const evidence = Array.isArray(result.evidence) ? result.evidence : [];
  return new Set(evidence.map((item) => item.type).filter(Boolean));
}

function evaluateCase(fixture, result) {
  const expectations = fixture.expectations || {};
  const answer = String(result?.answer || '');
  const evidenceText = evidenceCorpus(result || {});
  const types = evidenceTypeSet(result || {});
  const requiredAnswerPatterns = expectations.required_answer_patterns || [];
  const forbiddenAnswerPatterns = expectations.forbidden_answer_patterns || [];
  const requiredEvidenceTypes = expectations.required_evidence_types || [];
  const requiredEvidencePhrases = expectations.required_evidence_phrases || [];

  const missingAnswerPatterns = requiredAnswerPatterns.filter((pattern) => !patternMatches(answer, pattern));
  const matchedForbiddenAnswerPatterns = forbiddenAnswerPatterns.filter((pattern) => patternMatches(answer, pattern));
  const missingEvidenceTypes = requiredEvidenceTypes.filter((type) => !types.has(type));
  const missingEvidencePhrases = requiredEvidencePhrases.filter((phrase) => !plainIncludes(evidenceText, phrase));
  const internalLeaks = INTERNAL_LEAK_PATTERNS.filter((pattern) => patternMatches(answer, pattern));
  const forbiddenClaims = REPORT_FALSE_CLAIM_PATTERNS.filter((pattern) => patternMatches(answer, pattern));
  const sideEffectSignals = TEMPLATE_SIDE_EFFECT_PATTERNS.filter((pattern) => patternMatches(answer, pattern));
  const reportedSideEffect = hasReportedSideEffect(result || {});
  if (reportedSideEffect) {
    sideEffectSignals.push('reported_side_effect');
  }
  if (expectations.forbid_report_generation && (result?.report_triggered || result?.reportTriggered)) {
    sideEffectSignals.push('report_triggered');
  }

  const templateReuseMissing = Boolean(expectations.requires_template_reuse && !hasTemplateReuseLanguage(answer));
  const ordinaryQuestionMisroute = Boolean(
    expectations.ordinary_question
      && (
        result?.report_triggered
        || result?.reportTriggered
        || reportedSideEffect
        || /新百报表|经营分析模板|门店|生成报表/.test(answer)
      ),
  );

  const failureReasons = [];
  if (missingAnswerPatterns.length > 0) {
    failureReasons.push('missing_required_answer_pattern');
  }
  if (matchedForbiddenAnswerPatterns.length > 0) {
    failureReasons.push('forbidden_answer_pattern');
  }
  if (missingEvidenceTypes.length > 0 || missingEvidencePhrases.length > 0) {
    failureReasons.push('missing_required_evidence');
  }
  if (internalLeaks.length > 0) {
    failureReasons.push('internal_leak');
  }
  if (forbiddenClaims.length > 0) {
    failureReasons.push('forbidden_false_claim');
  }
  if (sideEffectSignals.length > 0) {
    failureReasons.push('template_or_report_side_effect');
  }
  if (templateReuseMissing) {
    failureReasons.push('template_reuse_language_missing');
  }
  if (ordinaryQuestionMisroute) {
    failureReasons.push('ordinary_question_misroute');
  }

  return {
    case_id: fixture.case_id,
    category: fixture.category,
    prompt: fixture.prompt,
    passed: failureReasons.length === 0,
    failure_reasons: failureReasons,
    answer_pattern_matched: missingAnswerPatterns.length === 0 && matchedForbiddenAnswerPatterns.length === 0,
    evidence_use_ok: missingEvidenceTypes.length === 0 && missingEvidencePhrases.length === 0,
    template_reuse_ok: !templateReuseMissing,
    ordinary_question_ok: !ordinaryQuestionMisroute,
    missing_answer_patterns: missingAnswerPatterns.map(patternLabel),
    matched_forbidden_answer_patterns: matchedForbiddenAnswerPatterns.map(patternLabel),
    missing_evidence_types: missingEvidenceTypes,
    missing_evidence_phrases: missingEvidencePhrases,
    internal_leaks: internalLeaks.map(patternLabel),
    forbidden_claims: forbiddenClaims.map(patternLabel),
    template_side_effect_signals: sideEffectSignals.map(patternLabel),
  };
}

function buildSyntheticGuardFixtures() {
  const baseFixture = {
    case_id: 'guard-template-reuse',
    category: 'template_reuse_guard',
    prompt: '在现有新百报表模板上解释租售比异常原因，不要重新生成新的可视化页面。',
    expectations: {
      required_answer_patterns: ['租售比'],
      forbidden_answer_patterns: ['HTML兜底'],
      required_evidence_types: ['database_aggregate'],
      required_evidence_phrases: ['租售比'],
      requires_template_reuse: true,
      forbid_report_generation: true,
    },
  };
  return [
    {
      name: 'internal leak detection',
      fixture: baseFixture,
      result: {
        answer: '租售比答案如下。evidence_state={}',
        evidence: [{ type: 'database_aggregate', text: '租售比' }],
      },
      expectedReason: 'internal_leak',
    },
    {
      name: 'false report claim detection',
      fixture: baseFixture,
      result: {
        answer: '租售比答案如下，已生成报表链接。',
        evidence: [{ type: 'database_aggregate', text: '租售比' }],
      },
      expectedReason: 'forbidden_false_claim',
    },
    {
      name: 'template side effect detection',
      fixture: baseFixture,
      result: {
        answer: '租售比答案如下，我会重新生成100个页面并使用HTML兜底。',
        evidence: [{ type: 'database_aggregate', text: '租售比' }],
        side_effects: { new_template_generated: true },
      },
      expectedReason: 'template_or_report_side_effect',
    },
    {
      name: 'ordinary question misroute detection',
      fixture: {
        case_id: 'guard-ordinary-question',
        category: 'ordinary_question_guard',
        prompt: '解释现金流折现。',
        expectations: {
          required_answer_patterns: ['现金流折现'],
          required_evidence_types: [],
          required_evidence_phrases: [],
          ordinary_question: true,
          forbid_report_generation: true,
        },
      },
      result: {
        answer: '现金流折现说明如下。',
        evidence: [],
        report_triggered: true,
      },
      expectedReason: 'ordinary_question_misroute',
    },
  ];
}

function assertSyntheticGuards() {
  for (const guard of buildSyntheticGuardFixtures()) {
    const evaluated = evaluateCase(guard.fixture, guard.result);
    assert(
      evaluated.failure_reasons.includes(guard.expectedReason),
      `${guard.name} should detect ${guard.expectedReason}`,
    );
  }
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function buildReport(fixtures, resultsByCaseId, sourceMode) {
  const caseResults = fixtures.map((fixture) => {
    const result = resultsByCaseId.get(fixture.case_id);
    if (!result) {
      return {
        case_id: fixture.case_id,
        category: fixture.category,
        prompt: fixture.prompt,
        passed: false,
        failure_reasons: ['missing_result'],
        answer_pattern_matched: false,
        evidence_use_ok: false,
        template_reuse_ok: !fixture.expectations?.requires_template_reuse,
        ordinary_question_ok: !fixture.expectations?.ordinary_question,
        missing_answer_patterns: fixture.expectations?.required_answer_patterns || [],
        matched_forbidden_answer_patterns: [],
        missing_evidence_types: fixture.expectations?.required_evidence_types || [],
        missing_evidence_phrases: fixture.expectations?.required_evidence_phrases || [],
        internal_leaks: [],
        forbidden_claims: [],
        template_side_effect_signals: [],
      };
    }
    return evaluateCase(fixture, result);
  });

  const count = caseResults.length || 1;
  const internalLeakCount = caseResults.reduce((sum, item) => sum + item.internal_leaks.length, 0);
  const forbiddenClaimCount = caseResults.reduce((sum, item) => sum + item.forbidden_claims.length, 0);
  const templateSideEffectCount = caseResults.reduce(
    (sum, item) => sum + item.template_side_effect_signals.length,
    0,
  );
  const ordinaryQuestionMisrouteCount = caseResults.filter((item) =>
    item.failure_reasons.includes('ordinary_question_misroute'),
  ).length;

  return {
    smoke: 'newbai-customer-answer',
    generated_at: new Date().toISOString(),
    ready: caseResults.every((item) => item.passed),
    source_mode: sourceMode,
    case_count: caseResults.length,
    passed_case_count: caseResults.filter((item) => item.passed).length,
    failed_case_count: caseResults.filter((item) => !item.passed).length,
    answer_pattern_match_rate: Number(
      (caseResults.filter((item) => item.answer_pattern_matched).length / count).toFixed(4),
    ),
    evidence_use_rate: Number((caseResults.filter((item) => item.evidence_use_ok).length / count).toFixed(4)),
    internal_leak_count: internalLeakCount,
    forbidden_claim_count: forbiddenClaimCount,
    template_side_effect_count: templateSideEffectCount,
    ordinary_question_misroute_count: ordinaryQuestionMisrouteCount,
    template_reuse_failure_count: caseResults.filter((item) =>
      item.failure_reasons.includes('template_reuse_language_missing'),
    ).length,
    evaluator_guard_count: buildSyntheticGuardFixtures().length,
    safety: {
      dataMaxCalled: false,
      providerCalled: false,
      databaseMutated: false,
      staticPagePublished: false,
      reportRenderEnqueued: false,
      postgresLexicalEnabled: false,
    },
    cases: caseResults,
  };
}

function markdownReport(report, jsonPath) {
  const lines = [
    '# NewBai Customer Answer Smoke',
    '',
    `- Status: ${report.ready ? 'passed' : 'failed'}`,
    `- Source mode: ${report.source_mode}`,
    `- Generated: ${report.generated_at}`,
    `- JSON report: ${jsonPath}`,
    `- Cases: ${report.passed_case_count}/${report.case_count} passed`,
    `- Answer pattern match rate: ${report.answer_pattern_match_rate}`,
    `- Evidence use rate: ${report.evidence_use_rate}`,
    `- Internal leak count: ${report.internal_leak_count}`,
    `- Forbidden claim count: ${report.forbidden_claim_count}`,
    `- Template side effect count: ${report.template_side_effect_count}`,
    `- Ordinary question misroute count: ${report.ordinary_question_misroute_count}`,
    '',
    '## Failed Cases',
    '',
  ];
  const failed = report.cases.filter((item) => !item.passed);
  if (failed.length === 0) {
    lines.push('- none');
  } else {
    for (const item of failed) {
      lines.push(`- ${item.case_id}: ${item.failure_reasons.join(', ')}`);
    }
  }
  lines.push(
    '',
    '## Safety',
    '',
    `- DataMax called: ${report.safety.dataMaxCalled}`,
    `- Provider called: ${report.safety.providerCalled}`,
    `- Database mutated: ${report.safety.databaseMutated}`,
    `- Static page published: ${report.safety.staticPagePublished}`,
    `- Report render enqueued: ${report.safety.reportRenderEnqueued}`,
    `- postgres_lexical enabled: ${report.safety.postgresLexicalEnabled}`,
    '',
  );
  return `${lines.join('\n')}\n`;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.selfTest && !args.resultsJsonl) {
    printHelp();
    throw new Error('--self-test or --results-jsonl is required');
  }

  assertSyntheticGuards();

  const fixtures = await readJsonl(args.fixturePath);
  const results = args.resultsJsonl
    ? await readJsonl(args.resultsJsonl)
    : fixtures.map((fixture) => ({ ...fixture.sample_result, case_id: fixture.case_id }));
  const resultsByCaseId = new Map(results.map((result) => [resultCaseId(result), result]));
  const report = buildReport(fixtures, resultsByCaseId, args.resultsJsonl ? 'results_jsonl' : 'fixture_sample');

  if ((args.selfTest || args.requireReady) && !report.ready) {
    const failed = report.cases
      .filter((item) => !item.passed)
      .map((item) => `${item.case_id}:${item.failure_reasons.join('|')}`)
      .join(', ');
    throw new Error(`NewBai customer answer smoke did not pass required readiness: ${failed}`);
  }

  await mkdir(args.outputDir, { recursive: true });
  const basename = `newbai-customer-answer-smoke-${makeRunId()}`;
  const jsonPath = join(args.outputDir, `${basename}.json`);
  const mdPath = join(args.outputDir, `${basename}.md`);
  await writeFile(jsonPath, `${JSON.stringify(report, null, args.pretty ? 2 : 0)}\n`, 'utf8');
  await writeFile(mdPath, markdownReport(report, jsonPath), 'utf8');

  console.log(
    `OK newbai customer answer smoke: ready=${report.ready} cases=${report.case_count} report=${jsonPath} summary=${mdPath}`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
