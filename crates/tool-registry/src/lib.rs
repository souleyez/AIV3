use llm_gateway::LlmToolCall;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolInvocationMode {
    Cli,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolCliOutputMode {
    Json,
    Text,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCliContract {
    pub argv: Vec<String>,
    pub env_allowlist: Vec<String>,
    pub output_mode: ToolCliOutputMode,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub key: String,
    pub title: String,
    pub scope_policy: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub invocation_mode: ToolInvocationMode,
    pub cli: Option<ToolCliContract>,
}

impl ToolDefinition {
    pub fn cli(
        key: impl Into<String>,
        title: impl Into<String>,
        scope_policy: impl Into<String>,
        input_schema: Value,
        output_schema: Value,
        cli: ToolCliContract,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            scope_policy: scope_policy.into(),
            input_schema,
            output_schema,
            invocation_mode: ToolInvocationMode::Cli,
            cli: Some(cli),
        }
    }

    pub fn internal(
        key: impl Into<String>,
        title: impl Into<String>,
        scope_policy: impl Into<String>,
        input_schema: Value,
        output_schema: Value,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            scope_policy: scope_policy.into(),
            input_schema,
            output_schema,
            invocation_mode: ToolInvocationMode::Internal,
            cli: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct InMemoryToolRegistry {
    tools: BTreeMap<String, ToolDefinition>,
}

impl InMemoryToolRegistry {
    pub fn register(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.key.clone(), tool);
    }

    pub fn get(&self, key: &str) -> Option<&ToolDefinition> {
        self.tools.get(key)
    }

    pub fn list(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }

    pub fn list_cli(&self) -> Vec<&ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| tool.invocation_mode == ToolInvocationMode::Cli)
            .collect()
    }
}

pub fn bootstrap_default_tool_registry() -> InMemoryToolRegistry {
    let mut registry = InMemoryToolRegistry::default();
    registry.register(ToolDefinition::cli(
        "retrieval.search",
        "Retrieval Search",
        "dataset_or_session",
        serde_json::json!({
            "type": "object",
            "properties": {
                "dataset_id": { "type": "string" },
                "query": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 }
            },
            "required": ["query"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "hits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "retrieval_evidence_id": { "type": "string" },
                            "score": { "type": "number" },
                            "summary": { "type": "string" }
                        },
                        "required": ["retrieval_evidence_id", "score"]
                    }
                }
            },
            "required": ["hits"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "retrieval-cli".to_string(),
                "--".to_string(),
                "search".to_string(),
            ],
            env_allowlist: vec!["PLATFORM_DATABASE_URL".to_string()],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "weather.lookup",
        "Weather Lookup",
        "session",
        serde_json::json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "required": ["city"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string" }
            },
            "required": ["summary"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "weather-tool".to_string(),
            ],
            env_allowlist: vec!["WEATHER_API_KEY".to_string()],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(15_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "runtime.inspect",
        "Workflow Runtime Inspect",
        "execution",
        serde_json::json!({
            "type": "object",
            "properties": {
                "execution_id": { "type": "string" }
            },
            "required": ["execution_id"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "execution": { "type": "object" },
                "model_facing": { "type": ["object", "null"] },
                "dataset_output": { "type": ["object", "null"] },
                "chat_session": { "type": ["object", "null"] },
                "chat_messages": { "type": "array" },
                "llm_invocations": { "type": "array" },
                "tool_executions": { "type": "array" },
                "pretty_summaries": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": [
                "execution",
                "model_facing",
                "dataset_output",
                "chat_session",
                "chat_messages",
                "llm_invocations",
                "tool_executions",
                "pretty_summaries"
            ]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "runtime-inspect-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec!["PLATFORM_DATABASE_URL".to_string()],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "workflow.retry",
        "Workflow Retry",
        "execution",
        serde_json::json!({
            "type": "object",
            "properties": {
                "execution_id": { "type": "string" },
                "reason": { "type": "string" }
            },
            "required": ["execution_id", "reason"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "retry_transition": { "type": "object" },
                "restart_transition": { "type": ["object", "null"] }
            },
            "required": ["retry_transition", "restart_transition"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "workflow-retry-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "report.plan",
        "Report Plan Continue",
        "report_plan",
        serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": { "type": "string" }
            },
            "required": ["plan_id"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "plan": { "type": "object" },
                "workflow_execution": { "type": "object" }
            },
            "required": ["plan", "workflow_execution"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "report-plan-continue-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "report.publish",
        "Report Publish",
        "report_plan",
        serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": { "type": "string" },
                "surface": {
                    "type": "string",
                    "enum": ["pc", "mobile"]
                },
                "publish_note": { "type": ["string", "null"] }
            },
            "required": ["plan_id", "surface"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "report": { "type": "object" },
                "version": { "type": "object" },
                "source_render_output": { "type": "object" }
            },
            "required": ["report", "version", "source_render_output"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "report-publish-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "report.read_published",
        "Report Read Published",
        "report_plan",
        serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": { "type": "string" }
            },
            "required": ["plan_id"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "report": { "type": "object" },
                "current_version": { "type": ["object", "null"] },
                "versions": {
                    "type": "array",
                    "items": { "type": "object" }
                }
            },
            "required": ["report", "current_version", "versions"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "report-read-published-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "document.read_detail",
        "Document Read Detail",
        "document",
        serde_json::json!({
            "type": "object",
            "properties": {
                "document_id": { "type": "string" }
            },
            "required": ["document_id"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "document": { "type": "object" },
                "chunks": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "retrieval_evidences": {
                    "type": "array",
                    "items": { "type": "object" }
                }
            },
            "required": ["document", "chunks", "retrieval_evidences"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "document-detail-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "document.compare",
        "Document Compare",
        "document",
        serde_json::json!({
            "type": "object",
            "properties": {
                "document_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 2
                }
            },
            "required": ["document_ids"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "documents": {
                    "type": "array",
                    "items": { "type": "object" }
                }
            },
            "required": ["documents"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "document-compare-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "memory_directory.refresh",
        "Memory Directory Refresh",
        "dataset",
        serde_json::json!({
            "type": "object",
            "properties": {
                "dataset_id": { "type": "string" }
            },
            "required": ["dataset_id"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "workflow_execution": { "type": "object" }
            },
            "required": ["workflow_execution"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "memory-directory-refresh-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "chat_session.report_entry",
        "Chat Session Report Entry",
        "session",
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
                "action": {
                    "type": "string",
                    "enum": [
                        "request_confirmation",
                        "stay_material_service",
                        "enter_report_service"
                    ]
                },
                "title": { "type": "string" },
                "objective": { "type": "string" }
            },
            "required": ["session_id", "action"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "chat_session": { "type": "object" },
                "report_plan": { "type": ["object", "null"] },
                "workflow_execution": { "type": ["object", "null"] }
            },
            "required": ["chat_session", "report_plan", "workflow_execution"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "chat-session-report-entry-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry.register(ToolDefinition::cli(
        "report.render",
        "Report Render",
        "report_plan",
        serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": { "type": "string" },
                "surface": {
                    "type": "string",
                    "enum": ["pc", "mobile"]
                }
            },
            "required": ["plan_id", "surface"]
        }),
        serde_json::json!({
            "type": "object",
            "properties": {
                "workflow_execution": { "type": "object" },
                "requested_ast_version_id": { "type": "string" },
                "surface": {
                    "type": "string",
                    "enum": ["pc", "mobile"]
                }
            },
            "required": ["workflow_execution", "requested_ast_version_id", "surface"]
        }),
        ToolCliContract {
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "platform-api".to_string(),
                "--bin".to_string(),
                "report-render-cli".to_string(),
                "--".to_string(),
            ],
            env_allowlist: vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ],
            output_mode: ToolCliOutputMode::Json,
            timeout_ms: Some(30_000),
        },
    ));
    registry
}

pub fn find_default_tool(key: &str) -> Option<ToolDefinition> {
    bootstrap_default_tool_registry().get(key).cloned()
}

pub fn find_default_tool_snapshot_value(key: &str) -> Option<Value> {
    find_default_tool(key).map(tool_definition_snapshot_value)
}

pub fn render_tool_trace_manifest(tool_calls: &[LlmToolCall]) -> Value {
    Value::Array(
        tool_calls
            .iter()
            .map(|tool_call| {
                let mut value =
                    serde_json::to_value(tool_call).expect("tool call should serialize");
                if let Some(object) = value.as_object_mut() {
                    let tool_snapshot = find_default_tool_snapshot_value(&tool_call.tool_name)
                        .unwrap_or(Value::Null);
                    object.insert("tool".to_string(), tool_snapshot);
                }
                value
            })
            .collect(),
    )
}

fn tool_definition_snapshot_value(tool: ToolDefinition) -> Value {
    serde_json::json!({
        "key": tool.key,
        "title": tool.title,
        "scope_policy": tool.scope_policy,
        "invocation_mode": match tool.invocation_mode {
            ToolInvocationMode::Cli => "cli",
            ToolInvocationMode::Internal => "internal",
        },
        "cli": tool.cli.map(|cli| serde_json::json!({
            "argv": cli.argv,
            "env_allowlist": cli.env_allowlist,
            "output_mode": match cli.output_mode {
                ToolCliOutputMode::Json => "json",
                ToolCliOutputMode::Text => "text",
            },
            "timeout_ms": cli.timeout_ms,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_gateway::LlmToolCallStatus;
    use serde_json::json;

    #[test]
    fn registry_keeps_cli_contract_metadata() {
        let mut registry = InMemoryToolRegistry::default();
        let cli_tool = ToolDefinition::cli(
            "weather.lookup",
            "Weather Lookup",
            "dataset_or_session",
            json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }),
            json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
            ToolCliContract {
                argv: vec![
                    "cargo".to_string(),
                    "run".to_string(),
                    "-p".to_string(),
                    "weather-tool".to_string(),
                ],
                env_allowlist: vec!["WEATHER_API_KEY".to_string()],
                output_mode: ToolCliOutputMode::Json,
                timeout_ms: Some(30_000),
            },
        );

        registry.register(cli_tool);

        let tool = registry
            .get("weather.lookup")
            .expect("tool should be registered");
        assert_eq!(tool.invocation_mode, ToolInvocationMode::Cli);
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.output_mode.clone()),
            Some(ToolCliOutputMode::Json)
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.clone()),
            Some(vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "weather-tool".to_string(),
            ])
        );
        assert_eq!(registry.list_cli().len(), 1);
    }

    #[test]
    fn list_cli_excludes_internal_tools() {
        let mut registry = InMemoryToolRegistry::default();
        registry.register(ToolDefinition::internal(
            "memory.refresh",
            "Memory Refresh",
            "dataset",
            json!({
                "type": "object"
            }),
            json!({
                "type": "object"
            }),
        ));

        assert!(registry.list_cli().is_empty());
    }

    #[test]
    fn bootstrap_default_tool_registry_registers_expected_cli_tools() {
        let registry = bootstrap_default_tool_registry();

        assert!(registry.get("retrieval.search").is_some());
        assert!(registry.get("weather.lookup").is_some());
        assert!(registry.get("runtime.inspect").is_some());
        assert!(registry.get("workflow.retry").is_some());
        assert!(registry.get("report.plan").is_some());
        assert!(registry.get("report.publish").is_some());
        assert!(registry.get("report.read_published").is_some());
        assert!(registry.get("document.read_detail").is_some());
        assert!(registry.get("document.compare").is_some());
        assert!(registry.get("memory_directory.refresh").is_some());
        assert!(registry.get("chat_session.report_entry").is_some());
        assert!(registry.get("report.render").is_some());
        assert_eq!(registry.list_cli().len(), 12);
    }

    #[test]
    fn runtime_inspect_tool_output_schema_includes_model_facing_and_pretty_summaries() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("runtime.inspect")
            .expect("runtime.inspect should be registered");

        assert_eq!(
            tool.output_schema["properties"]["model_facing"]["type"],
            json!(["object", "null"])
        );
        assert_eq!(
            tool.output_schema["properties"]["pretty_summaries"]["type"],
            json!("array")
        );
        assert_eq!(
            tool.output_schema["properties"]["pretty_summaries"]["items"]["type"],
            json!("string")
        );
        assert!(tool.output_schema["required"]
            .as_array()
            .expect("required should be an array")
            .iter()
            .any(|value| value == "model_facing"));
        assert!(tool.output_schema["required"]
            .as_array()
            .expect("required should be an array")
            .iter()
            .any(|value| value == "pretty_summaries"));
    }

    #[test]
    fn workflow_retry_tool_exposes_cli_contract_and_execution_scope() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("workflow.retry")
            .expect("workflow.retry should be registered");

        assert_eq!(tool.scope_policy, "execution");
        assert_eq!(
            tool.input_schema["required"],
            json!(["execution_id", "reason"])
        );
        assert_eq!(
            tool.output_schema["required"],
            json!(["retry_transition", "restart_transition"])
        );
        assert_eq!(
            tool.output_schema["properties"]["restart_transition"]["type"],
            json!(["object", "null"])
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn report_plan_tool_exposes_cli_contract_and_report_plan_scope() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("report.plan")
            .expect("report.plan should be registered");

        assert_eq!(tool.scope_policy, "report_plan");
        assert_eq!(tool.input_schema["required"], json!(["plan_id"]));
        assert_eq!(
            tool.output_schema["required"],
            json!(["plan", "workflow_execution"])
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn report_publish_tool_exposes_cli_contract_and_publish_payload() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("report.publish")
            .expect("report.publish should be registered");

        assert_eq!(tool.scope_policy, "report_plan");
        assert_eq!(tool.input_schema["required"], json!(["plan_id", "surface"]));
        assert_eq!(
            tool.input_schema["properties"]["surface"]["enum"],
            json!(["pc", "mobile"])
        );
        assert_eq!(
            tool.output_schema["required"],
            json!(["report", "version", "source_render_output"])
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn report_read_published_tool_exposes_cli_contract_and_detail_payload() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("report.read_published")
            .expect("report.read_published should be registered");

        assert_eq!(tool.scope_policy, "report_plan");
        assert_eq!(tool.input_schema["required"], json!(["plan_id"]));
        assert_eq!(
            tool.output_schema["required"],
            json!(["report", "current_version", "versions"])
        );
        assert_eq!(
            tool.output_schema["properties"]["current_version"]["type"],
            json!(["object", "null"])
        );
        assert_eq!(
            tool.output_schema["properties"]["versions"]["type"],
            json!("array")
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn chat_session_report_entry_tool_exposes_cli_contract_and_nullable_report_outputs() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("chat_session.report_entry")
            .expect("chat_session.report_entry should be registered");

        assert_eq!(
            tool.input_schema["properties"]["action"]["enum"]
                .as_array()
                .map(Vec::len),
            Some(3)
        );
        assert_eq!(
            tool.output_schema["properties"]["report_plan"]["type"],
            json!(["object", "null"])
        );
        assert_eq!(
            tool.output_schema["properties"]["workflow_execution"]["type"],
            json!(["object", "null"])
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn memory_directory_refresh_tool_exposes_cli_contract_and_dataset_scope() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("memory_directory.refresh")
            .expect("memory_directory.refresh should be registered");

        assert_eq!(tool.scope_policy, "dataset");
        assert_eq!(tool.input_schema["required"], json!(["dataset_id"]));
        assert_eq!(
            tool.output_schema["properties"]["workflow_execution"]["type"],
            json!("object")
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn document_read_detail_tool_exposes_cli_contract_and_document_scope() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("document.read_detail")
            .expect("document.read_detail should be registered");

        assert_eq!(tool.scope_policy, "document");
        assert_eq!(tool.input_schema["required"], json!(["document_id"]));
        assert_eq!(
            tool.output_schema["required"],
            json!(["document", "chunks", "retrieval_evidences"])
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn document_compare_tool_exposes_cli_contract_and_document_scope() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("document.compare")
            .expect("document.compare should be registered");

        assert_eq!(tool.scope_policy, "document");
        assert_eq!(tool.input_schema["required"], json!(["document_ids"]));
        assert_eq!(
            tool.input_schema["properties"]["document_ids"]["minItems"],
            json!(2)
        );
        assert_eq!(tool.output_schema["required"], json!(["documents"]));
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn report_render_tool_exposes_cli_contract_and_surface_schema() {
        let registry = bootstrap_default_tool_registry();
        let tool = registry
            .get("report.render")
            .expect("report.render should be registered");

        assert_eq!(tool.scope_policy, "report_plan");
        assert_eq!(
            tool.input_schema["properties"]["surface"]["enum"],
            json!(["pc", "mobile"])
        );
        assert_eq!(
            tool.output_schema["properties"]["requested_ast_version_id"]["type"],
            json!("string")
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.argv.last().cloned()),
            Some(Some("--".to_string()))
        );
        assert_eq!(
            tool.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec![
                "PLATFORM_DATABASE_URL".to_string(),
                "PLATFORM_TENANT_KEY".to_string(),
                "PLATFORM_TENANT_NAME".to_string(),
            ])
        );
    }

    #[test]
    fn render_tool_trace_manifest_embeds_tool_snapshot() {
        let trace = render_tool_trace_manifest(&[LlmToolCall {
            call_id: Some("call_weather".to_string()),
            tool_name: "weather.lookup".to_string(),
            status: LlmToolCallStatus::Completed,
            arguments: Some(json!({ "city": "Shanghai" })),
            result: Some(json!({ "summary": "sunny" })),
        }]);

        assert_eq!(trace[0]["tool_name"], json!("weather.lookup"));
        assert_eq!(trace[0]["tool"]["key"], json!("weather.lookup"));
        assert_eq!(trace[0]["tool"]["invocation_mode"], json!("cli"));
        assert_eq!(trace[0]["tool"]["cli"]["output_mode"], json!("json"));
    }
}
