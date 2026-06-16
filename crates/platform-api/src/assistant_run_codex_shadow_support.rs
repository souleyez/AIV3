use assistant_runtime::CodexConversationExecutorOutput;
use serde_json::{json, Value};

use crate::assistant_run_scope_policy_support::assistant_run_scope_intent;
use crate::{assistant_run_is_static_page_artifact, AssistantRunReactEvent};

pub(crate) fn assistant_run_codex_shadow_comparison(
    output: &CodexConversationExecutorOutput,
    direct_react_enabled: bool,
    direct_runtime_manifest: &Value,
    direct_output_artifacts: &[Value],
    direct_react_events: &[AssistantRunReactEvent],
    selected_scope: &Value,
    current_artifact: Option<&Value>,
) -> Value {
    let artifact_types: Vec<Value> = direct_output_artifacts
        .iter()
        .filter_map(|artifact| artifact.get("type").and_then(Value::as_str))
        .map(|value| json!(value))
        .collect();
    let react_event_names: Vec<Value> = direct_react_events
        .iter()
        .map(|event| json!(event.event_name.as_str()))
        .collect();
    let direct_action_types =
        assistant_run_codex_direct_action_types(direct_output_artifacts, direct_react_events);
    let selected_intent = assistant_run_scope_intent(selected_scope);
    let static_page_context = selected_intent == "static_page"
        || current_artifact
            .map(assistant_run_is_static_page_artifact)
            .unwrap_or(false)
        || direct_output_artifacts.iter().any(|artifact| {
            artifact
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| {
                    matches!(
                        kind,
                        "static_page_draft" | "static_page_image_job" | "static_page_render_output"
                    )
                })
        });
    let suggested_action_type = output
        .suggested_action
        .as_ref()
        .and_then(|action| {
            action
                .get("action_type")
                .or_else(|| action.get("actionType"))
                .or_else(|| action.get("type"))
        })
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let suggested_action_allowed = suggested_action_type.as_ref().is_none_or(|action_type| {
        output
            .planned_action_types
            .iter()
            .any(|planned| planned == action_type)
    });
    let comparison_status = assistant_run_codex_shadow_comparison_status(
        &direct_action_types,
        suggested_action_type.as_deref(),
        suggested_action_allowed,
    );
    let direct_action_values: Vec<Value> = direct_action_types
        .iter()
        .map(|value| json!(value))
        .collect();

    json!({
        "mode": "shadow_comparison",
        "authoritative_executor": "direct",
        "codex_mutation_allowed": false,
        "codex_queue_allowed": false,
        "direct_state_authoritative": true,
        "static_page_flow_preserved": true,
        "static_page_context": static_page_context,
        "direct": {
            "react_enabled": direct_react_enabled,
            "runtime_mode": direct_runtime_manifest.get("mode").cloned().unwrap_or(Value::Null),
            "provider": direct_runtime_manifest.get("provider").cloned().unwrap_or(Value::Null),
            "model": direct_runtime_manifest.get("model").cloned().unwrap_or(Value::Null),
            "artifact_types": artifact_types,
            "react_event_names": react_event_names,
            "action_types": direct_action_values,
            "selected_intent": selected_intent,
        },
        "codex": {
            "transport": output.transport.as_str(),
            "status": output.status.as_str(),
            "codex_invoked": output.codex_invoked,
            "fallback_to_direct": output.fallback_to_direct,
            "planned_action_types": output.planned_action_types.clone(),
            "suggested_action_type": suggested_action_type,
            "suggested_action_allowed": suggested_action_allowed,
            "model_gateway": output.model_gateway.clone(),
        },
        "comparison": {
            "status": comparison_status,
            "codex_has_suggestion": output.suggested_action.is_some(),
            "actionable": false,
            "equivalence_scored": output.suggested_action.is_some(),
            "reason": if suggested_action_allowed {
                "shadow mode only; direct execution remains authoritative"
            } else {
                "codex suggested action is not in DataMax-provided action contracts"
            },
            "next_gate": "enable Codex mutation only after repeated matched shadow runs",
        }
    })
}

fn assistant_run_codex_direct_action_types(
    direct_output_artifacts: &[Value],
    direct_react_events: &[AssistantRunReactEvent],
) -> Vec<String> {
    let mut actions = Vec::new();
    for artifact in direct_output_artifacts {
        let Some(kind) = artifact.get("type").and_then(Value::as_str) else {
            continue;
        };
        let action = match kind {
            "static_page_draft" => Some("create_static_page_draft"),
            "static_page_image_job" => Some("submit_static_page_image_preview"),
            "static_page_render_output" => Some("render_static_page"),
            "assistant_message" => Some("final_answer"),
            "report_draft" => Some("create_report_draft"),
            _ => None,
        };
        if let Some(action) = action {
            push_unique_codex_action(&mut actions, action);
        }
    }
    for event in direct_react_events {
        if let Some(action) = assistant_run_action_type_from_event_name(&event.event_name) {
            push_unique_codex_action(&mut actions, action);
        }
    }
    actions
}

fn assistant_run_action_type_from_event_name(event_name: &str) -> Option<&'static str> {
    if event_name.contains("retrieve_evidence") {
        Some("retrieve_evidence")
    } else if event_name.contains("read_document_detail") {
        Some("read_document_detail")
    } else if event_name.contains("recall_conversation_memory") {
        Some("recall_conversation_memory")
    } else if event_name.contains("create_static_page_draft") {
        Some("create_static_page_draft")
    } else if event_name.contains("update_static_page_module") {
        Some("update_static_page_module")
    } else if event_name.contains("submit_static_page_image_preview") {
        Some("submit_static_page_image_preview")
    } else if event_name.contains("render_static_page") {
        Some("render_static_page")
    } else if event_name.contains("static_page_revision_publish")
        || event_name.contains("publish_static_page_revision")
    {
        Some("publish_static_page_revision")
    } else if event_name.contains("create_report_draft") {
        Some("create_report_draft")
    } else if event_name.contains("report_choice") {
        Some("report_choice")
    } else if event_name.contains("codex_host_task") {
        Some("codex_host_task")
    } else {
        None
    }
}

fn assistant_run_codex_shadow_comparison_status(
    direct_action_types: &[String],
    suggested_action_type: Option<&str>,
    suggested_action_allowed: bool,
) -> &'static str {
    let Some(suggested_action_type) = suggested_action_type else {
        return "no_codex_suggestion";
    };
    if !suggested_action_allowed {
        return "invalid_suggestion";
    }
    if direct_action_types
        .iter()
        .any(|action_type| action_type == suggested_action_type)
    {
        "matched"
    } else if direct_action_types.is_empty() {
        "codex_only"
    } else {
        "diverged"
    }
}

fn push_unique_codex_action(items: &mut Vec<String>, item: &str) {
    let item = item.trim();
    if item.is_empty() {
        return;
    }
    if !items.iter().any(|existing| existing == item) {
        items.push(item.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_runtime::CodexConversationExecutorStatus;
    use contracts::{AssistantRunCodexContextBudgetView, AssistantRunExecutorTransportView};

    fn codex_output(suggested_action: Option<Value>) -> CodexConversationExecutorOutput {
        CodexConversationExecutorOutput {
            transport: AssistantRunExecutorTransportView::CodexPlanOnly,
            status: CodexConversationExecutorStatus::PlanOnly,
            codex_invoked: false,
            fallback_to_direct: true,
            assistant_message: None,
            suggested_action,
            planned_action_types: vec![
                "create_static_page_draft".to_string(),
                "render_static_page".to_string(),
            ],
            execution_trail: Vec::new(),
            context_budget: AssistantRunCodexContextBudgetView::default(),
            model_gateway: json!({"lane": "codex_conversation"}),
            output_schema: None,
            host_invocation: None,
        }
    }

    #[test]
    fn assistant_run_codex_shadow_support_maps_direct_actions_without_duplicates() {
        let actions = assistant_run_codex_direct_action_types(
            &[
                json!({"type": "static_page_draft"}),
                json!({"type": "static_page_draft"}),
                json!({"type": "assistant_message"}),
            ],
            &[
                AssistantRunReactEvent {
                    event_name: "retrieve_evidence.completed".to_string(),
                    payload: json!({}),
                },
                AssistantRunReactEvent {
                    event_name: "static_page_revision_publish.completed".to_string(),
                    payload: json!({}),
                },
            ],
        );

        assert_eq!(
            actions,
            vec![
                "create_static_page_draft".to_string(),
                "final_answer".to_string(),
                "retrieve_evidence".to_string(),
                "publish_static_page_revision".to_string()
            ]
        );
    }

    #[test]
    fn assistant_run_codex_shadow_support_reports_matched_and_invalid_suggestions() {
        let matched = assistant_run_codex_shadow_comparison(
            &codex_output(Some(json!({"action_type": "create_static_page_draft"}))),
            true,
            &json!({"mode": "provider", "provider": "placeholder"}),
            &[json!({"type": "static_page_draft"})],
            &[],
            &json!({"intent": "static_page"}),
            None,
        );

        assert_eq!(matched["comparison"]["status"], json!("matched"));
        assert_eq!(
            matched["direct"]["action_types"],
            json!(["create_static_page_draft"])
        );
        assert_eq!(matched["static_page_context"], json!(true));
        assert_eq!(matched["codex_mutation_allowed"], json!(false));

        let invalid = assistant_run_codex_shadow_comparison(
            &codex_output(Some(json!({"actionType": "unsafe_os_command"}))),
            true,
            &json!({"mode": "provider"}),
            &[json!({"type": "static_page_draft"})],
            &[],
            &json!({"intent": "static_page"}),
            None,
        );

        assert_eq!(invalid["comparison"]["status"], json!("invalid_suggestion"));
        assert_eq!(invalid["codex"]["suggested_action_allowed"], json!(false));
        assert_eq!(
            invalid["comparison"]["reason"],
            json!("codex suggested action is not in DataMax-provided action contracts")
        );
    }
}
