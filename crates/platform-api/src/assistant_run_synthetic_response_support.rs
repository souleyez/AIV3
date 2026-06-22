use llm_gateway::{
    LlmFinishReason, LlmResponse, LlmRuntimeMetadata, LlmRuntimeMode, MODEL_LANE_ASSISTANT_CHAT,
};

pub(crate) fn assistant_run_direct_answer_response(output_text: String) -> LlmResponse {
    assistant_run_synthetic_response(
        output_text,
        "platform_direct_answer",
        "dataset-entity-scan-direct-v1",
    )
}

pub(crate) fn assistant_run_answer_quality_synthetic_response(output_text: String) -> LlmResponse {
    assistant_run_synthetic_response(
        output_text,
        "platform_answer_quality_gate",
        "synthetic-candidate-answer",
    )
}

fn assistant_run_synthetic_response(
    output_text: String,
    provider: &str,
    model: &str,
) -> LlmResponse {
    LlmResponse {
        output_text,
        runtime: LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Placeholder,
            provider: provider.to_string(),
            model: model.to_string(),
            lane: Some(MODEL_LANE_ASSISTANT_CHAT.to_string()),
            request_id: None,
            finish_reason: Some(LlmFinishReason::Stop),
            provider_failure: None,
            latency_ms: Some(0),
            usage: None,
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: 0,
        },
        tool_calls: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_common_synthetic_response(response: &LlmResponse, text: &str) {
        assert_eq!(response.output_text, text);
        assert!(matches!(response.runtime.mode, LlmRuntimeMode::Placeholder));
        assert_eq!(
            response.runtime.lane.as_deref(),
            Some(MODEL_LANE_ASSISTANT_CHAT)
        );
        assert!(matches!(
            response.runtime.finish_reason,
            Some(LlmFinishReason::Stop)
        ));
        assert_eq!(response.runtime.latency_ms, Some(0));
        assert_eq!(response.runtime.tool_trace_count, 0);
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn direct_answer_response_preserves_platform_metadata() {
        let response = assistant_run_direct_answer_response("直接回答".to_string());

        assert_common_synthetic_response(&response, "直接回答");
        assert_eq!(response.runtime.provider, "platform_direct_answer");
        assert_eq!(response.runtime.model, "dataset-entity-scan-direct-v1");
    }

    #[test]
    fn answer_quality_synthetic_response_preserves_platform_metadata() {
        let response =
            assistant_run_answer_quality_synthetic_response("质量门禁候选答案".to_string());

        assert_common_synthetic_response(&response, "质量门禁候选答案");
        assert_eq!(response.runtime.provider, "platform_answer_quality_gate");
        assert_eq!(response.runtime.model, "synthetic-candidate-answer");
    }
}
