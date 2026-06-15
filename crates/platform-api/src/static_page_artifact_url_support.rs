use crate::{
    codex_host_fixed_task_public_artifact_url_allowed,
    external_channel_generated_artifact_public_base_url,
};

pub(crate) fn static_page_prompt_generated_artifact_urls(prompt: &str) -> Vec<String> {
    let prefixes = vec![
        "https://v3.elepcloud.com/generated-artifacts/".to_string(),
        format!(
            "{}/",
            external_channel_generated_artifact_public_base_url().trim_end_matches('/')
        ),
    ];
    let mut urls = Vec::new();
    for prefix in prefixes {
        let mut search_from = 0usize;
        while let Some(offset) = prompt[search_from..].find(&prefix) {
            let start = search_from + offset;
            let raw = &prompt[start..];
            let end = raw
                .find(|ch: char| {
                    ch.is_whitespace()
                        || matches!(
                            ch,
                            '"' | '\''
                                | '<'
                                | '>'
                                | '，'
                                | '。'
                                | '、'
                                | '；'
                                | '（'
                                | '）'
                                | '('
                                | ')'
                                | '【'
                                | '】'
                                | '['
                                | ']'
                        )
                })
                .unwrap_or(raw.len());
            let candidate = raw[..end].trim_end_matches(|ch: char| {
                matches!(ch, ',' | '.' | ';' | ':' | '，' | '。' | '；')
            });
            if let Ok(mut url) = reqwest::Url::parse(candidate) {
                url.set_query(None);
                url.set_fragment(None);
                let normalized = url.as_str().trim_end_matches('/').to_string();
                if codex_host_fixed_task_public_artifact_url_allowed(&normalized)
                    && !urls.iter().any(|existing| existing == &normalized)
                {
                    urls.push(normalized);
                }
            }
            search_from = start + end.max(prefix.len());
            if search_from >= prompt.len() {
                break;
            }
        }
    }
    urls
}

pub(crate) fn static_page_artifact_sibling_url(
    public_url: &str,
    file_name: &str,
) -> Option<String> {
    let mut url = reqwest::Url::parse(public_url).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    let mut segments = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())?;
    if segments
        .last()
        .is_some_and(|segment| segment.eq_ignore_ascii_case("index.html"))
    {
        segments.pop();
    }
    segments.push(file_name);
    url.set_path(&format!("/{}", segments.join("/")));
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_generated_artifact_urls_normalize_dedupe_and_strip_query_fragment() {
        let prompt = concat!(
            "更新页面 https://v3.elepcloud.com/generated-artifacts/database-static-pages/x/index.html?focus=a#frag，",
            "再看 https://v3.elepcloud.com/generated-artifacts/database-static-pages/x/index.html?focus=b."
        );

        assert_eq!(
            static_page_prompt_generated_artifact_urls(prompt),
            vec![
                "https://v3.elepcloud.com/generated-artifacts/database-static-pages/x/index.html"
                    .to_string()
            ]
        );
    }

    #[test]
    fn prompt_generated_artifact_urls_reject_pending_and_disallowed_hosts() {
        let prompt = concat!(
            "pending https://v3.elepcloud.com/generated-artifacts/pending-abc/index.html ",
            "external https://example.com/generated-artifacts/database-static-pages/x/index.html"
        );

        assert!(static_page_prompt_generated_artifact_urls(prompt).is_empty());
    }

    #[test]
    fn artifact_sibling_url_replaces_index_with_requested_file_and_strips_query() {
        assert_eq!(
            static_page_artifact_sibling_url(
                "https://v3.elepcloud.com/generated-artifacts/database-static-pages/x/index.html?focus=x#frag",
                "table-data.csv",
            ),
            Some(
                "https://v3.elepcloud.com/generated-artifacts/database-static-pages/x/table-data.csv"
                    .to_string()
            )
        );
    }

    #[test]
    fn artifact_sibling_url_appends_after_non_index_leaf() {
        assert_eq!(
            static_page_artifact_sibling_url(
                "https://v3.elepcloud.com/generated-artifacts/database-static-pages/x/current",
                "report.md",
            ),
            Some(
                "https://v3.elepcloud.com/generated-artifacts/database-static-pages/x/current/report.md"
                    .to_string()
            )
        );
    }
}
