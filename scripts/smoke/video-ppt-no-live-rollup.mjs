#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const DEFAULT_OUTPUT_DIR = 'target/video-ppt-no-live-rollup';

const COMMANDS = [
  {
    id: 'upload_main_syntax',
    description: 'main-site upload smoke syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/video-ppt-upload-main.mjs'],
  },
  {
    id: 'upload_main_self_test',
    description: 'main-site upload smoke offline artifact contract',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-upload-main.mjs', '--self-test'],
  },
  {
    id: 'upload_main_preflight',
    description: 'main-site upload smoke live gate preflight without network',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-upload-main.mjs', '--preflight'],
  },
  {
    id: 'external_video_ppt_syntax',
    description: 'third-party video PPT smoke syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/external-video-ppt.mjs'],
  },
  {
    id: 'external_video_ppt_self_test',
    description: 'third-party video PPT offline reply and deliverable contract',
    command: process.execPath,
    args: ['scripts/smoke/external-video-ppt.mjs', '--self-test'],
  },
  {
    id: 'external_video_ppt_preflight',
    description: 'third-party video PPT live gate preflight without network',
    command: process.execPath,
    args: ['scripts/smoke/external-video-ppt.mjs', '--preflight', '--allow-missing-bearer'],
  },
  {
    id: 'video_ppt_handoff_syntax',
    description: 'login-gated video handoff syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/video-ppt-handoff.mjs'],
  },
  {
    id: 'video_ppt_handoff_self_test',
    description: 'login-gated video handoff offline contract',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-handoff.mjs', '--self-test'],
  },
  {
    id: 'video_ppt_handoff_preflight',
    description: 'login-gated video handoff live gate preflight without network',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-handoff.mjs', '--preflight', '--allow-missing-bearer'],
  },
  {
    id: 'authorized_capture_syntax',
    description: 'authorized capture helper syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/capture-authorized-video.mjs'],
  },
  {
    id: 'authorized_capture_self_test',
    description: 'authorized capture helper authorization gates',
    command: process.execPath,
    args: ['scripts/capture-authorized-video.mjs', '--self-test'],
  },
  {
    id: 'quality_matrix_syntax',
    description: 'quality matrix syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/video-ppt-quality-matrix.mjs'],
  },
  {
    id: 'quality_matrix_self_test',
    description: 'quality matrix deterministic self-test and review-risk regression',
    command: process.execPath,
    args: [
      'scripts/smoke/video-ppt-quality-matrix.mjs',
      '--self-test',
      '--output-dir',
      'target/video-ppt-no-live-rollup-quality-matrix',
    ],
  },
  {
    id: 'video_deliverables_validator_syntax',
    description: 'video deliverables validator syntax check',
    command: process.execPath,
    args: ['--check', 'tools/validate-video-deliverables.mjs'],
  },
  {
    id: 'video_deliverables_validator_tests',
    description: 'video deliverables validator test suite',
    command: 'npm',
    args: ['run', 'test:video-deliverables'],
  },
];

function parseArgs(argv) {
  const args = {
    selfTest: false,
    outputDir: process.env.VIDEO_PPT_NO_LIVE_ROLLUP_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    pretty: false,
    help: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--output-dir') {
      args.outputDir = requiredValue(argv, index += 1, arg);
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function requiredValue(argv, index, flag) {
  const value = argv[index];
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function usage() {
  return `Usage:
  npm run smoke:video-ppt-no-live-rollup -- --self-test [--pretty] [--output-dir target/video-ppt-no-live-rollup]

Checks:
  - runs video PPT upload self-test and preflight without network calls
  - runs third-party video PPT self-test and preflight without network calls
  - runs login-gated video handoff self-test and preflight without network calls
  - runs authorized-capture self-test without opening a browser or FFmpeg
  - runs quality matrix self-test, including review-risk regression
  - runs video deliverables validator tests

Safety:
  - does not call DataMax live endpoints
  - does not download videos
  - does not upload files or create datasets/documents/runs
  - does not open browsers, record screens, or run FFmpeg capture
  - does not deploy, restart, or touch any server`;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(usage());
    return;
  }
  if (!args.selfTest) {
    throw new Error('--self-test is required for the no-live rollup');
  }
  const startedAt = new Date();
  const results = [];
  for (const command of COMMANDS) {
    process.stdout.write(`running ${command.id}... `);
    const result = runCommand(command);
    results.push(result);
    process.stdout.write(`${result.status}\n`);
  }
  const report = buildReport({ startedAt, results });
  validateReport(report);
  const reportPath = writeReport(args.outputDir, report, args.pretty);
  const failedCount = report.summary.failed_count;
  console.log([
    'OK video PPT no-live rollup:',
    `commands=${report.summary.command_count}`,
    `passed=${report.summary.passed_count}`,
    `failed=${failedCount}`,
    `report=${reportPath}`,
  ].join(' '));
  if (failedCount > 0) {
    process.exitCode = 1;
  }
}

function runCommand(command) {
  const started = Date.now();
  const child = spawnSync(command.command, command.args, {
    cwd: process.cwd(),
    env: {
      ...process.env,
      NO_COLOR: '1',
    },
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  const durationMs = Date.now() - started;
  const exitCode = typeof child.status === 'number' ? child.status : 1;
  return {
    id: command.id,
    description: command.description,
    status: exitCode === 0 ? 'passed' : 'failed',
    exit_code: exitCode,
    duration_ms: durationMs,
    command: displayCommand(command),
    stdout_line_count: lineCount(child.stdout),
    stderr_line_count: lineCount(child.stderr),
    failure_excerpt: exitCode === 0
      ? null
      : sanitizeExcerpt(`${child.stderr || ''}\n${child.stdout || ''}`),
  };
}

function displayCommand(command) {
  if (command.command === process.execPath) {
    return ['node', ...command.args].join(' ');
  }
  return [command.command, ...command.args].join(' ');
}

function lineCount(text) {
  if (!text) {
    return 0;
  }
  return text.trim().split(/\r?\n/).filter(Boolean).length;
}

function sanitizeExcerpt(text) {
  return text
    .split(/\r?\n/)
    .filter(Boolean)
    .slice(-20)
    .join('\n')
    .replaceAll(process.cwd(), '[repo]')
    .replaceAll(process.env.HOME || '', '[home]')
    .replace(/[A-Za-z]:[\\/][^\s"']+/g, '[redacted-path]')
    .replace(/\/Users\/[^\s"']+/g, '[redacted-path]')
    .replace(/\/home\/[^\s"']+/g, '[redacted-path]')
    .replace(/https?:\/\/[^\s"']+/g, '[redacted-url]')
    .replace(/(bearer|authorization|cookie|token|password|secret|provider_key)=?[^\s"']*/gi, '$1=[redacted]');
}

function buildReport({ startedAt, results }) {
  const failed = results.filter((result) => result.status !== 'passed');
  return {
    schema: 'v3.video_ppt_no_live_rollup.v1',
    status: failed.length === 0 ? 'passed' : 'failed',
    started_at: startedAt.toISOString(),
    completed_at: new Date().toISOString(),
    summary: {
      command_count: results.length,
      passed_count: results.length - failed.length,
      failed_count: failed.length,
    },
    gates: {
      live_smoke_run: false,
      production_write_allowed: false,
      network_download_allowed: false,
      file_upload_allowed: false,
      browser_capture_allowed: false,
      service_deployment_allowed: false,
      server_8_touched: false,
      server_120_touched: false,
    },
    commands: results,
    redaction: {
      status: 'applied',
      source_urls_included: false,
      local_paths_included: false,
      credentials_included: false,
      provider_payloads_included: false,
    },
    next_actions: failed.length === 0
      ? [
        'keep this as the no-live regression rollup before live upload, third-party, handoff, capture, or deployment gates',
      ]
      : [
        'inspect failed command locally and rerun the no-live rollup before any live gate',
      ],
  };
}

function validateReport(report) {
  if (report.schema !== 'v3.video_ppt_no_live_rollup.v1') {
    throw new Error('invalid no-live rollup schema');
  }
  if (report.summary.command_count !== COMMANDS.length) {
    throw new Error('no-live rollup command count mismatch');
  }
  if (
    report.gates.live_smoke_run
    || report.gates.production_write_allowed
    || report.gates.network_download_allowed
    || report.gates.file_upload_allowed
    || report.gates.browser_capture_allowed
    || report.gates.service_deployment_allowed
    || report.gates.server_8_touched
    || report.gates.server_120_touched
  ) {
    throw new Error('no-live rollup safety gates are invalid');
  }
  const serialized = JSON.stringify(report);
  if (serialized.match(/[A-Za-z]:[\\/]|[\\/]Users[\\/]|[\\/]home[\\/]|https?:\/\/|token=|cookie=|bearer=/i)) {
    throw new Error('no-live rollup report contains unredacted local path, URL, or token-like text');
  }
}

function writeReport(outputDir, report, pretty) {
  fs.mkdirSync(outputDir, { recursive: true });
  const timestamp = new Date().toISOString().replace(/[-:]/g, '').replace(/\..+/, '');
  const reportPath = path.join(outputDir, `${timestamp}-${process.pid}-no-live-rollup.json`);
  fs.writeFileSync(reportPath, JSON.stringify(report, null, pretty ? 2 : 0));
  return reportPath;
}

try {
  main();
} catch (error) {
  console.error(`video PPT no-live rollup failed: ${error.message}`);
  process.exitCode = 1;
}
