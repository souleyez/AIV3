import assert from 'node:assert/strict';
import test from 'node:test';
import {
  buildStaticPageExportPackage,
  buildStaticPageExportZipBlob,
  buildStaticPageStandaloneHtml,
  dataQualityModulesFromManifest,
  dataQualitySummaryFromManifest,
  hasDataQualitySummary,
  staticPageExportFilename,
  staticPageHtmlFilename,
  staticPageZipFilename,
} from './static-page-export-package.js';

function testDraft(overrides = {}) {
  return {
    id: 'draft-local:unsafe id',
    backendDraftId: 'draft_backend_1',
    objective: '客户经营静态页',
    status: 'rendered',
    modules: [
      { id: 'hero', title: '核心结论' },
      { id: 'trend', title: '趋势图' },
    ],
    renderSpec: {
      componentModel: 'dom-text-svg-chart',
    },
    imageJob: {
      id: 'image-job-1',
      status: 'confirmed',
      queuePosition: 1,
      queueMessage: '已跳过排队',
    },
    previewImage: {
      assetKey: 'static-page-previews/image-job-1.json',
    },
    previewContract: {
      status: 'confirmed',
      assetKey: 'static-page-previews/image-job-1.json',
      draftFingerprint: 'design-abc123',
      confirmedAt: '2026-05-08T08:00:00Z',
    },
    finalPage: {
      status: 'rendered',
      renderOutputId: 'render-output-1',
      imageJobId: 'image-job-1',
      html: '<main>最终静态页</main>',
      assetManifest: {
        renderer: 'static-page-renderer-v1',
        data_snapshot: {
          source: 'selected_scope',
          moduleBindings: [{ moduleId: 'trend', dataQuality: 'complete' }],
        },
        chart_runtime: {
          deterministicModules: 1,
          echartsRequestedModules: 1,
          dataQualitySummary: {
            confirmedModules: 1,
            partialModules: 0,
            missingModules: 0,
            attentionModules: 0,
          },
          modules: [{
            moduleId: 'trend',
            title: '趋势图',
            visualizationType: 'line-chart',
            chartRuntime: 'echarts',
            dataQuality: 'complete',
            dataQualityStatus: 'confirmed',
            dataQualityReason: 'module_has_renderable_data',
            recommendedAction: '可直接交付；如客户要求精确口径，可继续补充字段说明。',
            sampleDataRows: 1,
            fallback: true,
            fallbackRuntime: 'deterministic-svg-html',
          }],
        },
        export_package: {
          kind: 'static-page-export-package',
          version: 1,
          files: [
            { path: 'index.html', role: 'rendered_static_page', mime: 'text/html' },
            { path: 'asset-manifest.json', role: 'renderer_manifest', mime: 'application/json' },
            { path: 'data-snapshot.json', role: 'render_data_snapshot', mime: 'application/json' },
            { path: 'modules.json', role: 'editable_module_plan', mime: 'application/json' },
          ],
          runtime_requirements: [{
            name: 'Apache ECharts',
            package: 'echarts',
            license: 'Apache-2.0',
            required: false,
          }],
        },
      },
    },
    ...overrides,
  };
}

test('static page export package includes html manifest data modules and readme', () => {
  const draft = testDraft();
  const payload = {
    dataSnapshot: {
      source: 'payload_snapshot',
      moduleBindings: [{ moduleId: 'trend', sampleData: [{ label: '一月', value: 12 }] }],
    },
    modules: draft.modules,
    renderSpec: draft.renderSpec,
  };

  const artifact = buildStaticPageExportPackage(draft, payload, draft.finalPage.html);
  const files = new Map(artifact.files.map((file) => [file.path, file]));

  assert.equal(artifact.kind, 'static-page-export-package');
  assert.equal(artifact.renderOutputId, 'render-output-1');
  assert.match(files.get('export-package.json').content, /render-output-1/);
  assert.equal(files.get('index.html').content, '<main>最终静态页</main>');
  assert.match(files.get('asset-manifest.json').content, /static-page-renderer-v1/);
  assert.match(files.get('data-snapshot.json').content, /payload_snapshot/);
  assert.match(files.get('visual-bridge.json').content, /static-page-visual-bridge/);
  assert.match(files.get('visual-bridge.json').content, /design-abc123/);
  assert.match(files.get('data-quality-report.json').content, /static-page-data-quality-report/);
  assert.match(files.get('data-quality-report.json').content, /module_has_renderable_data/);
  assert.match(files.get('modules.json').content, /核心结论/);
  assert.match(files.get('render-spec.json').content, /dom-text-svg-chart/);
  assert.match(files.get('runtime-requirements.json').content, /Apache-2.0/);
  assert.match(files.get('README.md').content, /ECharts 1/);
  assert.match(files.get('README.md').content, /数据质量：已确认 1 \/ 部分 0 \/ 缺失 0/);
  assert.match(files.get('README.md').content, /模块级数据质量报告：data-quality-report\.json/);
  assert.match(files.get('README.md').content, /视觉合同：confirmed/);
  assert.match(files.get('README.md').content, /Apache ECharts（可选）/);
  assert.match(files.get('export-package.json').content, /"confirmedModules": 1/);
  assert.match(files.get('export-package.json').content, /"visual_bridge"/);
  assert.match(files.get('export-package.json').content, /"data_quality_modules"/);
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'README.md'));
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'export-package.json'));
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'data-quality-report.json'));
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'visual-bridge.json'));
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'render-spec.json'));
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'runtime-requirements.json'));
  assert.equal(artifact.packageManifest.runtime_requirements[0].required, false);
  assert.deepEqual(artifact.warnings, []);
});

test('static page export package can be written as a real zip file', async () => {
  const draft = testDraft();
  const artifact = buildStaticPageExportPackage(draft, {
    dataSnapshot: {
      moduleBindings: [{ moduleId: 'trend', sampleData: [{ label: '一月', value: 12 }] }],
    },
    modules: draft.modules,
    renderSpec: draft.renderSpec,
  }, draft.finalPage.html);

  const blob = buildStaticPageExportZipBlob(artifact, {
    now: new Date('2026-05-08T08:00:00Z'),
  });
  const bytes = new Uint8Array(await blob.arrayBuffer());
  const text = new TextDecoder('utf-8').decode(bytes);

  assert.equal(blob.type, 'application/zip');
  assert.equal(bytes[0], 0x50);
  assert.equal(bytes[1], 0x4b);
  assert.match(text, /index\.html/);
  assert.match(text, /asset-manifest\.json/);
  assert.match(text, /visual-bridge\.json/);
  assert.match(text, /data-quality-report\.json/);
  assert.match(text, /runtime-requirements\.json/);
  assert.match(text, /export-package\.json/);
  assert.match(text, /最终静态页/);
});

test('static page export package records missing backend html as a warning', () => {
  const draft = testDraft({
    finalPage: {
      ...testDraft().finalPage,
      html: '',
    },
  });

  const artifact = buildStaticPageExportPackage(draft, {}, '');
  const html = artifact.files.find((file) => file.path === 'index.html')?.content || '';
  const readme = artifact.files.find((file) => file.path === 'README.md')?.content || '';

  assert.equal(artifact.warnings.length, 1);
  assert.match(html, /not available yet/);
  assert.match(readme, /后端 HTML：未返回/);
});

test('static page export helpers sanitize filenames and prefer backend html', () => {
  const draft = testDraft({ backendDraftId: 'draft:id/with spaces' });

  assert.equal(staticPageExportFilename(draft), 'static-page-draft-id-with-spaces-package.json');
  assert.equal(staticPageZipFilename(draft), 'static-page-draft-id-with-spaces-package.zip');
  assert.equal(staticPageHtmlFilename(draft), 'static-page-draft-id-with-spaces-index.html');
  assert.equal(buildStaticPageStandaloneHtml(draft, '<main>fresh</main>'), '<main>fresh</main>');
});

test('static page export helpers expose data quality summary for UI surfaces', () => {
  const summary = dataQualitySummaryFromManifest(testDraft().finalPage.assetManifest);

  assert.equal(hasDataQualitySummary(summary), true);
  assert.deepEqual(summary, {
    confirmedModules: 1,
    partialModules: 0,
    missingModules: 0,
    attentionModules: 0,
  });
  assert.equal(dataQualityModulesFromManifest(testDraft().finalPage.assetManifest)[0].moduleId, 'trend');
  assert.equal(hasDataQualitySummary(dataQualitySummaryFromManifest({})), false);
});
