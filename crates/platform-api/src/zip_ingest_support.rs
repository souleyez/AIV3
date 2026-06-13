use std::path::Path as StdPath;

use crate::external_document_object_support::safe_external_path_segment;

pub(crate) fn safe_zip_entry_output_name(
    index: usize,
    entry_name: &str,
    extension: &str,
) -> String {
    let stem = StdPath::new(entry_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(safe_external_path_segment)
        .unwrap_or_else(|| "entry".to_string());
    let safe_extension = if extension.len() <= 16 {
        extension.to_string()
    } else {
        String::new()
    };
    format!("{:03}-{}{}", index + 1, stem, safe_extension)
}

pub(crate) fn zip_entry_title(entry_name: &str) -> String {
    StdPath::new(entry_name)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| entry_name.to_string())
}

pub(crate) fn zip_entry_should_skip(entry_name: &str) -> bool {
    let lower = entry_name.to_ascii_lowercase();
    lower.starts_with("__macosx/")
        || lower.ends_with("/.ds_store")
        || lower.contains("/.git/")
        || lower.contains("/node_modules/")
}

pub(crate) fn zip_entry_extension_supported(extension: &str) -> bool {
    matches!(
        extension,
        ".txt"
            | ".md"
            | ".csv"
            | ".json"
            | ".html"
            | ".htm"
            | ".xml"
            | ".pdf"
            | ".doc"
            | ".docx"
            | ".xlsx"
            | ".xlsm"
            | ".pptx"
            | ".pptm"
            | ".png"
            | ".jpg"
            | ".jpeg"
            | ".webp"
            | ".bmp"
            | ".tif"
            | ".tiff"
            | ".gif"
            | ".mp3"
            | ".wav"
            | ".m4a"
            | ".aac"
            | ".flac"
            | ".ogg"
            | ".opus"
            | ".mp4"
            | ".mov"
            | ".mkv"
            | ".webm"
            | ".avi"
            | ".mpeg"
            | ".mpg"
    )
}

pub(crate) fn infer_zip_child_content_type(extension: &str) -> &'static str {
    match extension {
        ".md" => "text/markdown",
        ".csv" => "text/csv",
        ".json" => "application/json",
        ".html" | ".htm" => "text/html",
        ".xml" => "application/xml",
        ".pdf" => "application/pdf",
        ".doc" => "application/msword",
        ".docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ".xlsx" | ".xlsm" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ".pptx" | ".pptm" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        ".png" => "image/png",
        ".jpg" | ".jpeg" => "image/jpeg",
        ".webp" => "image/webp",
        ".bmp" => "image/bmp",
        ".tif" | ".tiff" => "image/tiff",
        ".gif" => "image/gif",
        ".mp3" => "audio/mpeg",
        ".wav" => "audio/wav",
        ".m4a" => "audio/mp4",
        ".aac" => "audio/aac",
        ".flac" => "audio/flac",
        ".ogg" => "audio/ogg",
        ".opus" => "audio/opus",
        ".mp4" => "video/mp4",
        ".mov" => "video/quicktime",
        ".mkv" => "video/x-matroska",
        ".webm" => "video/webm",
        ".avi" => "video/x-msvideo",
        ".mpeg" | ".mpg" => "video/mpeg",
        _ => "text/plain",
    }
}

pub(crate) fn zip_ingest_env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

pub(crate) fn zip_ingest_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_output_name_keeps_existing_numbering_sanitization_and_extension_rules() {
        assert_eq!(
            safe_zip_entry_output_name(0, "docs/资料 1.md", ".md"),
            "001-___1.md"
        );
        assert_eq!(
            safe_zip_entry_output_name(11, "nested/report.final.csv", ".csv"),
            "012-report.final.csv"
        );
        assert_eq!(
            safe_zip_entry_output_name(2, "raw/name.bin", ".verylongextension"),
            "003-name"
        );
    }

    #[test]
    fn zip_entry_title_and_skip_rules_preserve_existing_filters() {
        assert_eq!(zip_entry_title("docs/readme.md"), "readme.md");
        assert_eq!(zip_entry_title("   "), "   ");

        assert!(zip_entry_should_skip("__MACOSX/._readme.md"));
        assert!(zip_entry_should_skip("docs/.DS_Store"));
        assert!(zip_entry_should_skip("repo/.git/config"));
        assert!(zip_entry_should_skip("app/node_modules/pkg/index.js"));
        assert!(!zip_entry_should_skip("docs/readme.md"));
    }

    #[test]
    fn zip_extension_and_content_type_tables_preserve_supported_entries() {
        for extension in [".md", ".docx", ".xlsx", ".pptx", ".png", ".mp4"] {
            assert!(
                zip_entry_extension_supported(extension),
                "{extension} should stay supported"
            );
        }
        assert!(!zip_entry_extension_supported(".exe"));

        assert_eq!(infer_zip_child_content_type(".md"), "text/markdown");
        assert_eq!(
            infer_zip_child_content_type(".docx"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
        assert_eq!(infer_zip_child_content_type(".mp4"), "video/mp4");
        assert_eq!(infer_zip_child_content_type(".unknown"), "text/plain");
    }

    #[test]
    fn zip_ingest_env_helpers_fall_back_when_unset_or_unparseable() {
        assert_eq!(
            zip_ingest_env_u64("DATAMAX_TEST_ZIP_INGEST_U64_SHOULD_NOT_EXIST", 42),
            42
        );
        assert_eq!(
            zip_ingest_env_usize("DATAMAX_TEST_ZIP_INGEST_USIZE_SHOULD_NOT_EXIST", 7),
            7
        );
    }
}
