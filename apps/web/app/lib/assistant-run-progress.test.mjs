import test from 'node:test';
import assert from 'node:assert/strict';
import {
  assistantRunCodexBudgetFromDiagnostics,
  assistantRunCodexHostValidationFromDiagnostics,
  assistantRunCodexLivenessFromDiagnostics,
  assistantRunCodexModelGatewayFromDiagnostics,
  assistantRunCodexReadinessFromDiagnostics,
  assistantRunCodexTransportPolicyFromDiagnostics,
  assistantRunProviderUsageFromDiagnostics,
  assistantRunSupplyQualityFromDiagnostics,
  buildAssistantRunProgress,
  sanitizeAssistantRunTrailStep,
} from './assistant-run-progress.js';

test('assistantRunCodexReadinessFromDiagnostics maps promotion checks for the progress panel', () => {
  const readiness = assistantRunCodexReadinessFromDiagnostics({
    codex_executor: {
      promotion_gate: {
        status: 'blocked_by_model_gateway',
        blocked_by: 'unsupported_codex_surface',
        next_step: 'fix_codex_model_gateway_profile_before_promotion',
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
  assert.equal(readiness.nextStep, 'fix_codex_model_gateway_profile_before_promotion');
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

test('assistantRunCodexReadinessFromDiagnostics summarizes structured blockers safely', () => {
  const readiness = assistantRunCodexReadinessFromDiagnostics({
    codex_executor: {
      promotion_gate: {
        status: 'blocked_by_shadow_gate',
        blocked_by: {
          comparison_status: 'invalid_suggestion',
          prompt: 'raw prompt should not render',
          arguments: { secret: 'should not render' },
        },
        next_step: 'continue_shadow_comparison_until_stable',
        readiness_checks: {
          shadow_gate: { ready: false, status: 'blocked' },
          host_validation: { ready: false, status: 'not_run' },
          model_gateway: { ready: true, status: 'ready' },
          all_ready: false,
        },
      },
    },
  });

  assert.equal(readiness.blockedBy, 'invalid_suggestion');
  assert.equal(readiness.nextStep, 'continue_shadow_comparison_until_stable');
  assert.doesNotMatch(JSON.stringify(readiness), /raw prompt|secret|should not render/);
});

test('assistantRunCodexReadinessFromDiagnostics returns null without readiness checks', () => {
  assert.equal(assistantRunCodexReadinessFromDiagnostics({ codex_executor: {} }), null);
  assert.equal(assistantRunCodexReadinessFromDiagnostics(null), null);
});

test('assistantRunProviderUsageFromDiagnostics summarizes safe provider usage metadata', () => {
  const usage = assistantRunProviderUsageFromDiagnostics({
    provider_usage: {
      summary: {
        request_count: 2,
        failed_request_count: 1,
        input_tokens: 120,
        output_tokens: 80,
        total_tokens: 200,
        last_request_id: 'resp_provider_usage_1234567890',
      },
      recent_events: [
        {
          provider: 'openclaw',
          model: 'legacy-model',
          request_id: 'resp_old',
          status: 'responded',
        },
        {
          provider: 'codex-shim',
          model: 'codex-compatible-profile-for-v3-shadow-validation',
          request_id: 'resp_provider_usage_1234567890',
          status: 'failed',
        },
      ],
    },
  });

  assert.equal(usage.requestCount, 2);
  assert.equal(usage.failedRequestCount, 1);
  assert.equal(usage.inputTokens, 120);
  assert.equal(usage.outputTokens, 80);
  assert.equal(usage.totalTokens, 200);
  assert.equal(usage.lastRequestId, 'resp_provider_usage_1234567890');
  assert.equal(usage.lastProvider, 'codex-shim');
  assert.equal(usage.lastModel, 'codex-compatible-profile-for-v3-shad...');
  assert.equal(usage.lastStatus, 'failed');
});

test('assistantRunProviderUsageFromDiagnostics stays quiet without provider requests', () => {
  assert.equal(assistantRunProviderUsageFromDiagnostics({ provider_usage: { summary: { request_count: 0 } } }), null);
  assert.equal(assistantRunProviderUsageFromDiagnostics(null), null);
});

test('assistantRunProviderUsageFromDiagnostics infers counts from recent events when summary is sparse', () => {
  const usage = assistantRunProviderUsageFromDiagnostics({
    provider_usage: {
      recent_events: [
        {
          provider: 'gpt',
          model: 'assistant-chat',
          request_id: 'resp_1',
          status: 'responded',
          input_tokens: 10,
          output_tokens: 20,
          total_tokens: 30,
        },
        {
          provider: 'gpt',
          model: 'assistant-chat',
          request_id: 'resp_2',
          status: 'failed',
          input_tokens: 4,
          output_tokens: 0,
          total_tokens: 4,
        },
      ],
    },
  });

  assert.equal(usage.requestCount, 2);
  assert.equal(usage.failedRequestCount, 1);
  assert.equal(usage.inputTokens, 14);
  assert.equal(usage.outputTokens, 20);
  assert.equal(usage.totalTokens, 34);
  assert.equal(usage.lastRequestId, 'resp_2');
  assert.equal(usage.lastStatus, 'failed');
});

test('assistantRunCodexBudgetFromDiagnostics summarizes context budget and tool trimming safely', () => {
  const budget = assistantRunCodexBudgetFromDiagnostics({
    codex_executor: {
      latest: {
        context_budget: {
          estimated_prompt_chars: 4096,
          max_prompt_chars: 12000,
          budget_pressure: 'normal',
          item_count: 8,
        },
        provider_shim_observability: {
          context_budget_report: {
            estimated_prompt_chars: 5000,
            max_prompt_chars: 15000,
            budget_pressure: 'attention',
            trimmed_item_count: 1,
          },
          tool_output_budget: {
            largest_output_chars: 2048,
            trimmed_output_count: 2,
            preserved_evidence_ref_count: 5,
          },
        },
      },
    },
  });

  assert.equal(budget.budgetPressure, 'normal');
  assert.equal(budget.estimatedPromptChars, 4096);
  assert.equal(budget.maxPromptChars, 12000);
  assert.equal(budget.itemCount, 8);
  assert.equal(budget.trimmedItemCount, 1);
  assert.equal(budget.trimmedOutputCount, 2);
  assert.equal(budget.preservedEvidenceRefCount, 5);
  assert.equal(budget.largestOutputChars, 2048);
});

test('assistantRunCodexBudgetFromDiagnostics does not expose raw context budget items', () => {
  const budget = assistantRunCodexBudgetFromDiagnostics({
    codex_executor: {
      latest: {
        provider_shim_observability: {
          context_budget_report: {
            estimated_prompt_chars: 5000,
            budget_pressure: 'attention',
            trimmed_item_count: 1,
            item_count: 2,
            items: [
              {
                category: 'retrieval_evidence',
                text: 'raw prompt evidence should not render',
                secret: 'do-not-copy',
              },
            ],
          },
        },
      },
    },
  });

  assert.equal(budget.budgetPressure, 'attention');
  assert.equal(budget.estimatedPromptChars, 5000);
  assert.equal(budget.itemCount, 2);
  assert.equal(budget.trimmedItemCount, 1);
  assert.doesNotMatch(JSON.stringify(budget), /raw prompt|do-not-copy|retrieval_evidence/);
});

test('assistantRunCodexLivenessFromDiagnostics summarizes retry decisions safely', () => {
  const liveness = assistantRunCodexLivenessFromDiagnostics({
    codex_executor: {
      latest: {
        provider_shim_observability: {
          liveness_events: {
            event_count: 2,
            latest: {
              event_type: 'tool_call_liveness_stall',
              status: 'recovered',
              retry_count: 1,
              action: 'continue',
              occurred_at: '2026-05-11T02:22:00Z',
              has_note: true,
              note: 'raw liveness note should not render',
            },
          },
        },
      },
    },
  });

  assert.equal(liveness.eventCount, 2);
  assert.equal(liveness.eventType, 'tool_call_liveness_stall');
  assert.equal(liveness.status, 'recovered');
  assert.equal(liveness.retryCount, 1);
  assert.equal(liveness.action, 'continue');
  assert.equal(liveness.occurredAt, '2026-05-11T02:22:00Z');
  assert.equal(liveness.hasNote, true);
  assert.doesNotMatch(JSON.stringify(liveness), /raw liveness note|should not render/);
});

test('assistantRunCodexLivenessFromDiagnostics stays quiet without liveness events', () => {
  assert.equal(assistantRunCodexLivenessFromDiagnostics({
    codex_executor: {
      latest: {
        provider_shim_observability: {
          liveness_events: { event_count: 0, latest: null },
        },
      },
    },
  }), null);
  assert.equal(assistantRunCodexLivenessFromDiagnostics(null), null);
});

test('assistantRunCodexModelGatewayFromDiagnostics summarizes safe gateway readiness', () => {
  const gateway = assistantRunCodexModelGatewayFromDiagnostics({
    codex_executor: {
      latest: {
        model_gateway: {
          lane: 'codex_conversation',
          selected_model: {
            provider: 'minimax',
            model: 'codex-compatible-profile-for-v3-shadow-validation',
          },
          profile_id: 'minimax-codex-shadow',
          provider_id: 'minimax',
          model_id: 'abab6.5',
          wire_api: 'codex_compatible_shim',
          auth_configured: true,
          capability_manifest: {
            codex_compatible: true,
            json_mode: true,
            tool_calling: true,
          },
          codex_surface: {
            wire_api: 'codex_compatible_shim',
            codex_compatible: true,
            json_actions_supported: true,
            tool_calls_supported: true,
            real_execution_block_reason: 'disabled_on_this_host',
          },
          codex_real_execution_allowed: false,
          secrets_redacted: true,
          raw_provider_payloads_allowed: false,
          profile_env_prefix: 'SHOULD_NOT_RENDER',
          api_key: 'sk-should-not-render',
          raw_provider_payload: { prompt: 'raw prompt should not render' },
        },
      },
      model_gateway_gate: {
        status: 'ready',
        ready_for_promotion_review: true,
        profile_available: true,
        profile_id: 'minimax-codex-shadow',
      },
    },
  });

  assert.equal(gateway.status, 'ready');
  assert.equal(gateway.readyForPromotion, true);
  assert.equal(gateway.lane, 'codex_conversation');
  assert.equal(gateway.profileAvailable, true);
  assert.equal(gateway.profileId, 'minimax-codex-shadow');
  assert.equal(gateway.provider, 'minimax');
  assert.equal(gateway.model, 'codex-compatible-profile-for-v3-shad...');
  assert.equal(gateway.wireApi, 'codex_compatible_shim');
  assert.equal(gateway.authConfigured, true);
  assert.equal(gateway.codexCompatible, true);
  assert.equal(gateway.jsonActionsSupported, true);
  assert.equal(gateway.toolCallsSupported, true);
  assert.deepEqual(gateway.capabilities, ['codex', 'json', 'tools']);
  assert.equal(gateway.realExecutionAllowed, false);
  assert.equal(gateway.realExecutionBlockReason, 'disabled_on_this_host');
  assert.equal(gateway.secretsRedacted, true);
  assert.equal(gateway.rawProviderPayloadsAllowed, false);
  assert.doesNotMatch(JSON.stringify(gateway), /SHOULD_NOT_RENDER|sk-should|raw prompt/);
});

test('assistantRunCodexModelGatewayFromDiagnostics stays quiet without gateway diagnostics', () => {
  assert.equal(assistantRunCodexModelGatewayFromDiagnostics({ codex_executor: {} }), null);
  assert.equal(assistantRunCodexModelGatewayFromDiagnostics(null), null);
});

test('assistantRunCodexHostValidationFromDiagnostics summarizes safe jump-host validation', () => {
  const hostValidation = assistantRunCodexHostValidationFromDiagnostics({
    codex_executor: {
      host_validation_summary: {
        status: 'validated',
        completed_count: 1,
        failed_count: 0,
        guard_failed_count: 0,
        pending_count: 0,
        codex_mutation_allowed: false,
        next_step: 'review_host_report_then_consider_feature_gate_promotion',
        latest: {
          mode: 'codex_exec',
          host_kind: 'windows_jump',
          host_kind_allowed: true,
          profile_kind: 'codex-compatible-shim',
          workspace_configured: true,
          prompt_redacted: true,
          task_memory_isolated: true,
          task_memory_space_configured: true,
          validation_requirements_met: true,
          command_plan: { args_without_prompt: ['exec', '--secret', 'sk-command-should-not-render'] },
          process: {
            stdout_excerpt: 'raw stdout should not render',
            stderr_excerpt: 'raw stderr should not render',
          },
          profile: { env_key: 'MINIMAX_API_KEY' },
        },
      },
    },
  });

  assert.equal(hostValidation.status, 'validated');
  assert.equal(hostValidation.completedCount, 1);
  assert.equal(hostValidation.failedCount, 0);
  assert.equal(hostValidation.guardFailedCount, 0);
  assert.equal(hostValidation.pendingCount, 0);
  assert.equal(hostValidation.codexMutationAllowed, false);
  assert.equal(hostValidation.latestMode, 'codex_exec');
  assert.equal(hostValidation.latestHostKind, 'windows_jump');
  assert.equal(hostValidation.hostKindAllowed, true);
  assert.equal(hostValidation.latestProfileKind, 'codex-compatible-shim');
  assert.equal(hostValidation.workspaceConfigured, true);
  assert.equal(hostValidation.promptRedacted, true);
  assert.equal(hostValidation.taskMemoryIsolated, true);
  assert.equal(hostValidation.taskMemorySpaceConfigured, true);
  assert.equal(hostValidation.validationRequirementsMet, true);
  assert.equal(hostValidation.nextStep, 'review_host_report_then_consider_feature_gate_promotion');
  assert.doesNotMatch(
    JSON.stringify(hostValidation),
    /sk-command|raw stdout|raw stderr|MINIMAX_API_KEY|args_without_prompt/,
  );
});

test('assistantRunCodexHostValidationFromDiagnostics stays quiet without host summary', () => {
  assert.equal(assistantRunCodexHostValidationFromDiagnostics({ codex_executor: {} }), null);
  assert.equal(assistantRunCodexHostValidationFromDiagnostics(null), null);
});

test('assistantRunCodexTransportPolicyFromDiagnostics summarizes safe real-transport gates', () => {
  const transportPolicy = assistantRunCodexTransportPolicyFromDiagnostics({
    codex_executor: {
      latest: {
        transport_policy: {
          requested_transport: 'codex_exec_schema',
          effective_transport: 'codex_plan_only',
          real_transport_requested: true,
          real_transport_feature_gate_enabled: false,
          real_transport_promotion_review_approved: false,
          downgraded: true,
          downgrade_reason: 'feature_gate_disabled',
          direct_execution_authoritative: true,
          codex_mutation_allowed: false,
          queue_allowed: false,
          manual_feature_gate_required_for_real_transport: true,
          promotion_review_required_for_real_transport: true,
          host_validation_required_for_real_transport: true,
          next_step: 'keep_codex_in_shadow_plan_only_until_promotion_gate_review',
          debug_token: 'debug-token-should-not-render',
          env_key: 'ASSISTANT_RUN_CODEX_REAL_TRANSPORT_FEATURE_GATE',
          raw_prompt: 'raw prompt should not render',
        },
      },
    },
  });

  assert.equal(transportPolicy.requestedTransport, 'codex_exec_schema');
  assert.equal(transportPolicy.effectiveTransport, 'codex_plan_only');
  assert.equal(transportPolicy.realTransportRequested, true);
  assert.equal(transportPolicy.realTransportFeatureGateEnabled, false);
  assert.equal(transportPolicy.realTransportPromotionReviewApproved, false);
  assert.equal(transportPolicy.downgraded, true);
  assert.equal(transportPolicy.downgradeReason, 'feature_gate_disabled');
  assert.equal(transportPolicy.directExecutionAuthoritative, true);
  assert.equal(transportPolicy.codexMutationAllowed, false);
  assert.equal(transportPolicy.queueAllowed, false);
  assert.equal(transportPolicy.manualFeatureGateRequired, true);
  assert.equal(transportPolicy.promotionReviewRequired, true);
  assert.equal(transportPolicy.hostValidationRequired, true);
  assert.equal(transportPolicy.nextStep, 'keep_codex_in_shadow_plan_only_until_promotion_gate_review');
  assert.doesNotMatch(
    JSON.stringify(transportPolicy),
    /debug-token|ASSISTANT_RUN_CODEX_REAL_TRANSPORT_FEATURE_GATE|raw prompt/,
  );
});

test('assistantRunCodexTransportPolicyFromDiagnostics stays quiet without transport policy', () => {
  assert.equal(assistantRunCodexTransportPolicyFromDiagnostics({ codex_executor: {} }), null);
  assert.equal(assistantRunCodexTransportPolicyFromDiagnostics(null), null);
});

test('assistantRunSupplyQualityFromDiagnostics summarizes counts without raw locators or notes', () => {
  const supply = assistantRunSupplyQualityFromDiagnostics({
    codex_executor: {
      latest: {
        supply_quality: {
          status: 'partial',
          supplyRequested: true,
          qualityFirst: true,
          selectedDatasetCount: 2,
          suppliedItemCount: 3,
          indexedEvidenceCount: 2,
          fallbackChunkCount: 1,
          conversationMemoryItemCount: 1,
          mediaContextCount: 1,
          detailTargetCount: 2,
          limit: 8,
          citationLocatorCount: 2,
          citationLocators: ['dataset://raw/source/should/not/render'],
          notes: ['raw supply note should not render'],
          modelGuidance: ['raw model guidance should not render'],
        },
      },
    },
  });

  assert.equal(supply.status, 'partial');
  assert.equal(supply.supplyRequested, true);
  assert.equal(supply.qualityFirst, true);
  assert.equal(supply.selectedDatasetCount, 2);
  assert.equal(supply.suppliedItemCount, 3);
  assert.equal(supply.indexedEvidenceCount, 2);
  assert.equal(supply.fallbackChunkCount, 1);
  assert.equal(supply.conversationMemoryItemCount, 1);
  assert.equal(supply.mediaContextCount, 1);
  assert.equal(supply.detailTargetCount, 2);
  assert.equal(supply.limit, 8);
  assert.equal(supply.citationLocatorCount, 2);
  assert.doesNotMatch(JSON.stringify(supply), /raw\/source|raw supply note|raw model guidance/);
});

test('assistantRunSupplyQualityFromDiagnostics falls back to evidence state and budget counts', () => {
  const supply = assistantRunSupplyQualityFromDiagnostics({
    codex_executor: {
      latest: {
        context_budget: {
          evidence_item_count: 4,
          selected_dataset_count: 1,
          hidden_memory_item_count: 2,
        },
      },
    },
  }, {
    status: 'supplied',
    supply_quality: {
      status: 'grounded',
      citation_locator_count: 1,
      fallback_chunk_count: 0,
      media_context_count: 0,
    },
  });

  assert.equal(supply.status, 'grounded');
  assert.equal(supply.suppliedItemCount, 4);
  assert.equal(supply.selectedDatasetCount, 1);
  assert.equal(supply.conversationMemoryItemCount, 2);
  assert.equal(supply.citationLocatorCount, 1);
  assert.equal(supply.fallbackChunkCount, 0);
});

test('assistantRunSupplyQualityFromDiagnostics stays quiet without supply diagnostics', () => {
  assert.equal(assistantRunSupplyQualityFromDiagnostics({ codex_executor: {} }), null);
  assert.equal(assistantRunSupplyQualityFromDiagnostics(null), null);
});

test('buildAssistantRunProgress preserves create-response diagnostics and latest trail windows', () => {
  const response = {
    assistant_run_id: 'run-create-1',
    evidence_state: {
      status: 'supplied',
      supply_quality: {
        status: 'partial',
        suppliedItemCount: 2,
        citationLocatorCount: 1,
        fallbackChunkCount: 1,
        notes: ['raw evidence note should not render'],
      },
    },
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
        latest: {
          context_budget: {
            estimated_prompt_chars: 4096,
            max_prompt_chars: 12000,
            budget_pressure: 'normal',
            item_count: 6,
          },
          provider_shim_observability: {
            tool_output_budget: {
              trimmed_output_count: 1,
              preserved_evidence_ref_count: 3,
            },
            liveness_events: {
              event_count: 1,
              latest: {
                event_type: 'tool_call_liveness_stall',
                status: 'recovered',
                retry_count: 1,
                action: 'continue',
                has_note: true,
              },
            },
          },
          transport_policy: {
            requested_transport: 'codex_exec_schema',
            effective_transport: 'codex_plan_only',
            real_transport_requested: true,
            real_transport_feature_gate_enabled: false,
            real_transport_promotion_review_approved: false,
            downgraded: true,
            downgrade_reason: 'promotion_review_required',
            direct_execution_authoritative: true,
            codex_mutation_allowed: false,
            queue_allowed: false,
            manual_feature_gate_required_for_real_transport: true,
            promotion_review_required_for_real_transport: true,
            host_validation_required_for_real_transport: true,
            debug_token: 'debug-token-should-not-render',
          },
          model_gateway: {
            lane: 'codex_conversation',
            selected_model: {
              provider: 'minimax',
              model: 'assistant-codex-shadow',
            },
            profile_id: 'minimax-codex-shadow',
            wire_api: 'codex_compatible_shim',
            auth_configured: true,
            capability_manifest: {
              codex_compatible: true,
              json_mode: true,
              tool_calling: true,
            },
            codex_surface: {
              codex_compatible: true,
              json_actions_supported: true,
              tool_calls_supported: true,
              real_execution_block_reason: 'disabled_on_this_host',
            },
            codex_real_execution_allowed: false,
          },
        },
        model_gateway_gate: {
          status: 'ready',
          ready_for_promotion_review: true,
          profile_available: true,
        },
        host_validation_summary: {
          status: 'validated',
          completed_count: 1,
          failed_count: 0,
          guard_failed_count: 0,
          pending_count: 0,
          latest: {
            mode: 'codex_exec',
            host_kind: 'windows_jump',
            host_kind_allowed: true,
            profile_kind: 'codex-compatible-shim',
            workspace_configured: true,
            prompt_redacted: true,
            task_memory_isolated: true,
            task_memory_space_configured: true,
            validation_requirements_met: true,
            command_plan: { args_without_prompt: ['exec', 'raw prompt should not render'] },
          },
        },
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
      provider_usage: {
        summary: {
          request_count: 1,
          failed_request_count: 0,
          total_tokens: 42,
          last_request_id: 'resp_create_1',
        },
        recent_events: [
          {
            provider: 'openclaw',
            model: 'assistant-chat',
            request_id: 'resp_create_1',
            status: 'responded',
          },
        ],
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
  assert.equal(progress.providerUsage.requestCount, 1);
  assert.equal(progress.providerUsage.totalTokens, 42);
  assert.equal(progress.providerUsage.lastProvider, 'openclaw');
  assert.equal(progress.codexBudget.budgetPressure, 'normal');
  assert.equal(progress.codexBudget.trimmedOutputCount, 1);
  assert.equal(progress.codexBudget.preservedEvidenceRefCount, 3);
  assert.equal(progress.codexLiveness.eventType, 'tool_call_liveness_stall');
  assert.equal(progress.codexLiveness.status, 'recovered');
  assert.equal(progress.codexLiveness.retryCount, 1);
  assert.equal(progress.codexModelGateway.status, 'ready');
  assert.equal(progress.codexModelGateway.profileId, 'minimax-codex-shadow');
  assert.equal(progress.codexModelGateway.readyForPromotion, true);
  assert.equal(progress.codexHostValidation.status, 'validated');
  assert.equal(progress.codexHostValidation.latestHostKind, 'windows_jump');
  assert.equal(progress.codexHostValidation.validationRequirementsMet, true);
  assert.doesNotMatch(JSON.stringify(progress.codexHostValidation), /raw prompt/);
  assert.equal(progress.codexTransportPolicy.requestedTransport, 'codex_exec_schema');
  assert.equal(progress.codexTransportPolicy.effectiveTransport, 'codex_plan_only');
  assert.equal(progress.codexTransportPolicy.downgraded, true);
  assert.equal(progress.codexTransportPolicy.directExecutionAuthoritative, true);
  assert.doesNotMatch(JSON.stringify(progress.codexTransportPolicy), /debug-token/);
  assert.equal(progress.supplyQuality.status, 'partial');
  assert.equal(progress.supplyQuality.suppliedItemCount, 2);
  assert.equal(progress.supplyQuality.citationLocatorCount, 1);
  assert.equal(progress.supplyQuality.fallbackChunkCount, 1);
  assert.doesNotMatch(JSON.stringify(progress.supplyQuality), /raw evidence note/);
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
      diagnostics: {
        codex_executor: {
          promotion_gate: {
            status: 'blocked_by_host_validation',
            readiness_checks: {
              shadow_gate: { ready: true, status: 'stable' },
              host_validation: { ready: false, status: 'not_run' },
              model_gateway: { ready: true, status: 'ready' },
              all_ready: false,
            },
          },
          latest: {
            context_budget: {
              estimated_prompt_chars: 2400,
              budget_pressure: 'normal',
            },
          },
        },
        provider_usage: {
          recent_events: [{
            provider: 'codex-shim',
            model: 'assistant-run-continue',
            request_id: 'resp_continue_1',
            status: 'responded',
            total_tokens: 33,
          }],
        },
      },
    },
  }, true);

  assert.equal(progress.runId, 'run-continue-1');
  assert.equal(progress.continued, true);
  assert.equal(progress.steps[0].label, '继续执行');
  assert.equal(progress.traceSteps[0].actionType, 'web_search');
  assert.equal(progress.traceSteps[0].deniedCount, 1);
  assert.equal(progress.codexReadiness.status, 'blocked_by_host_validation');
  assert.equal(progress.codexBudget.budgetPressure, 'normal');
  assert.equal(progress.providerUsage.requestCount, 1);
  assert.equal(progress.providerUsage.totalTokens, 33);
  assert.equal(progress.codexLiveness, null);
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
