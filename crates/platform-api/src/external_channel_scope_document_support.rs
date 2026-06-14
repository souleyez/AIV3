use domain_model::DocumentId;
use serde_json::{Map, Value};

use crate::document_id_from_scope_item;

pub(crate) fn selected_scope_document_id_by_external_ref(
    selected_scope: &Value,
    source_id: Option<&str>,
    document_external_id: &str,
) -> Option<DocumentId> {
    let source_id = source_id.map(str::trim).filter(|value| !value.is_empty());
    let document_external_id = document_external_id.trim();
    if document_external_id.is_empty() {
        return None;
    }
    let documents = selected_scope.get("documents").and_then(Value::as_array)?;
    for document in documents {
        let Some(object) = document.as_object() else {
            continue;
        };
        let matches_external_id = object_string(
            object,
            &[
                "document_external_id",
                "documentExternalId",
                "external_document_id",
                "externalDocumentId",
            ],
        )
        .is_some_and(|value| value == document_external_id);
        if !matches_external_id {
            continue;
        }
        if let Some(expected_source_id) = source_id {
            let matches_source = object_string(object, &["source_id", "sourceId"])
                .is_some_and(|value| value == expected_source_id);
            if !matches_source {
                continue;
            }
        }
        if let Some(document_id) = document_id_from_scope_item(document) {
            return Some(document_id);
        }
    }
    None
}

pub(crate) fn selected_scope_document_item_by_title<'a>(
    selected_scope: &'a Value,
    title: &str,
) -> Option<&'a Value> {
    let target = external_template_reference_title_key(title);
    if target.is_empty() {
        return None;
    }
    selected_scope
        .get("documents")
        .and_then(Value::as_array)?
        .iter()
        .find(|document| {
            document
                .as_object()
                .and_then(|object| object_string(object, &["title", "filename", "name"]))
                .is_some_and(|candidate| {
                    external_template_reference_title_key(&candidate) == target
                })
        })
}

fn external_template_reference_title_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn object_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selected_scope_document_id_by_external_ref_requires_external_id() {
        let document_id = DocumentId::new();
        let selected_scope = json!({
            "documents": [{
                "id": document_id.to_string(),
                "source_id": "source-a",
                "document_external_id": "doc-a"
            }]
        });

        assert_eq!(
            selected_scope_document_id_by_external_ref(
                &selected_scope,
                Some("source-a"),
                " doc-a "
            ),
            Some(document_id)
        );
        assert_eq!(
            selected_scope_document_id_by_external_ref(&selected_scope, Some("source-b"), "doc-a"),
            None
        );
        assert_eq!(
            selected_scope_document_id_by_external_ref(&selected_scope, None, " "),
            None
        );
    }

    #[test]
    fn selected_scope_document_id_by_external_ref_accepts_alias_keys() {
        let document_id = DocumentId::new();
        let selected_scope = json!({
            "documents": [{
                "id": document_id.to_string(),
                "sourceId": "source-a",
                "externalDocumentId": "doc-a"
            }]
        });

        assert_eq!(
            selected_scope_document_id_by_external_ref(&selected_scope, Some("source-a"), "doc-a"),
            Some(document_id)
        );
    }

    #[test]
    fn selected_scope_document_item_by_title_matches_title_without_spaces() {
        let document_id = DocumentId::new();
        let selected_scope = json!({
            "documents": [{
                "id": document_id.to_string(),
                "filename": "  经营 管理 月报.xlsx  "
            }]
        });

        let item = selected_scope_document_item_by_title(&selected_scope, "经营管理月报.xlsx")
            .expect("document should match");

        assert_eq!(item["id"], json!(document_id.to_string()));
        assert!(selected_scope_document_item_by_title(&selected_scope, " ").is_none());
    }
}
