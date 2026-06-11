import assert from 'node:assert/strict';
import test from 'node:test';
import {
  isTerminalCodexCustomerTaskStatus,
  mergeCodexCustomerArtifactBundles,
  mergeCodexCustomerTasks,
  normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse,
  normalizeCodexCustomerTasksFromAssistantRunResponse,
  promptMayUseCustomerCodex,
  safeCodexCustomerArtifactPath,
  safeCodexCustomerArtifactPublicUrl,
} from './codex-customer-artifacts.js';

const SAFE_SHA = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

test('promptMayUseCustomerCodex only allows the cc forwarding trigger', () => {
  assert.equal(promptMayUseCustomerCodex('cc'), true);
  assert.equal(promptMayUseCustomerCodex('CC 帮我分析这条客户经营需求，给出处理思路。'), true);
  assert.equal(promptMayUseCustomerCodex('cc: 生成一个客户产物包。'), true);
  assert.equal(promptMayUseCustomerCodex('cc：修改这个报表。'), true);
  assert.equal(promptMayUseCustomerCodex('cc，修改这个报表。'), true);
  assert.equal(promptMayUseCustomerCodex('cc123 修改这个报表。'), false);
  assert.equal(promptMayUseCustomerCodex('用 Codex 帮我分析这条客户经营需求，给出处理思路。'), false);
  assert.equal(promptMayUseCustomerCodex('做一个新百经营分析页面。'), false);
  assert.equal(promptMayUseCustomerCodex('做个管理层经营分析看板。'), false);
  assert.equal(promptMayUseCustomerCodex('创建一份客户沟通方案文档。'), false);
  assert.equal(promptMayUseCustomerCodex('这是一个复杂任务，帮我拆解执行计划。'), false);
  assert.equal(promptMayUseCustomerCodex('写一份客户运营方案，并附带执行脚本。'), false);
  assert.equal(promptMayUseCustomerCodex('出一个交付文档包和脚本包。'), false);
  assert.equal(promptMayUseCustomerCodex('生成一个客户产物包，里面包含脚本和说明文件。'), false);
  assert.equal(promptMayUseCustomerCodex('帮我修改 V3 主站页面样式。'), false);
  assert.equal(promptMayUseCustomerCodex('今天天气不错。'), false);
});

function customerBundle(overrides = {}) {
  return {
    type: 'codex_customer_artifact_bundle',
    artifact_type: 'codex_customer_artifacts',
    artifact_kind: 'customer_artifact_bundle',
    status: 'available',
    workflow_execution_id: '11111111-1111-4111-8111-111111111111',
    customer_artifacts: {
      schema: 'v3.customer_codex_artifacts',
      version: 1,
      status: 'available',
      title: '经营分析产物',
      summary: '已生成管理层页面和说明文件。',
      manifest_path: 'customer-artifact-manifest.json',
      artifacts: [
        {
          path: 'reports/index.html',
          title: '管理层页面',
          kind: 'html',
          mime_type: 'text/html',
          bytes: 4096,
          sha256: SAFE_SHA,
        },
      ],
    },
    artifact_manifest: {
      schema: 'v3.output_artifact_manifest',
      schema_version: 1,
      artifact_type: 'codex_customer_artifacts',
      artifact_kind: 'customer_artifact_bundle',
      title: '经营分析产物',
      status: 'available',
      primary_url: null,
      refs: {
        workflow_execution_id: '11111111-1111-4111-8111-111111111111',
        artifact_paths: ['reports/index.html'],
        manifest_path: 'customer-artifact-manifest.json',
      },
      safety: {
        credentials_exposed: false,
        raw_logs_exposed: false,
        workspace_paths_only: true,
        absolute_paths_exposed: false,
        published: false,
        requires_datamax_publish_validation: true,
      },
    },
    ...overrides,
  };
}

test('safeCodexCustomerArtifactPath accepts workspace relative paths only', () => {
  assert.equal(safeCodexCustomerArtifactPath('reports/index.html'), 'reports/index.html');
  assert.equal(safeCodexCustomerArtifactPath('/Users/manslive01/report.html'), '');
  assert.equal(safeCodexCustomerArtifactPath('../report.html'), '');
  assert.equal(safeCodexCustomerArtifactPath('node_modules/pkg/index.js'), '');
  assert.equal(safeCodexCustomerArtifactPath('reports/.env'), '');
  assert.equal(safeCodexCustomerArtifactPath('C:\\tmp\\report.html'), '');
});

test('safeCodexCustomerArtifactPath rejects secret-like customer artifact names', () => {
  [
    'artifacts/.env.local',
    'artifacts/report.env',
    'artifacts/secret.json',
    'artifacts/credentials.json',
    'artifacts/access_token.txt',
    'artifacts/refresh_token.txt',
    'artifacts/api_key.txt',
    'artifacts/apikey.txt',
    'artifacts/private_key.txt',
    'artifacts/passwords.csv',
    'artifacts/id_ecdsa',
    'artifacts/id_ed25519',
    'artifacts/authorized_keys',
  ].forEach((path) => {
    assert.equal(safeCodexCustomerArtifactPath(path), '', path);
  });
});

test('safeCodexCustomerArtifactPublicUrl accepts generated artifact URLs only', () => {
  assert.equal(
    safeCodexCustomerArtifactPublicUrl('https://v3.elepcloud.com/generated-artifacts/customer-codex/run/index.html'),
    'https://v3.elepcloud.com/generated-artifacts/customer-codex/run/index.html',
  );
  assert.equal(
    safeCodexCustomerArtifactPublicUrl('/generated-artifacts/customer-codex/run/index.html'),
    '/generated-artifacts/customer-codex/run/index.html',
  );
  assert.equal(safeCodexCustomerArtifactPublicUrl('https://example.com/generated-artifacts/run/index.html'), '');
  assert.equal(safeCodexCustomerArtifactPublicUrl('https://v3.elepcloud.com/generated-artifacts/pending/run/index.html'), '');
});

test('normalizes assistant run Codex customer artifact bundles safely', () => {
  const bundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    run: {
      output_artifacts: [
        customerBundle({
          customer_artifacts: {
            ...customerBundle().customer_artifacts,
            artifacts: [
              ...customerBundle().customer_artifacts.artifacts,
              {
                path: '/Users/manslive01/secrets/raw.txt',
                title: 'must not leak',
                kind: 'text',
              },
            ],
          },
        }),
      ],
    },
  });

  assert.equal(bundles.length, 1);
  assert.equal(bundles[0].title, '经营分析产物');
  assert.equal(bundles[0].published, false);
  assert.equal(bundles[0].requiresPublishValidation, true);
  assert.deepEqual(bundles[0].files.map((file) => file.path), ['reports/index.html']);
  assert.equal(JSON.stringify(bundles).includes('/Users/'), false);
  assert.equal(JSON.stringify(bundles).includes('must not leak'), false);
});

test('normalizes published Codex customer artifact links', () => {
  const publicUrl = 'https://v3.elepcloud.com/generated-artifacts/customer-codex/customer_artifact_request/run/reports/index.html';
  const notesUrl = publicUrl.replace('index.html', 'notes.md');
  const bundle = customerBundle({
    status: 'published',
    published: true,
    primary_url: publicUrl,
    public_url: publicUrl,
    customer_artifacts: {
      ...customerBundle().customer_artifacts,
      status: 'published',
      published: true,
      primary_url: publicUrl,
      public_url: publicUrl,
      published_manifest_url: publicUrl.replace('reports/index.html', 'manifest.json'),
      artifacts: [
        {
          ...customerBundle().customer_artifacts.artifacts[0],
          published: true,
          public_url: publicUrl,
        },
        {
          path: 'reports/notes.md',
          title: '说明文件',
          kind: 'markdown',
          mime_type: 'text/markdown',
          bytes: 1024,
          sha256: SAFE_SHA,
          published: true,
          public_url: notesUrl,
        },
      ],
    },
    artifact_manifest: {
      ...customerBundle().artifact_manifest,
      status: 'published',
      primary_url: publicUrl,
      safety: {
        credentials_exposed: false,
        raw_logs_exposed: false,
        workspace_paths_only: true,
        absolute_paths_exposed: false,
        published: true,
        requires_datamax_publish_validation: false,
      },
    },
  });

  const bundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [bundle],
  });

  assert.equal(bundles.length, 1);
  assert.equal(bundles[0].published, true);
  assert.equal(bundles[0].requiresPublishValidation, false);
  assert.equal(bundles[0].statusLabel, '已发布');
  assert.equal(bundles[0].primaryUrl, publicUrl);
  assert.deepEqual(bundles[0].files.map((file) => file.publicUrl), [publicUrl, notesUrl]);
});

test('strips unsafe published URLs from Codex customer bundles', () => {
  const bundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [
      customerBundle({
        status: 'published',
        published: true,
        primary_url: 'https://example.com/report.html',
        customer_artifacts: {
          ...customerBundle().customer_artifacts,
          status: 'published',
          published: true,
          primary_url: 'https://example.com/report.html',
          artifacts: [
            {
              ...customerBundle().customer_artifacts.artifacts[0],
              published: true,
              public_url: 'https://example.com/report.html',
            },
          ],
        },
        artifact_manifest: {
          ...customerBundle().artifact_manifest,
          status: 'published',
          primary_url: 'https://example.com/report.html',
          safety: {
            credentials_exposed: false,
            raw_logs_exposed: false,
            workspace_paths_only: true,
            absolute_paths_exposed: false,
            published: true,
            requires_datamax_publish_validation: false,
          },
        },
      }),
    ],
  });

  assert.equal(bundles.length, 1);
  assert.equal(bundles[0].published, false);
  assert.equal(bundles[0].primaryUrl, '');
  assert.equal(bundles[0].files[0].publicUrl, '');
  assert.equal(JSON.stringify(bundles).includes('example.com'), false);
});

test('rejects unsafe customer bundle safety flags', () => {
  const bundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [
      customerBundle({
        artifact_manifest: {
          ...customerBundle().artifact_manifest,
          safety: {
            credentials_exposed: true,
          },
        },
      }),
    ],
  });

  assert.deepEqual(bundles, []);
});

test('strips unsafe customer artifact display text', () => {
  const bundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [
      customerBundle({
        title: 'Authorization: Bearer should-not-leak',
        summary: 'secret token should-not-leak',
        customer_artifacts: {
          ...customerBundle().customer_artifacts,
          title: 'Authorization: Bearer should-not-leak',
          summary: 'secret token should-not-leak',
          artifacts: [{
            ...customerBundle().customer_artifacts.artifacts[0],
            title: 'Authorization: Bearer should-not-leak',
          }],
        },
        artifact_manifest: {
          ...customerBundle().artifact_manifest,
          title: 'Authorization: Bearer should-not-leak',
        },
      }),
    ],
  });

  assert.equal(bundles.length, 1);
  assert.equal(bundles[0].title, 'Codex 待发布产物');
  assert.equal(bundles[0].summary, '1 个文件，等待 DataMax 发布校验');
  assert.equal(bundles[0].files[0].title, 'reports/index.html');
  assert.equal(JSON.stringify(bundles).includes('should-not-leak'), false);
  assert.equal(JSON.stringify(bundles).includes('Authorization'), false);
});

test('normalizes customer artifact bundles to artifact-producing capabilities only', () => {
  const unsafeBundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [
      customerBundle({
        capability: 'v3_product_change_request',
        route: 'v3_product_change_request',
        customer_artifacts: {
          ...customerBundle().customer_artifacts,
          capability: 'v3_product_change_request',
          route: 'v3_product_change_request',
        },
      }),
    ],
  });

  assert.equal(unsafeBundles.length, 1);
  assert.equal(unsafeBundles[0].capability, '');
  assert.equal(unsafeBundles[0].route, '');
  assert.equal(JSON.stringify(unsafeBundles).includes('v3_product_change_request'), false);

  const routeOnlyBundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [
      customerBundle({
        capability: 'v3_product_change_request',
        route: 'generated_static_page_publish',
        customer_artifacts: {
          ...customerBundle().customer_artifacts,
          capability: 'v3_product_change_request',
          route: 'generated_static_page_publish',
        },
      }),
    ],
  });

  assert.equal(routeOnlyBundles.length, 1);
  assert.equal(routeOnlyBundles[0].capability, 'generated_static_page_publish');
  assert.equal(routeOnlyBundles[0].route, 'generated_static_page_publish');
});

test('mergeCodexCustomerArtifactBundles dedupes by bundle id and prefers incoming', () => {
  const existing = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [customerBundle({ status: 'old' })],
  });
  const incoming = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse({
    output_artifacts: [customerBundle({ status: 'available' })],
  });

  const merged = mergeCodexCustomerArtifactBundles(existing, incoming);

  assert.equal(merged.length, 1);
  assert.equal(merged[0].status, 'available');
});

test('normalizes customer Codex sidecar task events', () => {
  const workflowExecutionId = '22222222-2222-4222-8222-222222222222';
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [
      {
        event_name: 'assistant_run.codex_sidecar_queued',
        sequence_no: 1,
        created_at: '2026-06-09T08:00:00Z',
        payload: {
          capability: 'customer_complex_request',
          workflow_execution_id: workflowExecutionId,
          non_blocking: true,
          main_answer_path_preserved: true,
        },
      },
      {
        event_name: 'codex_host_task.exec_completed',
        sequence_no: 2,
        created_at: '2026-06-09T08:00:10Z',
        payload: {
          capability: 'customer_complex_request',
          workflow_execution_id: workflowExecutionId,
          status: 'completed',
          customer_result_summary: {
            schema: 'v3.customer_codex_result_summary',
            schema_version: 1,
            status: 'completed',
            title: '经营分析处理思路',
            summary: '应按经营分析意图组织回答，并优先使用数据集证据。',
            findings: ['客户要的是经营分析，不是文档摘要。'],
            recommended_next_actions: ['补齐销售、毛利、客流和门店维度证据。'],
            safety: {
              raw_logs_exposed: false,
              credentials_exposed: false,
              absolute_paths_exposed: false,
              prompt_exposed: false,
            },
          },
          raw_prompt: 'must not leak',
          stdout: 'must not leak',
        },
      },
    ],
  });

  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].capability, 'customer_complex_request');
  assert.equal(tasks[0].status, 'completed');
  assert.equal(tasks[0].statusLabel, '已完成');
  assert.equal(tasks[0].summary, '应按经营分析意图组织回答，并优先使用数据集证据。');
  assert.equal(tasks[0].resultSummary.title, '经营分析处理思路');
  assert.deepEqual(tasks[0].resultSummary.findings, ['客户要的是经营分析，不是文档摘要。']);
  assert.deepEqual(tasks[0].resultSummary.recommendedNextActions, ['补齐销售、毛利、客流和门店维度证据。']);
  assert.equal(tasks[0].workflowExecutionId, workflowExecutionId);
  assert.equal(JSON.stringify(tasks).includes('must not leak'), false);
});

test('normalizes data ingestion Codex sidecar task events', () => {
  const workflowExecutionId = '33333333-3333-4333-8333-333333333333';
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [
      {
        event_name: 'assistant_run.codex_sidecar_queued',
        sequence_no: 1,
        created_at: '2026-06-09T08:00:00Z',
        payload: {
          capability: 'data_ingestion_analysis',
          route: 'assistant_run_data_ingestion_analysis',
          workflow_execution_id: workflowExecutionId,
          permission_scope: 'read-only data-ingestion analysis and staging-spec planning',
          non_blocking: true,
          main_answer_path_preserved: true,
        },
      },
      {
        event_name: 'codex_host_task.exec_completed',
        sequence_no: 2,
        created_at: '2026-06-09T08:00:10Z',
        payload: {
          capability: 'data_ingestion_analysis',
          route: 'assistant_run_data_ingestion_analysis',
          workflow_execution_id: workflowExecutionId,
          status: 'completed',
          customer_result_summary: {
            schema: 'v3.customer_codex_result_summary',
            schema_version: 1,
            status: 'completed',
            title: '数据接入分析',
            summary: '已生成字段映射、staging plan 和校验建议。',
            findings: ['当前业务库需要先做只读字段盘点。'],
            recommended_next_actions: ['人工确认后再创建或更新 staging 数据集。'],
            safety: {
              raw_logs_exposed: false,
              credentials_exposed: false,
              absolute_paths_exposed: false,
              prompt_exposed: false,
            },
          },
        },
      },
    ],
  });

  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].title, 'Codex 数据接入');
  assert.equal(tasks[0].capability, 'data_ingestion_analysis');
  assert.equal(tasks[0].route, 'data_ingestion_analysis');
  assert.equal(tasks[0].status, 'completed');
  assert.equal(tasks[0].summary, '已生成字段映射、staging plan 和校验建议。');
  assert.equal(tasks[0].permissionScope, '只读数据接入分析');
  assert.equal(tasks[0].resultSummary.artifactIntent, true);
});

test('rejects unsafe customer Codex result summary fields', () => {
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'codex_host_task.exec_completed',
      sequence_no: 1,
      payload: {
        capability: 'customer_complex_request',
        workflow_execution_id: '55555555-5555-4555-8555-555555555555',
        status: 'completed',
        customer_result_summary: {
          schema: 'v3.customer_codex_result_summary',
          status: 'completed',
          title: 'Authorization: Bearer must-not-leak',
          summary: 'secret token must not leak',
          findings: ['/Users/manslive01/.codex/session.json'],
          safety: {
            raw_logs_exposed: true,
          },
        },
      },
    }],
  });

  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].resultSummary, null);
  assert.equal(tasks[0].summary, 'Codex 已完成复杂任务处理，结果通过主回答或后续事件回传。');
  assert.equal(JSON.stringify(tasks).includes('must-not-leak'), false);
  assert.equal(JSON.stringify(tasks).includes('/Users/'), false);
});

test('normalizes V3 product change blocked sidecar event', () => {
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'assistant_run.codex_sidecar_scope_blocked',
      sequence_no: 1,
      payload: {
        capability: 'v3_product_change_request',
        route: 'v3_product_change_request',
        status: 'needs_operator_review',
        reason: 'v3_product_change_not_customer_writable',
        non_blocking: true,
        main_answer_path_preserved: true,
      },
    }],
  });

  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].title, '需人工审核');
  assert.equal(tasks[0].capability, 'v3_product_change_request');
  assert.equal(tasks[0].route, 'v3_product_change_request');
  assert.equal(tasks[0].status, 'blocked');
  assert.equal(tasks[0].statusLabel, '需人工审核');
  assert.equal(tasks[0].summary, '该请求涉及 V3 产品变更，需要平台管理员或开发人员审核。');
});

test('artifact-ready task events ignore non-artifact capabilities', () => {
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'assistant_run.customer_artifact_request_artifacts_ready',
      sequence_no: 1,
      payload: {
        capability: 'v3_product_change_request',
        route: 'v3_product_change_request',
        workflow_execution_id: '88888888-8888-4888-8888-888888888888',
        status: 'available',
      },
    }],
  });

  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].title, 'Codex 产物任务');
  assert.equal(tasks[0].capability, 'customer_artifact_request');
  assert.equal(tasks[0].route, 'customer_artifact_request');
  assert.equal(tasks[0].status, 'completed');
  assert.equal(JSON.stringify(tasks).includes('v3_product_change_request'), false);
});

test('terminal customer Codex task statuses include blocked', () => {
  assert.equal(isTerminalCodexCustomerTaskStatus('completed'), true);
  assert.equal(isTerminalCodexCustomerTaskStatus('blocked'), true);
  assert.equal(isTerminalCodexCustomerTaskStatus('running'), false);
  assert.equal(isTerminalCodexCustomerTaskStatus('needs_operator_review'), false);
});

test('strips unsafe customer Codex task display fields', () => {
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'codex_host_task.exec_failed',
      sequence_no: 1,
      payload: {
        capability: 'customer_complex_request',
        route: '/Users/manslive01/.codex/session.json',
        workflow_execution_id: '77777777-7777-4777-8777-777777777777',
        reason: 'Authorization: Bearer should-not-leak',
        permission_scope: '/srv/aiv3/repo should-not-leak',
      },
    }],
  });

  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].route, 'customer_complex_request');
  assert.equal(tasks[0].summary, '客户 Codex 任务已进入 DataMax 受控执行链路。');
  assert.equal(tasks[0].permissionScope, '只读分析');
  assert.equal(JSON.stringify(tasks).includes('should-not-leak'), false);
  assert.equal(JSON.stringify(tasks).includes('/Users/'), false);
  assert.equal(JSON.stringify(tasks).includes('/srv/aiv3/repo'), false);
});

test('mergeCodexCustomerTasks dedupes by workflow id and prefers incoming status', () => {
  const workflowExecutionId = '33333333-3333-4333-8333-333333333333';
  const queued = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'assistant_run.codex_sidecar_queued',
      sequence_no: 1,
      payload: {
        capability: 'customer_artifact_request',
        workflow_execution_id: workflowExecutionId,
      },
    }],
  });
  const completed = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'codex_host_task.exec_completed',
      sequence_no: 2,
      payload: {
        capability: 'customer_artifact_request',
        workflow_execution_id: workflowExecutionId,
        status: 'completed',
      },
    }],
  });

  const merged = mergeCodexCustomerTasks(queued, completed);

  assert.equal(merged.length, 1);
  assert.equal(merged[0].status, 'completed');
});

test('normalizes generated static page publish task events', () => {
  const workflowExecutionId = '66666666-6666-4666-8666-666666666666';
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'assistant_run.customer_artifact_request_artifacts_ready',
      sequence_no: 3,
      created_at: '2026-06-09T09:00:00Z',
      payload: {
        capability: 'generated_static_page_publish',
        route: 'generated_static_page_publish',
        workflow_execution_id: workflowExecutionId,
        status: 'available',
        customer_artifacts: {
          status: 'published',
          artifact_count: 1,
        },
      },
    }],
  });

  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].title, 'Codex 页面发布');
  assert.equal(tasks[0].capability, 'generated_static_page_publish');
  assert.equal(tasks[0].route, 'generated_static_page_publish');
  assert.equal(tasks[0].status, 'completed');
  assert.equal(tasks[0].workflowExecutionId, workflowExecutionId);
});

test('ignores non-customer Codex fixed task events', () => {
  const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse({
    events: [{
      event_name: 'codex_host_task.exec_completed',
      sequence_no: 1,
      payload: {
        capability: 'static_page_image2_data_publish',
        workflow_execution_id: '44444444-4444-4444-8444-444444444444',
        status: 'completed',
      },
    }],
  });

  assert.deepEqual(tasks, []);
});
