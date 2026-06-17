use crate::ApiError;
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const V3_CLIENT_ARTIFACT_OBJECT_DIR_ENV: &str = "V3_CLIENT_ARTIFACT_OBJECT_DIR";
const V3_CLIENT_ARTIFACT_DATABASE_FILE_BYTES_ENV: &str = "V3_CLIENT_ARTIFACT_DATABASE_FILE_BYTES";
const DEFAULT_V3_CLIENT_ARTIFACT_DATABASE_FILE_BYTES: usize = 1024 * 1024;
const STORAGE_KIND_DATABASE: &str = "database";
const STORAGE_KIND_FILESYSTEM: &str = "filesystem";
const OBJECT_LOCATOR_PREFIX: &str = "client-artifacts";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedClientArtifactFileStorage {
    pub storage_kind: &'static str,
    pub object_locator: Option<String>,
    pub database_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientArtifactFileStorageRecord {
    pub storage_kind: String,
    pub object_locator: Option<String>,
    pub database_bytes: Option<Vec<u8>>,
}

pub fn client_artifact_object_root_from_env() -> Option<PathBuf> {
    std::env::var(V3_CLIENT_ARTIFACT_OBJECT_DIR_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn client_artifact_database_file_bytes_limit_from_env() -> usize {
    std::env::var(V3_CLIENT_ARTIFACT_DATABASE_FILE_BYTES_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or(DEFAULT_V3_CLIENT_ARTIFACT_DATABASE_FILE_BYTES)
}

pub fn prepare_client_artifact_file_storage(
    object_root: Option<&Path>,
    database_file_bytes_limit: usize,
    tenant_id: Uuid,
    artifact_id: &str,
    file_index: i32,
    sha256: &str,
    bytes: &[u8],
) -> std::result::Result<PreparedClientArtifactFileStorage, ApiError> {
    let should_use_filesystem = object_root.is_some() && bytes.len() > database_file_bytes_limit;
    if !should_use_filesystem {
        return Ok(PreparedClientArtifactFileStorage {
            storage_kind: STORAGE_KIND_DATABASE,
            object_locator: None,
            database_bytes: Some(bytes.to_vec()),
        });
    }

    let object_root = object_root.expect("checked above");
    let object_locator =
        client_artifact_object_locator(tenant_id, artifact_id, file_index, sha256)?;
    let target_path = client_artifact_object_path(object_root, &object_locator)?;
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ApiError::internal(
                "client_artifact_object_write_failed",
                format!("failed to prepare client artifact object directory: {error}"),
            )
        })?;
    }
    fs::write(&target_path, bytes).map_err(|error| {
        ApiError::internal(
            "client_artifact_object_write_failed",
            format!("failed to write client artifact object payload: {error}"),
        )
    })?;

    Ok(PreparedClientArtifactFileStorage {
        storage_kind: STORAGE_KIND_FILESYSTEM,
        object_locator: Some(object_locator),
        database_bytes: None,
    })
}

pub fn read_client_artifact_file_storage(
    record: &ClientArtifactFileStorageRecord,
) -> std::result::Result<Vec<u8>, ApiError> {
    read_client_artifact_file_storage_from_root(
        client_artifact_object_root_from_env().as_deref(),
        record,
    )
}

fn read_client_artifact_file_storage_from_root(
    object_root: Option<&Path>,
    record: &ClientArtifactFileStorageRecord,
) -> std::result::Result<Vec<u8>, ApiError> {
    match record.storage_kind.trim() {
        STORAGE_KIND_DATABASE => record.database_bytes.clone().ok_or_else(|| {
            ApiError::internal(
                "client_artifact_file_payload_missing",
                "database-backed client artifact file is missing byte payload".to_string(),
            )
        }),
        STORAGE_KIND_FILESYSTEM => {
            let object_root = object_root.ok_or_else(|| {
                ApiError::internal(
                    "client_artifact_object_storage_not_configured",
                    format!("{V3_CLIENT_ARTIFACT_OBJECT_DIR_ENV} is required to read filesystem-backed client artifact files"),
                )
            })?;
            let locator = record.object_locator.as_deref().ok_or_else(|| {
                ApiError::internal(
                    "client_artifact_file_payload_missing",
                    "filesystem-backed client artifact file is missing object locator".to_string(),
                )
            })?;
            let path = client_artifact_object_path(object_root, locator)?;
            fs::read(&path).map_err(|error| {
                ApiError::internal(
                    "client_artifact_object_read_failed",
                    format!("failed to read client artifact object payload: {error}"),
                )
            })
        }
        other => Err(ApiError::internal(
            "client_artifact_storage_kind_invalid",
            format!("unsupported client artifact storage kind {other}"),
        )),
    }
}

fn client_artifact_object_locator(
    tenant_id: Uuid,
    artifact_id: &str,
    file_index: i32,
    sha256: &str,
) -> std::result::Result<String, ApiError> {
    if file_index < 0 {
        return Err(ApiError::internal(
            "client_artifact_object_locator_invalid",
            "file_index must be non-negative for client artifact object storage".to_string(),
        ));
    }
    let artifact_id = artifact_id.trim();
    let sha256 = sha256.trim();
    if !safe_object_locator_segment(artifact_id) || !safe_object_locator_segment(sha256) {
        return Err(ApiError::internal(
            "client_artifact_object_locator_invalid",
            "client artifact object locator contains an unsafe segment".to_string(),
        ));
    }
    let locator =
        format!("{OBJECT_LOCATOR_PREFIX}/{tenant_id}/{artifact_id}/{file_index:04}-{sha256}");
    validate_client_artifact_object_locator(&locator)?;
    Ok(locator)
}

fn client_artifact_object_path(
    object_root: &Path,
    object_locator: &str,
) -> std::result::Result<PathBuf, ApiError> {
    let locator = object_locator.trim();
    validate_client_artifact_object_locator(locator)?;
    let mut path = object_root.to_path_buf();
    for segment in locator.split('/') {
        path.push(segment);
    }
    Ok(path)
}

fn validate_client_artifact_object_locator(
    object_locator: &str,
) -> std::result::Result<(), ApiError> {
    let locator = object_locator.trim();
    if locator.is_empty()
        || locator.contains('\\')
        || locator.contains(':')
        || locator.starts_with('/')
        || locator.ends_with('/')
    {
        return Err(ApiError::internal(
            "client_artifact_object_locator_invalid",
            "client artifact object locator must be a safe relative path".to_string(),
        ));
    }
    let mut segments = locator.split('/');
    if segments.next() != Some(OBJECT_LOCATOR_PREFIX) {
        return Err(ApiError::internal(
            "client_artifact_object_locator_invalid",
            "client artifact object locator has an invalid prefix".to_string(),
        ));
    }
    if !locator.split('/').all(safe_object_locator_segment) {
        return Err(ApiError::internal(
            "client_artifact_object_locator_invalid",
            "client artifact object locator contains an unsafe segment".to_string(),
        ));
    }
    Ok(())
}

fn safe_object_locator_segment(segment: &str) -> bool {
    let segment = segment.trim();
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_artifact_storage_uses_database_without_object_root() {
        let storage = prepare_client_artifact_file_storage(
            None,
            1,
            Uuid::nil(),
            "v3ca_fixture",
            0,
            "abc123",
            b"large",
        )
        .expect("database storage should be selected");

        assert_eq!(storage.storage_kind, STORAGE_KIND_DATABASE);
        assert_eq!(storage.object_locator, None);
        assert_eq!(storage.database_bytes.as_deref(), Some(b"large".as_slice()));
    }

    #[test]
    fn client_artifact_storage_writes_large_file_to_configured_root() {
        let root = std::env::temp_dir().join(format!(
            "v3-client-artifact-storage-test-{}",
            Uuid::new_v4()
        ));
        let tenant_id =
            Uuid::parse_str("00000000-0000-0000-0000-000000000042").expect("valid test uuid");
        let storage = prepare_client_artifact_file_storage(
            Some(&root),
            3,
            tenant_id,
            "v3ca_fixture",
            2,
            "abcdef123456",
            b"large",
        )
        .expect("filesystem storage should be selected");

        assert_eq!(storage.storage_kind, STORAGE_KIND_FILESYSTEM);
        assert!(storage.database_bytes.is_none());
        assert!(storage.object_locator.as_deref().unwrap().starts_with(
            "client-artifacts/00000000-0000-0000-0000-000000000042/v3ca_fixture/0002-"
        ));

        let record = ClientArtifactFileStorageRecord {
            storage_kind: STORAGE_KIND_FILESYSTEM.to_string(),
            object_locator: storage.object_locator,
            database_bytes: None,
        };
        let bytes = read_client_artifact_file_storage_from_root(Some(&root), &record)
            .expect("filesystem payload should be readable");
        assert_eq!(bytes, b"large");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn client_artifact_storage_rejects_escaping_locator() {
        let error =
            client_artifact_object_path(&std::env::temp_dir(), "client-artifacts/tenant/../secret")
                .expect_err("escaping locator should fail");

        assert_eq!(error.payload.code, "client_artifact_object_locator_invalid");
    }

    #[test]
    fn client_artifact_storage_reads_database_payload() {
        let record = ClientArtifactFileStorageRecord {
            storage_kind: STORAGE_KIND_DATABASE.to_string(),
            object_locator: None,
            database_bytes: Some(b"inline".to_vec()),
        };

        let bytes = read_client_artifact_file_storage_from_root(None, &record)
            .expect("database payload should be returned");
        assert_eq!(bytes, b"inline");
    }
}
