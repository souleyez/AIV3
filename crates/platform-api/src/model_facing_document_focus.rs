#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelFacingDocumentFocus {
    Unknown,
    SingleDocument,
    MultiDocument,
}

pub(crate) fn format_model_facing_document_focus(value: ModelFacingDocumentFocus) -> &'static str {
    match value {
        ModelFacingDocumentFocus::Unknown => "unknown",
        ModelFacingDocumentFocus::SingleDocument => "single_document",
        ModelFacingDocumentFocus::MultiDocument => "multi_document",
    }
}

pub(crate) fn infer_model_facing_document_focus(
    distinct_document_count: usize,
    indexed_document_count: usize,
) -> ModelFacingDocumentFocus {
    let count = if distinct_document_count > 0 {
        distinct_document_count
    } else {
        indexed_document_count
    };

    match count {
        0 => ModelFacingDocumentFocus::Unknown,
        1 => ModelFacingDocumentFocus::SingleDocument,
        _ => ModelFacingDocumentFocus::MultiDocument,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_focus_formatters_keep_protocol_strings() {
        assert_eq!(
            format_model_facing_document_focus(ModelFacingDocumentFocus::Unknown),
            "unknown",
        );
        assert_eq!(
            format_model_facing_document_focus(ModelFacingDocumentFocus::SingleDocument),
            "single_document",
        );
        assert_eq!(
            format_model_facing_document_focus(ModelFacingDocumentFocus::MultiDocument),
            "multi_document",
        );
    }

    #[test]
    fn document_focus_prefers_distinct_document_count_before_indexed_count() {
        assert_eq!(
            infer_model_facing_document_focus(0, 0),
            ModelFacingDocumentFocus::Unknown,
        );
        assert_eq!(
            infer_model_facing_document_focus(0, 1),
            ModelFacingDocumentFocus::SingleDocument,
        );
        assert_eq!(
            infer_model_facing_document_focus(0, 2),
            ModelFacingDocumentFocus::MultiDocument,
        );
        assert_eq!(
            infer_model_facing_document_focus(1, 8),
            ModelFacingDocumentFocus::SingleDocument,
        );
        assert_eq!(
            infer_model_facing_document_focus(2, 1),
            ModelFacingDocumentFocus::MultiDocument,
        );
    }
}
