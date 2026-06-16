use contracts::{AssistantRunEventView, AssistantRunView};
use domain_model::{AssistantRun, AssistantRunEvent};

use crate::value_array;

pub(crate) fn to_assistant_run_view(run: AssistantRun) -> AssistantRunView {
    AssistantRunView {
        id: run.id,
        local_thread_id: run.local_thread_id,
        user_prompt: run.user_prompt,
        startup_briefing: run.startup_briefing,
        selected_scope: run.selected_scope,
        scope_candidates: value_array(run.scope_candidates),
        context_policy: run.context_policy,
        evidence_state: run.evidence_state,
        service_lane: run.service_lane,
        execution_trail: value_array(run.execution_trail),
        output_artifacts: value_array(run.output_artifacts),
        runtime: run.runtime_manifest,
        created_at: run.created_at,
        updated_at: run.updated_at,
    }
}

pub(crate) fn to_assistant_run_event_view(event: AssistantRunEvent) -> AssistantRunEventView {
    AssistantRunEventView {
        id: event.id,
        run_id: event.run_id,
        sequence_no: event.sequence_no,
        event_name: event.event_name,
        payload: event.payload,
        created_at: event.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};
    use serde_json::json;

    #[test]
    fn assistant_run_view_preserves_arrays_and_runtime_manifest() {
        let now = Utc::now();
        let run_id = AssistantRunId::new();
        let run = AssistantRun {
            id: run_id,
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("local-thread-1".to_string()),
            user_prompt: "生成经营报告".to_string(),
            startup_briefing: json!({"source": "test"}),
            selected_scope: json!({"intent": "ordinary_chat"}),
            scope_candidates: json!([{"dataset": "a"}, {"dataset": "b"}]),
            context_policy: json!({"max_items": 12}),
            evidence_state: json!({"status": "ready"}),
            service_lane: "ordinary_chat".to_string(),
            execution_trail: json!([{"step": "retrieve"}, {"step": "answer"}]),
            output_artifacts: json!([{"type": "static_page"}]),
            runtime_manifest: json!({"provider": "minimax", "model": "m3"}),
            created_at: now,
            updated_at: now,
        };

        let view = to_assistant_run_view(run);

        assert_eq!(view.id, run_id);
        assert_eq!(view.local_thread_id.as_deref(), Some("local-thread-1"));
        assert_eq!(view.user_prompt, "生成经营报告");
        assert_eq!(view.scope_candidates.len(), 2);
        assert_eq!(view.execution_trail.len(), 2);
        assert_eq!(view.output_artifacts.len(), 1);
        assert_eq!(view.runtime, json!({"provider": "minimax", "model": "m3"}));
        assert_eq!(view.created_at, now);
        assert_eq!(view.updated_at, now);
    }

    #[test]
    fn assistant_run_view_turns_non_array_fields_into_empty_vectors() {
        let now = Utc::now();
        let run = AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: None,
            user_prompt: "检查诊断".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({}),
            scope_candidates: json!({"not": "array"}),
            context_policy: json!({}),
            evidence_state: json!({}),
            service_lane: "ordinary_chat".to_string(),
            execution_trail: json!(null),
            output_artifacts: json!("not-array"),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        };

        let view = to_assistant_run_view(run);

        assert!(view.scope_candidates.is_empty());
        assert!(view.execution_trail.is_empty());
        assert!(view.output_artifacts.is_empty());
    }

    #[test]
    fn assistant_run_event_view_preserves_event_fields() {
        let now = Utc::now();
        let event_id = AssistantRunEventId::new();
        let run_id = AssistantRunId::new();
        let event = AssistantRunEvent {
            id: event_id,
            tenant_id: TenantId::new(),
            run_id,
            sequence_no: 3,
            event_name: "assistant_run.completed".to_string(),
            payload: json!({"runtime": {"provider": "minimax"}}),
            created_at: now,
        };

        let view = to_assistant_run_event_view(event);

        assert_eq!(view.id, event_id);
        assert_eq!(view.run_id, run_id);
        assert_eq!(view.sequence_no, 3);
        assert_eq!(view.event_name, "assistant_run.completed");
        assert_eq!(view.payload, json!({"runtime": {"provider": "minimax"}}));
        assert_eq!(view.created_at, now);
    }
}
