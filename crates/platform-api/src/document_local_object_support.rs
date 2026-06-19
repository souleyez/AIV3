use chrono::{DateTime, Utc};
use domain_model::Document;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path as StdPath, PathBuf},
};

use crate::{zip_ingest_support::zip_ingest_env_u64, AppState};

pub(crate) fn resolve_platform_local_object_path(object_key: &str) -> Option<PathBuf> {
    let raw = object_key.trim().trim_start_matches("file://");
    if raw.is_empty() {
        return None;
    }

    let direct = PathBuf::from(raw);
    if direct.is_file() {
        return Some(direct);
    }

    if cfg!(windows) {
        if let Some(rest) = raw.strip_prefix("/mnt/") {
            let mut parts = rest.splitn(2, '/');
            if let (Some(drive), Some(path)) = (parts.next(), parts.next()) {
                if drive.len() == 1 {
                    let windows_path = format!("{}:\\{}", drive, path.replace('/', "\\"));
                    let candidate = PathBuf::from(windows_path);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    let root = std::env::var("PLATFORM_LOCAL_OBJECT_ROOT").ok()?;
    let rooted = StdPath::new(&root).join(raw);
    rooted.is_file().then_some(rooted)
}

pub(crate) async fn record_local_document_content_fingerprint_if_available(
    state: &AppState,
    document: Document,
    recorded_at: DateTime<Utc>,
) -> Document {
    let Some((content_sha256, content_size_bytes)) =
        local_document_content_fingerprint_from_object_key(&document.object_key)
    else {
        return document;
    };
    match state
        .storage
        .documents()
        .record_content_fingerprint(
            state.tenant_id,
            document.id,
            &content_sha256,
            content_size_bytes,
            recorded_at,
        )
        .await
    {
        Ok(updated) => updated,
        Err(error) => {
            tracing::warn!(
                error = ?error,
                document_id = %document.id,
                "skipping local document content fingerprint after storage failure"
            );
            document
        }
    }
}

fn local_document_content_fingerprint_from_object_key(object_key: &str) -> Option<(String, i64)> {
    let path = resolve_platform_local_object_path(object_key)?;
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let max_bytes = zip_ingest_env_u64("DOCUMENT_FINGERPRINT_MAX_BYTES", 300 * 1024 * 1024).max(1);
    if metadata.len() > max_bytes || metadata.len() > i64::MAX as u64 {
        tracing::warn!(
            object_key = %object_key,
            file_size_bytes = metadata.len(),
            max_bytes,
            "skipping local document content fingerprint because file is too large"
        );
        return None;
    }

    let mut file = File::open(&path).ok()?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > max_bytes || total > i64::MAX as u64 {
            return None;
        }
        hasher.update(&buffer[..read]);
    }

    Some((format!("{:x}", hasher.finalize()), total as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use uuid::Uuid;

    #[test]
    fn local_document_content_fingerprint_reads_small_files() {
        let root = std::env::temp_dir().join(format!("datamax-fingerprint-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("fingerprint temp root should be created");
        let path = root.join("doc.md");
        fs::write(&path, b"# Hello\n").expect("fingerprint source should write");

        let fingerprint =
            local_document_content_fingerprint_from_object_key(&path.to_string_lossy())
                .expect("small local file should fingerprint");

        assert_eq!(
            fingerprint.0,
            "90f8ec5669cd34183b9b0fdf8b94f5efb4c3672876330f4aa76088c2b4ad17be"
        );
        assert_eq!(fingerprint.1, 8);
    }

    #[test]
    fn resolve_platform_local_object_path_uses_configured_root_for_relative_keys() {
        let root = std::env::temp_dir().join(format!("datamax-local-root-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("local object root should be created");
        let path = root.join("objects").join("doc.txt");
        fs::create_dir_all(path.parent().expect("path should have parent"))
            .expect("object parent should be created");
        let mut file = File::create(&path).expect("object file should be created");
        file.write_all(b"hello").expect("object file should write");

        let previous_root = std::env::var_os("PLATFORM_LOCAL_OBJECT_ROOT");
        std::env::set_var("PLATFORM_LOCAL_OBJECT_ROOT", &root);

        assert_eq!(
            resolve_platform_local_object_path("objects/doc.txt"),
            Some(path)
        );

        match previous_root {
            Some(value) => std::env::set_var("PLATFORM_LOCAL_OBJECT_ROOT", value),
            None => std::env::remove_var("PLATFORM_LOCAL_OBJECT_ROOT"),
        }
    }
}
