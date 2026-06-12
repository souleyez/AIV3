pub(crate) fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

pub(crate) fn non_empty_trimmed_string(value: &str) -> Option<String> {
    let trimmed = value.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_optional_preserves_existing_empty_and_whitespace_semantics() {
        assert_eq!(trim_optional(None), None);
        assert_eq!(trim_optional(Some("   ".to_string())), None);
        assert_eq!(
            trim_optional(Some("  dataset title  ".to_string())),
            Some("dataset title".to_string())
        );
    }

    #[test]
    fn non_empty_trimmed_string_preserves_existing_empty_and_whitespace_semantics() {
        assert_eq!(non_empty_trimmed_string(""), None);
        assert_eq!(non_empty_trimmed_string("  \n\t  "), None);
        assert_eq!(
            non_empty_trimmed_string("  external-source-main  "),
            Some("external-source-main".to_string())
        );
    }
}
