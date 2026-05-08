function finalPageManifest(draft) {
  return draft?.finalPage?.assetManifest && typeof draft.finalPage.assetManifest === 'object'
    ? draft.finalPage.assetManifest
    : {};
}

function safeJson(value) {
  return JSON.stringify(value ?? {}, null, 2);
}

function fallbackPackageManifest(draft) {
  return {
    kind: 'static-page-export-package',
    version: 1,
    status: draft?.finalPage?.status || draft?.status || 'unknown',
    files: [
      { path: 'index.html', role: 'rendered_static_page', mime: 'text/html' },
      { path: 'asset-manifest.json', role: 'renderer_manifest', mime: 'application/json' },
      { path: 'data-snapshot.json', role: 'render_data_snapshot', mime: 'application/json' },
      { path: 'modules.json', role: 'editable_module_plan', mime: 'application/json' },
      { path: 'README.md', role: 'human_handoff_note', mime: 'text/markdown' },
    ],
  };
}

function normalizePackageManifest(draft, manifest) {
  const packageManifest = manifest.export_package && typeof manifest.export_package === 'object'
    ? manifest.export_package
    : fallbackPackageManifest(draft);
  const files = Array.isArray(packageManifest.files) ? [...packageManifest.files] : [];
  const requiredFiles = [
    { path: 'README.md', role: 'human_handoff_note', mime: 'text/markdown' },
    { path: 'render-spec.json', role: 'render_contract', mime: 'application/json' },
  ];
  requiredFiles.forEach((requiredFile) => {
    if (!files.some((file) => file?.path === requiredFile.path)) {
      files.push(requiredFile);
    }
  });
  return {
    ...packageManifest,
    files,
  };
}

function buildReadme({ draft, manifest, backendHtml, warnings }) {
  const chartRuntime = manifest.chart_runtime || {};
  const lines = [
    `# ${draft?.objective || draft?.title || '静态页交付包'}`,
    '',
    '这个交付包由 AI Data Platform V3 生成，包含最终 HTML、渲染 manifest、数据快照和可编辑模块规划。',
    '',
    `- 草稿 ID：${draft?.backendDraftId || draft?.id || 'unknown'}`,
    `- 渲染状态：${draft?.finalPage?.status || draft?.status || 'unknown'}`,
    `- 图表运行时：基础 ${chartRuntime.deterministicModules ?? 0} / ECharts ${chartRuntime.echartsRequestedModules ?? 0}`,
    `- 后端 HTML：${backendHtml ? '已包含' : '未返回，需重新刷新或等待 worker 写回'}`,
  ];
  if (warnings.length) {
    lines.push('', '## 注意');
    warnings.forEach((warning) => lines.push(`- ${warning}`));
  }
  return `${lines.join('\n')}\n`;
}

function contentForPath(path, { draft, payload, manifest, backendHtml, warnings }) {
  if (path === 'index.html') {
    return backendHtml || '<!-- static page html is not available yet; refresh after worker completion -->';
  }
  if (path === 'asset-manifest.json') {
    return safeJson(manifest);
  }
  if (path === 'data-snapshot.json') {
    return safeJson(payload?.dataSnapshot || manifest.data_snapshot || {});
  }
  if (path === 'modules.json') {
    return safeJson(payload?.modules || draft?.modules || []);
  }
  if (path === 'render-spec.json') {
    return safeJson(payload?.renderSpec || draft?.renderSpec || manifest.render_spec || {});
  }
  if (path === 'README.md') {
    return buildReadme({ draft, manifest, backendHtml, warnings });
  }
  return '';
}

function normalizeFiles(packageManifest, context) {
  const knownPaths = new Set();
  const files = [];
  const sourceFiles = Array.isArray(packageManifest.files) ? packageManifest.files : [];
  sourceFiles.forEach((file) => {
    const path = typeof file?.path === 'string' ? file.path.trim() : '';
    if (!path || knownPaths.has(path)) return;
    knownPaths.add(path);
    files.push({
      path,
      role: file.role || 'supporting_file',
      mime: file.mime || 'text/plain',
      content: contentForPath(path, context),
    });
  });
  return files;
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
