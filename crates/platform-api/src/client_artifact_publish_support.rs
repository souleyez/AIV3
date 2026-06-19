use axum::http::{header, HeaderValue, Response, StatusCode};
use chrono::{DateTime, SecondsFormat, Utc};
use contracts::{
    ClientArtifactFileRecordView, ClientArtifactView, HtmlArtifactDataRefView,
    HtmlArtifactInteractionModeView, HtmlArtifactManifestView, HtmlArtifactOwnerScopeView,
    HtmlArtifactProvenanceView,
};
use domain_model::UserId;
use serde_json::{json, Value};
use std::{fs, path::Path};
use storage::NewHtmlArtifact;

use crate::{
    client_artifact_error_support::client_artifact_not_found_error,
    client_artifact_storage_support::{
        read_client_artifact_file_storage, ClientArtifactFileStorageRecord,
    },
    client_artifact_view_support::{
        load_client_artifact_preview_file, load_client_artifact_publish_record,
        load_client_artifact_view,
    },
    html_artifact_summary_support::html_artifact_serialized_variant,
    resource_access::ensure_owner_managed_resource,
    ApiError, AppState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientArtifactPublicHtmlCandidate {
    pub(crate) file_index: i32,
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) role: String,
    pub(crate) size_bytes: i64,
    pub(crate) storage_kind: String,
    pub(crate) object_locator: Option<String>,
    pub(crate) database_bytes: Option<Vec<u8>>,
}

pub(crate) struct ClientArtifactPreparedPublicHtmlFile {
    pub(crate) file_index: i32,
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) role: String,
    pub(crate) size_bytes: i64,
    pub(crate) sanitized_html: String,
}

pub(crate) struct ClientArtifactPublicHtmlFilePublication {
    pub(crate) file_index: i32,
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) role: String,
    pub(crate) size_bytes: i64,
    pub(crate) relative_dir: String,
    pub(crate) public_html: String,
}

pub(crate) fn client_artifact_file_download_url(artifact_id: &str, file_index: i32) -> String {
    format!(
        "/v1/client-artifacts/{}/files/{}",
        artifact_id.trim(),
        file_index.max(0)
    )
}

pub(crate) fn client_artifact_file_preview_url(
    artifact_id: &str,
    file_index: i32,
    filename: &str,
    content_type: &str,
    role: &str,
    artifact_status: &str,
) -> Option<String> {
    if artifact_status != "published" || !client_artifact_file_is_html(filename, content_type, role)
    {
        return None;
    }
    Some(format!(
        "/v1/client-artifacts/{}/files/{}/preview",
        artifact_id.trim(),
        file_index.max(0)
    ))
}

pub(crate) fn client_artifact_file_public_url(manifest: &Value, file_index: i32) -> Option<String> {
    let public_html = manifest
        .pointer("/metadata/v3_public_html")
        .and_then(Value::as_object)?;
    if public_html
        .get("file_index")
        .and_then(Value::as_i64)
        .is_some_and(|value| value == file_index as i64)
    {
        return public_html
            .get("public_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
    }
    None
}

pub(crate) fn client_artifact_file_record_view(
    artifact_id: &str,
    artifact_status: &str,
    manifest: &Value,
    file_index: i32,
    filename: String,
    content_type: String,
    role: String,
    size_bytes: i64,
    sha256: String,
) -> ClientArtifactFileRecordView {
    ClientArtifactFileRecordView {
        file_index,
        download_url: client_artifact_file_download_url(artifact_id, file_index),
        preview_url: client_artifact_file_preview_url(
            artifact_id,
            file_index,
            &filename,
            &content_type,
            &role,
            artifact_status,
        ),
        public_url: client_artifact_file_public_url(manifest, file_index),
        filename,
        content_type,
        role,
        size_bytes,
        sha256,
    }
}

pub(crate) fn client_artifact_file_is_html(filename: &str, content_type: &str, role: &str) -> bool {
    let role = role.trim();
    let content_type = content_type.trim().to_ascii_lowercase();
    let filename = filename.trim().to_ascii_lowercase();
    role == "primary_html" || content_type.contains("html") || filename.ends_with(".html")
}

pub(crate) fn select_client_artifact_public_html_candidate(
    candidates: Vec<ClientArtifactPublicHtmlCandidate>,
) -> std::result::Result<ClientArtifactPublicHtmlCandidate, ApiError> {
    candidates
        .into_iter()
        .find(|candidate| {
            client_artifact_file_is_html(
                &candidate.filename,
                &candidate.content_type,
                &candidate.role,
            )
        })
        .ok_or_else(|| {
            ApiError::bad_request(
                "client_artifact_public_html_not_found",
                "client artifact does not contain an HTML file that can be published".to_string(),
            )
        })
}

pub(crate) fn prepare_client_artifact_public_html_candidate(
    candidate: ClientArtifactPublicHtmlCandidate,
) -> std::result::Result<ClientArtifactPreparedPublicHtmlFile, ApiError> {
    let bytes = read_client_artifact_file_storage(&ClientArtifactFileStorageRecord {
        storage_kind: candidate.storage_kind,
        object_locator: candidate.object_locator,
        database_bytes: candidate.database_bytes,
    })?;
    let sanitized_html = sanitize_client_artifact_html_bytes(
        bytes,
        "public HTML publication requires UTF-8 encoded content",
    )?;
    Ok(ClientArtifactPreparedPublicHtmlFile {
        file_index: candidate.file_index,
        filename: candidate.filename,
        content_type: candidate.content_type,
        role: candidate.role,
        size_bytes: candidate.size_bytes,
        sanitized_html,
    })
}

pub(crate) fn build_client_artifact_public_html_file_publication(
    artifact_id: &str,
    title: &str,
    file: ClientArtifactPreparedPublicHtmlFile,
) -> std::result::Result<ClientArtifactPublicHtmlFilePublication, ApiError> {
    let public_html =
        client_artifact_sandboxed_html_document(title, &file.filename, &file.sanitized_html);
    let relative_dir = client_artifact_public_html_relative_dir(artifact_id, file.file_index)?;
    Ok(ClientArtifactPublicHtmlFilePublication {
        file_index: file.file_index,
        filename: file.filename,
        content_type: file.content_type,
        role: file.role,
        size_bytes: file.size_bytes,
        relative_dir,
        public_html,
    })
}

pub(crate) fn ensure_client_artifact_can_publish_public_html(
    status: &str,
) -> std::result::Result<(), ApiError> {
    if status != "published" {
        return Err(ApiError::bad_request(
            "client_artifact_not_published",
            "client artifact must be privately published before public HTML publication is available"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn ensure_client_artifact_file_can_preview_html(
    artifact_status: &str,
    filename: &str,
    content_type: &str,
    role: &str,
    size_bytes: i64,
    max_size_bytes: i64,
) -> std::result::Result<(), ApiError> {
    if artifact_status != "published" {
        return Err(ApiError::bad_request(
            "client_artifact_not_published",
            "client artifact must be published before HTML preview is available".to_string(),
        ));
    }
    if !client_artifact_file_is_html(filename, content_type, role) {
        return Err(ApiError::bad_request(
            "client_artifact_preview_not_html",
            "only HTML client artifact files can be previewed inline".to_string(),
        ));
    }
    if size_bytes > max_size_bytes {
        return Err(ApiError::bad_request(
            "client_artifact_preview_too_large",
            format!("HTML preview is limited to {max_size_bytes} bytes"),
        ));
    }
    Ok(())
}

pub(crate) fn client_artifact_file_download_response(
    filename: &str,
    content_type: &str,
    bytes: Vec<u8>,
) -> std::result::Result<Response<axum::body::Body>, ApiError> {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename.replace('"', "")),
        );
    if let Ok(length) = HeaderValue::from_str(&bytes.len().to_string()) {
        builder = builder.header(header::CONTENT_LENGTH, length);
    }
    builder
        .body(axum::body::Body::from(bytes))
        .map_err(|error| {
            ApiError::internal(
                "client_artifact_download_response_failed",
                format!("failed to build client artifact download response: {error}"),
            )
        })
}

pub(crate) fn client_artifact_html_preview_response(
    title: &str,
    filename: &str,
    sanitized_html: &str,
) -> std::result::Result<Response<axum::body::Body>, ApiError> {
    let html = client_artifact_sandboxed_html_document(title, filename, sanitized_html);
    let bytes = html.into_bytes();
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_DISPOSITION, "inline")
        .header(
            "content-security-policy",
            "default-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'self' data: blob:; img-src data:; style-src 'unsafe-inline'; script-src 'none'; connect-src 'none'",
        )
        .header("referrer-policy", "no-referrer");
    if let Ok(length) = HeaderValue::from_str(&bytes.len().to_string()) {
        builder = builder.header(header::CONTENT_LENGTH, length);
    }
    builder
        .body(axum::body::Body::from(bytes))
        .map_err(|error| {
            ApiError::internal(
                "client_artifact_preview_response_failed",
                format!("failed to build client artifact preview response: {error}"),
            )
        })
}

pub(crate) fn client_artifact_sandboxed_html_document(
    title: &str,
    filename: &str,
    sanitized_html: &str,
) -> String {
    let display_title = trim_or_default(title, "DataMax Client Artifact Preview");
    let escaped_title = escape_html_text(&display_title);
    let escaped_filename = escape_html_text(filename);
    let escaped_srcdoc = escape_html_attr(sanitized_html);
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{escaped_title}</title>
  <style>
    :root {{ color-scheme: light; }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f8fb; color: #182033; }}
    header {{ display: flex; justify-content: space-between; gap: 16px; align-items: center; padding: 12px 16px; border-bottom: 1px solid #d8e0ec; background: #fff; }}
    header strong {{ font-size: 14px; }}
    header span {{ font-size: 12px; color: #667085; }}
    iframe {{ width: 100%; height: calc(100vh - 50px); border: 0; background: #fff; }}
  </style>
</head>
<body>
  <header>
    <strong>{escaped_title}</strong>
    <span>安全沙箱预览 · {escaped_filename}</span>
  </header>
  <iframe sandbox="" referrerpolicy="no-referrer" srcdoc="{escaped_srcdoc}"></iframe>
</body>
</html>"#,
    )
}

pub(crate) fn client_artifact_public_html_relative_dir(
    artifact_id: &str,
    file_index: i32,
) -> std::result::Result<String, ApiError> {
    let artifact_id = artifact_id.trim();
    if file_index < 0 || !safe_generated_artifact_path_segment(artifact_id) {
        return Err(ApiError::internal(
            "client_artifact_public_path_invalid",
            "client artifact public path contains an unsafe segment".to_string(),
        ));
    }
    Ok(format!("client-artifacts/{artifact_id}/html-{file_index}"))
}

pub(crate) fn client_artifact_public_html_artifact_manifest(
    artifact_id: &str,
    title: &str,
    file_index: i32,
    filename: &str,
    content_type: &str,
    role: &str,
    size_bytes: i64,
    public_url: &str,
    dataset_ids: &[String],
    asset_library_ids: &[String],
    published_at: DateTime<Utc>,
) -> HtmlArtifactManifestView {
    let mut data_refs = Vec::new();
    data_refs.extend(dataset_ids.iter().map(|id| HtmlArtifactDataRefView {
        kind: "dataset".to_string(),
        id: id.clone(),
        label: id.clone(),
    }));
    data_refs.extend(asset_library_ids.iter().map(|id| HtmlArtifactDataRefView {
        kind: "asset_library".to_string(),
        id: id.clone(),
        label: id.clone(),
    }));
    let download_url = client_artifact_file_download_url(artifact_id, file_index);
    let preview_url = client_artifact_file_preview_url(
        artifact_id,
        file_index,
        filename,
        content_type,
        role,
        "published",
    );
    HtmlArtifactManifestView {
        kind: "html_artifact".to_string(),
        version: 1,
        id: format!("html-artifact-client-{artifact_id}-{file_index}"),
        title: trim_or_default(title, "客户端上传 HTML 产物"),
        source_type: contracts::HtmlArtifactSourceTypeView::ClientArtifact,
        template_id: contracts::HtmlArtifactTemplateIdView::ClientArtifactHtml,
        owner_scope: HtmlArtifactOwnerScopeView {
            scope_type: "v3_client_artifact".to_string(),
            id: artifact_id.to_string(),
        },
        data_refs,
        provenance: HtmlArtifactProvenanceView {
            producer: "v3-client-artifact-publisher".to_string(),
            reason: "client artifact public HTML sandbox publication".to_string(),
            source_run_id: None,
        },
        interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
        created_at: published_at,
        payload: json!({
            "status": "published",
            "mode": "anonymous_public_sandbox_html",
            "public_url": public_url,
            "generated_artifact_url": public_url,
            "artifact_id": artifact_id,
            "file_index": file_index,
            "filename": filename,
            "content_type": content_type,
            "role": role,
            "size_bytes": size_bytes,
            "download_url": download_url,
            "preview_url": preview_url,
            "published_at": published_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        }),
    }
}

pub(crate) fn client_artifact_html_artifact_manifest_value(
    manifest: &HtmlArtifactManifestView,
) -> std::result::Result<Value, ApiError> {
    serde_json::to_value(manifest).map_err(|error| {
        ApiError::internal(
            "html_artifact_serialize_failed",
            format!("failed to serialize client artifact HTML manifest: {error}"),
        )
    })
}

pub(crate) async fn mark_client_artifact_private_published(
    state: &AppState,
    artifact_id: &str,
    manifest: &Value,
) -> std::result::Result<(), ApiError> {
    let result = sqlx::query(
        r#"
        update v3_client_artifacts
        set status = 'published', manifest = $3, updated_at = now()
        where tenant_id = $1 and artifact_id = $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(artifact_id)
    .bind(manifest)
    .execute(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?;
    if result.rows_affected() == 0 {
        return Err(client_artifact_not_found_error(artifact_id));
    }
    Ok(())
}

pub(crate) async fn finalize_client_artifact_private_publication(
    state: &AppState,
    artifact_id: &str,
    manifest: Value,
    published_at: DateTime<Utc>,
) -> std::result::Result<(), ApiError> {
    let manifest = mark_client_artifact_manifest_published(manifest, published_at)?;
    mark_client_artifact_private_published(state, artifact_id, &manifest).await
}

pub(crate) async fn publish_client_artifact_private_and_load_view(
    state: &AppState,
    artifact_id: &str,
    current_user_id: UserId,
    published_at: DateTime<Utc>,
) -> std::result::Result<ClientArtifactView, ApiError> {
    let artifact = load_client_artifact_publish_record(state, artifact_id).await?;
    ensure_owner_managed_resource(
        "client_artifact",
        artifact_id.to_string(),
        artifact.owner_user_id,
        Some(current_user_id),
    )?;
    finalize_client_artifact_private_publication(
        state,
        artifact_id,
        artifact.manifest,
        published_at,
    )
    .await?;
    load_client_artifact_view(state, artifact_id).await
}

pub(crate) async fn client_artifact_html_preview_response_for_user(
    state: &AppState,
    artifact_id: &str,
    file_index: i32,
    current_user_id: UserId,
    max_html_preview_bytes: i64,
) -> std::result::Result<Response<axum::body::Body>, ApiError> {
    let file = load_client_artifact_preview_file(state, artifact_id, file_index).await?;
    ensure_owner_managed_resource(
        "client_artifact",
        artifact_id.trim().to_string(),
        file.owner_user_id,
        Some(current_user_id),
    )?;
    ensure_client_artifact_file_can_preview_html(
        &file.status,
        &file.filename,
        &file.content_type,
        &file.role,
        file.size_bytes,
        max_html_preview_bytes,
    )?;
    let sanitized = sanitize_client_artifact_html_bytes(
        file.bytes,
        "HTML preview requires UTF-8 encoded content",
    )?;
    client_artifact_html_preview_response(&file.title, &file.filename, &sanitized)
}

pub(crate) async fn update_client_artifact_public_html_manifest(
    state: &AppState,
    artifact_id: &str,
    manifest: &Value,
) -> std::result::Result<(), ApiError> {
    sqlx::query(
        r#"
        update v3_client_artifacts
        set manifest = $3, updated_at = now()
        where tenant_id = $1 and artifact_id = $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(artifact_id)
    .bind(manifest)
    .execute(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?;
    Ok(())
}

pub(crate) fn write_client_artifact_public_html_file(
    artifact_root: &Path,
    relative_dir: &str,
    public_html: &str,
) -> std::result::Result<(), ApiError> {
    let artifact_dir = artifact_root.join(relative_dir);
    fs::create_dir_all(&artifact_dir).map_err(|error| {
        ApiError::internal(
            "client_artifact_public_publish_failed",
            format!("failed to create public client artifact directory: {error}"),
        )
    })?;
    fs::write(artifact_dir.join("index.html"), public_html).map_err(|error| {
        ApiError::internal(
            "client_artifact_public_publish_failed",
            format!("failed to write public client artifact HTML: {error}"),
        )
    })?;
    Ok(())
}

pub(crate) async fn upsert_client_artifact_public_html_artifact(
    state: &AppState,
    owner_user_id: UserId,
    artifact: &HtmlArtifactManifestView,
) -> std::result::Result<(), ApiError> {
    let manifest = client_artifact_html_artifact_manifest_value(artifact)?;
    state
        .storage
        .html_artifacts()
        .upsert(
            state.tenant_id,
            &NewHtmlArtifact {
                id: artifact.id.clone(),
                owner_user_id: Some(owner_user_id),
                assistant_run_id: None,
                local_thread_id: None,
                source_type: html_artifact_serialized_variant(&artifact.source_type),
                template_id: html_artifact_serialized_variant(&artifact.template_id),
                interaction_mode: html_artifact_serialized_variant(&artifact.interaction_mode),
                manifest,
                created_at: artifact.created_at,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    Ok(())
}

pub(crate) async fn finalize_client_artifact_public_html_publication(
    state: &AppState,
    owner_user_id: UserId,
    artifact_id: &str,
    title: &str,
    artifact_manifest: Value,
    publication: &ClientArtifactPublicHtmlFilePublication,
    public_url: &str,
    dataset_ids: &[String],
    asset_library_ids: &[String],
    published_at: DateTime<Utc>,
) -> std::result::Result<HtmlArtifactManifestView, ApiError> {
    let html_artifact = client_artifact_public_html_artifact_manifest(
        artifact_id,
        title,
        publication.file_index,
        &publication.filename,
        &publication.content_type,
        &publication.role,
        publication.size_bytes,
        public_url,
        dataset_ids,
        asset_library_ids,
        published_at,
    );
    upsert_client_artifact_public_html_artifact(state, owner_user_id, &html_artifact).await?;
    let manifest = mark_client_artifact_manifest_public_html_published(
        artifact_manifest,
        publication.file_index,
        public_url,
        &html_artifact.id,
        published_at,
    )?;
    update_client_artifact_public_html_manifest(state, artifact_id, &manifest).await?;
    Ok(html_artifact)
}

pub(crate) fn sanitize_client_artifact_preview_html(html: &str) -> String {
    let mut output = html.to_string();
    for tag in ["script", "iframe", "object", "embed", "form"] {
        output = strip_html_block_case_insensitive(&output, tag);
    }
    for tag in ["link", "meta", "base"] {
        output = strip_html_tag_case_insensitive(&output, tag);
    }
    sanitize_html_tags_and_styles(&output)
}

pub(crate) fn sanitize_client_artifact_html_bytes(
    bytes: Vec<u8>,
    utf8_error_message: &'static str,
) -> std::result::Result<String, ApiError> {
    let html = String::from_utf8(bytes).map_err(|_| {
        ApiError::bad_request(
            "client_artifact_html_not_utf8",
            utf8_error_message.to_string(),
        )
    })?;
    Ok(sanitize_client_artifact_preview_html(&html))
}

pub(crate) fn mark_client_artifact_manifest_published(
    mut manifest: Value,
    published_at: DateTime<Utc>,
) -> std::result::Result<Value, ApiError> {
    if !manifest.is_object() {
        return Err(ApiError::internal(
            "client_artifact_manifest_invalid",
            "client artifact manifest must be a JSON object".to_string(),
        ));
    }
    if !manifest.get("metadata").is_some_and(Value::is_object) {
        manifest["metadata"] = json!({});
    }
    manifest["metadata"]["v3_publish"] = json!({
        "status": "published_private",
        "mode": "enterprise_private_download",
        "public_url": Value::Null,
        "published_at": published_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "html_inline_preview": false,
        "note": "Artifact is published inside V3. Public/inline HTML preview requires the V3 HTML safety pipeline.",
    });
    Ok(manifest)
}

pub(crate) fn mark_client_artifact_manifest_public_html_published(
    mut manifest: Value,
    file_index: i32,
    public_url: &str,
    html_artifact_id: &str,
    published_at: DateTime<Utc>,
) -> std::result::Result<Value, ApiError> {
    if !manifest.is_object() {
        return Err(ApiError::internal(
            "client_artifact_manifest_invalid",
            "client artifact manifest must be a JSON object".to_string(),
        ));
    }
    if !manifest.get("metadata").is_some_and(Value::is_object) {
        manifest["metadata"] = json!({});
    }
    if !manifest
        .pointer("/metadata/v3_publish")
        .is_some_and(Value::is_object)
    {
        manifest["metadata"]["v3_publish"] = json!({});
    }
    let published_at = published_at.to_rfc3339_opts(SecondsFormat::Secs, true);
    manifest["metadata"]["v3_publish"]["status"] = json!("published_public");
    manifest["metadata"]["v3_publish"]["mode"] = json!("anonymous_public_sandbox_html");
    manifest["metadata"]["v3_publish"]["public_url"] = json!(public_url);
    manifest["metadata"]["v3_publish"]["html_inline_preview"] = json!(true);
    manifest["metadata"]["v3_publish"]["published_at"] = json!(published_at.clone());
    manifest["metadata"]["v3_publish"]["note"] = json!(
        "Artifact HTML is published as a DataMax-generated public sandbox page; source file remains governed by V3 artifact permissions."
    );
    manifest["metadata"]["v3_public_html"] = json!({
        "status": "published",
        "mode": "anonymous_public_sandbox_html",
        "file_index": file_index,
        "public_url": public_url,
        "html_artifact_id": html_artifact_id,
        "published_at": published_at,
    });
    Ok(manifest)
}

fn trim_or_default(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn safe_generated_artifact_path_segment(segment: &str) -> bool {
    let segment = segment.trim();
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn strip_html_block_case_insensitive(input: &str, tag: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;
    let open_pattern = format!("<{}", tag.to_ascii_lowercase());
    let close_pattern = format!("</{}>", tag.to_ascii_lowercase());
    loop {
        let lower = remaining.to_ascii_lowercase();
        let Some(start) = lower.find(&open_pattern) else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..start]);
        let after_open = &remaining[start..];
        let after_open_lower = &lower[start..];
        if let Some(close_start) = after_open_lower.find(&close_pattern) {
            let close_end = close_start + close_pattern.len();
            remaining = &after_open[close_end..];
        } else if let Some(open_end) = after_open.find('>') {
            remaining = &after_open[open_end + 1..];
        } else {
            break;
        }
    }
    output
}

fn strip_html_tag_case_insensitive(input: &str, tag: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;
    let open_pattern = format!("<{}", tag.to_ascii_lowercase());
    loop {
        let lower = remaining.to_ascii_lowercase();
        let Some(start) = lower.find(&open_pattern) else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..start]);
        let after_open = &remaining[start..];
        if let Some(open_end) = after_open.find('>') {
            remaining = &after_open[open_end + 1..];
        } else {
            break;
        }
    }
    output
}

fn sanitize_html_tags_and_styles(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find('>') else {
            output.push_str(&escape_html_text(after));
            return output;
        };
        let tag = &after[..=end];
        output.push_str(&sanitize_html_tag(tag));
        rest = &after[end + 1..];
    }
    output.push_str(rest);
    sanitize_style_content(&output)
}

fn sanitize_html_tag(tag: &str) -> String {
    if tag.starts_with("</") || tag.starts_with("<!") || tag.starts_with("<?") {
        return tag.to_string();
    }
    let inner = tag.trim_start_matches('<').trim_end_matches('>').trim();
    let Some(name_end) = inner.find(char::is_whitespace) else {
        return tag.to_string();
    };
    let tag_name = &inner[..name_end];
    let mut attributes = Vec::new();
    for raw_attr in inner[name_end..].split_whitespace() {
        let attr = raw_attr.trim();
        if attr.is_empty() {
            continue;
        }
        let attr_name = attr
            .split_once('=')
            .map(|(name, _)| name)
            .unwrap_or(attr)
            .trim()
            .to_ascii_lowercase();
        if attr_name.starts_with("on") || attr_name == "srcdoc" {
            continue;
        }
        if matches!(
            attr_name.as_str(),
            "src" | "href" | "xlink:href" | "poster" | "action" | "formaction"
        ) && !client_artifact_preview_attr_has_safe_url(attr)
        {
            continue;
        }
        if attr_name == "style" && attr.to_ascii_lowercase().contains("url(") {
            continue;
        }
        attributes.push(attr);
    }
    if attributes.is_empty() {
        format!("<{tag_name}>")
    } else {
        format!("<{} {}>", tag_name, attributes.join(" "))
    }
}

fn client_artifact_preview_attr_has_safe_url(attr: &str) -> bool {
    let Some((_, value)) = attr.split_once('=') else {
        return false;
    };
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_ascii_lowercase();
    value.starts_with('#') || value.starts_with("data:image/")
}

fn sanitize_style_content(input: &str) -> String {
    let without_imports = input
        .replace("@import", "/* blocked import */")
        .replace("@IMPORT", "/* blocked import */");
    strip_css_url_functions(&without_imports)
}

fn strip_css_url_functions(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;
    loop {
        let lower = remaining.to_ascii_lowercase();
        let Some(start) = lower.find("url(") else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..start]);
        output.push_str("blocked-url()");
        let after_url = &remaining[start + 4..];
        if let Some(end) = after_url.find(')') {
            remaining = &after_url[end + 1..];
        } else {
            break;
        }
    }
    output
}

fn escape_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(value: &str) -> String {
    escape_html_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn client_artifact_publish_support_sanitizes_active_html() {
        let sanitized = sanitize_client_artifact_preview_html(
            r#"<html><head><script>alert(1)</script><link rel="stylesheet" href="https://x.test/a.css"></head>
            <body onload="alert(1)">
              <img src="https://x.test/a.png" onerror="alert(1)">
              <img src="data:image/png;base64,abc">
              <a href="javascript:alert(1)">bad</a>
              <iframe src="https://x.test/embed"></iframe>
              <style>@import url("https://x.test/a.css"); .hero{background:url(https://x.test/a.png)}</style>
            </body></html>"#,
        );

        assert!(!sanitized.to_ascii_lowercase().contains("<script"));
        assert!(!sanitized.to_ascii_lowercase().contains("<iframe"));
        assert!(!sanitized.to_ascii_lowercase().contains("<link"));
        assert!(!sanitized.to_ascii_lowercase().contains("onload"));
        assert!(!sanitized.to_ascii_lowercase().contains("onerror"));
        assert!(!sanitized.to_ascii_lowercase().contains("javascript:"));
        assert!(!sanitized.contains("https://x.test/a.png"));
        assert!(sanitized.contains("data:image/png;base64,abc"));
    }

    #[test]
    fn client_artifact_publish_support_marks_public_manifest_and_url() {
        let published_at = Utc
            .with_ymd_and_hms(2026, 6, 17, 9, 0, 0)
            .single()
            .expect("valid timestamp");
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/client-artifacts/v3ca_1/html-0/index.html";

        let updated = mark_client_artifact_manifest_public_html_published(
            json!({ "metadata": {} }),
            0,
            public_url,
            "html-artifact-client-v3ca_1-0",
            published_at,
        )
        .expect("public publish marker should be added");

        assert_eq!(
            updated["metadata"]["v3_publish"]["mode"],
            "anonymous_public_sandbox_html"
        );
        assert_eq!(
            client_artifact_file_public_url(&updated, 0).as_deref(),
            Some(public_url)
        );
        assert!(client_artifact_file_public_url(&updated, 1).is_none());
    }

    #[test]
    fn client_artifact_publish_support_builds_contract_manifest() {
        let published_at = Utc
            .with_ymd_and_hms(2026, 6, 17, 9, 30, 0)
            .single()
            .expect("valid timestamp");
        let artifact = client_artifact_public_html_artifact_manifest(
            "v3ca_1",
            "客户页",
            0,
            "index.html",
            "text/html",
            "primary_html",
            120,
            "https://v3.elepcloud.com/generated-artifacts/client-artifacts/v3ca_1/html-0/index.html",
            &["dataset-1".to_string()],
            &["asset-1".to_string()],
            published_at,
        );

        assert_eq!(artifact.id, "html-artifact-client-v3ca_1-0");
        assert_eq!(
            artifact.source_type,
            contracts::HtmlArtifactSourceTypeView::ClientArtifact
        );
        assert_eq!(
            artifact.template_id,
            contracts::HtmlArtifactTemplateIdView::ClientArtifactHtml
        );
        assert_eq!(artifact.owner_scope.scope_type, "v3_client_artifact");
        assert!(artifact
            .data_refs
            .iter()
            .any(|reference| reference.kind == "dataset" && reference.id == "dataset-1"));
        assert_eq!(
            artifact.payload["preview_url"],
            json!("/v1/client-artifacts/v3ca_1/files/0/preview")
        );
    }

    #[test]
    fn client_artifact_publish_support_serializes_contract_manifest() {
        let published_at = Utc
            .with_ymd_and_hms(2026, 6, 17, 9, 45, 0)
            .single()
            .expect("valid timestamp");
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/client-artifacts/v3ca_1/html-0/index.html";
        let artifact = client_artifact_public_html_artifact_manifest(
            "v3ca_1",
            "客户页",
            0,
            "index.html",
            "text/html",
            "primary_html",
            120,
            public_url,
            &["dataset-1".to_string()],
            &[],
            published_at,
        );

        let value = client_artifact_html_artifact_manifest_value(&artifact)
            .expect("manifest should serialize");

        assert_eq!(value["id"], "html-artifact-client-v3ca_1-0");
        assert_eq!(value["payload"]["status"], "published");
        assert_eq!(value["payload"]["public_url"], public_url);
    }

    #[test]
    fn client_artifact_publish_support_builds_file_record_view() {
        let manifest = json!({
            "metadata": {
                "v3_public_html": {
                    "file_index": 0,
                    "public_url": "https://v3.elepcloud.com/generated-artifacts/client-artifacts/v3ca_1/html-0/index.html"
                }
            }
        });

        let file = client_artifact_file_record_view(
            "v3ca_1",
            "published",
            &manifest,
            0,
            "index.html".to_string(),
            "text/html".to_string(),
            "primary_html".to_string(),
            128,
            "sha256-value".to_string(),
        );

        assert_eq!(file.file_index, 0);
        assert_eq!(file.filename, "index.html");
        assert_eq!(file.download_url, "/v1/client-artifacts/v3ca_1/files/0");
        assert_eq!(
            file.preview_url.as_deref(),
            Some("/v1/client-artifacts/v3ca_1/files/0/preview")
        );
        assert_eq!(
            file.public_url.as_deref(),
            Some("https://v3.elepcloud.com/generated-artifacts/client-artifacts/v3ca_1/html-0/index.html")
        );
        assert_eq!(file.sha256, "sha256-value");
    }

    #[test]
    fn client_artifact_publish_support_builds_download_response() {
        let response = client_artifact_file_download_response(
            "report\"final.md",
            "text/markdown",
            b"# report".to_vec(),
        )
        .expect("download response should build");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/markdown"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"reportfinal.md\""
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_eq!(response.headers().get(header::CONTENT_LENGTH).unwrap(), "8");
    }

    #[test]
    fn client_artifact_publish_support_sanitizes_html_bytes() {
        let sanitized = sanitize_client_artifact_html_bytes(
            br#"<main onclick="alert(1)"><script>alert(1)</script><img src="data:image/png;base64,abc"></main>"#.to_vec(),
            "HTML preview requires UTF-8 encoded content",
        )
        .expect("valid UTF-8 should sanitize");

        assert!(!sanitized.to_ascii_lowercase().contains("<script"));
        assert!(!sanitized.to_ascii_lowercase().contains("onclick"));
        assert!(sanitized.contains("data:image/png;base64,abc"));
    }

    #[test]
    fn client_artifact_publish_support_rejects_non_utf8_html_bytes() {
        let error = sanitize_client_artifact_html_bytes(
            vec![0xff, 0xfe],
            "public HTML publication requires UTF-8 encoded content",
        )
        .expect_err("invalid UTF-8 should fail");

        assert_eq!(error.payload.code, "client_artifact_html_not_utf8");
        assert_eq!(
            error.payload.message,
            "public HTML publication requires UTF-8 encoded content"
        );
    }

    #[test]
    fn client_artifact_publish_support_builds_public_html_file_publication() {
        let publication = build_client_artifact_public_html_file_publication(
            "v3ca_1",
            "客户页面",
            ClientArtifactPreparedPublicHtmlFile {
                file_index: 2,
                filename: "index.html".to_string(),
                content_type: "text/html".to_string(),
                role: "primary_html".to_string(),
                size_bytes: 128,
                sanitized_html: "<main>ok</main>".to_string(),
            },
        )
        .expect("publication should build");

        assert_eq!(publication.file_index, 2);
        assert_eq!(publication.filename, "index.html");
        assert_eq!(publication.content_type, "text/html");
        assert_eq!(publication.role, "primary_html");
        assert_eq!(publication.size_bytes, 128);
        assert_eq!(publication.relative_dir, "client-artifacts/v3ca_1/html-2");
        assert!(publication.public_html.contains("客户页面"));
        assert!(publication.public_html.contains("index.html"));
        assert!(publication
            .public_html
            .contains("&lt;main&gt;ok&lt;/main&gt;"));
    }

    #[test]
    fn client_artifact_publish_support_selects_first_public_html_candidate() {
        let selected = select_client_artifact_public_html_candidate(vec![
            ClientArtifactPublicHtmlCandidate {
                file_index: 1,
                filename: "report.md".to_string(),
                content_type: "text/markdown".to_string(),
                role: "source_summary".to_string(),
                size_bytes: 16,
                storage_kind: "database".to_string(),
                object_locator: None,
                database_bytes: Some(b"# report".to_vec()),
            },
            ClientArtifactPublicHtmlCandidate {
                file_index: 0,
                filename: "index.html".to_string(),
                content_type: "text/html".to_string(),
                role: "primary_html".to_string(),
                size_bytes: 24,
                storage_kind: "database".to_string(),
                object_locator: None,
                database_bytes: Some(b"<main>ok</main>".to_vec()),
            },
        ])
        .expect("HTML candidate should be selected");

        assert_eq!(selected.file_index, 0);
        assert_eq!(selected.filename, "index.html");
        assert_eq!(selected.role, "primary_html");
    }

    #[test]
    fn client_artifact_publish_support_rejects_public_html_candidate_missing_html() {
        let error =
            select_client_artifact_public_html_candidate(vec![ClientArtifactPublicHtmlCandidate {
                file_index: 0,
                filename: "report.md".to_string(),
                content_type: "text/markdown".to_string(),
                role: "source_summary".to_string(),
                size_bytes: 16,
                storage_kind: "database".to_string(),
                object_locator: None,
                database_bytes: Some(b"# report".to_vec()),
            }])
            .expect_err("missing HTML candidate should fail");

        assert_eq!(error.payload.code, "client_artifact_public_html_not_found");
    }

    #[test]
    fn client_artifact_publish_support_requires_private_publish_before_public_html() {
        ensure_client_artifact_can_publish_public_html("published")
            .expect("published artifacts can be publicly published");

        let error = ensure_client_artifact_can_publish_public_html("received")
            .expect_err("unpublished artifacts should fail");

        assert_eq!(error.payload.code, "client_artifact_not_published");
        assert_eq!(
            error.payload.message,
            "client artifact must be privately published before public HTML publication is available"
        );
    }

    #[test]
    fn client_artifact_publish_support_checks_preview_readiness() {
        ensure_client_artifact_file_can_preview_html(
            "published",
            "index.html",
            "text/html",
            "primary_html",
            1024,
            2048,
        )
        .expect("published html below limit should preview");

        let unpublished = ensure_client_artifact_file_can_preview_html(
            "received",
            "index.html",
            "text/html",
            "primary_html",
            1024,
            2048,
        )
        .expect_err("unpublished artifact should fail");
        assert_eq!(unpublished.payload.code, "client_artifact_not_published");
        assert_eq!(
            unpublished.payload.message,
            "client artifact must be published before HTML preview is available"
        );

        let non_html = ensure_client_artifact_file_can_preview_html(
            "published",
            "report.md",
            "text/markdown",
            "source_summary",
            1024,
            2048,
        )
        .expect_err("non-html file should fail");
        assert_eq!(non_html.payload.code, "client_artifact_preview_not_html");

        let too_large = ensure_client_artifact_file_can_preview_html(
            "published",
            "index.html",
            "text/html",
            "primary_html",
            4096,
            2048,
        )
        .expect_err("oversized html should fail");
        assert_eq!(too_large.payload.code, "client_artifact_preview_too_large");
        assert_eq!(
            too_large.payload.message,
            "HTML preview is limited to 2048 bytes"
        );
    }
}
