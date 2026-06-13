use std::{fs, path::PathBuf};

use contracts::{HtmlArtifactManifestView, HtmlArtifactTemplateIdView};
use serde_json::Value;

use crate::ApiError;

#[derive(Debug)]
pub(crate) struct HtmlArtifactDownloadableFile {
    pub(crate) path: PathBuf,
    pub(crate) file_name: String,
    pub(crate) content_type: String,
}

pub(crate) fn html_artifact_downloadable_file(
    artifact: &HtmlArtifactManifestView,
    file_index: usize,
) -> std::result::Result<HtmlArtifactDownloadableFile, ApiError> {
    if !matches!(
        artifact.template_id,
        HtmlArtifactTemplateIdView::VideoExtractionSummary
            | HtmlArtifactTemplateIdView::ThirdPartyHandoffDocument
    ) {
        return Err(ApiError::bad_request(
            "html_artifact_file_download_unsupported",
            "this HTML artifact does not expose downloadable generated files".to_string(),
        ));
    }
    let generated_artifacts = artifact
        .payload
        .get("generated_artifacts")
        .or_else(|| artifact.payload.get("generatedArtifacts"))
        .ok_or_else(|| {
            ApiError::not_found(
                "html_artifact_file_not_found",
                "HTML artifact does not expose generated files".to_string(),
            )
        })?;
    let files = generated_artifacts
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ApiError::not_found(
                "html_artifact_file_not_found",
                "HTML artifact generated file list is missing".to_string(),
            )
        })?;
    let file = files.get(file_index).ok_or_else(|| {
        ApiError::not_found(
            "html_artifact_file_not_found",
            format!("generated file index {file_index} was not found"),
        )
    })?;
    let raw_path = html_artifact_file_string(file, &["path"]).ok_or_else(|| {
        ApiError::not_found(
            "html_artifact_file_not_found",
            "generated file path is missing".to_string(),
        )
    })?;
    let target_path = html_artifact_safe_local_path(&raw_path)?;
    let canonical_target = fs::canonicalize(&target_path).map_err(|error| {
        ApiError::not_found(
            "html_artifact_file_unavailable",
            format!("generated file is not available: {error}"),
        )
    })?;
    let allowed_roots = html_artifact_allowed_generated_roots(generated_artifacts);
    if allowed_roots.is_empty()
        || !allowed_roots
            .iter()
            .any(|root| canonical_target.starts_with(root))
    {
        return Err(ApiError::bad_request(
            "html_artifact_file_path_denied",
            "generated file path is outside the artifact workspace".to_string(),
        ));
    }
    let file_name = html_artifact_download_file_name(file, &canonical_target);
    let content_type = html_artifact_file_string(file, &["format", "mime", "content_type"])
        .unwrap_or_else(|| "application/octet-stream".to_string());
    Ok(HtmlArtifactDownloadableFile {
        path: canonical_target,
        file_name,
        content_type,
    })
}

fn html_artifact_file_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn html_artifact_safe_local_path(raw_path: &str) -> std::result::Result<PathBuf, ApiError> {
    let raw_path = raw_path.trim();
    if raw_path.contains("://") || raw_path.starts_with("\\\\") {
        return Err(ApiError::bad_request(
            "html_artifact_file_path_denied",
            "generated file path must be a local file path".to_string(),
        ));
    }
    let path = PathBuf::from(raw_path);
    if !path.is_absolute() {
        return Err(ApiError::bad_request(
            "html_artifact_file_path_denied",
            "generated file path must be absolute".to_string(),
        ));
    }
    Ok(path)
}

fn html_artifact_allowed_generated_roots(generated_artifacts: &Value) -> Vec<PathBuf> {
    ["artifacts_dir", "session_dir", "manifest_path"]
        .into_iter()
        .filter_map(|key| generated_artifacts.get(key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains("://") && !value.starts_with("\\\\"))
        .filter_map(|value| {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return None;
            }
            let root = if path.is_file() {
                path.parent().map(PathBuf::from)?
            } else {
                path
            };
            fs::canonicalize(root).ok()
        })
        .collect()
}

fn html_artifact_download_file_name(file: &Value, path: &std::path::Path) -> String {
    let raw = html_artifact_file_string(file, &["file_name", "filename", "name"])
        .or_else(|| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "video-artifact.bin".to_string());
    let safe = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if safe.is_empty() {
        "video-artifact.bin".to_string()
    } else {
        safe.chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn file_string_keeps_existing_first_key_then_trim_behavior() {
        let value = json!({
            "format": "   ",
            "mime": " text/markdown ",
            "content_type": "application/json"
        });

        assert_eq!(
            html_artifact_file_string(&value, &["format", "mime", "content_type"]).as_deref(),
            None
        );
        assert_eq!(
            html_artifact_file_string(&value, &["mime", "content_type"]).as_deref(),
            Some("text/markdown")
        );
    }

    #[test]
    fn safe_local_path_rejects_remote_unc_and_relative_paths() {
        assert!(html_artifact_safe_local_path("https://example.com/a.pptx").is_err());
        assert!(html_artifact_safe_local_path("\\\\server\\share\\a.pptx").is_err());
        assert!(html_artifact_safe_local_path("relative/path.md").is_err());
        assert!(html_artifact_safe_local_path(
            &std::env::temp_dir().join("a.md").display().to_string()
        )
        .is_ok());
    }

    #[test]
    fn download_file_name_keeps_existing_ascii_sanitization_and_fallback() {
        let fallback_path = std::env::temp_dir().join("fallback.md");
        assert_eq!(
            html_artifact_download_file_name(
                &json!({"file_name": " 销售 月报?.md "}),
                &fallback_path,
            ),
            ".md"
        );
        assert_eq!(
            html_artifact_download_file_name(&json!({}), &fallback_path),
            "fallback.md"
        );
        assert_eq!(
            html_artifact_download_file_name(&json!({"name": "   ***   "}), &fallback_path),
            "video-artifact.bin"
        );
    }
}
