#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';
import { platform as osPlatform } from 'node:os';

const DEFAULT_OUTPUT_DIR = 'target/authorized-capture-smoke';
const DEFAULT_DURATION_SECONDS = 60;
const DEFAULT_RETENTION_DAYS = 7;
const DEFAULT_WIDTH = 1280;
const DEFAULT_HEIGHT = 720;
const DEFAULT_FRAMERATE = 15;
const MAX_DURATION_SECONDS = 300;
const SUPPORTED_CAPTURE_MODES = new Set([
  'macos-avfoundation',
  'linux-x11grab',
  'windows-gdigrab',
  'custom',
]);

function parseArgs(argv) {
  const args = {
    selfTest: parseBoolean(process.env.AUTHORIZED_CAPTURE_SELF_TEST),
    dryRun: parseBoolean(process.env.AUTHORIZED_CAPTURE_DRY_RUN),
    runCapture: parseBoolean(process.env.AUTHORIZED_CAPTURE_RUN_CAPTURE),
    ackAuthorized: parseBoolean(process.env.AUTHORIZED_CAPTURE_ACK_AUTHORIZED),
    approvalId: process.env.AUTHORIZED_CAPTURE_APPROVAL_ID || '',
    approvedBy: process.env.AUTHORIZED_CAPTURE_APPROVED_BY || '',
    url: process.env.AUTHORIZED_CAPTURE_URL || '',
    purpose: process.env.AUTHORIZED_CAPTURE_PURPOSE || '',
    outputDir: process.env.AUTHORIZED_CAPTURE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    durationSeconds: Number(
      process.env.AUTHORIZED_CAPTURE_DURATION_SECONDS || DEFAULT_DURATION_SECONDS,
    ),
    retentionDays: Number(process.env.AUTHORIZED_CAPTURE_RETENTION_DAYS || DEFAULT_RETENTION_DAYS),
    captureAudio: parseBoolean(process.env.AUTHORIZED_CAPTURE_AUDIO),
    keepProfile: parseBoolean(process.env.AUTHORIZED_CAPTURE_KEEP_PROFILE),
    keepFailedOutput: parseBoolean(process.env.AUTHORIZED_CAPTURE_KEEP_FAILED_OUTPUT),
    browserBin: process.env.AUTHORIZED_CAPTURE_BROWSER_BIN || '',
    browserArg: [],
    ffmpegBin: process.env.AUTHORIZED_CAPTURE_FFMPEG_BIN || 'ffmpeg',
    ffmpegArg: [],
    ffmpegInput: process.env.AUTHORIZED_CAPTURE_FFMPEG_INPUT || '',
    captureMode: process.env.AUTHORIZED_CAPTURE_MODE || defaultCaptureMode(),
    width: Number(process.env.AUTHORIZED_CAPTURE_WIDTH || DEFAULT_WIDTH),
    height: Number(process.env.AUTHORIZED_CAPTURE_HEIGHT || DEFAULT_HEIGHT),
    framerate: Number(process.env.AUTHORIZED_CAPTURE_FRAMERATE || DEFAULT_FRAMERATE),
    handoff: process.env.AUTHORIZED_CAPTURE_HANDOFF || 'none',
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--self-test') {
      args.selfTest = true;
      args.dryRun = true;
    } else if (arg === '--dry-run') {
      args.dryRun = true;
    } else if (arg === '--run-capture') {
      args.runCapture = true;
    } else if (arg === '--ack-authorized') {
      args.ackAuthorized = true;
    } else if (arg === '--approval-id') {
      args.approvalId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--approved-by') {
      args.approvedBy = requireValue(arg, next);
      index += 1;
    } else if (arg === '--url') {
      args.url = requireValue(arg, next);
      index += 1;
    } else if (arg === '--purpose') {
      args.purpose = requireValue(arg, next);
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--duration-seconds') {
      args.durationSeconds = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--retention-days') {
      args.retentionDays = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--capture-audio') {
      args.captureAudio = true;
    } else if (arg === '--keep-profile') {
      args.keepProfile = true;
    } else if (arg === '--keep-failed-output') {
      args.keepFailedOutput = true;
    } else if (arg === '--browser-bin') {
      args.browserBin = requireValue(arg, next);
      index += 1;
    } else if (arg === '--browser-arg') {
      args.browserArg.push(requireValue(arg, next));
      index += 1;
    } else if (arg === '--ffmpeg-bin') {
      args.ffmpegBin = requireValue(arg, next);
      index += 1;
    } else if (arg === '--ffmpeg-arg') {
      args.ffmpegArg.push(requireValue(arg, next));
      index += 1;
    } else if (arg === '--ffmpeg-input') {
      args.ffmpegInput = requireValue(arg, next);
      index += 1;
    } else if (arg === '--capture-mode') {
      args.captureMode = requireValue(arg, next);
      index += 1;
    } else if (arg === '--width') {
      args.width = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--height') {
      args.height = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--framerate') {
      args.framerate = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--handoff') {
      args.handoff = requireValue(arg, next);
      index += 1;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (args.selfTest) {
    args.dryRun = true;
    args.ackAuthorized = true;
    args.approvalId ||= 'self-test-approval';
    args.approvedBy ||= 'self-test-operator';
    args.url ||= 'https://example.com/authorized-courseware-video';
    args.purpose ||= 'self-test command planning only';
  }
  validateArgs(args);
  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run capture:authorized-video -- --self-test

  npm run capture:authorized-video -- \\
    --dry-run \\
    --ack-authorized \\
    --approval-id APPROVAL-20260607-001 \\
    --approved-by operator-name \\
    --url https://example.com/authorized-video-page \\
    --purpose "authorized customer courseware sample"

  npm run capture:authorized-video -- \\
    --run-capture \\
    --ack-authorized \\
    --approval-id APPROVAL-20260607-001 \\
    --approved-by operator-name \\
    --url https://example.com/authorized-video-page \\
    --purpose "authorized customer courseware sample" \\
    --browser-bin "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \\
    --capture-mode macos-avfoundation \\
    --ffmpeg-input "1:none" \\
    --duration-seconds 60 \\
    --handoff upload-main

Checks:
  - requires explicit authorization metadata before any live capture
  - opens the URL in an isolated temporary browser profile
  - records a bounded MP4 through FFmpeg only when --run-capture is set
  - defaults to no audio and deletes the temporary browser profile
  - writes only redacted source metadata and capture receipt fields

Notes:
  - --self-test and --dry-run do not open a browser or run FFmpeg
  - this script does not upload the MP4; use the printed handoff command after review
  - do not use this to bypass login, collect cookies, save QR screenshots, or record without approval
`);
}

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y', 'on'].includes(String(value || '').trim().toLowerCase());
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function defaultCaptureMode() {
  const platform = osPlatform();
  if (platform === 'darwin') return 'macos-avfoundation';
  if (platform === 'linux') return 'linux-x11grab';
  if (platform === 'win32') return 'windows-gdigrab';
  return 'custom';
}

function validateArgs(args) {
  if (!args.dryRun && !args.runCapture) {
    throw new Error('use --self-test, --dry-run, or --run-capture');
  }
  if (args.runCapture && args.dryRun) {
    throw new Error('--dry-run and --run-capture cannot be combined');
  }
  if (!args.ackAuthorized) {
    throw new Error('--ack-authorized is required for dry-run and live capture planning');
  }
  if (!args.approvalId.trim()) {
    throw new Error('--approval-id is required');
  }
  if (!args.approvedBy.trim()) {
    throw new Error('--approved-by is required');
  }
  if (!args.purpose.trim()) {
    throw new Error('--purpose is required');
  }
  validateUrl(args.url);
  if (!Number.isInteger(args.durationSeconds)
    || args.durationSeconds < 5
    || args.durationSeconds > MAX_DURATION_SECONDS) {
    throw new Error(`--duration-seconds must be an integer from 5 to ${MAX_DURATION_SECONDS}`);
  }
  if (!Number.isInteger(args.retentionDays) || args.retentionDays < 0 || args.retentionDays > 7) {
    throw new Error('--retention-days must be an integer from 0 to 7');
  }
  if (!Number.isInteger(args.width) || args.width < 320 || args.width > 7680) {
    throw new Error('--width must be an integer from 320 to 7680');
  }
  if (!Number.isInteger(args.height) || args.height < 240 || args.height > 4320) {
    throw new Error('--height must be an integer from 240 to 4320');
  }
  if (!Number.isInteger(args.framerate) || args.framerate < 1 || args.framerate > 60) {
    throw new Error('--framerate must be an integer from 1 to 60');
  }
  if (!SUPPORTED_CAPTURE_MODES.has(args.captureMode)) {
    throw new Error(`--capture-mode must be one of: ${[...SUPPORTED_CAPTURE_MODES].join(', ')}`);
  }
  if (args.runCapture && !args.browserBin.trim()) {
    throw new Error('--browser-bin is required when --run-capture is set');
  }
  if (args.runCapture && !args.ffmpegBin.trim()) {
    throw new Error('--ffmpeg-bin is required when --run-capture is set');
  }
  if (args.runCapture && args.captureMode === 'macos-avfoundation' && !args.ffmpegInput.trim()) {
    throw new Error('--ffmpeg-input is required for macos-avfoundation, for example "1:none"');
  }
  if (args.runCapture && args.captureMode === 'custom' && args.ffmpegArg.length === 0) {
    throw new Error('--ffmpeg-arg is required when --capture-mode custom is used with --run-capture');
  }
  if (args.captureAudio && !args.runCapture) {
    throw new Error('--capture-audio is only meaningful with --run-capture');
  }
  if (!['none', 'upload-main', 'external-video'].includes(args.handoff)) {
    throw new Error('--handoff must be one of: none, upload-main, external-video');
  }
}

function validateUrl(rawUrl) {
  if (!rawUrl) {
    throw new Error('--url is required');
  }
  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    throw new Error('--url must be a valid URL');
  }
  if (!['http:', 'https:'].includes(url.protocol)) {
    throw new Error('--url must use http or https');
  }
  if (!url.hostname || url.hostname === 'localhost' || url.hostname === '127.0.0.1') {
    throw new Error('--url must identify the authorized source host, not localhost');
  }
  return url;
}

function makeRunId() {
  const timestamp = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 17);
  return `${timestamp}-${process.pid}`;
}

function sourceSummary(rawUrl) {
  const url = validateUrl(rawUrl);
  const pathParts = url.pathname.split('/').filter(Boolean);
  return {
    scheme: url.protocol.replace(/:$/, ''),
    host: url.host,
    pathDepth: pathParts.length,
    pathExtension: pathParts.length && pathParts[pathParts.length - 1].includes('.')
      ? pathParts[pathParts.length - 1].split('.').pop().slice(0, 16)
      : '',
  };
}

function browserArgs(args, profileDir) {
  return [
    `--user-data-dir=${profileDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-sync',
    '--disable-background-networking',
    '--disable-extensions',
    '--disable-application-cache',
    '--autoplay-policy=no-user-gesture-required',
    `--window-size=${args.width},${args.height}`,
    ...args.browserArg,
    args.url,
  ];
}

function ffmpegArgs(args, outputPath) {
  const common = [
    '-y',
    '-t',
    String(args.durationSeconds),
    '-r',
    String(args.framerate),
  ];
  if (args.captureMode === 'macos-avfoundation') {
    return [
      ...common,
      '-f',
      'avfoundation',
      '-framerate',
      String(args.framerate),
      '-video_size',
      `${args.width}x${args.height}`,
      '-i',
      args.ffmpegInput,
      ...videoOutputArgs(args),
      ...args.ffmpegArg,
      outputPath,
    ];
  }
  if (args.captureMode === 'linux-x11grab') {
    return [
      ...common,
      '-f',
      'x11grab',
      '-video_size',
      `${args.width}x${args.height}`,
      '-i',
      args.ffmpegInput || process.env.DISPLAY || ':0.0',
      ...videoOutputArgs(args),
      ...args.ffmpegArg,
      outputPath,
    ];
  }
  if (args.captureMode === 'windows-gdigrab') {
    return [
      ...common,
      '-f',
      'gdigrab',
      '-i',
      args.ffmpegInput || 'desktop',
      ...videoOutputArgs(args),
      ...args.ffmpegArg,
      outputPath,
    ];
  }
  return [
    ...args.ffmpegArg,
    outputPath,
  ];
}

function videoOutputArgs(args) {
  return [
    ...(args.captureAudio ? [] : ['-an']),
    '-pix_fmt',
    'yuv420p',
    '-movflags',
    '+faststart',
  ];
}

function spawnAndWait(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      stdio: options.stdio || 'ignore',
      detached: Boolean(options.detached),
    });
    child.on('error', (error) => {
      resolve({ ok: false, exitCode: null, signal: null, error: error.message });
    });
    child.on('exit', (exitCode, signal) => {
      resolve({ ok: exitCode === 0, exitCode, signal, error: null });
    });
    if (options.unref) {
      child.unref();
      resolve({ ok: true, exitCode: null, signal: null, error: null, detached: true });
    }
  });
}

async function fileReceipt(outputPath) {
  if (!existsSync(outputPath)) {
    return null;
  }
  const fileStat = await stat(outputPath);
  const bytes = await readFile(outputPath);
  return {
    fileName: basename(outputPath),
    bytes: fileStat.size,
    sha256Prefix: createHash('sha256').update(bytes).digest('hex').slice(0, 16),
  };
}

function handoffSuggestion(args, outputPath) {
  if (args.handoff === 'upload-main') {
    return {
      mode: 'upload-main',
      command:
        `npm run smoke:video-ppt-upload-main -- --fixture-file ${shellQuote(outputPath)} --fixture-name ${shellQuote(basename(outputPath))}`,
    };
  }
  if (args.handoff === 'external-video') {
    return {
      mode: 'external-video',
      command:
        `npm run smoke:external-video-ppt -- --fixture-file ${shellQuote(outputPath)} --fixture-name ${shellQuote(basename(outputPath))} --loopback-fixture`,
    };
  }
  return { mode: 'none', command: null };
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}

async function runSelfTest(args, runId) {
  const runDir = join(process.cwd(), args.outputDir, runId);
  const profileDir = join(runDir, 'browser-profile');
  const outputPath = join(runDir, 'authorized-capture.mp4');
  const report = buildPlanReport(args, runId, runDir, profileDir, outputPath);
  report.summary.ok = true;
  report.summary.selfTest = true;
  report.capture = {
    attempted: false,
    dryRun: true,
    browserWouldRun: true,
    ffmpegWouldRun: true,
  };
  await mkdir(runDir, { recursive: true });
  const reportPath = join(runDir, 'report.json');
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({ ok: true, selfTest: true, runId, reportPath }, null, 2));
}

function buildPlanReport(args, runId, runDir, profileDir, outputPath) {
  const plannedBrowserArgs = browserArgs(args, profileDir);
  const plannedFfmpegArgs = ffmpegArgs(args, outputPath);
  return {
    summary: {
      ok: false,
      runId,
      mode: args.runCapture ? 'run-capture' : 'dry-run',
      approvalId: args.approvalId,
      approvedBy: args.approvedBy,
      source: sourceSummary(args.url),
      durationSeconds: args.durationSeconds,
      retentionDays: args.retentionDays,
      captureAudioAllowed: args.captureAudio,
      handoff: args.handoff,
      createdAt: new Date().toISOString(),
    },
    safety: {
      ackAuthorized: args.ackAuthorized,
      isolatedBrowserProfile: true,
      persistentProfileDisabled: !args.keepProfile,
      rawUrlStored: false,
      cookiesStored: false,
      harStored: false,
      qrScreenshotStored: false,
      runDir,
    },
    commands: {
      browser: {
        bin: args.browserBin || defaultBrowserHint(),
        args: redactBrowserArgs(plannedBrowserArgs),
      },
      ffmpeg: {
        bin: args.ffmpegBin,
        mode: args.captureMode,
        args: redactFfmpegArgs(plannedFfmpegArgs, outputPath),
      },
    },
    output: {
      fileName: basename(outputPath),
      outputPath,
      profileDir,
      handoff: handoffSuggestion(args, outputPath),
    },
  };
}

function redactBrowserArgs(args) {
  return args.map((arg) => {
    if (/^https?:\/\//i.test(arg)) return '<redacted-authorized-url>';
    return arg;
  });
}

function redactFfmpegArgs(args, outputPath) {
  return args.map((arg) => (arg === outputPath ? '<capture-output-mp4>' : arg));
}

function defaultBrowserHint() {
  if (osPlatform() === 'darwin') {
    return '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  }
  if (osPlatform() === 'linux') {
    return 'google-chrome';
  }
  if (osPlatform() === 'win32') {
    return 'chrome.exe';
  }
  return 'browser';
}

async function runDryRun(args, runId) {
  const runDir = join(process.cwd(), args.outputDir, runId);
  const profileDir = join(runDir, 'browser-profile');
  const outputPath = join(runDir, 'authorized-capture.mp4');
  const report = buildPlanReport(args, runId, runDir, profileDir, outputPath);
  report.summary.ok = true;
  report.capture = {
    attempted: false,
    dryRun: true,
    browserWouldRun: Boolean(args.browserBin || defaultBrowserHint()),
    ffmpegWouldRun: Boolean(args.ffmpegBin),
  };
  await mkdir(runDir, { recursive: true });
  const reportPath = join(runDir, 'report.json');
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({
    ok: true,
    dryRun: true,
    runId,
    source: report.summary.source,
    durationSeconds: args.durationSeconds,
    captureMode: args.captureMode,
    handoff: report.output.handoff.mode,
    reportPath,
  }, null, 2));
}

async function runCapture(args, runId) {
  const runDir = resolve(process.cwd(), args.outputDir, runId);
  const profileDir = join(runDir, 'browser-profile');
  const outputPath = join(runDir, 'authorized-capture.mp4');
  await mkdir(profileDir, { recursive: true });
  const report = buildPlanReport(args, runId, runDir, profileDir, outputPath);
  let browserResult = null;
  let ffmpegResult = null;
  try {
    browserResult = await spawnAndWait(args.browserBin, browserArgs(args, profileDir), {
      detached: true,
      unref: true,
    });
    if (!browserResult.ok) {
      throw new Error(`browser launch failed: ${browserResult.error || browserResult.signal || 'unknown'}`);
    }
    await wait(2_000);
    ffmpegResult = await spawnAndWait(args.ffmpegBin, ffmpegArgs(args, outputPath));
    const receipt = await fileReceipt(outputPath);
    report.capture = {
      attempted: true,
      dryRun: false,
      browser: browserResult,
      ffmpeg: ffmpegResult,
      file: receipt,
    };
    report.summary.ok = Boolean(ffmpegResult.ok && receipt?.bytes > 0);
    if (!report.summary.ok) {
      throw new Error(`ffmpeg capture failed: ${ffmpegResult.error || ffmpegResult.signal || ffmpegResult.exitCode}`);
    }
  } catch (error) {
    report.summary.ok = false;
    report.capture = {
      attempted: true,
      dryRun: false,
      browser: browserResult,
      ffmpeg: ffmpegResult,
      error: error instanceof Error ? error.message : String(error),
    };
    if (!args.keepFailedOutput) {
      await rm(outputPath, { force: true }).catch(() => {});
    }
  } finally {
    if (!args.keepProfile) {
      await rm(profileDir, { recursive: true, force: true }).catch(() => {});
      report.safety.profileRemoved = true;
    } else {
      report.safety.profileRemoved = false;
    }
  }
  const reportPath = join(runDir, 'report.json');
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({
    ok: report.summary.ok,
    runId,
    source: report.summary.source,
    captureMode: args.captureMode,
    file: report.capture?.file || null,
    handoff: report.output.handoff.mode,
    reportPath,
  }, null, 2));
  if (!report.summary.ok) {
    process.exitCode = 1;
  }
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const runId = makeRunId();
  if (args.selfTest) {
    await runSelfTest(args, runId);
  } else if (args.dryRun) {
    await runDryRun(args, runId);
  } else {
    await runCapture(args, runId);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
