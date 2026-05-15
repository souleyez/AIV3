#!/usr/bin/env node
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateDeliveryManifest } from './validate-external-handoff-release.mjs';

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

function validateDelivery({
  packageRootInput = '.',
  archivePathInput = '',
  sidecarPathInput = '',
  releaseReportPathInput = '',
  releaseMarkdownPathInput = '',
  deliveryManifestInput = '',
} = {}) {
  const packageRoot = path.resolve(packageRootInput || '.');
  const packageName = path.basename(packageRoot);
  const deliveryRoot = path.dirname(packageRoot);
  const archivePath = path.resolve(archivePathInput || path.join(deliveryRoot, `${packageName}.tar.gz`));
  const sidecarPath = path.resolve(sidecarPathInput || `${archivePath}.sha256`);
  const releaseReportPath = path.resolve(releaseReportPathInput || path.join(deliveryRoot, `${packageName}.release.json`));
  const releaseMarkdownPath = path.resolve(releaseMarkdownPathInput || path.join(deliveryRoot, `${packageName}.release.md`));

  const delivery = validateDeliveryManifest({
    packageRoot,
    archivePath,
    sidecarPath,
    releaseReportPath,
    releaseMarkdownPath,
    deliveryManifestPath: deliveryManifestInput,
    required: true,
  });

  return {
    report_type: 'external_third_party_handoff_delivery_validation',
    delivery_manifest_ready: delivery.delivery_manifest_ready === true,
    package_root: packageRoot,
    archive_path: archivePath,
    sidecar_path: sidecarPath,
    release_report_path: releaseReportPath,
    release_markdown_path: releaseMarkdownPath,
    ...delivery,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = validateDelivery({
    packageRootInput: args.package || '.',
    archivePathInput: args.archive || '',
    sidecarPathInput: args.sha256 || '',
    releaseReportPathInput: args.release || '',
    releaseMarkdownPathInput: args.releaseMarkdown || '',
    deliveryManifestInput: args.manifest || '',
  });
  console.log(JSON.stringify(result, null, 2));
  if (!result.delivery_manifest_ready) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { validateDelivery };
