use contracts::{
    DatasetOutputView, LlmInvocationView, MemoryDirectoryView, RetrievalEvidenceView,
    ToolExecutionView,
};
use domain_model::{
    DatasetId, DatasetOutput, MemoryDirectoryId, ReportPlanId, RetrievalEvidenceId,
};
use serde_json::Value;
use uuid::Uuid;

use crate::manifest_runtime_view_support::{
    parse_manifest_context_binding, parse_manifest_runtime,
};
use crate::runtime_manifest_support::{
    manifest_runtime_from_latest_llm_invocation, manifest_tool_trace_from_tool_executions,
};
use crate::tool_view_support::parse_manifest_tool_trace;

fn parse_retrieval_evidence_ids(value: &Value) -> Option<Vec<RetrievalEvidenceId>> {
    value
        .as_array()?
        .iter()
        .map(|value| {
            Uuid::parse_str(value.as_str()?)
                .ok()
                .map(RetrievalEvidenceId::from)
        })
        .collect::<Option<Vec<_>>>()
}

fn parse_dataset_output_format(value: &str) -> Option<contracts::DatasetOutputFormatView> {
    match value {
        "markdown" => Some(contracts::DatasetOutputFormatView::Markdown),
        _ => None,
    }
}

fn parse_dataset_output_section_kind(
    value: &str,
) -> Option<contracts::DatasetOutputSectionKindView> {
    match value {
        "summary" => Some(contracts::DatasetOutputSectionKindView::Summary),
        _ => None,
    }
}

fn parse_dataset_output_content(value: &Value) -> Option<contracts::DatasetOutputContentView> {
    let output = value.as_object()?;
    let sections = output
        .get("sections")?
        .as_array()?
        .iter()
        .map(|value| {
            let section = value.as_object()?;
            Some(contracts::DatasetOutputSectionView {
                section_key: section.get("section_key")?.as_str()?.to_string(),
                kind: parse_dataset_output_section_kind(section.get("kind")?.as_str()?)?,
                title: section.get("title")?.as_str()?.to_string(),
                content: section.get("content")?.as_str()?.to_string(),
                retrieval_evidence_ids: parse_retrieval_evidence_ids(
                    section.get("retrieval_evidence_ids")?,
                )?,
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(contracts::DatasetOutputContentView {
        format: parse_dataset_output_format(output.get("format")?.as_str()?)?,
        sections,
    })
}

pub(crate) fn parse_dataset_output_manifest(
    value: &Value,
) -> Option<contracts::DatasetOutputManifestView> {
    let object = value.as_object()?;
    let tool_trace = parse_manifest_tool_trace(object.get("tool_trace"))?;
    let runtime = object
        .get("runtime")
        .and_then(parse_manifest_runtime)
        .or_else(|| {
            object
                .get("generator")
                .and_then(Value::as_str)
                .filter(|generator| *generator == "dataset-output-worker")
                .map(|_| contracts::ManifestRuntimeView {
                    mode: contracts::ManifestRuntimeModeView::Placeholder,
                    provider: None,
                    model: None,
                    request_id: None,
                    finish_reason: None,
                    provider_failure: None,
                    latency_ms: None,
                    usage: None,
                    system_prompt_key: None,
                    system_prompt_version: None,
                    tool_trace_count: Some(tool_trace.len()),
                })
        });

    Some(contracts::DatasetOutputManifestView {
        generator: object.get("generator")?.as_str()?.to_string(),
        schema_version: object.get("schema_version")?.as_str()?.to_string(),
        dataset_id: DatasetId::from(Uuid::parse_str(object.get("dataset_id")?.as_str()?).ok()?),
        prompt: object.get("prompt")?.as_str()?.to_string(),
        indexed_document_count: object.get("indexed_document_count")?.as_u64()? as usize,
        refreshed_chunks: object.get("refreshed_chunks")?.as_u64()? as usize,
        memory_directory_id: object
            .get("memory_directory_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(MemoryDirectoryId::from),
        memory_directory_version_no: object
            .get("memory_directory_version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        retrieval_evidence_count: object.get("retrieval_evidence_count")?.as_u64()? as usize,
        retrieval_evidence_ids: parse_retrieval_evidence_ids(
            object.get("retrieval_evidence_ids")?,
        )?,
        output: object.get("output").and_then(parse_dataset_output_content),
        service_handoff: object
            .get("service_handoff")
            .and_then(crate::parse_manifest_service_handoff),
        tool_trace,
        context_binding: parse_manifest_context_binding(object.get("context_binding")?.as_str()?)?,
        runtime,
    })
}

pub(crate) fn hydrate_dataset_output_manifest_view(
    value: &Value,
    llm_invocations: &[LlmInvocationView],
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::DatasetOutputManifestView> {
    let mut view = parse_dataset_output_manifest(value)?;
    if let Some(runtime) = manifest_runtime_from_latest_llm_invocation(llm_invocations) {
        view.runtime = Some(runtime);
    }
    if !tool_executions.is_empty() {
        view.tool_trace = manifest_tool_trace_from_tool_executions(tool_executions);
        if let Some(runtime) = view.runtime.as_mut() {
            runtime.tool_trace_count = Some(tool_executions.len());
        }
    }
    Some(view)
}

pub(crate) fn to_dataset_output_view(
    output: DatasetOutput,
    memory_directory: Option<MemoryDirectoryView>,
    retrieval_evidences: Vec<RetrievalEvidenceView>,
    llm_invocations: Vec<LlmInvocationView>,
    tool_executions: Vec<ToolExecutionView>,
) -> DatasetOutputView {
    let output_manifest_view = hydrate_dataset_output_manifest_view(
        &output.output_manifest,
        &llm_invocations,
        &tool_executions,
    );

    DatasetOutputView {
        id: output.id,
        dataset_id: output.dataset_id,
        execution_id: output.execution_id,
        prompt: output.prompt,
        output_text: output.output_text,
        memory_directory_id: output.memory_directory_id,
        memory_directory,
        retrieval_evidence_ids: output.retrieval_evidence_ids,
        retrieval_evidences,
        llm_invocations,
        tool_executions,
        output_manifest_view,
        output_manifest: output.output_manifest,
        model_facing: None,
        created_at: output.created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use domain_model::{DatasetOutputId, TenantId, ToolExecutionId, WorkflowExecutionId};

    use super::*;

    #[test]
    fn dataset_output_manifest_preserves_content_handoff_and_placeholder_runtime() {
        let dataset_id = DatasetId::new();
        let evidence_ids = vec![RetrievalEvidenceId::new(), RetrievalEvidenceId::new()];
        let report_plan_id = ReportPlanId::new();

        let manifest = parse_dataset_output_manifest(&json!({
            "generator": "dataset-output-worker",
            "schema_version": "0.4.0",
            "dataset_id": dataset_id,
            "prompt": "Summarize",
            "indexed_document_count": 2,
            "refreshed_chunks": 8,
            "memory_directory_id": null,
            "memory_directory_version_no": 3,
            "retrieval_evidence_count": evidence_ids.len(),
            "retrieval_evidence_ids": evidence_ids,
            "output": {
                "format": "markdown",
                "sections": [{
                    "section_key": "dataset_summary",
                    "kind": "summary",
                    "title": "Dataset Summary",
                    "content": "Fresh output",
                    "retrieval_evidence_ids": evidence_ids
                }]
            },
            "service_handoff": {
                "source": "chat_session_report_entry",
                "service_lane": "report_service",
                "report_entry_state": "confirmed",
                "requested_at": "2026-06-14T00:00:00Z",
                "resolved_at": "2026-06-14T00:01:00Z",
                "resolved_action": "enter_report_service",
                "suggested_title": "Monthly Report",
                "suggested_objective": "Summarize sales signals",
                "confirmed_report_plan_id": report_plan_id
            },
            "tool_trace": [],
            "context_binding": "creation_time"
        }))
        .expect("dataset output manifest should parse");

        assert_eq!(manifest.dataset_id, dataset_id);
        assert_eq!(manifest.memory_directory_version_no, Some(3));
        assert_eq!(manifest.retrieval_evidence_count, 2);
        assert_eq!(
            manifest
                .output
                .as_ref()
                .and_then(|output| output.sections.first())
                .map(|section| section.kind.clone()),
            Some(contracts::DatasetOutputSectionKindView::Summary)
        );
        assert_eq!(
            manifest
                .service_handoff
                .as_ref()
                .map(|handoff| handoff.confirmed_report_plan_id),
            Some(Some(report_plan_id))
        );
        assert_eq!(
            manifest
                .runtime
                .as_ref()
                .map(|runtime| runtime.mode.clone()),
            Some(contracts::ManifestRuntimeModeView::Placeholder)
        );
        assert_eq!(
            manifest
                .runtime
                .as_ref()
                .map(|runtime| runtime.tool_trace_count),
            Some(Some(0))
        );
    }

    #[test]
    fn dataset_output_view_overrides_manifest_runtime_and_tool_trace_from_persisted_records() {
        let now = Utc::now();
        let execution_id = WorkflowExecutionId::new();
        let output_id = DatasetOutputId::new();
        let dataset_id = DatasetId::new();
        let output = DatasetOutput {
            id: output_id,
            tenant_id: TenantId::new(),
            execution_id,
            dataset_id,
            owner_user_id: None,
            prompt: "Summarize".to_string(),
            output_text: "Fresh output".to_string(),
            memory_directory_id: None,
            retrieval_evidence_ids: Vec::new(),
            output_manifest: json!({
                "generator": "dataset-output-worker",
                "schema_version": "0.4.0",
                "dataset_id": dataset_id,
                "prompt": "Summarize",
                "indexed_document_count": 1,
                "refreshed_chunks": 1,
                "memory_directory_id": null,
                "memory_directory_version_no": null,
                "retrieval_evidence_count": 0,
                "retrieval_evidence_ids": [],
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "dataset_summary",
                        "kind": "summary",
                        "title": "Dataset Summary",
                        "content": "Fresh output",
                        "retrieval_evidence_ids": []
                    }]
                },
                "tool_trace": [{
                    "call_id": "call_stale",
                    "tool_name": "stale.tool",
                    "status": "failed",
                    "arguments": { "query": "stale" },
                    "result": { "error": "stale" }
                }],
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-dataset-output-v1",
                    "request_id": "req_stale",
                    "finish_reason": "error",
                    "latency_ms": 1,
                    "tool_trace_count": 99
                }
            }),
            created_at: now,
        };
        let llm_invocations = vec![contracts::LlmInvocationView {
            id: domain_model::LlmInvocationId::new(),
            execution_id,
            source_kind: contracts::LlmInvocationSourceKindView::DatasetOutput,
            dataset_output_id: Some(output_id),
            chat_message_id: None,
            sequence_no: 1,
            mode: contracts::LlmInvocationModeView::Provider,
            provider: Some("openai".to_string()),
            model: Some("gpt-5.4".to_string()),
            request_id: Some("req_fresh".to_string()),
            finish_reason: Some(contracts::LlmInvocationFinishReasonView::ToolCalls),
            latency_ms: Some(321),
            usage: Some(contracts::LlmTokenUsageView {
                input_tokens: 144,
                output_tokens: 55,
                total_tokens: 199,
            }),
            system_prompt_key: Some("dataset_output.live".to_string()),
            system_prompt_version: Some("v2".to_string()),
            tool_trace_count: Some(3),
            created_at: now,
        }];
        let tool_executions = vec![contracts::ToolExecutionView {
            id: ToolExecutionId::new(),
            execution_id,
            source_kind: contracts::ToolExecutionSourceKindView::DatasetOutput,
            dataset_output_id: Some(output_id),
            chat_message_id: None,
            sequence_no: 1,
            call_id: Some("call_fresh".to_string()),
            tool_name: "retrieval.search".to_string(),
            tool: None,
            status: contracts::ManifestToolCallStatusView::Completed,
            arguments: Some(json!({ "query": "fresh" })),
            result: Some(json!({ "hits": 2 })),
            created_at: now,
        }];

        let view =
            to_dataset_output_view(output, None, Vec::new(), llm_invocations, tool_executions);

        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.provider.as_deref()),
            Some("openai")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.request_id.as_deref()),
            Some("req_fresh")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.tool_trace_count),
            Some(Some(1))
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.call_id.as_deref()),
            Some("call_fresh")
        );
    }
}
