use domain_model::Document;
use serde_json::{json, Value};

pub(crate) fn assistant_run_document_external_ref(document: &Document) -> Option<Value> {
    let external_source = document
        .metadata
        .get("external_source")
        .or_else(|| document.metadata.get("externalSource"))?
        .as_object()?;
    let source_id = external_source
        .get("source_id")
        .or_else(|| external_source.get("sourceId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let document_external_id = external_source
        .get("document_external_id")
        .or_else(|| external_source.get("documentExternalId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(json!({
        "source_id": source_id,
        "document_external_id": document_external_id,
        "revision_external_id": external_source
            .get("revision_external_id")
            .or_else(|| external_source.get("revisionExternalId"))
            .and_then(Value::as_str),
    }))
}
