use contracts::LlmInvocationView;
use domain_model::{
    LlmInvocation, LlmInvocationFinishReason, LlmInvocationMode, LlmInvocationSourceKind,
};

pub(crate) fn to_llm_invocation_view(llm_invocation: LlmInvocation) -> LlmInvocationView {
    LlmInvocationView {
        id: llm_invocation.id,
        execution_id: llm_invocation.execution_id,
        source_kind: match llm_invocation.source_kind {
            LlmInvocationSourceKind::DatasetOutput => {
                contracts::LlmInvocationSourceKindView::DatasetOutput
            }
            LlmInvocationSourceKind::ChatMessage => {
                contracts::LlmInvocationSourceKindView::ChatMessage
            }
            LlmInvocationSourceKind::WorkflowExecution => {
                contracts::LlmInvocationSourceKindView::WorkflowExecution
            }
        },
        dataset_output_id: llm_invocation.dataset_output_id,
        chat_message_id: llm_invocation.chat_message_id,
        sequence_no: llm_invocation.sequence_no,
        mode: match llm_invocation.mode {
            LlmInvocationMode::Placeholder => contracts::LlmInvocationModeView::Placeholder,
            LlmInvocationMode::Provider => contracts::LlmInvocationModeView::Provider,
        },
        provider: llm_invocation.provider,
        model: llm_invocation.model,
        request_id: llm_invocation.request_id,
        finish_reason: llm_invocation.finish_reason.map(|reason| match reason {
            LlmInvocationFinishReason::Stop => contracts::LlmInvocationFinishReasonView::Stop,
            LlmInvocationFinishReason::ToolCalls => {
                contracts::LlmInvocationFinishReasonView::ToolCalls
            }
            LlmInvocationFinishReason::Length => contracts::LlmInvocationFinishReasonView::Length,
            LlmInvocationFinishReason::ContentFilter => {
                contracts::LlmInvocationFinishReasonView::ContentFilter
            }
            LlmInvocationFinishReason::Error => contracts::LlmInvocationFinishReasonView::Error,
            LlmInvocationFinishReason::Other(value) => {
                contracts::LlmInvocationFinishReasonView::Other(value)
            }
        }),
        latency_ms: llm_invocation.latency_ms,
        usage: llm_invocation
            .usage
            .map(|usage| contracts::LlmTokenUsageView {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
            }),
        system_prompt_key: llm_invocation.system_prompt_key,
        system_prompt_version: llm_invocation.system_prompt_version,
        tool_trace_count: llm_invocation.tool_trace_count,
        created_at: llm_invocation.created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        ChatMessageId, LlmInvocationId, LlmTokenUsage, TenantId, WorkflowExecutionId,
    };

    use super::*;

    #[test]
    fn llm_invocation_view_preserves_provider_metadata_and_usage() {
        let execution_id = WorkflowExecutionId::new();
        let chat_message_id = ChatMessageId::new();
        let invocation = LlmInvocation {
            id: LlmInvocationId::new(),
            tenant_id: TenantId::new(),
            execution_id,
            source_kind: LlmInvocationSourceKind::ChatMessage,
            dataset_output_id: None,
            chat_message_id: Some(chat_message_id),
            sequence_no: 3,
            mode: LlmInvocationMode::Provider,
            provider: Some("openai".to_string()),
            model: Some("gpt-5.4".to_string()),
            request_id: Some("req_chat".to_string()),
            finish_reason: Some(LlmInvocationFinishReason::ToolCalls),
            latency_ms: Some(42),
            usage: Some(LlmTokenUsage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            }),
            system_prompt_key: Some("chat.answer".to_string()),
            system_prompt_version: Some("v2".to_string()),
            tool_trace_count: Some(2),
            created_at: Utc::now(),
        };

        let view = to_llm_invocation_view(invocation);

        assert_eq!(view.execution_id, execution_id);
        assert_eq!(
            view.source_kind,
            contracts::LlmInvocationSourceKindView::ChatMessage
        );
        assert_eq!(view.chat_message_id, Some(chat_message_id));
        assert_eq!(view.sequence_no, 3);
        assert_eq!(view.mode, contracts::LlmInvocationModeView::Provider);
        assert_eq!(view.provider.as_deref(), Some("openai"));
        assert_eq!(view.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(view.request_id.as_deref(), Some("req_chat"));
        assert_eq!(
            view.finish_reason,
            Some(contracts::LlmInvocationFinishReasonView::ToolCalls)
        );
        assert_eq!(view.latency_ms, Some(42));
        assert_eq!(
            view.usage.as_ref().map(|usage| usage.total_tokens),
            Some(30)
        );
        assert_eq!(view.system_prompt_key.as_deref(), Some("chat.answer"));
        assert_eq!(view.system_prompt_version.as_deref(), Some("v2"));
        assert_eq!(view.tool_trace_count, Some(2));
    }

    #[test]
    fn llm_invocation_view_preserves_other_finish_reason_and_placeholder_mode() {
        let invocation = LlmInvocation {
            id: LlmInvocationId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: LlmInvocationSourceKind::WorkflowExecution,
            dataset_output_id: None,
            chat_message_id: None,
            sequence_no: 0,
            mode: LlmInvocationMode::Placeholder,
            provider: None,
            model: None,
            request_id: None,
            finish_reason: Some(LlmInvocationFinishReason::Other(
                "provider_custom".to_string(),
            )),
            latency_ms: None,
            usage: None,
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: None,
            created_at: Utc::now(),
        };

        let view = to_llm_invocation_view(invocation);

        assert_eq!(
            view.source_kind,
            contracts::LlmInvocationSourceKindView::WorkflowExecution
        );
        assert_eq!(view.mode, contracts::LlmInvocationModeView::Placeholder);
        assert_eq!(
            view.finish_reason,
            Some(contracts::LlmInvocationFinishReasonView::Other(
                "provider_custom".to_string()
            ))
        );
    }
}
