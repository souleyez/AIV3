use crate::static_page_evidence_signal_support::{
    static_page_evidence_ids, static_page_evidence_ref,
};
use crate::{media_numeric_field, media_string_field};
use serde_json::{json, Value};

pub(crate) fn build_static_page_media_sample_points(
    evidence_items: &[Value],
    field_path: &str,
) -> Vec<Value> {
    let Some(array_key) = static_page_media_sample_array_key(field_path) else {
        return Vec::new();
    };
    evidence_items
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str).unwrap_or_default() == "retrieval_evidence"
        })
        .flat_map(|item| {
            item.get("media_context")
                .and_then(|context| context.get(array_key))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(move |window| {
                    let text = static_page_media_sample_text(window, array_key)?;
                    let start_seconds = media_numeric_field(
                        window,
                        &["start_seconds", "start", "timestamp_seconds", "timestamp"],
                    );
                    let end_seconds = media_numeric_field(window, &["end_seconds", "end"]);
                    let timestamp_label =
                        static_page_media_timestamp_label(start_seconds, end_seconds);
                    let citation_label =
                        static_page_media_citation_label(item, timestamp_label.as_deref());
                    Some(json!({
                        "label": static_page_media_sample_label(window, array_key),
                        "value": 1.0,
                        "kind": "media_window",
                        "fieldPath": field_path,
                        "text": text,
                        "startSeconds": start_seconds,
                        "endSeconds": end_seconds,
                        "timestampLabel": timestamp_label,
                        "citationLabel": citation_label,
                        "source": media_string_field(window, &["source"]).unwrap_or_else(|| "media".to_string()),
                        "sourceLocator": item.get("source_locator").cloned().unwrap_or(Value::Null),
                        "evidenceIds": static_page_evidence_ids(item),
                        "evidenceRef": static_page_evidence_ref(item),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .take(6)
        .collect()
}

fn static_page_media_sample_array_key(field_path: &str) -> Option<&'static str> {
    let normalized = field_path.to_ascii_lowercase();
    if normalized.contains("media.transcript") {
        Some("transcript_windows")
    } else if normalized.contains("media.scene") {
        Some("scene_windows")
    } else if normalized.contains("media.keyframe") || normalized.contains("media.ocr") {
        Some("keyframe_ocr_snippets")
    } else {
        None
    }
}

fn static_page_media_sample_text(window: &Value, array_key: &str) -> Option<String> {
    match array_key {
        "scene_windows" => media_string_field(window, &["summary", "label"]),
        _ => media_string_field(window, &["text", "content", "summary"]),
    }
}

fn static_page_media_sample_label(window: &Value, array_key: &str) -> String {
    let timestamp = media_numeric_field(
        window,
        &["start_seconds", "start", "timestamp_seconds", "timestamp"],
    );
    let prefix = match array_key {
        "scene_windows" => "场景",
        "keyframe_ocr_snippets" => "关键帧",
        _ => "转写",
    };
    timestamp
        .map(|seconds| format!("{prefix} {:.1}s", seconds))
        .unwrap_or_else(|| prefix.to_string())
}

fn static_page_media_timestamp_label(
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
) -> Option<String> {
    match (start_seconds, end_seconds) {
        (Some(start), Some(end)) => Some(format!(
            "{} - {}",
            static_page_format_media_timestamp(start),
            static_page_format_media_timestamp(end)
        )),
        (Some(start), None) => Some(static_page_format_media_timestamp(start)),
        (None, Some(end)) => Some(static_page_format_media_timestamp(end)),
        (None, None) => None,
    }
}

fn static_page_media_citation_label(item: &Value, timestamp_label: Option<&str>) -> String {
    let source = item
        .get("source_locator")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("media");
    timestamp_label
        .map(|label| format!("{source} @ {label}"))
        .unwrap_or_else(|| source.to_string())
}

fn static_page_format_media_timestamp(seconds: f64) -> String {
    let safe_seconds = seconds.max(0.0);
    let total = safe_seconds.floor() as u64;
    let millis = ((safe_seconds - total as f64) * 1000.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}.{millis:03}")
    } else {
        format!("{minutes:02}:{secs:02}.{millis:03}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn media_sample_points_return_empty_for_non_media_field() {
        let points = build_static_page_media_sample_points(&[media_evidence()], "orders.amount");

        assert!(points.is_empty());
    }

    #[test]
    fn media_sample_points_build_transcript_window_with_citation() {
        let evidence = media_evidence();

        let points = build_static_page_media_sample_points(&[evidence], "media.transcript_windows");

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("转写 1.2s")));
        assert_eq!(points[0].get("value"), Some(&json!(1.0)));
        assert_eq!(points[0].get("kind"), Some(&json!("media_window")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("media.transcript_windows"))
        );
        assert_eq!(points[0].get("text"), Some(&json!("客户询问订单状态")));
        assert_eq!(points[0].get("startSeconds"), Some(&json!(1.2)));
        assert_eq!(points[0].get("endSeconds"), Some(&json!(2.7)));
        assert_eq!(
            points[0].get("timestampLabel"),
            Some(&json!("00:01.200 - 00:02.700"))
        );
        assert_eq!(
            points[0].get("citationLabel"),
            Some(&json!("documents/call.mp3#chunk=0 @ 00:01.200 - 00:02.700"))
        );
        assert_eq!(points[0].get("source"), Some(&json!("local-transcribe")));
        assert_eq!(
            points[0].pointer("/evidenceRef/sourceLocator"),
            Some(&json!("documents/call.mp3#chunk=0"))
        );
    }

    #[test]
    fn media_sample_points_use_scene_summary_and_prefix() {
        let points = build_static_page_media_sample_points(&[media_evidence()], "media.scene");

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("场景 5.0s")));
        assert_eq!(points[0].get("text"), Some(&json!("门店入口画面")));
        assert_eq!(
            points[0].get("timestampLabel"),
            Some(&json!("00:05.000 - 00:08.000"))
        );
    }

    #[test]
    fn media_sample_points_support_keyframe_ocr_and_cap_at_six() {
        let mut evidence = media_evidence();
        let snippets = (0..8)
            .map(|index| {
                json!({
                    "timestamp_seconds": index,
                    "text": format!("客流 {}", index)
                })
            })
            .collect::<Vec<_>>();
        evidence["media_context"]["keyframe_ocr_snippets"] = json!(snippets);

        let points = build_static_page_media_sample_points(&[evidence], "media.ocr");

        assert_eq!(points.len(), 6);
        assert_eq!(points[0].get("label"), Some(&json!("关键帧 0.0s")));
        assert_eq!(points[5].get("label"), Some(&json!("关键帧 5.0s")));
        assert_eq!(points[0].get("source"), Some(&json!("media")));
    }

    #[test]
    fn media_timestamp_format_clamps_negative_seconds() {
        assert_eq!(static_page_format_media_timestamp(-2.5), "00:00.000");
        assert_eq!(static_page_format_media_timestamp(3661.25), "01:01:01.250");
    }

    fn media_evidence() -> Value {
        json!({
            "type": "retrieval_evidence",
            "dataset_id": "dataset-1",
            "document_id": "document-1",
            "document_chunk_id": "chunk-1",
            "retrieval_evidence_id": "evidence-1",
            "source_locator": "documents/call.mp3#chunk=0",
            "media_context": {
                "transcript_windows": [{
                    "start_seconds": 1.2,
                    "end_seconds": 2.7,
                    "text": "客户询问订单状态",
                    "source": "local-transcribe"
                }],
                "scene_windows": [{
                    "start_seconds": 5.0,
                    "end_seconds": 8.0,
                    "summary": "门店入口画面",
                    "source": "scene-detector"
                }],
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 9.5,
                    "text": "今日客流 2180",
                    "source": "keyframe-ocr"
                }]
            }
        })
    }
}
