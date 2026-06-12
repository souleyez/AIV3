use serde_json::{json, Value};

use crate::ApiError;

pub(crate) fn sse_json_event(event_name: &str, data: Value) -> String {
    let data = serde_json::to_string(&data).unwrap_or_else(|error| {
        json!({
            "error": "sse_payload_serialize_failed",
            "message": error.to_string(),
        })
        .to_string()
    });
    format!("event: {event_name}\ndata: {data}\n\n")
}

pub(crate) fn sse_error_event(error: ApiError) -> String {
    sse_json_event(
        "error",
        json!({
            "status": error.status.as_u16(),
            "error": error.payload,
        }),
    ) + &sse_json_event("done", json!({"ok": false}))
}

pub(crate) fn sse_text_delta_events(event_name: &str, text: &str) -> String {
    let mut encoded = String::new();
    let mut chunk = String::new();
    let mut index = 0usize;
    for ch in text.chars() {
        chunk.push(ch);
        if chunk.chars().count() >= 24 {
            encoded.push_str(&sse_json_event(
                event_name,
                json!({
                    "index": index,
                    "delta": chunk,
                }),
            ));
            chunk = String::new();
            index += 1;
        }
    }
    if !chunk.is_empty() {
        encoded.push_str(&sse_json_event(
            event_name,
            json!({
                "index": index,
                "delta": chunk,
            }),
        ));
    }
    encoded
}

pub(crate) fn sse_text_delta_event(event_name: &str, index: usize, delta: &str) -> String {
    sse_json_event(
        event_name,
        json!({
            "index": index,
            "delta": delta,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_json_event_formats_event_and_json_payload() {
        assert_eq!(
            sse_json_event("assistant_run.delta", json!({"index": 1, "delta": "ok"})),
            "event: assistant_run.delta\ndata: {\"delta\":\"ok\",\"index\":1}\n\n"
        );
    }

    #[test]
    fn sse_text_delta_events_chunks_by_twenty_four_chars() {
        let text = "一二三四五六七八九十一二三四五六七八九十一二三四五六";
        let encoded = sse_text_delta_events("assistant_run.delta", text);

        assert!(encoded.contains("\"index\":0"));
        assert!(encoded.contains("\"index\":1"));
        assert!(encoded.contains("一二三四五六七八九十一二三四五六七八九十一二三四"));
        assert!(encoded.contains("五六"));
    }

    #[test]
    fn sse_text_delta_event_formats_single_delta() {
        assert_eq!(
            sse_text_delta_event("external_channel.delta", 2, "继续"),
            "event: external_channel.delta\ndata: {\"delta\":\"继续\",\"index\":2}\n\n"
        );
    }
}
