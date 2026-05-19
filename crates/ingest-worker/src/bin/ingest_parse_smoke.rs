use domain_model::{DatasetId, DocumentId};
use ingest_worker::{IngestJob, IngestProcessor, LocalIngestProcessor};
use serde_json::json;
use std::path::Path;

fn main() {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        eprintln!(
            "Usage: ingest_parse_smoke <file-path> [content-type]\n\
             Example: ingest_parse_smoke sample.pdf application/pdf"
        );
        std::process::exit(if args.is_empty() { 2 } else { 0 });
    }

    let object_key = args.remove(0);
    let content_type = args
        .first()
        .cloned()
        .unwrap_or_else(|| infer_content_type(&object_key).to_string());
    let title = Path::new(&object_key)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("smoke-document")
        .to_string();

    let outcome = LocalIngestProcessor.process(&IngestJob {
        dataset_id: DatasetId::new(),
        document_id: DocumentId::new(),
        title,
        object_key,
        content_type,
    });

    let first_chunk_preview = outcome
        .chunks
        .first()
        .map(|chunk| preview_text(chunk, 1200))
        .unwrap_or_default();
    let report = json!({
        "parse_method": outcome.parse_method,
        "extracted_chars": outcome.extracted_chars,
        "chunk_count": outcome.chunk_count(),
        "used_placeholder": outcome.used_placeholder,
        "first_chunk_preview": first_chunk_preview,
        "metadata": outcome.metadata,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("smoke report should serialize")
    );
}

fn infer_content_type(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

fn preview_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}
