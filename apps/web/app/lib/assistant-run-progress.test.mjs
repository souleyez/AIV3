import test from 'node:test';
import assert from 'node:assert/strict';
import {
  assistantRunCodexReadinessFromDiagnostics,
  buildAssistantRunProgress,
  sanitizeAssistantRunTrailStep,
} from './assistant-run-progress.js';

test('assistantRunCodexReadinessFromDiagnostics maps promotion checks for the progress panel', () => {
  const readiness = assistantRunCodexReadinessFromDiagnostics({
    codex_executor: {
      promotion_gate: {
        status: 'blocked_by_model_gateway',
        blocked_by: 'unsupported_codex_surface',
        readiness_checks: {
          shadow_gate: { ready: true, status: 'eligible_for_host_validation' },
          host_validation: { ready: true, status: 'validated' },
          model_gateway: { ready: false, status: 'unsupported_codex_surface' },
          all_ready: false,
        },
      },
    },
  });

  assert.equal(readiness.status, 'blocked_by_model_gateway');
  assert.equal(readiness.blockedBy, 'unsupported_codex_surface');
  assert.equal(readiness.allReady, false);
  assert.deepEqual(
    readiness.checks.map((check) => [check.key, check.label, check.ready, check.status]),
    [
      ['shadow_gate', 'Shadow', true, 'eligible_for_host_validation'],
      ['host_validation', 'Host', true, 'validated'],
      ['model_gateway', 'Model', false, 'unsupported_codex_surface'],
    ],
  );
});

test('assistantRunCodexReadinessFromDiagnostics returns null without readiness checks', () => {
  assert.equal(assistantRunCodexReadinessFromDiagnostics({ codex_executor: {} }), null);
  assert.equal(assistantRunCodexReadinessFromDiagnostics(null), null);
});

test('buildAssistantRunProgress preserves create-response diagnostics and latest trail windows', () => {
  const response = {
    assistant_run_id: 'run-create-1',
    execution_trail: Array.from({ length: 10 }, (_, index) => ({
      label: `step-${index}`,
      status: 'completed',
      returned_count: index,
    })),
    runtime: {
      react_trace: {
        steps: Array.from({ length: 7 }, (_, index) => ({
          action_type: `tool_${index}`,
          status: 'observed',
          duration_ms: index * 10,
        })),
      },
    },
    diagnostics: {
      codex_executor: {
        promotion_gate: {
          status: 'blocked_by_shadow_gate',
          readiness_checks: {
            shadow_gate: { ready: false, status: 'insufficient_sample' },
            host_validation: { ready: false, status: 'not_run' },
            model_gateway: { ready: false, status: 'not_run' },
            all_ready: false,
          },
        },
      },
    },
  };

  const progress = buildAssistantRunProgress(response, false);

  assert.equal(progress.runId, 'run-create-1');
  assert.equal(progress.continued, false);
  assert.equal(progress.steps.length, 8);
  assert.equal(progress.steps[0].label, 'step-2');
  assert.equal(progress.steps.at(-1).returnedCount, 9);
  assert.equal(progress.traceSteps.length, 6);
  assert.equal(progress.traceSteps[0].actionType, 'tool_1');
  assert.equal(progress.codexReadiness.status, 'blocked_by_shadow_gate');
  assert.equal(progress.codexReadiness.allReady, false);
});

test('buildAssistantRunProgress reads continue responses from nested run fields', () => {
  const progress = buildAssistantRunProgress({
    run: {
      id: 'run-continue-1',
      execution_trail: [{ label: '继续执行', max_steps: 5 }],
      runtime: {
        react_trace: {
          steps: [{ action_type: 'web_search', status: 'rejected', denied_count: 1 }],
        },
      },
    },
  }, true);

  assert.equal(progress.runId, 'run-continue-1');
  assert.equal(progress.continued, true);
  assert.equal(progress.steps[0].label, '继续执行');
  assert.equal(progress.traceSteps[0].actionType, 'web_search');
  assert.equal(progress.traceSteps[0].deniedCount, 1);
  assert.equal(progress.codexReadiness, null);
});

test('sanitizeAssistantRunTrailStep rejects empty or invalid trail entries', () => {
  assert.equal(sanitizeAssistantRunTrailStep(null), null);
  assert.equal(sanitizeAssistantRunTrailStep({}), null);
  assert.deepEqual(sanitizeAssistantRunTrailStep({ react_action: 'retrieve', item_count: '3' }), {
    label: 'retrieve',
    status: 'completed',
    message: '',
    suppliedCount: null,
    detailTargetCount: null,
    returnedCount: 3,
    deniedCount: null,
    stepCount: null,
    reactStep: null,
  });
});
