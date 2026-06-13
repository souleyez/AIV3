use serde_json::Value;

pub(crate) fn parse_manifest_context_binding(
    value: &str,
) -> Option<contracts::ManifestContextBindingView> {
    match value {
        "creation_time" => Some(contracts::ManifestContextBindingView::CreationTime),
        _ => None,
    }
}

fn parse_manifest_runtime_mode(value: &str) -> Option<contracts::ManifestRuntimeModeView> {
    match value {
        "placeholder" => Some(contracts::ManifestRuntimeModeView::Placeholder),
        "provider" => Some(contracts::ManifestRuntimeModeView::Provider),
        _ => None,
    }
}

pub(crate) fn parse_manifest_finish_reason(value: &str) -> contracts::ManifestFinishReasonView {
    match value {
        "stop" => contracts::ManifestFinishReasonView::Stop,
        "tool_calls" => contracts::ManifestFinishReasonView::ToolCalls,
        "length" => contracts::ManifestFinishReasonView::Length,
        "content_filter" => contracts::ManifestFinishReasonView::ContentFilter,
        "error" => contracts::ManifestFinishReasonView::Error,
        _ => contracts::ManifestFinishReasonView::Other(value.to_string()),
    }
}

fn parse_manifest_provider_failure_kind(
    value: &str,
) -> Option<contracts::ManifestProviderFailureKindView> {
    match value {
        "request_failed" => Some(contracts::ManifestProviderFailureKindView::RequestFailed),
        "request_timeout" => Some(contracts::ManifestProviderFailureKindView::RequestTimeout),
        "http_status" => Some(contracts::ManifestProviderFailureKindView::HttpStatus),
        "response_body_read_failed" => {
            Some(contracts::ManifestProviderFailureKindView::ResponseBodyReadFailed)
        }
        "invalid_json" => Some(contracts::ManifestProviderFailureKindView::InvalidJson),
        "invalid_response" => Some(contracts::ManifestProviderFailureKindView::InvalidResponse),
        "finish_reason_error" => {
            Some(contracts::ManifestProviderFailureKindView::FinishReasonError)
        }
        _ => None,
    }
}

pub(crate) fn parse_manifest_provider_failure(
    value: &Value,
) -> Option<contracts::ManifestProviderFailureView> {
    let object = value.as_object()?;
    Some(contracts::ManifestProviderFailureView {
        kind: parse_manifest_provider_failure_kind(object.get("kind")?.as_str()?)?,
        message: object.get("message")?.as_str()?.to_string(),
    })
}

fn parse_manifest_usage(value: &Value) -> Option<contracts::ManifestTokenUsageView> {
    let object = value.as_object()?;

    Some(contracts::ManifestTokenUsageView {
        input_tokens: object.get("input_tokens")?.as_u64()? as usize,
        output_tokens: object.get("output_tokens")?.as_u64()? as usize,
        total_tokens: object.get("total_tokens")?.as_u64()? as usize,
    })
}

pub(crate) fn parse_manifest_runtime(value: &Value) -> Option<contracts::ManifestRuntimeView> {
    let object = value.as_object()?;

    Some(contracts::ManifestRuntimeView {
        mode: parse_manifest_runtime_mode(object.get("mode")?.as_str()?)?,
        provider: object
            .get("provider")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: object
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        request_id: object
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        finish_reason: object
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(parse_manifest_finish_reason),
        provider_failure: object
            .get("provider_failure")
            .and_then(parse_manifest_provider_failure),
        latency_ms: object.get("latency_ms").and_then(Value::as_u64),
        usage: object.get("usage").and_then(parse_manifest_usage),
        system_prompt_key: object
            .get("system_prompt_key")
            .and_then(Value::as_str)
            .map(str::to_string),
        system_prompt_version: object
            .get("system_prompt_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        tool_trace_count: object
            .get("tool_trace_count")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn manifest_runtime_preserves_provider_metadata_usage_and_failure() {
        let runtime = parse_manifest_runtime(&json!({
            "mode": "provider",
            "provider": "openai",
            "model": "gpt-5.1",
            "request_id": "req_123",
            "finish_reason": "provider_custom",
            "provider_failure": {
                "kind": "http_status",
                "message": "upstream returned HTTP 502"
            },
            "latency_ms": 2500,
            "usage": {
                "input_tokens": 11,
                "output_tokens": 22,
                "total_tokens": 33
            },
            "system_prompt_key": "dataset.output",
            "system_prompt_version": "v3",
            "tool_trace_count": 4
        }))
        .expect("valid provider runtime should parse");

        assert_eq!(runtime.mode, contracts::ManifestRuntimeModeView::Provider);
        assert_eq!(runtime.provider.as_deref(), Some("openai"));
        assert_eq!(runtime.model.as_deref(), Some("gpt-5.1"));
        assert_eq!(runtime.request_id.as_deref(), Some("req_123"));
        assert_eq!(
            runtime.finish_reason,
            Some(contracts::ManifestFinishReasonView::Other(
                "provider_custom".to_string()
            ))
        );
        assert_eq!(
            runtime
                .provider_failure
                .as_ref()
                .map(|failure| &failure.kind),
            Some(&contracts::ManifestProviderFailureKindView::HttpStatus)
        );
        assert_eq!(
            runtime
                .provider_failure
                .as_ref()
                .map(|failure| failure.message.as_str()),
            Some("upstream returned HTTP 502")
        );
        assert_eq!(runtime.latency_ms, Some(2500));
        assert_eq!(
            runtime.usage.as_ref().map(|usage| usage.total_tokens),
            Some(33)
        );
        assert_eq!(runtime.system_prompt_key.as_deref(), Some("dataset.output"));
        assert_eq!(runtime.system_prompt_version.as_deref(), Some("v3"));
        assert_eq!(runtime.tool_trace_count, Some(4));
    }

    #[test]
    fn manifest_runtime_rejects_unknown_mode_and_unknown_failure_kind() {
        assert!(parse_manifest_runtime(&json!({"mode": "streaming"})).is_none());
        assert!(parse_manifest_provider_failure(&json!({
            "kind": "unknown",
            "message": "ignored"
        }))
        .is_none());
    }

    #[test]
    fn manifest_context_binding_and_finish_reason_keep_known_and_custom_values() {
        assert_eq!(
            parse_manifest_context_binding("creation_time"),
            Some(contracts::ManifestContextBindingView::CreationTime)
        );
        assert_eq!(parse_manifest_context_binding("runtime"), None);
        assert_eq!(
            parse_manifest_finish_reason("tool_calls"),
            contracts::ManifestFinishReasonView::ToolCalls
        );
        assert_eq!(
            parse_manifest_finish_reason("provider_custom"),
            contracts::ManifestFinishReasonView::Other("provider_custom".to_string())
        );
    }
}
