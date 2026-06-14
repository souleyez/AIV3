pub(crate) fn model_gateway_u64_to_i32(value: u64) -> Option<i32> {
    i32::try_from(value).ok()
}

pub(crate) fn model_gateway_sanitized_error_kind(reason: &str) -> String {
    reason
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.'))
        .take(120)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_gateway_u64_to_i32_preserves_in_range_values_and_rejects_overflow() {
        assert_eq!(model_gateway_u64_to_i32(0), Some(0));
        assert_eq!(model_gateway_u64_to_i32(i32::MAX as u64), Some(i32::MAX));
        assert_eq!(model_gateway_u64_to_i32(i32::MAX as u64 + 1), None);
    }

    #[test]
    fn model_gateway_sanitized_error_kind_keeps_safe_ascii_and_truncates() {
        let raw = format!(
            "provider_error:429.rate_limit-too_many_requests_{}中文 !@#",
            "x".repeat(160)
        );
        let sanitized = model_gateway_sanitized_error_kind(&raw);

        assert!(sanitized.starts_with("provider_error:429.rate_limit-too_many_requests_"));
        assert_eq!(sanitized.len(), 120);
        assert!(sanitized
            .chars()
            .all(|ch| { ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.') }));
    }
}
