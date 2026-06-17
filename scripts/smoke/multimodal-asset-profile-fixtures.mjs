#!/usr/bin/env node

import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';

import {
  assetProfileKindOptions,
  filterAssetProfileHints,
  normalizeAssetLibraryScope,
} from '../../apps/web/app/lib/asset-library-view-model.js';

const DEFAULT_OUTPUT_DIR = 'target/multimodal-asset-profile-fixtures';
const ROOT_DIR = path.resolve(fileURLToPath(new URL('../..', import.meta.url)));
const GENERATED_VIDEO_FIXTURE_FILENAME = 'generated-video-ppt-fixture.mp4';

const IMAGE_FIXTURE_PATHS = [
  'apps/web/public/external-integrations/v3-enterprise-assistant-hero.png',
  'apps/web/public/external-integrations/datamax-v3-public-enterprise-solution/slide-01.png',
  'apps/web/public/external-integrations/datamax-v3-public-enterprise-solution/slide-08.png',
];

const PRESENTATION_FIXTURE_PATHS = [
  'target/presentations/datamax-v3-public-enterprise-solution.pptx',
];

const PRESENTATION_MANIFEST_PATHS = [
  'outputs/manual-20260613-v3-public-solution/presentations/datamax-v3-public-enterprise-solution/output/artifact-build-manifest.json',
  'outputs/manual-20260613-v3-codex-solution/presentations/datamax-v3-enterprise-codex/output/artifact-build-manifest.json',
];

const PRESENTATION_PREVIEW_PATHS = [
  'outputs/manual-20260613-v3-public-solution/presentations/datamax-v3-public-enterprise-solution/preview/slide-01.png',
  'apps/web/public/external-integrations/datamax-v3-public-enterprise-solution/slide-01.png',
];

const VIDEO_FIXTURE_PATHS = [
  'target/video-ppt-fixture.mp4',
  'fixtures/video-ppt/sample.mp4',
  'fixtures/video-ppt/react-in-5-minutes.mp4',
];

function parseArgs(argv) {
  const args = {
    outputDir: process.env.MULTIMODAL_ASSET_PROFILE_FIXTURE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    pretty: false,
    selfTest: false,
    requireRealVideo: false,
    generateVideo: process.env.MULTIMODAL_ASSET_PROFILE_GENERATE_VIDEO !== '0',
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--require-real-video') {
      args.requireRealVideo = true;
    } else if (arg === '--no-generate-video') {
      args.generateVideo = false;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return args;
}

function requireValue(flag, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`
Usage:
  node scripts/smoke/multimodal-asset-profile-fixtures.mjs --self-test
  node scripts/smoke/multimodal-asset-profile-fixtures.mjs --self-test --pretty
  node scripts/smoke/multimodal-asset-profile-fixtures.mjs --require-real-video

This local-only smoke validates multimodal asset profile fixtures:
- real PNG assets from the V3 public page/presentation previews;
- optional real PPTX/manifest evidence when local generated artifacts exist;
- generated or pre-existing MP4 evidence when ffmpeg/local fixtures are available;
- video profile metadata contract, explicitly marked metadata-only when no local video file exists.
`);
}

function resolveOutputDir(args) {
  return path.resolve(ROOT_DIR, args.outputDir);
}

function relativeToRoot(absolutePath) {
  return slash(path.relative(ROOT_DIR, absolutePath));
}

function runProcess(command, commandArgs, options = {}) {
  return new Promise((resolve, reject) => {
    execFile(
      command,
      commandArgs,
      {
        windowsHide: true,
        timeout: 30_000,
        ...options,
      },
      (error, stdout, stderr) => {
        if (error) {
          const processError = new Error(stderr?.trim() || error.message);
          processError.cause = error;
          reject(processError);
          return;
        }
        resolve({ stdout, stderr });
      },
    );
  });
}

async function generateVideoFixture(args) {
  if (!args.generateVideo) {
    return {
      attempted: false,
      created: false,
      reused: false,
      skipped_reason: 'disabled',
    };
  }

  const fixtureDir = path.join(resolveOutputDir(args), 'fixtures');
  const absoluteVideoPath = path.join(fixtureDir, GENERATED_VIDEO_FIXTURE_FILENAME);
  const relativePath = relativeToRoot(absoluteVideoPath);

  try {
    const existing = await stat(absoluteVideoPath);
    if (existing.isFile() && existing.size > 0) {
      return {
        attempted: false,
        created: false,
        reused: true,
        relative_path: relativePath,
        bytes: existing.size,
      };
    }
  } catch {
    // Missing fixture is expected on the first run.
  }

  await mkdir(fixtureDir, { recursive: true });

  try {
    await runProcess('ffmpeg', [
      '-hide_banner',
      '-loglevel',
      'error',
      '-y',
      '-f',
      'lavfi',
      '-i',
      'testsrc=size=640x360:rate=1:duration=2',
      '-pix_fmt',
      'yuv420p',
      absoluteVideoPath,
    ]);
    const generated = await stat(absoluteVideoPath);
    return {
      attempted: true,
      created: true,
      reused: false,
      relative_path: relativePath,
      bytes: generated.size,
    };
  } catch (error) {
    return {
      attempted: true,
      created: false,
      reused: false,
      relative_path: relativePath,
      error: error.message,
    };
  }
}

async function fileEvidence(relativePath) {
  const absolutePath = path.resolve(ROOT_DIR, relativePath);
  try {
    const fileStat = await stat(absolutePath);
    if (!fileStat.isFile()) return null;
    return {
      relative_path: slash(relativePath),
      bytes: fileStat.size,
      real_file_available: true,
    };
  } catch {
    return null;
  }
}

async function collectExistingFiles(relativePaths) {
  const files = [];
  for (const relativePath of relativePaths) {
    const evidence = await fileEvidence(relativePath);
    if (evidence) files.push(evidence);
  }
  return files;
}

async function readFirstJson(relativePaths) {
  for (const relativePath of relativePaths) {
    const absolutePath = path.resolve(ROOT_DIR, relativePath);
    try {
      const content = await readFile(absolutePath, 'utf8');
      return {
        relative_path: slash(relativePath),
        json: JSON.parse(content),
      };
    } catch {
      // Optional generated artifacts are allowed to be absent on a clean checkout.
    }
  }
  return null;
}

async function buildFixtureEvidence(args) {
  const generatedVideoFixture = await generateVideoFixture(args);
  const generatedVideoPaths = generatedVideoFixture.relative_path
    ? [generatedVideoFixture.relative_path]
    : [];
  const imageFiles = await collectExistingFiles(IMAGE_FIXTURE_PATHS);
  const presentationFiles = await collectExistingFiles(PRESENTATION_FIXTURE_PATHS);
  const presentationPreviews = await collectExistingFiles(PRESENTATION_PREVIEW_PATHS);
  const videoFiles = await collectExistingFiles([...generatedVideoPaths, ...VIDEO_FIXTURE_PATHS]);
  const presentationManifest = await readFirstJson(PRESENTATION_MANIFEST_PATHS);

  return {
    image_files: imageFiles,
    presentation_files: presentationFiles,
    presentation_preview_files: presentationPreviews,
    presentation_manifest: presentationManifest
      ? {
          relative_path: presentationManifest.relative_path,
          slide_count: Number(presentationManifest.json.slideCount || 0) || null,
          preview_count: Array.isArray(presentationManifest.json.previewPaths)
            ? presentationManifest.json.previewPaths.length
            : null,
        }
      : null,
    video_files: videoFiles,
    generated_video_fixture: generatedVideoFixture,
  };
}

function buildAssetProfileHints(evidence) {
  const slideCount = evidence.presentation_manifest?.slide_count || 22;
  const presentationEvidenceKind = evidence.presentation_files.length
    ? 'real_pptx'
    : evidence.presentation_manifest
      ? 'real_manifest'
      : evidence.presentation_preview_files.length
        ? 'real_preview_image'
        : 'metadata_only';
  const videoEvidenceKind = evidence.video_files.length ? 'real_video' : 'metadata_only';
  const realVideoAvailable = evidence.video_files.length > 0;

  return [
    {
      asset_id: 'fixture-image-datamax-hero',
      title: 'DataMax V3 public hero image',
      asset_kind: 'image',
      source_kind: 'local_fixture',
      profile_kind: 'image_semantic',
      summary: 'Enterprise data assistant visual that shows documents, databases, business systems, Q&A, reports, HTML artifacts, and callback capability.',
      noun_terms: ['DataMax V3', 'documents', 'databases', 'business systems', 'reports', 'HTML artifacts'],
      facets: ['file_evidence:real_png', 'layout:hero', 'semantic_source:vlm_fixture'],
      file_evidence_kind: 'real_png',
    },
    {
      asset_id: 'fixture-presentation-public-solution',
      title: 'DataMax V3 public enterprise solution presentation',
      asset_kind: 'presentation',
      source_kind: 'local_fixture',
      profile_kind: 'presentation_outline',
      summary: `Enterprise solution deck fixture with ${slideCount} slides covering V3 capabilities, cases, data control, and enterprise Codex client positioning.`,
      noun_terms: ['enterprise solution', 'architecture', 'cases', 'data control', 'Codex client'],
      facets: [
        `file_evidence:${presentationEvidenceKind}`,
        `slide_count:${slideCount}`,
        'outline:available',
      ],
      file_evidence_kind: presentationEvidenceKind,
    },
    {
      asset_id: 'fixture-video-ppt-contract',
      title: 'Video PPT extraction metadata contract',
      asset_kind: 'video',
      source_kind: realVideoAvailable ? 'local_fixture' : 'metadata_fixture',
      profile_kind: 'video_summary',
      summary: 'Video/PPT extraction contract fixture covering transcript, scene summaries, keyframe OCR, extraction manifest, and deliverable handoff status.',
      noun_terms: ['video extraction', 'transcript', 'keyframe OCR', 'scene summary', 'PPT deliverable'],
      facets: [
        `file_evidence:${videoEvidenceKind}`,
        'parse_status:metadata_regression',
        `real_video_file_available:${realVideoAvailable}`,
      ],
      file_evidence_kind: videoEvidenceKind,
    },
  ];
}

function validateScopeProfileHints(hints, args) {
  const scope = normalizeAssetLibraryScope({
    asset_profile_hints: hints,
    asset_profile_hint_count: hints.length,
  });
  const kinds = assetProfileKindOptions(scope);

  assert.equal(scope.assetProfileHints.length, 3);
  assert.deepEqual(kinds, ['image', 'presentation', 'video']);
  assert.equal(filterAssetProfileHints(scope, { assetKind: 'image' }).length, 1);
  assert.equal(filterAssetProfileHints(scope, { query: 'reports' })[0].assetId, 'fixture-image-datamax-hero');
  assert.equal(
    filterAssetProfileHints(scope, { query: 'slide_count' })[0].assetId,
    'fixture-presentation-public-solution',
  );
  assert.equal(
    filterAssetProfileHints(scope, { assetKind: 'video' })[0].assetId,
    'fixture-video-ppt-contract',
  );
  assert.equal(JSON.stringify(scope).includes('raw_provider_payload'), false);
  assert.equal(JSON.stringify(scope).includes('Authorization'), false);

  if (args.requireRealVideo) {
    assert.equal(
      scope.assetProfileHints.find((hint) => hint.assetKind === 'video')?.facets.includes('file_evidence:real_video'),
      true,
      '--require-real-video was set but no local video fixture was available',
    );
  }

  return { scope, kinds };
}

function buildMarkdown(report) {
  return [
    '# Multimodal Asset Profile Fixture Smoke',
    '',
    `- ok: ${report.ok}`,
    `- image_real_file_count: ${report.real_file_summary.image_real_file_count}`,
    `- presentation_real_file_available: ${report.real_file_summary.presentation_real_file_available}`,
    `- presentation_manifest_available: ${report.real_file_summary.presentation_manifest_available}`,
    `- video_real_file_available: ${report.real_file_summary.video_real_file_available}`,
    `- generated_video_fixture: ${report.real_file_summary.generated_video_fixture_status}`,
    `- hint_count: ${report.scope_summary.hint_count}`,
    `- asset_kinds: ${report.scope_summary.asset_kinds.join(', ')}`,
    '',
    '## Notes',
    '',
    ...report.notes.map((note) => `- ${note}`),
    '',
  ].join('\n');
}

function slash(value) {
  return String(value || '').replaceAll(path.sep, '/');
}

function makeRunId() {
  const timestamp = new Date().toISOString().replace(/[-:.]/g, '').replace(/Z$/, 'Z');
  return `${timestamp}-${process.pid}`;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const evidence = await buildFixtureEvidence(args);
  assert.ok(evidence.image_files.length >= 2, 'at least two real PNG fixture files are required');

  const hints = buildAssetProfileHints(evidence);
  const { scope, kinds } = validateScopeProfileHints(hints, args);
  const notes = [];
  if (!evidence.presentation_files.length) {
    notes.push('No local PPTX was found; presentation smoke used manifest/preview image evidence when available.');
  }
  if (!evidence.video_files.length) {
    notes.push('No local video file was found; video smoke covered metadata contract only and did not claim real video parsing.');
  }
  if (evidence.generated_video_fixture.error) {
    notes.push(`Generated video fixture was unavailable: ${evidence.generated_video_fixture.error}`);
  }

  const report = {
    smoke: 'multimodal-asset-profile-fixtures',
    ok: true,
    self_test: args.selfTest,
    generated_at: new Date().toISOString(),
    real_file_summary: {
      image_real_file_count: evidence.image_files.length,
      presentation_real_file_available: evidence.presentation_files.length > 0,
      presentation_manifest_available: Boolean(evidence.presentation_manifest),
      presentation_preview_real_file_count: evidence.presentation_preview_files.length,
      video_real_file_available: evidence.video_files.length > 0,
      generated_video_fixture_status: evidence.generated_video_fixture.created
        ? 'created'
        : evidence.generated_video_fixture.reused
          ? 'reused'
          : evidence.generated_video_fixture.skipped_reason || (evidence.generated_video_fixture.error ? 'failed' : 'not_available'),
    },
    evidence,
    scope_summary: {
      hint_count: scope.assetProfileHintCount,
      normalized_hint_count: scope.assetProfileHints.length,
      asset_kinds: kinds,
      filtered_image_count: filterAssetProfileHints(scope, { assetKind: 'image' }).length,
      filtered_video_count: filterAssetProfileHints(scope, { assetKind: 'video' }).length,
    },
    fixture_hints: scope.assetProfileHints.map((hint) => ({
      asset_id: hint.assetId,
      title: hint.title,
      asset_kind: hint.assetKind,
      profile_kind: hint.profileKind,
      summary: hint.summary,
      noun_terms: hint.nounTerms,
      facets: hint.facets,
    })),
    notes,
  };

  const outputDir = resolveOutputDir(args);
  await mkdir(outputDir, { recursive: true });
  const baseName = `multimodal-asset-profile-fixtures-${makeRunId()}`;
  const jsonPath = path.join(outputDir, `${baseName}.json`);
  const mdPath = path.join(outputDir, `${baseName}.md`);
  await writeFile(jsonPath, `${JSON.stringify(report, null, args.pretty ? 2 : 0)}\n`, 'utf8');
  await writeFile(mdPath, buildMarkdown(report), 'utf8');

  console.log(
    [
      'OK multimodal asset profile fixture smoke:',
      `ok=${report.ok}`,
      `imageFiles=${report.real_file_summary.image_real_file_count}`,
      `presentationRealFile=${report.real_file_summary.presentation_real_file_available}`,
      `videoRealFile=${report.real_file_summary.video_real_file_available}`,
      `report=${slash(path.relative(ROOT_DIR, jsonPath))}`,
      `summary=${slash(path.relative(ROOT_DIR, mdPath))}`,
    ].join(' '),
  );
}

main().catch((error) => {
  console.error(`FAIL multimodal asset profile fixture smoke: ${error.message}`);
  process.exit(1);
});
