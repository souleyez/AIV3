import assert from 'node:assert/strict';
import test from 'node:test';
import {
  buildStaticPageExportPackage,
  buildStaticPageStandaloneHtml,
  staticPageExportFilename,
  staticPageHtmlFilename,
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
  assert.equal(files.get('index.html').content, '<main>最终静态页</main>');
  assert.match(files.get('asset-manifest.json').content, /static-page-renderer-v1/);
  assert.match(files.get('data-snapshot.json').content, /payload_snapshot/);
  assert.match(files.get('modules.json').content, /核心结论/);
  assert.match(files.get('render-spec.json').content, /dom-text-svg-chart/);
  assert.match(files.get('runtime-requirements.json').content, /Apache-2.0/);
  assert.match(files.get('README.md').content, /ECharts 1/);
  assert.match(files.get('README.md').content, /Apache ECharts（可选）/);
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'README.md'));
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'render-spec.json'));
  assert.ok(artifact.packageManifest.files.some((file) => file.path === 'runtime-requirements.json'));
  assert.equal(artifact.packageManifest.runtime_requirements[0].required, false);
  assert.deepEqual(artifact.warnings, []);
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
  assert.equal(staticPageHtmlFilename(draft), 'static-page-draft-id-with-spaces-index.html');
  assert.equal(buildStaticPageStandaloneHtml(draft, '<main>fresh</main>'), '<main>fresh</main>');
});
