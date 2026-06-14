use std::collections::HashSet;

const EXTERNAL_CHANNEL_PUBLIC_REPLY_TEXT_LIMIT: usize = 6000;
const EXTERNAL_CHANNEL_PUBLIC_STREAM_TEXT_LIMIT: usize = 1200;

pub(crate) fn external_channel_public_status(status: &str) -> String {
    match status.trim() {
        "static_page_image_preview_queued" => "static_page_generation_queued".to_string(),
        "static_page_effect_image_ready" => "static_page_preview_ready".to_string(),
        "static_page_image2_auto_publish_pending" => "static_page_generation_pending".to_string(),
        "static_page_image2_auto_publish_running" => "static_page_generation_running".to_string(),
        "static_page_image2_auto_publish_retrying" => "static_page_generation_retrying".to_string(),
        "static_page_image2_auto_publish_failed" => "static_page_generation_failed".to_string(),
        other => external_channel_public_text(other),
    }
}

pub(crate) fn external_channel_public_task_status(task_status: &str) -> &str {
    if !task_status.starts_with("static_page_") {
        return task_status;
    }
    match task_status {
        "static_page_published" => "static_page_published",
        "static_page_stable_artifact_reused" => "static_page_published",
        "static_page_publish_failed" => "processing",
        "static_page_publish_cancelled" => "failed",
        "static_page_publish_needs_human" => "processing",
        _ => "processing",
    }
}

pub(crate) fn external_channel_public_text(text: &str) -> String {
    let trimmed = text.trim_start();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("/generated-artifacts/")
        || trimmed.starts_with("generated-artifacts/")
    {
        return text.to_string();
    }
    let mut value = text.to_string();
    for (raw, replacement) in [
        (
            "已创建静态页草稿并提交 Image2 效果图队列；效果图无需客户确认，生成后会继续进入固定 Cloudflare Codex 发布链路。",
            "已创建报表页面草稿并进入生成队列；无需客户确认，生成后会自动发布页面链接。",
        ),
        (
            "已创建静态页草稿并提交 Image2 效果图队列；固定发布链路当前未启用或未加入 allowlist。",
            "已创建报表页面草稿并进入生成队列；当前会按可用发布链路生成页面。",
        ),
        (
            "已创建静态页草稿并提交 GPT-Image-2 效果图队列；效果图无需客户确认，生成后会继续进入 Image2 视觉合同发布链路，由 GPT-5.5/Codex 依据效果图生成最终动态网站。",
            "已创建报表页面草稿并进入生成队列；无需客户确认，生成后会自动发布最终动态页面。",
        ),
        (
            "已创建静态页草稿并提交 GPT-Image-2 效果图队列；效果图无需客户确认，生成后会继续进入 Image2 视觉合同发布链路生成最终动态网站。",
            "已创建报表页面草稿并进入生成队列；无需客户确认，生成后会自动发布最终动态页面。",
        ),
        (
            "DataMax 已先生成可发送的静态页链接；最终 Codex 页面仍在后台继续发布。",
            "DataMax 已先生成可发送的静态页链接；最终页面仍在后台继续优化发布。",
        ),
    ] {
        value = value.replace(raw, replacement);
    }
    for (raw, replacement) in [
        ("GPT-Image-2", "页面生成"),
        ("Image2", "页面生成"),
        ("Cloudflare Codex", "DataMax 后台"),
        ("Cloudflare", "DataMax 后台"),
        ("Codex", "DataMax"),
        ("效果图", "可视化预览"),
        ("生图", "页面生成"),
        ("视觉合同", "页面生成"),
    ] {
        value = value.replace(raw, replacement);
    }
    value
}

pub(crate) fn external_channel_public_reply_text(text: &str) -> String {
    let public_text = external_channel_public_text(text);
    let value = external_channel_public_readable_text(&public_text);
    if value.trim().is_empty()
        && external_channel_text_looks_like_internal_context_leak(&public_text)
    {
        return "本轮回复包含内部处理上下文，DataMax 已拦截该部分。请继续提问或指定需要查看的结论，我会重新基于已授权资料回答。".to_string();
    }
    if external_channel_text_looks_like_internal_context_leak(&value) {
        return "本轮回复包含内部处理上下文，DataMax 已拦截该部分。请继续提问或指定需要查看的结论，我会重新基于已授权资料回答。".to_string();
    }
    truncate_external_channel_public_text(&value, EXTERNAL_CHANNEL_PUBLIC_REPLY_TEXT_LIMIT)
}

pub(crate) fn external_channel_public_stream_text(text: &str) -> String {
    let public_text = external_channel_public_text(text);
    let value = external_channel_public_readable_text(&public_text);
    if value.trim().is_empty()
        && external_channel_text_looks_like_internal_context_leak(&public_text)
    {
        return "DataMax 正在处理，本轮内部上下文不会对外展示。".to_string();
    }
    if external_channel_text_looks_like_internal_context_leak(&value) {
        return "DataMax 正在处理，本轮内部上下文不会对外展示。".to_string();
    }
    truncate_external_channel_public_text(&value, EXTERNAL_CHANNEL_PUBLIC_STREAM_TEXT_LIMIT)
}

fn external_channel_public_readable_text(text: &str) -> String {
    let mut value = text
        .trim()
        .replace("\\r\\n", "\n")
        .replace("\\n", "\n")
        .replace("\\t", " ");
    value = strip_embedded_external_channel_json_envelope(&value);
    value = dedupe_public_generated_artifact_links_in_text(&value);
    collapse_public_text_spacing(&value)
}

fn strip_embedded_external_channel_json_envelope(text: &str) -> String {
    let markers = [
        "{\"assistant_run_id\"",
        "\"assistant_run_id\"",
        "{\"card\"",
        "{\"conversation_external_id\"",
        "\"conversation_external_id\"",
        "{\"data\":{\"assistant_run_id\"",
        "\"idempotency_key\"",
        "\"poll_after_seconds\"",
        "\"status_url\"",
    ];
    let mut cut_at: Option<usize> = None;
    for marker in markers {
        if let Some(index) = text.find(marker) {
            let candidate = text[..index].rfind('{').unwrap_or(index);
            cut_at = Some(cut_at.map_or(candidate, |current| current.min(candidate)));
        }
    }
    let cleaned = cut_at.map_or_else(
        || text.to_string(),
        |index| text[..index].trim().to_string(),
    );
    cleaned
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("{\"assistant_run_id\"")
                || trimmed.starts_with("{\"card\"")
                || trimmed.starts_with("{\"data\":{\"assistant_run_id\""))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn dedupe_public_generated_artifact_links_in_text(text: &str) -> String {
    let mut seen_links = HashSet::new();
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            let link = trimmed
                .split_whitespace()
                .find(|part| {
                    part.contains("/generated-artifacts/")
                        || part.starts_with("https://v3.elepcloud.com/generated-artifacts/")
                })
                .map(|part| {
                    part.trim_matches(|ch: char| {
                        matches!(ch, ')' | ']' | '>' | '。' | '，' | ',' | ';' | '；')
                    })
                    .to_string()
                });
            match link {
                Some(link) if !link.is_empty() => seen_links.insert(link),
                _ => true,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn collapse_public_text_spacing(text: &str) -> String {
    let mut output = Vec::new();
    let mut blank_seen = false;
    for line in text.lines() {
        let trimmed_end = line.trim_end();
        if trimmed_end.trim().is_empty() {
            if !blank_seen {
                output.push(String::new());
            }
            blank_seen = true;
            continue;
        }
        output.push(trimmed_end.to_string());
        blank_seen = false;
    }
    output.join("\n").trim().to_string()
}

fn truncate_external_channel_public_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let mut value: String = text.chars().take(max_chars).collect();
    value.push_str("...");
    value
}

fn external_channel_text_looks_like_internal_context_leak(text: &str) -> bool {
    let trimmed = text.trim_start();
    let text_lc = trimmed.to_ascii_lowercase();
    trimmed.contains("供料证据")
        || trimmed.contains("启动简报")
        || trimmed.contains("当前选中范围")
        || trimmed.contains("范围候选")
        || trimmed.contains("本轮外部回答要求")
        || text_lc.contains("supplied_items")
        || text_lc.contains("evidence_state")
        || text_lc.contains("runtime_manifest")
        || text_lc.contains("modelguidance")
        || text_lc.contains("data_snapshot")
        || text_lc.contains("module_bindings")
        || text_lc.contains("\"assistant_run_id\"")
        || text_lc.contains("\"conversation_external_id\"")
        || text_lc.contains("\"idempotency_key\"")
        || text_lc.contains("\"poll_after_seconds\"")
        || text_lc.contains("\"status_url\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_channel_public_status_maps_internal_static_page_steps() {
        assert_eq!(
            external_channel_public_status("static_page_image_preview_queued"),
            "static_page_generation_queued"
        );
        assert_eq!(
            external_channel_public_status("static_page_image2_auto_publish_running"),
            "static_page_generation_running"
        );
    }

    #[test]
    fn external_channel_public_task_status_maps_static_page_terminal_states() {
        assert_eq!(
            external_channel_public_task_status("static_page_published"),
            "static_page_published"
        );
        assert_eq!(
            external_channel_public_task_status("static_page_stable_artifact_reused"),
            "static_page_published"
        );
        assert_eq!(
            external_channel_public_task_status("static_page_publish_failed"),
            "processing"
        );
        assert_eq!(
            external_channel_public_task_status("static_page_publish_cancelled"),
            "failed"
        );
        assert_eq!(
            external_channel_public_task_status("static_page_publish_needs_human"),
            "processing"
        );
        assert_eq!(
            external_channel_public_task_status("static_page_effect_image_ready"),
            "processing"
        );
        assert_eq!(external_channel_public_task_status("answered"), "answered");
    }

    #[test]
    fn external_channel_public_text_keeps_generated_artifact_url_as_is() {
        let url = "https://v3.elepcloud.com/generated-artifacts/demo/index.html";
        assert_eq!(external_channel_public_text(url), url);
    }

    #[test]
    fn external_channel_public_stream_text_replaces_internal_context_leak() {
        let text = r#"{"assistant_run_id":"run-1","status_url":"/internal"}"#;
        assert_eq!(
            external_channel_public_stream_text(text),
            "DataMax 正在处理，本轮内部上下文不会对外展示。"
        );
    }
}
