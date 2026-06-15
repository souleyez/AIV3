use axum::body::Body;
use axum::http::{header, HeaderValue, Response, StatusCode};
use domain_model::{StaticPageRenderOutput, StaticPageRenderOutputStatus};

use crate::ApiError;

pub(crate) fn ensure_static_page_html_ready(
    output: &StaticPageRenderOutput,
) -> std::result::Result<(), ApiError> {
    if matches!(output.status, StaticPageRenderOutputStatus::Rendered)
        && !output.html.trim().is_empty()
    {
        return Ok(());
    }
    Err(ApiError::bad_request(
        "static_page_html_not_ready",
        "static page HTML is not ready for download".to_string(),
    ))
}

pub(crate) fn static_page_html_preview_response(
    output: StaticPageRenderOutput,
) -> std::result::Result<Response<Body>, ApiError> {
    let bytes = output.html.into_bytes();
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_DISPOSITION, "inline");
    if let Ok(length) = HeaderValue::from_str(&bytes.len().to_string()) {
        builder = builder.header(header::CONTENT_LENGTH, length);
    }
    builder.body(Body::from(bytes)).map_err(|error| {
        ApiError::internal(
            "static_page_html_preview_response_failed",
            format!("failed to build static page HTML preview response: {error}"),
        )
    })
}

pub(crate) fn static_page_html_download_response(
    output: StaticPageRenderOutput,
) -> std::result::Result<Response<Body>, ApiError> {
    ensure_static_page_html_ready(&output)?;
    let file_name = format!("v3-static-page-{}.html", output.id);
    let bytes = output.html.into_bytes();
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file_name}\""),
        );
    if let Ok(length) = HeaderValue::from_str(&bytes.len().to_string()) {
        builder = builder.header(header::CONTENT_LENGTH, length);
    }
    builder.body(Body::from(bytes)).map_err(|error| {
        ApiError::internal(
            "static_page_html_download_response_failed",
            format!("failed to build static page HTML download response: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftId, StaticPageRenderOutputId, TenantId};
    use serde_json::json;

    fn render_output(status: StaticPageRenderOutputStatus, html: &str) -> StaticPageRenderOutput {
        StaticPageRenderOutput {
            id: StaticPageRenderOutputId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            image_job_id: None,
            status,
            html: html.to_string(),
            asset_manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn html_ready_requires_rendered_status_and_non_blank_html() {
        ensure_static_page_html_ready(&render_output(
            StaticPageRenderOutputStatus::Rendered,
            "<html>ok</html>",
        ))
        .expect("rendered HTML should be ready");

        let failed = ensure_static_page_html_ready(&render_output(
            StaticPageRenderOutputStatus::Rendered,
            "   ",
        ))
        .expect_err("blank HTML is not ready");
        assert_eq!(failed.payload.code, "static_page_html_not_ready");

        let rendering = ensure_static_page_html_ready(&render_output(
            StaticPageRenderOutputStatus::Rendering,
            "<html>pending</html>",
        ))
        .expect_err("non-rendered output is not ready");
        assert_eq!(rendering.payload.code, "static_page_html_not_ready");
    }

    #[test]
    fn preview_response_uses_inline_html_headers() {
        let html = "<html><body>preview</body></html>";
        let response = static_page_html_preview_response(render_output(
            StaticPageRenderOutputStatus::Rendered,
            html,
        ))
        .expect("preview response should build");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "inline"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            &html.len().to_string()
        );
    }

    #[test]
    fn download_response_requires_ready_html_and_uses_attachment_headers() {
        let html = "<html><body>download</body></html>";
        let output = render_output(StaticPageRenderOutputStatus::Rendered, html);
        let output_id = output.id;
        let response =
            static_page_html_download_response(output).expect("download response should build");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            &format!("attachment; filename=\"v3-static-page-{output_id}.html\"")
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            &html.len().to_string()
        );

        let error = static_page_html_download_response(render_output(
            StaticPageRenderOutputStatus::Queued,
            html,
        ))
        .expect_err("download should reject queued output");
        assert_eq!(error.payload.code, "static_page_html_not_ready");
    }
}
