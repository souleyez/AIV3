use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use domain_model::{DatasetId, Document, DocumentId};
use serde::Serialize;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path as StdPath, PathBuf},
};

use crate::{
    active_secret_binding_ids_from_headers, current_auth_user_id,
    list_documents_for_visible_dataset_scopes, load_visible_dataset_for_user_with_local_scope,
    local_thread_id_from_headers, ApiError, AppState,
};

const TABULAR_SCHEMA_VERSION: &str = "1.0.0";
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_TABLES: usize = 64;
const MAX_COLUMNS: usize = 100;
const MAX_TOTAL_COLUMNS: usize = 1_024;
const MAX_COLUMN_NAME_CHARS: usize = 96;
const LEGACY_OBJECT_ROOT_ENV: &str = "PLATFORM_LEGACY_UPLOAD_ROOT";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct DatasetTabularSchemaColumnView {
    pub ordinal: usize,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct DatasetTabularSchemaTableView {
    pub document_id: DocumentId,
    pub title: String,
    pub content_type: String,
    pub updated_at: DateTime<Utc>,
    pub structural_source: &'static str,
    pub columns: Vec<DatasetTabularSchemaColumnView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct DatasetTabularSchemaResponse {
    pub schema_version: &'static str,
    pub dataset_id: DatasetId,
    pub tabular_document_count: usize,
    pub column_count: usize,
    pub skipped_tabular_document_count: usize,
    pub tables: Vec<DatasetTabularSchemaTableView>,
}

pub(crate) async fn get_dataset_tabular_schema(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<DatasetId>,
) -> std::result::Result<Json<DatasetTabularSchemaResponse>, ApiError> {
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let current_user_id = current_auth_user_id(&state, &headers).await?;
    let local_thread_id = local_thread_id_from_headers(&headers);
    load_visible_dataset_for_user_with_local_scope(
        &state,
        dataset_id,
        &active_secret_binding_ids,
        current_user_id,
        local_thread_id.as_deref(),
    )
    .await?;

    let visible_dataset_ids = HashSet::from([dataset_id]);
    let documents =
        list_documents_for_visible_dataset_scopes(&state, &visible_dataset_ids, current_user_id)
            .await?;
    let (mut tables, tabular_document_count) = tokio::task::spawn_blocking(move || {
        let mut candidates = documents
            .into_iter()
            .filter_map(|document| {
                let delimiter = tabular_document_delimiter(&document)?;
                Some((document, delimiter))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|(left, _), (right, _)| {
            left.title
                .to_ascii_lowercase()
                .cmp(&right.title.to_ascii_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        let tabular_document_count = candidates.len();
        let mut total_columns = 0usize;
        let mut tables = Vec::new();
        for (document, delimiter) in candidates.into_iter().take(MAX_TABLES) {
            let Some(table) = tabular_schema_table_from_document(&document, delimiter) else {
                continue;
            };
            if total_columns.saturating_add(table.columns.len()) > MAX_TOTAL_COLUMNS {
                break;
            }
            total_columns = total_columns.saturating_add(table.columns.len());
            tables.push(table);
        }
        (tables, tabular_document_count)
    })
    .await
    .map_err(|error| {
        ApiError::internal(
            "dataset_tabular_schema_task_failed",
            format!("dataset tabular schema task failed: {error}"),
        )
    })?;

    tables.sort_by(|left, right| {
        left.title
            .to_ascii_lowercase()
            .cmp(&right.title.to_ascii_lowercase())
            .then_with(|| left.document_id.cmp(&right.document_id))
    });
    let column_count = tables.iter().map(|table| table.columns.len()).sum();

    Ok(Json(DatasetTabularSchemaResponse {
        schema_version: TABULAR_SCHEMA_VERSION,
        dataset_id,
        tabular_document_count: tables.len(),
        column_count,
        skipped_tabular_document_count: tabular_document_count.saturating_sub(tables.len()),
        tables,
    }))
}

fn tabular_document_delimiter(document: &Document) -> Option<char> {
    let content_type = document.content_type.trim().to_ascii_lowercase();
    let title = document.title.trim().to_ascii_lowercase();
    let object_key = document.object_key.trim().to_ascii_lowercase();
    if content_type.contains("tab-separated-values")
        || title.ends_with(".tsv")
        || object_key.ends_with(".tsv")
    {
        return Some('\t');
    }
    if content_type.contains("csv") || title.ends_with(".csv") || object_key.ends_with(".csv") {
        return Some(',');
    }
    None
}

fn tabular_schema_table_from_document(
    document: &Document,
    delimiter: char,
) -> Option<DatasetTabularSchemaTableView> {
    let path = resolve_tabular_schema_local_object_path(&document.object_key)?;
    tabular_schema_table_from_path(document, delimiter, &path)
}

fn tabular_schema_table_from_path(
    document: &Document,
    delimiter: char,
    path: &StdPath,
) -> Option<DatasetTabularSchemaTableView> {
    let header = read_tabular_header_record(path)?;
    let column_names = parse_tabular_header_bytes(&header, delimiter)?;
    validate_structural_header(&column_names)?;
    let columns = column_names
        .into_iter()
        .enumerate()
        .map(|(index, name)| DatasetTabularSchemaColumnView {
            ordinal: index + 1,
            name,
        })
        .collect::<Vec<_>>();

    Some(DatasetTabularSchemaTableView {
        document_id: document.id,
        title: document.title.clone(),
        content_type: document.content_type.clone(),
        updated_at: document.updated_at,
        structural_source: "file_header",
        columns,
    })
}

fn read_tabular_header_record(path: &StdPath) -> Option<Vec<u8>> {
    let mut reader = BufReader::new(File::open(path).ok()?);
    let mut record = Vec::with_capacity(512);
    let mut quoted = false;
    loop {
        let mut byte = [0u8; 1];
        let read = reader.read(&mut byte).ok()?;
        if read == 0 {
            break;
        }
        match byte[0] {
            b'"' => quoted = !quoted,
            b'\n' | b'\r' if !quoted => break,
            _ => {}
        }
        record.push(byte[0]);
        if record.len() > MAX_HEADER_BYTES {
            return None;
        }
    }
    (!record.is_empty() && !quoted).then_some(record)
}

fn resolve_tabular_schema_local_object_path(object_key: &str) -> Option<PathBuf> {
    let mut configured_roots = Vec::with_capacity(2);
    if let Some(root) = std::env::var_os("PLATFORM_LOCAL_OBJECT_ROOT") {
        configured_roots.push(PathBuf::from(root));
    }
    if let Some(root) = std::env::var_os(LEGACY_OBJECT_ROOT_ENV) {
        configured_roots.push(PathBuf::from(root));
    }
    resolve_tabular_schema_path_within_roots(object_key, &configured_roots)
}

fn resolve_tabular_schema_path_within_roots(
    object_key: &str,
    configured_roots: &[PathBuf],
) -> Option<PathBuf> {
    configured_roots
        .iter()
        .find_map(|root| resolve_tabular_schema_path_within_root(object_key, root))
}

fn resolve_tabular_schema_path_within_root(
    object_key: &str,
    configured_root: &StdPath,
) -> Option<PathBuf> {
    let root = fs::canonicalize(configured_root).ok()?;
    let raw = object_key
        .trim()
        .strip_prefix("file://")
        .unwrap_or(object_key.trim());
    if raw.is_empty() {
        return None;
    }
    let raw_path = PathBuf::from(raw);
    let candidate = if raw_path.is_absolute() {
        raw_path
    } else {
        root.join(raw_path)
    };
    let candidate = fs::canonicalize(candidate).ok()?;
    (candidate.starts_with(&root) && candidate.is_file()).then_some(candidate)
}

fn parse_tabular_header_bytes(bytes: &[u8], delimiter: char) -> Option<Vec<String>> {
    if bytes.is_empty() {
        return None;
    }
    let mut quoted = false;
    let mut index = 0usize;
    let mut record_end = None;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if quoted && bytes.get(index + 1) == Some(&b'"') => {
                index = index.saturating_add(1);
            }
            b'"' => quoted = !quoted,
            b'\n' | b'\r' if !quoted => {
                record_end = Some(index);
                break;
            }
            _ => {}
        }
        index = index.saturating_add(1);
    }
    if record_end.is_none() && bytes.len() > MAX_HEADER_BYTES {
        return None;
    }
    let record = &bytes[..record_end.unwrap_or(bytes.len())];
    let record = std::str::from_utf8(record)
        .ok()?
        .trim_start_matches('\u{feff}');
    parse_delimited_header_record(record, delimiter)
}

fn parse_delimited_header_record(record: &str, delimiter: char) -> Option<Vec<String>> {
    let mut columns = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut characters = record.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '"' {
            if quoted && characters.peek() == Some(&'"') {
                cell.push('"');
                characters.next();
            } else {
                quoted = !quoted;
            }
        } else if character == delimiter && !quoted {
            columns.push(normalize_header_cell(&cell)?);
            cell.clear();
        } else {
            cell.push(character);
        }
    }
    if quoted {
        return None;
    }
    columns.push(normalize_header_cell(&cell)?);
    if columns.len() < 2 || columns.len() > MAX_COLUMNS {
        return None;
    }
    let looks_like_header = columns.iter().any(|column| {
        column.chars().any(char::is_alphabetic) || column.contains('_') || column.contains('-')
    });
    looks_like_header.then_some(columns)
}

fn validate_structural_header(columns: &[String]) -> Option<()> {
    if columns.len() < 2 || columns.len() > MAX_COLUMNS {
        return None;
    }
    let mut canonical = HashSet::with_capacity(columns.len());
    for column in columns {
        if !safe_header_identifier(column) || looks_sensitive_header_value(column) {
            return None;
        }
        let normalized = column.to_lowercase();
        if !canonical.insert(normalized.clone()) {
            return None;
        }
        let has_structural_cue = column.contains('_')
            || column
                .chars()
                .zip(column.chars().skip(1))
                .any(|(left, right)| left.is_lowercase() && right.is_uppercase())
            || matches!(
                normalized.as_str(),
                "id" | "name"
                    | "type"
                    | "value"
                    | "date"
                    | "day"
                    | "hour"
                    | "minute"
                    | "interval"
                    | "count"
                    | "share"
                    | "gender"
                    | "area"
                    | "floor"
                    | "status"
                    | "category"
                    | "code"
                    | "description"
                    | "label"
                    | "metric"
                    | "price"
                    | "product"
                    | "quantity"
                    | "remark"
                    | "text"
                    | "title"
                    | "total"
                    | "visitors"
            );
        if !has_structural_cue {
            return None;
        }
    }
    Some(())
}

fn safe_header_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_')
        && characters
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

fn looks_sensitive_header_value(value: &str) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || (value.contains('@') && value.rsplit_once('.').is_some())
    {
        return true;
    }
    let compact_digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    if (compact_digits.len() == 11 && compact_digits.starts_with('1'))
        || (compact_digits.len() == 18
            && value
                .chars()
                .all(|character| character.is_ascii_digit() || matches!(character, 'x' | 'X')))
    {
        return true;
    }
    let uuid_shape = value.len() == 36
        && [8usize, 13, 18, 23]
            .into_iter()
            .all(|index| value.as_bytes().get(index) == Some(&b'-'))
        && value
            .chars()
            .filter(|character| *character != '-')
            .all(|character| character.is_ascii_hexdigit());
    if uuid_shape {
        return true;
    }
    value.len() >= 24
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '_' | '=' | '-')
        })
        && value
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && value.chars().any(|character| character.is_ascii_digit())
}

fn normalize_header_cell(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('\u{feff}');
    if value.is_empty()
        || value.chars().count() > MAX_COLUMN_NAME_CHARS
        || value.chars().any(char::is_control)
    {
        return None;
    }
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, DocumentLifecycle, TenantId};
    use std::{collections::BTreeMap, fs};
    use uuid::Uuid;

    fn document(title: &str, object_key: String, content_type: &str) -> Document {
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: title.to_string(),
            object_key,
            content_type: content_type.to_string(),
            lifecycle: DocumentLifecycle::Indexed,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn tabular_header_parser_preserves_order_bom_and_quoted_delimiters() {
        let columns = parse_tabular_header_bytes(
            "\u{feff}\"day\",\"entity,name\",trafficIn,averageStay\r\n2026-07-01,一层,2,0"
                .as_bytes(),
            ',',
        )
        .expect("header should parse");

        assert_eq!(
            columns,
            vec!["day", "entity,name", "trafficIn", "averageStay"]
        );
    }

    #[test]
    fn tabular_header_parser_rejects_values_without_a_structural_header() {
        assert_eq!(parse_tabular_header_bytes(b"100,200,300\n", ','), None);
        assert_eq!(parse_tabular_header_bytes(b"only_one_column\n", ','), None);
        assert_eq!(parse_tabular_header_bytes(b"day,\"broken\n", ','), None);
    }

    #[test]
    fn tabular_schema_reads_only_header_and_does_not_serialize_storage_path_or_rows() {
        let root = std::env::temp_dir().join(format!("datamax-tabular-schema-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("temp directory should create");
        let path = root.join("traffic.csv");
        fs::write(
            &path,
            b"day,hour,trafficIn,PersonID\n2026-07-01,10,99,sensitive-person-value\n",
        )
        .expect("fixture should write");
        let fixture = document(
            "traffic.csv",
            path.to_string_lossy().to_string(),
            "text/csv",
        );

        let table = tabular_schema_table_from_path(&fixture, ',', &path)
            .expect("local CSV schema should load");
        let raw_header = read_tabular_header_record(&path).expect("logical header should read");
        let serialized = serde_json::to_string(&table).expect("table should serialize");

        assert_eq!(
            table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            vec!["day", "hour", "trafficIn", "PersonID"]
        );
        assert!(!serialized.contains(&path.to_string_lossy().to_string()));
        assert!(!serialized.contains("sensitive-person-value"));
        assert_eq!(
            std::str::from_utf8(&raw_header).expect("header should be UTF-8"),
            "day,hour,trafficIn,PersonID"
        );

        fs::remove_dir_all(root).expect("temp directory should clean up");
    }

    #[test]
    fn tabular_schema_rejects_headerless_sensitive_first_rows() {
        let root = std::env::temp_dir().join(format!("datamax-headerless-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("temp directory should create");
        let cases = [
            ("email.csv", "alice@example.com,13800138000\n"),
            ("uuid.csv", "f2a1d8d9-7a4b-4c5d-9e0f-123456789abc,profile\n"),
            ("names.csv", "Alice,Bob\n"),
            ("name-with-field.csv", "张三,group_type\n"),
            ("short-id-with-field.csv", "abc123,record_count\n"),
        ];
        for (title, content) in cases {
            let path = root.join(title);
            fs::write(&path, content.as_bytes()).expect("headerless fixture should write");
            let fixture = document(title, path.to_string_lossy().to_string(), "text/csv");
            assert_eq!(tabular_schema_table_from_path(&fixture, ',', &path), None);
        }

        fs::remove_dir_all(root).expect("temp directory should clean up");
    }

    #[test]
    fn public_tabular_schema_path_is_confined_to_configured_object_root() {
        let root = std::env::temp_dir().join(format!("datamax-tabular-root-{}", Uuid::new_v4()));
        let legacy_root =
            std::env::temp_dir().join(format!("datamax-tabular-legacy-{}", Uuid::new_v4()));
        let inside = root.join("datasets").join("safe.csv");
        let legacy_inside = legacy_root.join("legacy.csv");
        let outside =
            std::env::temp_dir().join(format!("datamax-tabular-outside-{}.csv", Uuid::new_v4()));
        fs::create_dir_all(inside.parent().expect("inside parent should exist"))
            .expect("root should create");
        fs::create_dir_all(&legacy_root).expect("legacy root should create");
        fs::write(&inside, b"day,count\n2026-07-01,1\n").expect("inside file should write");
        fs::write(&legacy_inside, b"day,count\n2026-07-01,2\n").expect("legacy file should write");
        fs::write(&outside, b"secret,value\nprivate,1\n").expect("outside file should write");

        assert_eq!(
            resolve_tabular_schema_path_within_root("datasets/safe.csv", &root),
            Some(fs::canonicalize(&inside).expect("inside path should canonicalize"))
        );
        assert_eq!(
            resolve_tabular_schema_path_within_root(&inside.to_string_lossy(), &root),
            Some(fs::canonicalize(&inside).expect("inside path should canonicalize"))
        );
        assert_eq!(
            resolve_tabular_schema_path_within_root(&outside.to_string_lossy(), &root),
            None
        );
        assert_eq!(
            resolve_tabular_schema_path_within_root("../outside.csv", &root),
            None
        );
        assert_eq!(
            resolve_tabular_schema_path_within_roots(
                &legacy_inside.to_string_lossy(),
                &[root.clone(), legacy_root.clone()],
            ),
            Some(fs::canonicalize(&legacy_inside).expect("legacy path should canonicalize"))
        );

        fs::remove_dir_all(root).expect("root should clean up");
        fs::remove_dir_all(legacy_root).expect("legacy root should clean up");
        fs::remove_file(outside).expect("outside file should clean up");
    }
}
