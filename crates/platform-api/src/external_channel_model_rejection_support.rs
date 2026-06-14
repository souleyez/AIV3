use crate::assistant_run_react_output_contains_internal_marker;
use llm_gateway::{LlmFinishReason, LlmResponse};

pub(crate) fn external_channel_model_reply_rejection_reason(
    response: &LlmResponse,
) -> Option<String> {
    if let Some(failure) = response.runtime.provider_failure.as_ref() {
        return Some(format!("provider_failure:{}", failure.kind.as_str()));
    }
    if matches!(
        response.runtime.finish_reason.as_ref(),
        Some(LlmFinishReason::ContentFilter)
    ) {
        return Some("model_output_suppressed".to_string());
    }

    let output_text = response.output_text.trim();
    external_channel_model_output_text_rejection_reason(output_text)
}

pub(crate) fn external_channel_model_output_text_rejection_reason(
    output_text: &str,
) -> Option<String> {
    let output_text = output_text.trim();
    if output_text.is_empty() {
        return Some("empty_output".to_string());
    }
    if assistant_run_react_output_contains_internal_marker(output_text) {
        return Some("internal_payload_marker".to_string());
    }
    if external_channel_output_is_generic_orchestration_ack(output_text) {
        return Some("generic_orchestration_ack".to_string());
    }
    None
}

pub(crate) fn external_channel_output_is_generic_orchestration_ack(output_text: &str) -> bool {
    let normalized = normalize_external_channel_output_for_rejection(output_text);
    if normalized.is_empty() {
        return false;
    }
    let ack_markers = [
        "已收到指令",
        "已收到你的问题",
        "已收到您的问题",
        "收到指令",
        "收到你的问题",
        "收到您的问题",
    ];
    if ack_markers.iter().any(|marker| normalized.contains(marker)) {
        return true;
    }
    let short_ack = normalized.chars().count() <= 24
        && ["已收到", "收到", "已接收", "已接受"]
            .iter()
            .any(|marker| normalized.starts_with(marker));
    if short_ack {
        return true;
    }
    let orchestration_markers = [
        "系统将结合知识库",
        "结合知识库与数据源",
        "为您输出结论",
        "为你输出结论",
        "稍后为您输出",
        "稍后为你输出",
        "正在为您分析",
        "正在为你分析",
        "请稍候",
    ];
    orchestration_markers
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn normalize_external_channel_output_for_rejection(output_text: &str) -> String {
    output_text
        .chars()
        .filter(|ch| {
            !ch.is_whitespace()
                && !matches!(
                    ch,
                    '。' | '，'
                        | '、'
                        | '；'
                        | '：'
                        | '！'
                        | '？'
                        | '.'
                        | ','
                        | ';'
                        | ':'
                        | '!'
                        | '?'
                        | '"'
                        | '\''
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_gateway::{
        LlmProviderFailure, LlmProviderFailureKind, LlmRuntimeMetadata, LlmRuntimeMode,
        MODEL_LANE_ASSISTANT_CHAT,
    };

    fn response_with_runtime(
        output_text: &str,
        finish_reason: Option<LlmFinishReason>,
        provider_failure: Option<LlmProviderFailure>,
    ) -> LlmResponse {
        LlmResponse {
            output_text: output_text.to_string(),
            runtime: LlmRuntimeMetadata {
                mode: LlmRuntimeMode::Provider,
                provider: "scripted".to_string(),
                model: "assistant-run-primary-v1".to_string(),
                lane: Some(MODEL_LANE_ASSISTANT_CHAT.to_string()),
                request_id: None,
                finish_reason,
                provider_failure,
                latency_ms: None,
                usage: None,
                system_prompt_key: None,
                system_prompt_version: None,
                tool_trace_count: 0,
            },
            tool_calls: Vec::new(),
        }
    }

    #[test]
    fn rejects_generic_orchestration_ack_texts() {
        for text in [
            "已收到指令。系统将结合知识库与数据源进行分析，并为您输出结论。",
            "已收到你的问题，请稍候。",
            "系统将结合知识库与数据源进行分析并为您输出结论。",
        ] {
            assert_eq!(
                external_channel_model_output_text_rejection_reason(text).as_deref(),
                Some("generic_orchestration_ack")
            );
        }

        assert_eq!(
            external_channel_model_output_text_rejection_reason(
                "订单延期风险主要集中在仓库交接和供应商确认两个环节。"
            ),
            None
        );
    }

    #[test]
    fn rejects_empty_internal_marker_and_suppressed_output() {
        assert_eq!(
            external_channel_model_output_text_rejection_reason(" \n\t").as_deref(),
            Some("empty_output")
        );
        assert_eq!(
            external_channel_model_output_text_rejection_reason(
                "[TOOL_CALL]\nretrieve_evidence:{}\n[/TOOL_CALL]"
            )
            .as_deref(),
            Some("internal_payload_marker")
        );

        let suppressed =
            response_with_runtime("内容已被过滤。", Some(LlmFinishReason::ContentFilter), None);
        assert_eq!(
            external_channel_model_reply_rejection_reason(&suppressed).as_deref(),
            Some("model_output_suppressed")
        );
    }

    #[test]
    fn provider_failure_reason_takes_precedence() {
        let failed = response_with_runtime(
            "已收到指令。",
            Some(LlmFinishReason::ContentFilter),
            Some(LlmProviderFailure {
                kind: LlmProviderFailureKind::HttpStatus,
                message: "scripted failure".to_string(),
            }),
        );

        assert_eq!(
            external_channel_model_reply_rejection_reason(&failed).as_deref(),
            Some("provider_failure:http_status")
        );
    }
}
