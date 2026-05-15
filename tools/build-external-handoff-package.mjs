#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';
import { validateExternalHandoffManifest } from './validate-external-handoff.mjs';
import { validateAll, writeAllReportFiles } from './validate-external-handoff-all.mjs';
import { renderReleaseMarkdown, validateRelease } from './validate-external-handoff-release.mjs';

const PACKAGE_TYPE = 'v3.external_third_party_handoff_package.v1';

const SOURCE_FILES = [
  {
    source: 'docs/integrations/third-party-integration-api.zh-CN.md',
    target: 'docs/third-party-integration-api.zh-CN.md',
    audience: 'third_party',
  },
  {
    source: 'docs/integrations/third-party-integration-api.md',
    target: 'docs/third-party-integration-api.md',
    audience: 'third_party',
  },
  {
    source: 'docs/integrations/third-party-handoff.sample.json',
    target: 'handoff/third-party-handoff.sample.json',
    audience: 'third_party',
  },
  {
    source: 'scripts/external-third-party-mock-gateway.mjs',
    target: 'sandbox/external-third-party-mock-gateway.mjs',
    audience: 'third_party',
  },
  {
    source: 'scripts/run-external-third-party-gateway-smoke.sh',
    target: 'sandbox/run-external-third-party-gateway-smoke.sh',
    audience: 'v3_operator',
  },
  {
    source: 'tools/validate-external-handoff.mjs',
    target: 'tools/validate-external-handoff.mjs',
    audience: 'third_party',
  },
  {
    source: 'tools/validate-external-handoff-package.mjs',
    target: 'tools/validate-external-handoff-package.mjs',
    audience: 'third_party',
  },
  {
    source: 'tools/validate-external-handoff-archive.mjs',
    target: 'tools/validate-external-handoff-archive.mjs',
    audience: 'third_party',
  },
  {
    source: 'tools/validate-external-handoff-release.mjs',
    target: 'tools/validate-external-handoff-release.mjs',
    audience: 'third_party',
  },
  {
    source: 'tools/validate-external-handoff-delivery.mjs',
    target: 'tools/validate-external-handoff-delivery.mjs',
    audience: 'third_party',
  },
  {
    source: 'tools/validate-external-handoff-all.mjs',
    target: 'tools/validate-external-handoff-all.mjs',
    audience: 'third_party',
  },
  {
    source: 'tools/external-third-party-readiness-report.mjs',
    target: 'tools/external-third-party-readiness-report.mjs',
    audience: 'v3_operator',
  },
];

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith('--')) {
      continue;
    }
    parsed[arg.slice(2)] = argv[index + 1];
    index += 1;
  }
  return parsed;
}

function repoRootFromModule() {
  const currentFile = fileURLToPath(import.meta.url);
  return path.resolve(path.dirname(currentFile), '..');
}

function timestampForPath(date = new Date()) {
  return date.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z');
}

function gitHead(repoRoot) {
  try {
    return execFileSync('git', ['-C', repoRoot, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim();
  } catch {
    return '';
  }
}

function sha256Hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function fileDigest(filePath) {
  const bytes = fs.readFileSync(filePath);
  return {
    bytes: bytes.length,
    sha256: sha256Hex(bytes),
  };
}

function ensureParentDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function copyFileWithDigest(repoRoot, packageRoot, fileSpec) {
  const sourcePath = path.join(repoRoot, fileSpec.source);
  const targetPath = path.join(packageRoot, fileSpec.target);
  const bytes = fs.readFileSync(sourcePath);
  ensureParentDir(targetPath);
  fs.writeFileSync(targetPath, bytes);
  if (fileSpec.target.endsWith('.sh') || fileSpec.target.endsWith('.mjs')) {
    fs.chmodSync(targetPath, 0o755);
  }
  return {
    path: fileSpec.target.replaceAll('\\', '/'),
    source: fileSpec.source.replaceAll('\\', '/'),
    audience: fileSpec.audience,
    bytes: bytes.length,
    sha256: sha256Hex(bytes),
  };
}

function writeJson(filePath, value) {
  ensureParentDir(filePath);
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function deliveryArtifact({ role, filePath, deliveryRoot, description }) {
  const digest = fileDigest(filePath);
  return {
    role,
    path: path.relative(deliveryRoot, filePath).replaceAll('\\', '/'),
    bytes: digest.bytes,
    sha256: digest.sha256,
    description,
  };
}

function writeTarOctal(header, value, offset, length) {
  const raw = Math.trunc(Number(value) || 0).toString(8);
  const text = raw.padStart(length - 1, '0').slice(-(length - 1));
  header.write(`${text}\0`, offset, length, 'ascii');
}

function writeTarChecksum(header, checksum) {
  const text = checksum.toString(8).padStart(6, '0').slice(-6);
  header.write(`${text}\0 `, 148, 8, 'ascii');
}

function tarHeader({ name, size, mode, mtime }) {
  const header = Buffer.alloc(512, 0);
  const normalizedName = name.replaceAll('\\', '/');
  if (Buffer.byteLength(normalizedName) > 100) {
    throw new Error(`tar entry path is too long: ${normalizedName}`);
  }
  header.write(normalizedName, 0, 100, 'utf8');
  writeTarOctal(header, mode, 100, 8);
  writeTarOctal(header, 0, 108, 8);
  writeTarOctal(header, 0, 116, 8);
  writeTarOctal(header, size, 124, 12);
  writeTarOctal(header, Math.floor(new Date(mtime).getTime() / 1000), 136, 12);
  header.fill(0x20, 148, 156);
  header.write('0', 156, 1, 'ascii');
  header.write('ustar\0', 257, 6, 'ascii');
  header.write('00', 263, 2, 'ascii');
  header.write('v3', 265, 32, 'ascii');
  header.write('v3', 297, 32, 'ascii');
  let checksum = 0;
  for (const byte of header) {
    checksum += byte;
  }
  writeTarChecksum(header, checksum);
  return header;
}

function tarPadding(size) {
  const remainder = size % 512;
  return remainder === 0 ? Buffer.alloc(0) : Buffer.alloc(512 - remainder, 0);
}

function createTarGzArchive({ packageRoot, archivePath, rootName, files, generatedAt }) {
  const chunks = [];
  for (const relativePath of files) {
    const absolutePath = path.join(packageRoot, relativePath);
    const bytes = fs.readFileSync(absolutePath);
    const mode = relativePath.endsWith('.sh') || relativePath.endsWith('.mjs') ? 0o755 : 0o644;
    chunks.push(tarHeader({
      name: `${rootName}/${relativePath.replaceAll('\\', '/')}`,
      size: bytes.length,
      mode,
      mtime: generatedAt,
    }));
    chunks.push(bytes);
    chunks.push(tarPadding(bytes.length));
  }
  chunks.push(Buffer.alloc(1024, 0));
  const archiveBytes = zlib.gzipSync(Buffer.concat(chunks), { level: 9, mtime: 0 });
  fs.writeFileSync(archivePath, archiveBytes);
  return {
    bytes: archiveBytes.length,
    sha256: sha256Hex(archiveBytes),
  };
}

function renderChineseReadme({ generatedAt, head }) {
  return `# V3 第三方沙箱交接包

生成时间：${generatedAt}
V3 提交：${head || 'unknown'}

## 这个包包含什么

- \`docs/third-party-integration-api.zh-CN.md\`：可发给第三方的中文接口说明。
- \`docs/third-party-integration-api.md\`：英文接口说明。
- \`handoff/third-party-handoff.sample.json\`：第三方沙箱交接清单样例。
- \`tools/validate-external-handoff.mjs\`：交接清单校验工具。
- \`tools/validate-external-handoff-package.mjs\`：交接包完整性校验工具。
- \`tools/validate-external-handoff-archive.mjs\`：交接归档包和 \`.sha256\` 校验工具。
- \`tools/validate-external-handoff-delivery.mjs\`：接收侧交付清单校验工具。
- \`tools/validate-external-handoff-release.mjs\`：目录、归档、摘要的一键 release 校验工具。
- \`tools/validate-external-handoff-all.mjs\`：一键聚合校验工具。
- \`sandbox/external-third-party-mock-gateway.mjs\`：第三方动作 endpoint 的本地 mock 示例。
- \`sandbox/run-external-third-party-gateway-smoke.sh\`：V3 部署目标使用的签名派发、结果回调和交接清单 smoke 入口。
- \`handoff-package-manifest.json\`：本包文件清单、SHA256 摘要和校验摘要。
- 包目录同级会生成 \`.tar.gz\` 归档、\`.sha256\` 校验文件、\`.release.json\` 校验报告、\`.release.md\` 人工摘要、\`.delivery-manifest.json\` 交付清单、\`.all.json\` 聚合校验证据和 \`.all.md\` 人工聚合摘要，用于发送和交付前校验。

## 第三方应先做什么

1. 阅读 \`docs/third-party-integration-api.zh-CN.md\`。
2. 复制 \`handoff/third-party-handoff.sample.json\`，按自己的测试环境填写。
3. 不要把真实 token、signing secret、password、private key 或 API key 写入清单。
4. 运行校验：

\`\`\`bash
npm run validate:handoff
npm run validate:package
npm run validate:archive
npm run validate:delivery
npm run validate:release
npm run validate:all
\`\`\`

5. 收到正式交付文件时，优先保留包目录、\`.tar.gz\`、\`.sha256\`、\`.release.json\`、\`.release.md\` 和 \`.delivery-manifest.json\` 在同一目录，再运行 \`validate:delivery\` 或 \`validate:all\` 核对交付清单。需要留档时可运行：\`npm run validate:all -- --out aggregate.json --markdown aggregate.md\`。
6. 将校验通过的清单、测试文档/权限样例、联调联系人和网络白名单信息交给 V3 项目组。

## V3 侧如何验收

V3 部署目标会运行 gateway smoke，验证签名派发、第三方结果回调、脱敏摘要和交接清单：

\`\`\`bash
EXTERNAL_THIRD_PARTY_HANDOFF_MANIFEST=/path/to/third-party-handoff.json \\
  bash sandbox/run-external-third-party-gateway-smoke.sh
\`\`\`

真实客户 HTTPS 沙箱可用后，V3 项目组会把同一套动作/回调契约切到客户 endpoint 进行联调。
`;
}

function renderEnglishReadme({ generatedAt, head }) {
  return `# V3 Third-Party Sandbox Handoff Package

Generated at: ${generatedAt}
V3 commit: ${head || 'unknown'}

This package contains third-party-facing API guides, a sandbox handoff manifest sample, validation tooling, and a mock gateway reference for V3 external action dispatch/result callback integration.

The builder also writes a \`.tar.gz\` archive, matching \`.sha256\` sidecar, \`.release.json\` validation report, \`.release.md\` summary, \`.delivery-manifest.json\` delivery manifest, \`.all.json\` aggregate validation evidence, and \`.all.md\` aggregate summary next to the package directory.

Recommended flow:

1. Read \`docs/third-party-integration-api.zh-CN.md\` or \`docs/third-party-integration-api.md\`.
2. Copy and fill \`handoff/third-party-handoff.sample.json\` for the customer sandbox.
3. Do not paste real tokens, signing secrets, passwords, private keys, or API keys into the manifest.
4. Run \`npm run validate:handoff\`.
5. Run \`npm run validate:package\`.
6. Run \`npm run validate:archive\` while the package directory, \`.tar.gz\` archive, and \`.sha256\` sidecar remain siblings.
7. Run \`npm run validate:delivery\` to verify the sibling delivery manifest against every expected delivery artifact.
8. Run \`npm run validate:release\` for the combined ready/not-ready report.
9. Run \`npm run validate:all\` when you want one JSON report covering every handoff gate. Add \`-- --out aggregate.json --markdown aggregate.md\` to keep evidence files.
10. Send the validated manifest, document/ACL fixtures, network allowlist details, and operations contacts to the V3 team.

The V3 operator smoke validates signed dispatch, result callback, redaction, and the handoff manifest before a live customer sandbox run.
`;
}

function packageJson() {
  return {
    private: true,
    name: 'v3-external-third-party-handoff-package',
    version: '0.1.0',
    type: 'module',
    scripts: {
      'validate:handoff': 'node tools/validate-external-handoff.mjs --manifest handoff/third-party-handoff.sample.json',
      'validate:package': 'node tools/validate-external-handoff-package.mjs --package .',
      'validate:archive': 'node tools/validate-external-handoff-archive.mjs',
      'validate:delivery': 'node tools/validate-external-handoff-delivery.mjs --package .',
      'validate:release': 'node tools/validate-external-handoff-release.mjs --package .',
      'validate:all': 'node tools/validate-external-handoff-all.mjs --package .',
      'start:mock-gateway': 'node sandbox/external-third-party-mock-gateway.mjs',
    },
  };
}

function buildDeliveryManifest({
  packageRoot,
  archivePath,
  archiveSha256Path,
  releaseReportPath,
  releaseMarkdownPath,
  releaseValidation,
  generatedAt,
  head,
}) {
  const deliveryRoot = path.dirname(packageRoot);
  const packageManifestPath = path.join(packageRoot, 'handoff-package-manifest.json');
  return {
    manifest_type: 'v3.external_third_party_handoff_delivery_manifest.v1',
    package_type: PACKAGE_TYPE,
    generated_at: generatedAt,
    repository_head: head || null,
    package_name: path.basename(packageRoot),
    package_root: path.basename(packageRoot),
    release_ready: releaseValidation.release_ready === true,
    delivery_root: '.',
    artifacts: [
      {
        role: 'package_directory',
        path: path.basename(packageRoot),
        type: 'directory',
        description: 'Expanded package directory for review or direct validation.',
      },
      deliveryArtifact({
        role: 'package_manifest',
        filePath: packageManifestPath,
        deliveryRoot,
        description: 'Package file manifest and per-file SHA256 checksums.',
      }),
      deliveryArtifact({
        role: 'archive',
        filePath: archivePath,
        deliveryRoot,
        description: 'Compressed handoff package for transfer.',
      }),
      deliveryArtifact({
        role: 'archive_sha256_sidecar',
        filePath: archiveSha256Path,
        deliveryRoot,
        description: 'SHA256 sidecar used to verify the compressed archive.',
      }),
      deliveryArtifact({
        role: 'release_json',
        filePath: releaseReportPath,
        deliveryRoot,
        description: 'Machine-readable package, archive, and sidecar release validation report.',
      }),
      deliveryArtifact({
        role: 'release_markdown',
        filePath: releaseMarkdownPath,
        deliveryRoot,
        description: 'Human-readable release validation summary.',
      }),
    ],
  };
}

function buildPackage({ repoRoot, outDir, basename, generatedAt = new Date().toISOString() }) {
  const head = gitHead(repoRoot);
  const packageRoot = path.resolve(outDir, basename || `external-third-party-handoff-package-${timestampForPath(new Date(generatedAt))}`);
  fs.mkdirSync(packageRoot, { recursive: true });

  const includedFiles = SOURCE_FILES.map((fileSpec) => copyFileWithDigest(repoRoot, packageRoot, fileSpec));
  const readmeCnPath = path.join(packageRoot, 'README.zh-CN.md');
  const readmeEnPath = path.join(packageRoot, 'README.md');
  const packageJsonPath = path.join(packageRoot, 'package.json');
  fs.writeFileSync(readmeCnPath, renderChineseReadme({ generatedAt, head }));
  fs.writeFileSync(readmeEnPath, renderEnglishReadme({ generatedAt, head }));
  writeJson(packageJsonPath, packageJson());

  for (const filePath of ['README.zh-CN.md', 'README.md', 'package.json']) {
    const bytes = fs.readFileSync(path.join(packageRoot, filePath));
    includedFiles.push({
      path: filePath,
      source: 'generated',
      audience: filePath === 'package.json' ? 'tooling' : 'third_party',
      bytes: bytes.length,
      sha256: sha256Hex(bytes),
    });
  }

  const sampleManifestPath = path.join(packageRoot, 'handoff/third-party-handoff.sample.json');
  const handoffValidation = validateExternalHandoffManifest(JSON.parse(fs.readFileSync(sampleManifestPath, 'utf8')));
  const packageManifest = {
    package_type: PACKAGE_TYPE,
    generated_at: generatedAt,
    repository_head: head || null,
    package_root: '.',
    handoff_validation: {
      ready_for_customer_sandbox: handoffValidation.ready_for_customer_sandbox === true,
      check_count: handoffValidation.checks.length,
      error_codes: handoffValidation.errors.map((error) => error.code),
      warning_codes: handoffValidation.warnings.map((warning) => warning.code),
    },
    included_files: includedFiles,
  };
  const packageManifestPath = path.join(packageRoot, 'handoff-package-manifest.json');
  writeJson(packageManifestPath, packageManifest);
  const archiveFileName = `${path.basename(packageRoot)}.tar.gz`;
  const archivePath = path.join(path.dirname(packageRoot), archiveFileName);
  const archiveFiles = [
    ...packageManifest.included_files.map((file) => file.path),
    'handoff-package-manifest.json',
  ];
  const archive = createTarGzArchive({
    packageRoot,
    archivePath,
    rootName: path.basename(packageRoot),
    files: archiveFiles,
    generatedAt,
  });
  const archiveSha256Path = `${archivePath}.sha256`;
  fs.writeFileSync(archiveSha256Path, `${archive.sha256}  ${archiveFileName}\n`);
  const releaseValidation = validateRelease({
    packageRootInput: packageRoot,
    archivePathInput: archivePath,
    sidecarPathInput: archiveSha256Path,
    deliveryManifestRequired: false,
  });
  const releaseReportPath = path.join(path.dirname(packageRoot), `${path.basename(packageRoot)}.release.json`);
  writeJson(releaseReportPath, releaseValidation);
  const releaseReportSha256 = sha256Hex(fs.readFileSync(releaseReportPath));
  const releaseMarkdownPath = path.join(path.dirname(packageRoot), `${path.basename(packageRoot)}.release.md`);
  fs.writeFileSync(releaseMarkdownPath, renderReleaseMarkdown(releaseValidation));
  const releaseMarkdownSha256 = sha256Hex(fs.readFileSync(releaseMarkdownPath));
  const deliveryManifestPath = path.join(path.dirname(packageRoot), `${path.basename(packageRoot)}.delivery-manifest.json`);
  writeJson(deliveryManifestPath, buildDeliveryManifest({
    packageRoot,
    archivePath,
    archiveSha256Path,
    releaseReportPath,
    releaseMarkdownPath,
    releaseValidation,
    generatedAt,
    head,
  }));
  const deliveryManifestSha256 = sha256Hex(fs.readFileSync(deliveryManifestPath));
  const allValidation = validateAll({
    packageRootInput: packageRoot,
    archivePathInput: archivePath,
    sidecarPathInput: archiveSha256Path,
  });
  const allReportPath = path.join(path.dirname(packageRoot), `${path.basename(packageRoot)}.all.json`);
  const allMarkdownPath = path.join(path.dirname(packageRoot), `${path.basename(packageRoot)}.all.md`);
  writeAllReportFiles(allValidation, {
    out: allReportPath,
    markdown: allMarkdownPath,
  });
  const allReportSha256 = sha256Hex(fs.readFileSync(allReportPath));
  const allMarkdownSha256 = sha256Hex(fs.readFileSync(allMarkdownPath));

  return {
    packageRoot,
    manifestPath: packageManifestPath,
    archivePath,
    archiveSha256Path,
    archiveSha256: archive.sha256,
    releaseReportPath,
    releaseReportSha256,
    releaseMarkdownPath,
    releaseMarkdownSha256,
    deliveryManifestPath,
    deliveryManifestSha256,
    allReportPath,
    allReportSha256,
    allMarkdownPath,
    allMarkdownSha256,
    allReady: allValidation.all_ready === true,
    releaseReady: releaseValidation.release_ready === true,
    ready: packageManifest.handoff_validation.ready_for_customer_sandbox,
    fileCount: packageManifest.included_files.length,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const repoRoot = path.resolve(args.repoRoot || repoRootFromModule());
  const outDir = path.resolve(args.outDir || path.join(repoRoot, 'target', 'external-third-party-handoff'));
  const result = buildPackage({
    repoRoot,
    outDir,
    basename: args.basename,
    generatedAt: args.generatedAt || new Date().toISOString(),
  });
  console.log(JSON.stringify(result, null, 2));
  if (!result.releaseReady || !result.allReady) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { PACKAGE_TYPE, SOURCE_FILES, buildPackage };
