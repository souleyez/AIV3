use anyhow::Result;
use domain_model::{DatasetId, MemoryDirectoryId, RetrievalEvidenceId};
use llm_gateway::{
    LlmProvider, LlmRequest, LlmRuntimeMetadata, LlmToolCall, MODEL_LANE_DATASET_OUTPUT,
};
use prompt_registry::DATASET_OUTPUT_PLACEHOLDER_PROMPT_KEY;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct DatasetOutputJob {
    pub dataset_id: DatasetId,
    pub prompt: String,
    pub indexed_document_count: usize,
    pub refreshed_chunks: usize,
    pub memory_directory_id: Option<MemoryDirectoryId>,
    pub memory_directory_version_no: Option<i32>,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
    pub tool_calls: Vec<LlmToolCall>,
    pub service_handoff: Option<contracts::ManifestServiceHandoffView>,
}

#[derive(Clone, Debug)]
pub struct DatasetOutputOutcome {
    pub output_text: String,
    pub output_manifest: Value,
    pub runtime: LlmRuntimeMetadata,
    pub tool_calls: Vec<LlmToolCall>,
}

pub trait DatasetOutputGenerator {
    fn generate(&self, job: &DatasetOutputJob) -> Result<DatasetOutputOutcome>;
}

#[derive(Clone, Debug)]
pub struct PlaceholderDatasetOutputGenerator {
    provider: Arc<dyn LlmProvider>,
    model: String,
}

impl PlaceholderDatasetOutputGenerator {
    pub fn new(provider: Arc<dyn LlmProvider>, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }
}

impl DatasetOutputGenerator for PlaceholderDatasetOutputGenerator {
    fn generate(&self, job: &DatasetOutputJob) -> Result<DatasetOutputOutcome> {
        let placeholder_text = format!(
            "# Dataset Output\n\n- Prompt: {}\n- Indexed documents: {}\n- Refreshed chunks: {}\n- Retrieval evidence: {}\n- Dataset: {}\n\nThis is a placeholder dataset-scoped answer synthesized from the bound memory directory, indexed document inventory, and bound retrieval evidence references.",
            job.prompt,
            job.indexed_document_count,
            job.refreshed_chunks,
            job.retrieval_evidence_ids.len(),
            job.dataset_id
        );
        let response = self.provider.complete(&LlmRequest {
            model: self.model.clone(),
            lane: Some(MODEL_LANE_DATASET_OUTPUT.to_string()),
            system_prompt_key: Some(DATASET_OUTPUT_PLACEHOLDER_PROMPT_KEY.to_string()),
            input: placeholder_text,
        })?;

        let mut tool_calls = job.tool_calls.clone();
        tool_calls.extend(response.tool_calls.clone());
        let mut runtime = response.runtime.clone();
        runtime.tool_trace_count = tool_calls.len();

        Ok(DatasetOutputOutcome {
            output_text: response.output_text.clone(),
            output_manifest: json!({
                "generator": "dataset-output-worker",
                "schema_version": "0.5.0",
                "dataset_id": job.dataset_id,
                "prompt": job.prompt,
                "indexed_document_count": job.indexed_document_count,
                "refreshed_chunks": job.refreshed_chunks,
                "memory_directory_id": job.memory_directory_id,
                "memory_directory_version_no": job.memory_directory_version_no,
                "retrieval_evidence_count": job.retrieval_evidence_ids.len(),
                "retrieval_evidence_ids": job.retrieval_evidence_ids,
                "output": {
                    "format": "markdown",
                    "sections": [
                        {
                            "section_key": "dataset_summary",
                            "kind": "summary",
                            "title": "Dataset Summary",
                            "content": response.output_text,
                            "retrieval_evidence_ids": job.retrieval_evidence_ids,
                        }
                    ]
                },
                "service_handoff": job.service_handoff.as_ref(),
                "context_binding": "creation_time",
            }),
            runtime,
            tool_calls,
        })
    }
}

pub fn dataset_runtime_error_message(runtime: &LlmRuntimeMetadata) -> Option<String> {
    match runtime.finish_reason {
        Some(llm_gateway::LlmFinishReason::Error) => Some(format!(
            "dataset output provider {} returned finish_reason=error{}{}",
            runtime.provider,
            runtime
                .request_id
                .as_ref()
                .map(|value| format!(" (request_id={value})"))
                .unwrap_or_default(),
            runtime
                .provider_failure
                .as_ref()
                .map(|failure| format!(" [{}: {}]", failure.kind.as_str(), failure.message))
                .unwrap_or_default()
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prompt_registry::bootstrap_default_prompt_registry;

    const PLACEHOLDER_MODEL: &str = "placeholder-dataset-output-v1";

    #[test]
    fn placeholder_output_mentions_prompt_and_counts() {
        let dataset_id = DatasetId::new();
        let generator = PlaceholderDatasetOutputGenerator::new(
            Arc::new(
                llm_gateway::PlaceholderLlmProvider::new("placeholder")
                    .with_prompt_registry(bootstrap_default_prompt_registry()),
            ),
            PLACEHOLDER_MODEL,
        );
        let outcome = generator
            .generate(&DatasetOutputJob {
                dataset_id,
                prompt: "Summarize the current dataset".to_string(),
                indexed_document_count: 2,
                refreshed_chunks: 12,
                memory_directory_id: None,
                memory_directory_version_no: Some(3),
                retrieval_evidence_ids: vec![
                    RetrievalEvidenceId::new(),
                    RetrievalEvidenceId::new(),
                ],
                tool_calls: vec![],
                service_handoff: None,
            })
            .expect("placeholder dataset output should succeed");

        assert!(outcome
            .output_text
            .contains("Summarize the current dataset"));
        assert_eq!(outcome.output_manifest["indexed_document_count"], json!(2));
        assert_eq!(outcome.output_manifest["refreshed_chunks"], json!(12));
        assert_eq!(
            outcome.output_manifest["memory_directory_version_no"],
            json!(3)
        );
        assert_eq!(
            outcome.output_manifest["retrieval_evidence_count"],
            json!(2)
        );
        assert_eq!(outcome.output_manifest["schema_version"], json!("0.5.0"));
        assert_eq!(
            outcome.output_manifest["output"]["format"],
            json!("markdown")
        );
        assert_eq!(
            outcome.output_manifest["output"]["sections"][0]["section_key"],
            json!("dataset_summary")
        );
        assert_eq!(
            outcome.output_manifest["output"]["sections"][0]["kind"],
            json!("summary")
        );
        assert_eq!(
            outcome.output_manifest["output"]["sections"][0]["title"],
            json!("Dataset Summary")
        );
        assert_eq!(
            outcome.output_manifest["output"]["sections"][0]["content"],
            outcome.output_text
        );
        assert_eq!(
            outcome.output_manifest["output"]["sections"][0]["retrieval_evidence_ids"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        assert!(outcome.output_manifest.get("runtime").is_none());
        assert_eq!(
            outcome.runtime.mode,
            llm_gateway::LlmRuntimeMode::Placeholder
        );
        assert_eq!(outcome.runtime.provider, "placeholder");
        assert_eq!(outcome.runtime.model, "placeholder-dataset-output-v1");
        assert_eq!(
            outcome.runtime.system_prompt_key.as_deref(),
            Some("dataset_output.placeholder")
        );
        assert_eq!(outcome.runtime.system_prompt_version.as_deref(), Some("v1"));
        assert_eq!(
            outcome.runtime.finish_reason,
            Some(llm_gateway::LlmFinishReason::Stop)
        );
        assert!(outcome.runtime.request_id.is_some());
        assert!(outcome.runtime.latency_ms.is_some());
        assert_eq!(
            outcome
                .runtime
                .usage
                .as_ref()
                .map(|usage| usage.total_tokens > 0),
            Some(true)
        );
        assert_eq!(outcome.runtime.tool_trace_count, 0);
        assert!(outcome.output_manifest.get("tool_trace").is_none());
        assert!(outcome.tool_calls.is_empty());
    }

    #[test]
    fn scripted_provider_emits_provider_mode_runtime_manifest() {
        let dataset_id = DatasetId::new();
        let generator = PlaceholderDatasetOutputGenerator::new(
            Arc::new(
                llm_gateway::ScriptedLlmProvider::new("openai")
                    .with_prompt_registry(bootstrap_default_prompt_registry())
                    .with_response_text("provider dataset output")
                    .with_request_id("req_dataset_output_provider")
                    .with_finish_reason(llm_gateway::LlmFinishReason::Stop)
                    .with_latency_ms(123)
                    .with_usage(llm_gateway::LlmTokenUsage {
                        input_tokens: 2,
                        output_tokens: 4,
                        total_tokens: 6,
                    })
                    .with_tool_calls(vec![llm_gateway::LlmToolCall {
                        call_id: Some("call_dataset_output".to_string()),
                        tool_name: "retrieval.search".to_string(),
                        status: llm_gateway::LlmToolCallStatus::Completed,
                        arguments: Some(json!({ "query": "dataset summary" })),
                        result: Some(json!({ "hits": 3 })),
                    }]),
            ),
            "gpt-5.4",
        );
        let outcome = generator
            .generate(&DatasetOutputJob {
                dataset_id,
                prompt: "Summarize the current dataset".to_string(),
                indexed_document_count: 2,
                refreshed_chunks: 12,
                memory_directory_id: None,
                memory_directory_version_no: Some(3),
                retrieval_evidence_ids: vec![RetrievalEvidenceId::new()],
                tool_calls: vec![],
                service_handoff: None,
            })
            .expect("scripted dataset output should succeed");

        assert_eq!(outcome.output_text, "provider dataset output");
        assert!(outcome.output_manifest.get("runtime").is_none());
        assert_eq!(outcome.runtime.mode, llm_gateway::LlmRuntimeMode::Provider);
        assert_eq!(outcome.runtime.provider, "openai");
        assert_eq!(
            outcome.runtime.request_id.as_deref(),
            Some("req_dataset_output_provider")
        );
        assert_eq!(outcome.runtime.latency_ms, Some(123));
        assert_eq!(
            outcome
                .runtime
                .usage
                .as_ref()
                .map(|usage| usage.total_tokens),
            Some(6)
        );
        assert_eq!(outcome.runtime.tool_trace_count, 1);
        assert!(outcome.output_manifest.get("tool_trace").is_none());
        assert_eq!(outcome.tool_calls.len(), 1);
        assert_eq!(outcome.tool_calls[0].tool_name, "retrieval.search");
        assert_eq!(
            outcome.tool_calls[0].status,
            llm_gateway::LlmToolCallStatus::Completed
        );
        assert_eq!(
            outcome.output_manifest["output"]["sections"][0]["content"],
            json!("provider dataset output")
        );
        assert_eq!(
            outcome.output_manifest["output"]["sections"][0]["retrieval_evidence_ids"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn precomputed_tool_calls_are_merged_with_provider_tool_trace() {
        let dataset_id = DatasetId::new();
        let retrieval_evidence_id = RetrievalEvidenceId::new();
        let generator = PlaceholderDatasetOutputGenerator::new(
            Arc::new(
                llm_gateway::ScriptedLlmProvider::new("openai")
                    .with_prompt_registry(bootstrap_default_prompt_registry())
                    .with_response_text("provider dataset output")
                    .with_tool_calls(vec![llm_gateway::LlmToolCall {
                        call_id: Some("call_provider".to_string()),
                        tool_name: "document.read_detail".to_string(),
                        status: llm_gateway::LlmToolCallStatus::Completed,
                        arguments: Some(json!({ "document_id": "doc_1" })),
                        result: Some(json!({ "summary": "detail loaded" })),
                    }]),
            ),
            "gpt-5.4",
        );

        let outcome = generator
            .generate(&DatasetOutputJob {
                dataset_id,
                prompt: "Summarize retrieval facts".to_string(),
                indexed_document_count: 1,
                refreshed_chunks: 2,
                memory_directory_id: None,
                memory_directory_version_no: None,
                retrieval_evidence_ids: vec![retrieval_evidence_id],
                tool_calls: vec![llm_gateway::LlmToolCall {
                    call_id: Some("call_retrieval".to_string()),
                    tool_name: "retrieval.search".to_string(),
                    status: llm_gateway::LlmToolCallStatus::Completed,
                    arguments: Some(json!({ "query": "retrieval facts" })),
                    result: Some(
                        json!({ "hits": [{ "retrieval_evidence_id": retrieval_evidence_id }] }),
                    ),
                }],
                service_handoff: None,
            })
            .expect("dataset output with precomputed tool call should succeed");

        assert_eq!(outcome.runtime.tool_trace_count, 2);
        assert_eq!(outcome.tool_calls.len(), 2);
        assert_eq!(outcome.tool_calls[0].tool_name, "retrieval.search");
        assert_eq!(outcome.tool_calls[1].tool_name, "document.read_detail");
    }

    #[test]
    fn dataset_runtime_error_message_only_flags_error_finish_reason() {
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_DATASET_OUTPUT.to_string()),
            request_id: Some("req_dataset_output_error".to_string()),
            finish_reason: Some(llm_gateway::LlmFinishReason::Error),
            provider_failure: None,
            latency_ms: Some(1),
            usage: None,
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: 0,
        };
        assert_eq!(
            dataset_runtime_error_message(&runtime),
            Some(
                "dataset output provider openai returned finish_reason=error (request_id=req_dataset_output_error)"
                    .to_string()
            )
        );

        let ok_runtime = LlmRuntimeMetadata {
            finish_reason: Some(llm_gateway::LlmFinishReason::Stop),
            ..runtime
        };
        assert_eq!(dataset_runtime_error_message(&ok_runtime), None);
    }

    #[test]
    fn dataset_runtime_error_message_includes_provider_failure_details() {
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_DATASET_OUTPUT.to_string()),
            request_id: None,
            finish_reason: Some(llm_gateway::LlmFinishReason::Error),
            provider_failure: Some(llm_gateway::LlmProviderFailure {
                kind: llm_gateway::LlmProviderFailureKind::InvalidJson,
                message: "openai returned invalid JSON payload".to_string(),
            }),
            latency_ms: Some(1),
            usage: None,
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: 0,
        };

        assert_eq!(
            dataset_runtime_error_message(&runtime),
            Some(
                "dataset output provider openai returned finish_reason=error [invalid_json: openai returned invalid JSON payload]"
                    .to_string()
            )
        );
    }

    #[test]
    fn dataset_output_manifest_carries_service_handoff_when_bound() {
        let requested_at = chrono::Utc::now();
        let report_plan_id = domain_model::ReportPlanId::new();
        let generator = PlaceholderDatasetOutputGenerator::new(
            Arc::new(
                llm_gateway::PlaceholderLlmProvider::new("placeholder")
                    .with_prompt_registry(bootstrap_default_prompt_registry()),
            ),
            PLACEHOLDER_MODEL,
        );
        let outcome = generator
            .generate(&DatasetOutputJob {
                dataset_id: DatasetId::new(),
                prompt: "Continue the report draft".to_string(),
                indexed_document_count: 2,
                refreshed_chunks: 5,
                memory_directory_id: None,
                memory_directory_version_no: Some(1),
                retrieval_evidence_ids: vec![RetrievalEvidenceId::new()],
                tool_calls: vec![],
                service_handoff: Some(contracts::ManifestServiceHandoffView {
                    source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                    service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                    report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                    requested_at: Some(requested_at),
                    resolved_at: Some(requested_at),
                    resolved_action: Some(
                        contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                    ),
                    suggested_title: Some("Dataset Report".to_string()),
                    suggested_objective: Some(
                        "Turn the current dataset context into a report-ready output.".to_string(),
                    ),
                    confirmed_report_plan_id: Some(report_plan_id),
                }),
            })
            .expect("dataset output with service handoff should succeed");

        assert_eq!(
            outcome.output_manifest["service_handoff"]["source"],
            json!("chat_session_report_entry")
        );
        assert_eq!(
            outcome.output_manifest["service_handoff"]["service_lane"],
            json!("report_service")
        );
        assert_eq!(
            outcome.output_manifest["service_handoff"]["report_entry_state"],
            json!("confirmed")
        );
        assert_eq!(
            outcome.output_manifest["service_handoff"]["confirmed_report_plan_id"],
            json!(report_plan_id)
        );
    }
}
