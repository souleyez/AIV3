function finalPageManifest(draft) {
  return draft?.finalPage?.assetManifest && typeof draft.finalPage.assetManifest === 'object'
    ? draft.finalPage.assetManifest
    : {};
}

function safeJson(value) {
  return JSON.stringify(value ?? {}, null, 2);
}

function fallbackRuntimeRequirements() {
  return [{
    name: 'Apache ECharts',
    package: 'echarts',
    license: 'Apache-2.0',
    required: false,
    role: 'optional_advanced_chart_hydration',
    note: 'index.html 保留 deterministic DOM/SVG 图表回退，不会主动注入远程脚本；受信任宿主可提供 ECharts 来增强安全 JSON 图表配置。',
  }];
}

function fallbackBrowserDeliveryContract() {
  return {
    entry: 'index.html',
    layout: 'responsive_static_html',
    remote_scripts_allowed: false,
    deterministic_chart_fallback: true,
    optional_echarts_hydration: 'safe_json_option_islands',
    mobile_viewport: 'responsive_no_horizontal_overflow_expected',
  };
}

function numberOrZero(value) {
  return Number.isFinite(Number(value)) ? Number(value) : 0;
}

export function dataQualitySummaryFromManifest(manifest) {
  const summary = manifest.export_package?.debug?.data_quality_summary
    || manifest.chart_runtime?.dataQualitySummary
    || manifest.data_quality_summary
    || {};
  return {
    confirmedModules: numberOrZero(summary.confirmedModules),
    partialModules: numberOrZero(summary.partialModules),
    missingModules: numberOrZero(summary.missingModules),
    attentionModules: numberOrZero(summary.attentionModules),
  };
}

export function hasDataQualitySummary(summary) {
  return Object.values(summary).some((value) => value > 0);
}

export function dataQualityModulesFromManifest(manifest) {
  const modules = manifest.export_package?.debug?.data_quality_modules
    || manifest.chart_runtime?.modules
    || manifest.data_quality_modules
    || [];
  return Array.isArray(modules) ? modules : [];
}

export function browserDeliveryContractFromManifest(manifest) {
  const contract = manifest.export_package?.browser_delivery_contract
    || manifest.browser_delivery_contract
    || {};
  return {
    ...fallbackBrowserDeliveryContract(),
    ...(contract && typeof contract === 'object' ? contract : {}),
  };
}

function fallbackPackageManifest(draft) {
  return {
    kind: 'static-page-export-package',
    version: 1,
    status: draft?.finalPage?.status || draft?.status || 'unknown',
    files: [
      { path: 'export-package.json', role: 'export_package_manifest', mime: 'application/json' },
      { path: 'index.html', role: 'rendered_static_page', mime: 'text/html' },
      { path: 'asset-manifest.json', role: 'renderer_manifest', mime: 'application/json' },
      { path: 'data-snapshot.json', role: 'render_data_snapshot', mime: 'application/json' },
      { path: 'visual-bridge.json', role: 'confirmed_visual_contract_bridge', mime: 'application/json' },
      { path: 'data-quality-report.json', role: 'module_data_quality_report', mime: 'application/json' },
      { path: 'modules.json', role: 'editable_module_plan', mime: 'application/json' },
      { path: 'runtime-requirements.json', role: 'optional_runtime_requirements', mime: 'application/json' },
      { path: 'README.md', role: 'human_handoff_note', mime: 'text/markdown' },
    ],
    runtime_requirements: fallbackRuntimeRequirements(),
    browser_delivery_contract: fallbackBrowserDeliveryContract(),
  };
}

function normalizePackageManifest(draft, manifest) {
  const packageManifest = manifest.export_package && typeof manifest.export_package === 'object'
    ? manifest.export_package
    : fallbackPackageManifest(draft);
  const files = Array.isArray(packageManifest.files) ? [...packageManifest.files] : [];
  const requiredFiles = [
    { path: 'export-package.json', role: 'export_package_manifest', mime: 'application/json' },
    { path: 'README.md', role: 'human_handoff_note', mime: 'text/markdown' },
    { path: 'data-quality-report.json', role: 'module_data_quality_report', mime: 'application/json' },
    { path: 'visual-bridge.json', role: 'confirmed_visual_contract_bridge', mime: 'application/json' },
    { path: 'render-spec.json', role: 'render_contract', mime: 'application/json' },
    { path: 'runtime-requirements.json', role: 'optional_runtime_requirements', mime: 'application/json' },
  ];
  requiredFiles.forEach((requiredFile) => {
    if (!files.some((file) => file?.path === requiredFile.path)) {
      files.push(requiredFile);
    }
  });
  return {
    ...packageManifest,
    runtime_requirements: Array.isArray(packageManifest.runtime_requirements)
      ? packageManifest.runtime_requirements
      : Array.isArray(manifest.runtime_requirements)
        ? manifest.runtime_requirements
        : fallbackRuntimeRequirements(),
    browser_delivery_contract: browserDeliveryContractFromManifest(manifest),
    files,
  };
}

function buildReadme({ draft, manifest, backendHtml, warnings }) {
  const chartRuntime = manifest.chart_runtime || {};
  const dataQualitySummary = dataQualitySummaryFromManifest(manifest);
  const dataQualityModules = dataQualityModulesFromManifest(manifest);
  const visualBridge = visualBridgeFromContext(draft, manifest);
  const browserDeliveryContract = browserDeliveryContractFromManifest(manifest);
  const runtimeRequirements = Array.isArray(manifest.export_package?.runtime_requirements)
    ? manifest.export_package.runtime_requirements
    : Array.isArray(manifest.runtime_requirements)
      ? manifest.runtime_requirements
      : fallbackRuntimeRequirements();
  const lines = [
    `# ${draft?.objective || draft?.title || '静态页交付包'}`,
    '',
    '这个交付包由 AI Data Platform V3 生成，包含最终 HTML、渲染 manifest、数据快照和可编辑模块规划。',
    '',
    `- 草稿 ID：${draft?.backendDraftId || draft?.id || 'unknown'}`,
    `- 渲染状态：${draft?.finalPage?.status || draft?.status || 'unknown'}`,
    `- 图表运行时：基础 ${chartRuntime.deterministicModules ?? 0} / ECharts ${chartRuntime.echartsRequestedModules ?? 0}`,
    `- 后端 HTML：${backendHtml ? '已包含' : '未返回，需重新刷新或等待 worker 写回'}`,
    `- 视觉合同：${visualBridge.status || 'unknown'} / ${visualBridge.previewAssetKey || 'no-preview'}（详见 visual-bridge.json）`,
  ];
  if (browserDeliveryContract.entry) {
    lines.push(
      `- 浏览器交付：${browserDeliveryContract.entry} 可直接打开；无远程脚本；图表保留 deterministic DOM/SVG 回退，ECharts 仅作为可选安全 JSON 增强。`,
    );
  }
  if (hasDataQualitySummary(dataQualitySummary)) {
    lines.push(
      `- 数据质量：已确认 ${dataQualitySummary.confirmedModules} / 部分 ${dataQualitySummary.partialModules} / 缺失 ${dataQualitySummary.missingModules}`,
    );
  }
  if (dataQualityModules.length) {
    lines.push(`- 模块级数据质量报告：data-quality-report.json（${dataQualityModules.length} 个模块）`);
  }
  if (runtimeRequirements.length) {
    lines.push(
      `- 可选运行时：${runtimeRequirements.map((item) => `${item.name || item.package || 'runtime'}${item.required ? '' : '（可选）'}`).join('、')}`,
    );
  }
  if (warnings.length) {
    lines.push('', '## 注意');
    warnings.forEach((warning) => lines.push(`- ${warning}`));
  }
  return `${lines.join('\n')}\n`;
}

function contentForPath(path, { draft, payload, manifest, backendHtml, warnings, packageManifest }) {
  if (path === 'export-package.json') {
    const visualBridge = visualBridgeFromContext(draft, manifest);
    return safeJson({
      kind: 'static-page-export-package',
      version: 1,
      draftId: draft?.id || null,
      backendDraftId: draft?.backendDraftId || null,
      renderOutputId: draft?.finalPage?.renderOutputId || null,
      imageJobId: draft?.finalPage?.imageJobId || draft?.imageJob?.id || null,
      visual_bridge: visualBridge,
      chart_runtime: manifest.chart_runtime || null,
      data_quality_summary: dataQualitySummaryFromManifest(manifest),
      data_quality_modules: dataQualityModulesFromManifest(manifest),
      runtime_requirements: contextRuntimeRequirements({ manifest, packageManifest }),
      browser_delivery_contract: browserDeliveryContractFromManifest(manifest),
      files: Array.isArray(packageManifest?.files) ? packageManifest.files : fallbackPackageManifest(draft).files,
    });
  }
  if (path === 'index.html') {
    return backendHtml || '<!-- static page html is not available yet; refresh after worker completion -->';
  }
  if (path === 'asset-manifest.json') {
    return safeJson(manifest);
  }
  if (path === 'data-snapshot.json') {
    return safeJson(payload?.dataSnapshot || manifest.data_snapshot || {});
  }
  if (path === 'visual-bridge.json') {
    return safeJson(visualBridgeFromContext(draft, manifest));
  }
  if (path === 'data-quality-report.json') {
    return safeJson({
      kind: 'static-page-data-quality-report',
      version: 1,
      summary: dataQualitySummaryFromManifest(manifest),
      modules: dataQualityModulesFromManifest(manifest),
    });
  }
  if (path === 'modules.json') {
    return safeJson(payload?.modules || draft?.modules || []);
  }
  if (path === 'render-spec.json') {
    return safeJson(payload?.renderSpec || draft?.renderSpec || manifest.render_spec || {});
  }
  if (path === 'runtime-requirements.json') {
    return safeJson(
      contextRuntimeRequirements({ manifest, packageManifest }),
    );
  }
  if (path === 'README.md') {
    return buildReadme({ draft, manifest, backendHtml, warnings });
  }
  return '';
}

function visualBridgeFromContext(draft = {}, manifest = {}) {
  const previewContract = draft?.previewContract || manifest.preview_contract || manifest.previewContract || {};
  const previewImage = draft?.previewImage || {};
  const imageJob = draft?.imageJob || {};
  return {
    kind: 'static-page-visual-bridge',
    version: 1,
    providerLane: 'gpt-image-2-cloudflare-queue',
    role: 'effect_preview_reference_only',
    rule: '效果图只锁定视觉方向和确认指纹；最终 HTML 由 Draft JSON、DataSnapshot、VisualSpec 和 renderer 生成。',
    status: previewContract.status || imageJob.status || manifest.status || 'unknown',
    imageJobStatus: imageJob.status || 'unknown',
    imageJobId: draft?.finalPage?.imageJobId || imageJob.id || manifest.image_job_id || previewContract.imageJobId || '',
    queuePosition: imageJob.queuePosition ?? null,
    queueMessage: imageJob.queueMessage || '',
    previewAssetKey: previewImage.assetKey || previewContract.assetKey || manifest.preview_asset_key || '',
    previousAssetKey: previewContract.previousAssetKey || '',
    draftFingerprint: previewContract.draftFingerprint || manifest.preview_contract?.draftFingerprint || '',
    confirmedAt: previewContract.confirmedAt || '',
    staleReason: previewContract.staleReason || '',
    styleDirection: draft?.styleDirection || manifest.style_direction || '',
    renderModel: draft?.renderSpec?.componentModel || manifest.render_spec?.componentModel || manifest.render_spec?.component_model || '',
    finalRenderStatus: draft?.finalPage?.status || manifest.status || 'unknown',
    finalHtmlSource: 'static-page-renderer',
  };
}

function contextRuntimeRequirements({ manifest, packageManifest }) {
  if (Array.isArray(packageManifest?.runtime_requirements)) {
    return packageManifest.runtime_requirements;
  }
  if (Array.isArray(manifest.export_package?.runtime_requirements)) {
    return manifest.export_package.runtime_requirements;
  }
  if (Array.isArray(manifest.runtime_requirements)) {
    return manifest.runtime_requirements;
  }
  return fallbackRuntimeRequirements();
}

function normalizeFiles(packageManifest, context) {
  const knownPaths = new Set();
  const files = [];
  const sourceFiles = Array.isArray(packageManifest.files) ? packageManifest.files : [];
  sourceFiles.forEach((file) => {
    const path = safePackagePath(file?.path);
    if (!path || knownPaths.has(path)) return;
    knownPaths.add(path);
    files.push({
      path,
      role: file.role || 'supporting_file',
      mime: file.mime || 'text/plain',
      content: contentForPath(path, { ...context, packageManifest }),
    });
  });
  return files;
}

function safePackagePath(path) {
  const normalized = String(path || '')
    .replace(/\\/g, '/')
    .trim()
    .replace(/^\/+/, '');
  const segments = normalized.split('/').filter(Boolean);
  if (!segments.length || segments.some((segment) => segment === '.' || segment === '..')) {
    return '';
  }
  return segments.join('/');
}

function staticPageSafeId(draft) {
  const rawId = String(draft?.backendDraftId || draft?.id || 'draft');
  return rawId
    .replace(/[^a-zA-Z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80) || 'draft';
}

export function staticPageExportFilename(draft, extension = 'json') {
  const safeId = staticPageSafeId(draft);
  const safeExtension = String(extension || 'json').replace(/[^a-zA-Z0-9]+/g, '') || 'json';
  return `static-page-${safeId}-package.${safeExtension}`;
}

export function staticPageHtmlFilename(draft) {
  return `static-page-${staticPageSafeId(draft)}-index.html`;
}

export function staticPageHtmlDownloadHref(url) {
  const value = String(url || '').trim();
  if (!value) return '';
  if (value.startsWith('/v1/')) {
    return `/api/v3/${value.slice('/v1/'.length)}`;
  }
  return value;
}

export function staticPageZipFilename(draft) {
  return staticPageExportFilename(draft, 'zip');
}

export function buildStaticPageStandaloneHtml(draft, backendHtml) {
  return backendHtml || draft?.finalPage?.html || '';
}

export function buildStaticPageExportPackage(draft, payload = {}, backendHtml = '') {
  const manifest = finalPageManifest(draft);
  const packageManifest = normalizePackageManifest(draft, manifest);
  const html = buildStaticPageStandaloneHtml(draft, backendHtml);
  const warnings = [];
  if (!html) {
    warnings.push('后端 HTML 尚未写回，index.html 仅为占位文件。请刷新状态或等待后台 worker 完成后重新下载。');
  }
  const files = normalizeFiles(packageManifest, {
    draft,
    payload,
    manifest,
    backendHtml: html,
    warnings,
  });
  return {
    kind: 'static-page-export-package',
    version: 1,
    draftId: draft?.id || null,
    backendDraftId: draft?.backendDraftId || null,
    renderOutputId: draft?.finalPage?.renderOutputId || null,
    imageJobId: draft?.finalPage?.imageJobId || draft?.imageJob?.id || null,
    createdAt: new Date().toISOString(),
    warnings,
    packageManifest,
    files,
    assets: packageManifest.assets || [],
  };
}

const CRC32_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[index] = value >>> 0;
  }
  return table;
})();

function crc32(bytes) {
  let crc = 0xffffffff;
  bytes.forEach((byte) => {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  });
  return (crc ^ 0xffffffff) >>> 0;
}

function littleEndian16(value) {
  const bytes = new Uint8Array(2);
  new DataView(bytes.buffer).setUint16(0, value & 0xffff, true);
  return bytes;
}

function littleEndian32(value) {
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setUint32(0, value >>> 0, true);
  return bytes;
}

function concatBytes(chunks) {
  const size = chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
  const output = new Uint8Array(size);
  let offset = 0;
  chunks.forEach((chunk) => {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  });
  return output;
}

function dosDateTime(now) {
  const source = now instanceof Date && !Number.isNaN(now.getTime()) ? now : new Date();
  const year = Math.max(1980, Math.min(2107, source.getFullYear()));
  const month = Math.max(1, Math.min(12, source.getMonth() + 1));
  const day = Math.max(1, Math.min(31, source.getDate()));
  const dosTime = (source.getHours() << 11) | (source.getMinutes() << 5) | Math.floor(source.getSeconds() / 2);
  const dosDate = ((year - 1980) << 9) | (month << 5) | day;
  return { dosDate, dosTime };
}

function localFileHeader({ nameBytes, contentBytes, crc, dosDate, dosTime }) {
  return concatBytes([
    littleEndian32(0x04034b50),
    littleEndian16(20),
    littleEndian16(0x0800),
    littleEndian16(0),
    littleEndian16(dosTime),
    littleEndian16(dosDate),
    littleEndian32(crc),
    littleEndian32(contentBytes.byteLength),
    littleEndian32(contentBytes.byteLength),
    littleEndian16(nameBytes.byteLength),
    littleEndian16(0),
    nameBytes,
  ]);
}

function centralDirectoryHeader({ nameBytes, contentBytes, crc, dosDate, dosTime, localHeaderOffset }) {
  return concatBytes([
    littleEndian32(0x02014b50),
    littleEndian16(20),
    littleEndian16(20),
    littleEndian16(0x0800),
    littleEndian16(0),
    littleEndian16(dosTime),
    littleEndian16(dosDate),
    littleEndian32(crc),
    littleEndian32(contentBytes.byteLength),
    littleEndian32(contentBytes.byteLength),
    littleEndian16(nameBytes.byteLength),
    littleEndian16(0),
    littleEndian16(0),
    littleEndian16(0),
    littleEndian16(0),
    littleEndian32(0),
    littleEndian32(localHeaderOffset),
    nameBytes,
  ]);
}

function endOfCentralDirectory({ entryCount, centralDirectorySize, centralDirectoryOffset }) {
  return concatBytes([
    littleEndian32(0x06054b50),
    littleEndian16(0),
    littleEndian16(0),
    littleEndian16(entryCount),
    littleEndian16(entryCount),
    littleEndian32(centralDirectorySize),
    littleEndian32(centralDirectoryOffset),
    littleEndian16(0),
  ]);
}

export function buildStaticPageExportZipBlob(artifact, options = {}) {
  const encoder = new TextEncoder();
  const { dosDate, dosTime } = dosDateTime(options.now);
  const entries = (Array.isArray(artifact?.files) ? artifact.files : [])
    .map((file) => ({
      path: safePackagePath(file?.path),
      content: file?.content instanceof Uint8Array ? file.content : encoder.encode(String(file?.content ?? '')),
    }))
    .filter((file) => file.path);

  const localChunks = [];
  const centralChunks = [];
  let offset = 0;

  entries.forEach((entry) => {
    const nameBytes = encoder.encode(entry.path);
    const contentBytes = entry.content;
    const contentCrc = crc32(contentBytes);
    const localHeader = localFileHeader({
      nameBytes,
      contentBytes,
      crc: contentCrc,
      dosDate,
      dosTime,
    });
    localChunks.push(localHeader, contentBytes);
    centralChunks.push(centralDirectoryHeader({
      nameBytes,
      contentBytes,
      crc: contentCrc,
      dosDate,
      dosTime,
      localHeaderOffset: offset,
    }));
    offset += localHeader.byteLength + contentBytes.byteLength;
  });

  const centralDirectoryOffset = offset;
  const centralDirectory = concatBytes(centralChunks);
  const zipBytes = concatBytes([
    ...localChunks,
    centralDirectory,
    endOfCentralDirectory({
      entryCount: entries.length,
      centralDirectorySize: centralDirectory.byteLength,
      centralDirectoryOffset,
    }),
  ]);

  return new Blob([zipBytes], {
    type: 'application/zip',
  });
}

export function downloadBlobArtifact({ blob, filename }) {
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    return false;
  }
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
  return true;
}

export function downloadStaticPageExportZip(draft, payload = {}, backendHtml = '') {
  const artifact = buildStaticPageExportPackage(draft, payload, backendHtml);
  return downloadBlobArtifact({
    blob: buildStaticPageExportZipBlob(artifact),
    filename: staticPageZipFilename(draft),
  });
}

export function downloadTextArtifact({ content, filename, mime }) {
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    return false;
  }
  const blob = new Blob([content], {
    type: mime || 'text/plain;charset=utf-8',
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
  return true;
}

export function downloadUrlArtifact({ href }) {
  const url = staticPageHtmlDownloadHref(href);
  if (!url || typeof window === 'undefined' || typeof document === 'undefined') {
    return false;
  }
  const link = document.createElement('a');
  link.href = url;
  document.body.appendChild(link);
  link.click();
  link.remove();
  return true;
}
