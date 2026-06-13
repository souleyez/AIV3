use crate::{
    DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT, HTML_ARTIFACT_LIST_DEFAULT_LIMIT,
    HTML_ARTIFACT_LIST_MAX_LIMIT, RETRIEVAL_SEARCH_BACKEND_ENV, RETRIEVAL_SEARCH_DEFAULT_LIMIT,
    RETRIEVAL_SEARCH_MAX_LIMIT, STATIC_PAGE_DRAFT_LIST_DEFAULT_LIMIT,
    STATIC_PAGE_DRAFT_LIST_MAX_LIMIT,
};

pub(crate) fn normalize_retrieval_search_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(RETRIEVAL_SEARCH_DEFAULT_LIMIT)
        .clamp(1, RETRIEVAL_SEARCH_MAX_LIMIT)
}

pub(crate) fn retrieval_search_scan_limit(limit: usize) -> i64 {
    let _ = limit;
    DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetrievalSearchBackend {
    LegacyScan,
    PostgresLexical,
}

pub(crate) fn retrieval_search_backend() -> RetrievalSearchBackend {
    parse_retrieval_search_backend(
        &std::env::var(RETRIEVAL_SEARCH_BACKEND_ENV).unwrap_or_else(|_| "legacy_scan".to_string()),
    )
}

pub(crate) fn parse_retrieval_search_backend(value: &str) -> RetrievalSearchBackend {
    match value.trim().to_ascii_lowercase().as_str() {
        "postgres_lexical" => RetrievalSearchBackend::PostgresLexical,
        "legacy_scan" | "" => RetrievalSearchBackend::LegacyScan,
        _ => RetrievalSearchBackend::LegacyScan,
    }
}

pub(crate) fn retrieval_search_candidate_limit(limit: usize) -> usize {
    limit.saturating_mul(16).clamp(32, 256)
}

pub(crate) fn normalize_static_page_draft_list_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(STATIC_PAGE_DRAFT_LIST_DEFAULT_LIMIT)
        .max(1)
        .min(STATIC_PAGE_DRAFT_LIST_MAX_LIMIT)
}

pub(crate) fn normalize_html_artifact_list_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(HTML_ARTIFACT_LIST_DEFAULT_LIMIT)
        .max(1)
        .min(HTML_ARTIFACT_LIST_MAX_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieval_search_limit_keeps_default_and_clamp_bounds() {
        assert_eq!(
            normalize_retrieval_search_limit(None),
            RETRIEVAL_SEARCH_DEFAULT_LIMIT
        );
        assert_eq!(normalize_retrieval_search_limit(Some(0)), 1);
        assert_eq!(normalize_retrieval_search_limit(Some(3)), 3);
        assert_eq!(
            normalize_retrieval_search_limit(Some(999)),
            RETRIEVAL_SEARCH_MAX_LIMIT
        );
    }

    #[test]
    fn retrieval_search_backend_and_candidates_keep_existing_fallbacks() {
        assert_eq!(
            parse_retrieval_search_backend(" POSTGRES_LEXICAL "),
            RetrievalSearchBackend::PostgresLexical
        );
        assert_eq!(
            parse_retrieval_search_backend("legacy_scan"),
            RetrievalSearchBackend::LegacyScan
        );
        assert_eq!(
            parse_retrieval_search_backend("unknown"),
            RetrievalSearchBackend::LegacyScan
        );
        assert_eq!(
            retrieval_search_scan_limit(4),
            DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT
        );
        assert_eq!(retrieval_search_candidate_limit(1), 32);
        assert_eq!(retrieval_search_candidate_limit(8), 128);
        assert_eq!(retrieval_search_candidate_limit(40), 256);
    }

    #[test]
    fn list_limits_keep_defaults_and_bounds() {
        assert_eq!(
            normalize_static_page_draft_list_limit(None),
            STATIC_PAGE_DRAFT_LIST_DEFAULT_LIMIT
        );
        assert_eq!(normalize_static_page_draft_list_limit(Some(0)), 1);
        assert_eq!(
            normalize_static_page_draft_list_limit(Some(999)),
            STATIC_PAGE_DRAFT_LIST_MAX_LIMIT
        );

        assert_eq!(
            normalize_html_artifact_list_limit(None),
            HTML_ARTIFACT_LIST_DEFAULT_LIMIT
        );
        assert_eq!(normalize_html_artifact_list_limit(Some(-5)), 1);
        assert_eq!(
            normalize_html_artifact_list_limit(Some(999)),
            HTML_ARTIFACT_LIST_MAX_LIMIT
        );
    }
}
