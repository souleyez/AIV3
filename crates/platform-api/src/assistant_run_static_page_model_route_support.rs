use domain_model::AssistantRun;
use llm_gateway::{resolve_runtime_selection_from_env, MODEL_LANE_ASSISTANT_CHAT};
use serde_json::Value;

use crate::DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL;

fn assistant_run_model_is_gpt_55(model: &str) -> bool {
    let normalized = model
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect::<String>();
    normalized.contains("gpt55")
}

fn assistant_run_primary_model_names(run: &AssistantRun) -> Vec<&str> {
    [
        run.runtime_manifest.pointer("/model"),
        run.runtime_manifest.pointer("/runtime/model"),
        run.runtime_manifest.pointer("/selected_model/model"),
        run.runtime_manifest
            .pointer("/model_gateway/selected_model/model"),
        run.runtime_manifest
            .pointer("/runtime/selected_model/model"),
        run.runtime_manifest
            .pointer("/runtime/model_gateway/selected_model/model"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .collect()
}

pub(crate) fn assistant_run_uses_gpt_55_primary_model(run: &AssistantRun) -> bool {
    assistant_run_primary_model_names(run)
        .into_iter()
        .any(assistant_run_model_is_gpt_55)
}

fn assistant_run_primary_model_unresolved_for_static_page(run: &AssistantRun) -> bool {
    let names = assistant_run_primary_model_names(run);
    names.is_empty()
        || names.iter().all(|model| {
            let normalized = model.trim().to_ascii_lowercase();
            normalized.is_empty()
                || normalized.contains("pending")
                || normalized.contains("unknown")
                || normalized.contains("unresolved")
        })
}

fn assistant_chat_runtime_uses_gpt_55_primary_model() -> bool {
    let runtime = resolve_runtime_selection_from_env(
        "ASSISTANT_RUN",
        MODEL_LANE_ASSISTANT_CHAT,
        DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL,
    );
    assistant_run_model_is_gpt_55(&runtime.model)
}

pub(crate) fn assistant_run_static_page_should_use_gpt_55_local_route(run: &AssistantRun) -> bool {
    assistant_run_uses_gpt_55_primary_model(run)
        || (run.service_lane == "external_channel"
            && assistant_run_primary_model_unresolved_for_static_page(run)
            && assistant_chat_runtime_uses_gpt_55_primary_model())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, TenantId};
    use serde_json::json;

    fn assistant_run_with_runtime_manifest(runtime_manifest: Value) -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: None,
            user_prompt: "生成静态页".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({}),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state: json!({}),
            service_lane: "ordinary_chat".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn gpt_55_model_detection_normalizes_provider_model_names() {
        assert!(assistant_run_model_is_gpt_55("GPT-5.5"));
        assert!(assistant_run_model_is_gpt_55("openai/gpt-5.5-high"));
        assert!(assistant_run_model_is_gpt_55("gpt_55"));
        assert!(!assistant_run_model_is_gpt_55("MiniMax-M3"));
    }

    #[test]
    fn primary_model_names_read_known_runtime_manifest_paths() {
        let run = assistant_run_with_runtime_manifest(json!({
            "runtime": {
                "model_gateway": {
                    "selected_model": {
                        "model": " openai/gpt-5.5 "
                    }
                }
            }
        }));

        assert_eq!(
            assistant_run_primary_model_names(&run),
            vec!["openai/gpt-5.5"]
        );
        assert!(assistant_run_uses_gpt_55_primary_model(&run));
    }

    #[test]
    fn unresolved_primary_models_are_limited_to_pending_unknown_or_empty() {
        let unresolved = assistant_run_with_runtime_manifest(json!({
            "model": "pending-assistant-run-executor",
            "runtime": {
                "model": "unknown"
            }
        }));
        assert!(assistant_run_primary_model_unresolved_for_static_page(
            &unresolved
        ));

        let resolved = assistant_run_with_runtime_manifest(json!({
            "model": "MiniMax-M3"
        }));
        assert!(!assistant_run_primary_model_unresolved_for_static_page(
            &resolved
        ));
    }
}
