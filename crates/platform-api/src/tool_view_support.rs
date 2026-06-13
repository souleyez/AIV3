use contracts::{ToolDefinitionView, ToolExecutionView};
use domain_model::{ToolExecution, ToolExecutionSourceKind, ToolExecutionStatus};
use serde_json::Value;
use tool_registry::{
    bootstrap_default_tool_registry, ToolCliOutputMode, ToolDefinition, ToolInvocationMode,
};

fn parse_manifest_tool_call_status(value: &str) -> Option<contracts::ManifestToolCallStatusView> {
    match value {
        "requested" => Some(contracts::ManifestToolCallStatusView::Requested),
        "completed" => Some(contracts::ManifestToolCallStatusView::Completed),
        "failed" => Some(contracts::ManifestToolCallStatusView::Failed),
        _ => None,
    }
}

fn to_tool_reference_view(tool: &ToolDefinition) -> contracts::ToolReferenceView {
    contracts::ToolReferenceView {
        key: tool.key.clone(),
        title: tool.title.clone(),
        scope_policy: tool.scope_policy.clone(),
        invocation_mode: match tool.invocation_mode {
            ToolInvocationMode::Cli => contracts::ToolInvocationModeView::Cli,
            ToolInvocationMode::Internal => contracts::ToolInvocationModeView::Internal,
        },
        cli: tool.cli.as_ref().map(|cli| contracts::ToolCliContractView {
            argv: cli.argv.clone(),
            env_allowlist: cli.env_allowlist.clone(),
            output_mode: match cli.output_mode {
                ToolCliOutputMode::Json => contracts::ToolCliOutputModeView::Json,
                ToolCliOutputMode::Text => contracts::ToolCliOutputModeView::Text,
            },
            timeout_ms: cli.timeout_ms,
        }),
    }
}

fn find_registered_tool_reference(tool_name: &str) -> Option<contracts::ToolReferenceView> {
    let registry = bootstrap_default_tool_registry();
    registry.get(tool_name).map(to_tool_reference_view)
}

fn parse_tool_invocation_mode(value: &str) -> Option<contracts::ToolInvocationModeView> {
    match value {
        "cli" => Some(contracts::ToolInvocationModeView::Cli),
        "internal" => Some(contracts::ToolInvocationModeView::Internal),
        _ => None,
    }
}

fn parse_tool_cli_output_mode(value: &str) -> Option<contracts::ToolCliOutputModeView> {
    match value {
        "json" => Some(contracts::ToolCliOutputModeView::Json),
        "text" => Some(contracts::ToolCliOutputModeView::Text),
        _ => None,
    }
}

fn parse_tool_cli_contract(value: &Value) -> Option<contracts::ToolCliContractView> {
    let object = value.as_object()?;
    Some(contracts::ToolCliContractView {
        argv: object
            .get("argv")?
            .as_array()?
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        env_allowlist: object
            .get("env_allowlist")?
            .as_array()?
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        output_mode: parse_tool_cli_output_mode(object.get("output_mode")?.as_str()?)?,
        timeout_ms: object.get("timeout_ms").and_then(Value::as_u64),
    })
}

fn parse_tool_reference(value: &Value) -> Option<contracts::ToolReferenceView> {
    let object = value.as_object()?;
    Some(contracts::ToolReferenceView {
        key: object.get("key")?.as_str()?.to_string(),
        title: object.get("title")?.as_str()?.to_string(),
        scope_policy: object.get("scope_policy")?.as_str()?.to_string(),
        invocation_mode: parse_tool_invocation_mode(object.get("invocation_mode")?.as_str()?)?,
        cli: object.get("cli").and_then(parse_tool_cli_contract),
    })
}

fn parse_manifest_tool_call(value: &Value) -> Option<contracts::ManifestToolCallView> {
    match value {
        Value::String(tool_name) => Some(contracts::ManifestToolCallView {
            call_id: None,
            tool_name: tool_name.clone(),
            tool: find_registered_tool_reference(tool_name),
            status: contracts::ManifestToolCallStatusView::Completed,
            arguments: None,
            result: None,
        }),
        Value::Object(object) => Some(contracts::ManifestToolCallView {
            call_id: object
                .get("call_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            tool_name: object.get("tool_name")?.as_str()?.to_string(),
            tool: object
                .get("tool")
                .and_then(parse_tool_reference)
                .or_else(|| find_registered_tool_reference(object.get("tool_name")?.as_str()?)),
            status: parse_manifest_tool_call_status(object.get("status")?.as_str()?)?,
            arguments: object.get("arguments").cloned(),
            result: object.get("result").cloned(),
        }),
        _ => None,
    }
}

pub(crate) fn parse_manifest_tool_trace(
    value: Option<&Value>,
) -> Option<Vec<contracts::ManifestToolCallView>> {
    let trace = match value {
        Some(Value::Array(entries)) => entries
            .iter()
            .map(parse_manifest_tool_call)
            .collect::<Option<Vec<_>>>()?,
        Some(_) => return None,
        None => Vec::new(),
    };

    Some(trace)
}

pub(crate) fn to_tool_definition_view(tool: &ToolDefinition) -> ToolDefinitionView {
    ToolDefinitionView {
        key: tool.key.clone(),
        title: tool.title.clone(),
        scope_policy: tool.scope_policy.clone(),
        input_schema: tool.input_schema.clone(),
        output_schema: tool.output_schema.clone(),
        invocation_mode: match tool.invocation_mode {
            ToolInvocationMode::Cli => contracts::ToolInvocationModeView::Cli,
            ToolInvocationMode::Internal => contracts::ToolInvocationModeView::Internal,
        },
        cli: tool.cli.as_ref().map(|cli| contracts::ToolCliContractView {
            argv: cli.argv.clone(),
            env_allowlist: cli.env_allowlist.clone(),
            output_mode: match cli.output_mode {
                ToolCliOutputMode::Json => contracts::ToolCliOutputModeView::Json,
                ToolCliOutputMode::Text => contracts::ToolCliOutputModeView::Text,
            },
            timeout_ms: cli.timeout_ms,
        }),
    }
}

pub(crate) fn to_tool_execution_view(tool_execution: ToolExecution) -> ToolExecutionView {
    ToolExecutionView {
        id: tool_execution.id,
        execution_id: tool_execution.execution_id,
        source_kind: match tool_execution.source_kind {
            ToolExecutionSourceKind::DatasetOutput => {
                contracts::ToolExecutionSourceKindView::DatasetOutput
            }
            ToolExecutionSourceKind::ChatMessage => {
                contracts::ToolExecutionSourceKindView::ChatMessage
            }
            ToolExecutionSourceKind::WorkflowExecution => {
                contracts::ToolExecutionSourceKindView::WorkflowExecution
            }
        },
        dataset_output_id: tool_execution.dataset_output_id,
        chat_message_id: tool_execution.chat_message_id,
        sequence_no: tool_execution.sequence_no,
        call_id: tool_execution.call_id,
        tool_name: tool_execution.tool_name,
        tool: tool_execution
            .tool_snapshot
            .as_ref()
            .and_then(parse_tool_reference),
        status: match tool_execution.status {
            ToolExecutionStatus::Requested => contracts::ManifestToolCallStatusView::Requested,
            ToolExecutionStatus::Completed => contracts::ManifestToolCallStatusView::Completed,
            ToolExecutionStatus::Failed => contracts::ManifestToolCallStatusView::Failed,
        },
        arguments: tool_execution.arguments,
        result: tool_execution.result,
        created_at: tool_execution.created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{ChatMessageId, TenantId, ToolExecutionId, WorkflowExecutionId};
    use serde_json::json;
    use tool_registry::ToolCliContract;

    use super::*;

    #[test]
    fn tool_definition_view_preserves_cli_contract() {
        let tool = ToolDefinition::cli(
            "weather.lookup",
            "Weather Lookup",
            "session",
            json!({ "type": "object" }),
            json!({ "type": "object" }),
            ToolCliContract {
                argv: vec!["cargo".to_string(), "run".to_string()],
                env_allowlist: vec!["WEATHER_API_KEY".to_string()],
                output_mode: ToolCliOutputMode::Json,
                timeout_ms: Some(10_000),
            },
        );

        let view = to_tool_definition_view(&tool);

        assert_eq!(view.key, "weather.lookup");
        assert_eq!(view.invocation_mode, contracts::ToolInvocationModeView::Cli);
        assert_eq!(
            view.cli.as_ref().map(|cli| cli.output_mode.clone()),
            Some(contracts::ToolCliOutputModeView::Json)
        );
        assert_eq!(
            view.cli.as_ref().map(|cli| cli.timeout_ms),
            Some(Some(10_000))
        );
    }

    #[test]
    fn manifest_tool_trace_expands_registered_and_inline_references() {
        let trace = parse_manifest_tool_trace(Some(&json!([
            "retrieval.search",
            {
                "call_id": "call_weather",
                "tool_name": "weather.lookup",
                "tool": {
                    "key": "weather.lookup",
                    "title": "Weather Lookup",
                    "scope_policy": "session",
                    "invocation_mode": "cli",
                    "cli": {
                        "argv": ["cargo", "run"],
                        "env_allowlist": ["WEATHER_API_KEY"],
                        "output_mode": "json",
                        "timeout_ms": 15000
                    }
                },
                "status": "failed",
                "arguments": { "city": "Shanghai" },
                "result": { "error": "timeout" }
            }
        ])))
        .expect("tool trace should parse");

        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].tool_name, "retrieval.search");
        assert_eq!(
            trace[0].status,
            contracts::ManifestToolCallStatusView::Completed
        );
        assert_eq!(
            trace[0].tool.as_ref().map(|tool| tool.key.as_str()),
            Some("retrieval.search")
        );
        assert_eq!(trace[1].call_id.as_deref(), Some("call_weather"));
        assert_eq!(trace[1].tool_name, "weather.lookup");
        assert_eq!(
            trace[1].status,
            contracts::ManifestToolCallStatusView::Failed
        );
        assert_eq!(
            trace[1]
                .tool
                .as_ref()
                .and_then(|tool| tool.cli.as_ref())
                .map(|cli| cli.output_mode.clone()),
            Some(contracts::ToolCliOutputModeView::Json)
        );
    }

    #[test]
    fn tool_execution_view_preserves_snapshot_and_source_kind() {
        let tool_execution = ToolExecution {
            id: ToolExecutionId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: ToolExecutionSourceKind::ChatMessage,
            dataset_output_id: None,
            chat_message_id: Some(ChatMessageId::new()),
            sequence_no: 0,
            call_id: Some("call_weather".to_string()),
            tool_name: "weather.lookup".to_string(),
            tool_snapshot: Some(json!({
                "key": "weather.lookup",
                "title": "Weather Lookup",
                "scope_policy": "session",
                "invocation_mode": "cli",
                "cli": {
                    "argv": ["cargo", "run"],
                    "env_allowlist": ["WEATHER_API_KEY"],
                    "output_mode": "json",
                    "timeout_ms": 15000
                }
            })),
            status: ToolExecutionStatus::Completed,
            arguments: Some(json!({ "city": "Shanghai" })),
            result: Some(json!({ "summary": "sunny" })),
            created_at: Utc::now(),
        };

        let view = to_tool_execution_view(tool_execution);

        assert_eq!(
            view.source_kind,
            contracts::ToolExecutionSourceKindView::ChatMessage
        );
        assert_eq!(view.call_id.as_deref(), Some("call_weather"));
        assert_eq!(view.tool_name, "weather.lookup");
        assert_eq!(
            view.status,
            contracts::ManifestToolCallStatusView::Completed
        );
        assert_eq!(
            view.tool.as_ref().map(|tool| tool.key.as_str()),
            Some("weather.lookup")
        );
    }
}
