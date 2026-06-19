use std::collections::BTreeSet;

use domain_model::AssistantRunEvent;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    assistant_run_text_support::truncate_assistant_supply_text,
    external_channel_generated_artifact_public_base_url,
    ASSISTANT_RUN_CUSTOMER_ARTIFACTS_READY_EVENT,
    ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT,
    CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST, CODEX_CAPABILITY_CUSTOMER_COMPLEX_REQUEST,
    CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT, CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH,
    CODEX_CAPABILITY_V3_PRODUCT_CHANGE_REQUEST,
};

pub(crate) fn assistant_run_append_deduped_output_artifacts(
    output_artifacts: Vec<Value>,
    additional_artifacts: Vec<Value>,
) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    output_artifacts
        .into_iter()
        .chain(additional_artifacts)
        .filter(|artifact| {
            let key = serde_json::to_string(artifact).unwrap_or_default();
            seen.insert(key)
        })
        .collect()
}

pub(crate) fn assistant_run_customer_codex_output_artifacts_from_events(
    events: &[AssistantRunEvent],
) -> Vec<Value> {
    assistant_run_append_deduped_output_artifacts(
        Vec::new(),
        events
            .iter()
            .filter_map(assistant_run_customer_codex_output_artifact_from_ready_event)
            .collect(),
    )
}

fn assistant_run_customer_codex_output_artifact_from_ready_event(
    event: &AssistantRunEvent,
) -> Option<Value> {
    let event_name = event.event_name.as_str();
    if !matches!(
        event_name,
        ASSISTANT_RUN_CUSTOMER_ARTIFACTS_READY_EVENT
            | ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT
    ) {
        return None;
    }
    let payload = &event.payload;
    if payload.get("source").and_then(Value::as_str) != Some("codex_host_customer_artifacts") {
        return None;
    }
    let mut customer_artifacts =
        assistant_run_safe_customer_codex_artifacts(payload.get("customer_artifacts")?)?;
    let artifacts = customer_artifacts
        .get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if artifacts.is_empty() {
        return None;
    }
    let artifact_paths = artifacts
        .iter()
        .filter_map(|artifact| artifact.get("path").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let default_capability =
        if event_name == ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT {
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT
        } else {
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
        };
    let capability = payload
        .get("capability")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_artifact_capability)
        .unwrap_or_else(|| default_capability.to_string());
    let default_route =
        assistant_run_customer_codex_default_route_for_capability(event_name, &capability);
    let route = payload
        .get("route")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_artifact_route)
        .unwrap_or(default_route);
    if let Some(object) = customer_artifacts.as_object_mut() {
        object.insert("capability".to_string(), json!(capability.clone()));
    }
    let status = assistant_run_safe_customer_codex_text(
        payload
            .get("status")
            .or_else(|| customer_artifacts.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("available"),
        40,
    );
    let workflow_execution_id = assistant_run_safe_uuid_string(payload, "workflow_execution_id");
    let title = customer_artifacts
        .get("title")
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 160))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if event_name == ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT {
                "Codex generated page edit artifacts".to_string()
            } else {
                "Codex customer artifacts".to_string()
            }
        });
    let manifest_path = customer_artifacts
        .get("manifest_path")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_relative_path);
    let artifact_count = artifacts.len();
    let primary_url = customer_artifacts
        .get("primary_url")
        .or_else(|| customer_artifacts.get("public_url"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url);
    let published = customer_artifacts
        .get("published")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && primary_url.is_some();
    let status = if published {
        "published".to_string()
    } else {
        status
    };
    let links =
        assistant_run_customer_codex_artifact_links(&customer_artifacts, primary_url.as_deref());
    let primary_url_value = primary_url
        .as_ref()
        .map(|url| json!(url))
        .unwrap_or(Value::Null);

    Some(json!({
        "type": "codex_customer_artifact_bundle",
        "artifact_type": "codex_customer_artifacts",
        "artifact_kind": "customer_artifact_bundle",
        "status": status.clone(),
        "capability": capability,
        "route": route,
        "workflow_execution_id": workflow_execution_id.clone(),
        "published": published,
        "primary_url": primary_url_value.clone(),
        "public_url": primary_url_value.clone(),
        "artifact_count": artifact_count,
        "customer_artifacts": customer_artifacts,
        "artifact_manifest": {
            "schema": "v3.output_artifact_manifest",
            "schema_version": 1,
            "artifact_type": "codex_customer_artifacts",
            "artifact_kind": "customer_artifact_bundle",
            "title": title,
            "status": status,
            "primary_url": primary_url_value,
            "links": links,
            "refs": {
                "workflow_execution_id": workflow_execution_id,
                "artifact_paths": artifact_paths,
                "manifest_path": manifest_path,
            },
            "safety": {
                "credentials_exposed": false,
                "raw_logs_exposed": false,
                "workspace_paths_only": true,
                "absolute_paths_exposed": false,
                "published": published,
                "requires_datamax_publish_validation": !published,
            },
        },
    }))
}

fn assistant_run_safe_customer_codex_artifacts(customer_artifacts: &Value) -> Option<Value> {
    if !customer_artifacts.is_object() {
        return None;
    }
    if customer_artifacts.get("schema").and_then(Value::as_str)
        != Some("v3.customer_codex_artifacts")
    {
        return None;
    }
    if customer_artifacts.get("version").and_then(Value::as_i64) != Some(1) {
        return None;
    }
    let artifacts = customer_artifacts
        .get("artifacts")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(assistant_run_safe_customer_codex_artifact_item)
        .collect::<Vec<_>>();
    if artifacts.is_empty() {
        return None;
    }
    let status = assistant_run_safe_customer_codex_text(
        customer_artifacts
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("available"),
        40,
    );
    let capability = customer_artifacts
        .get("capability")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_artifact_capability)
        .unwrap_or_else(|| CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST.to_string());
    let manifest_path = customer_artifacts
        .get("manifest_path")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_relative_path);
    let title = customer_artifacts
        .get("title")
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 160))
        .unwrap_or_default();
    let summary = customer_artifacts
        .get("summary")
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 600))
        .unwrap_or_default();
    let primary_url = customer_artifacts
        .get("primary_url")
        .or_else(|| customer_artifacts.get("public_url"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url)
        .or_else(|| {
            artifacts
                .iter()
                .filter_map(|artifact| artifact.get("public_url").and_then(Value::as_str))
                .find_map(assistant_run_safe_customer_codex_generated_artifact_url)
        });
    let published_manifest_url = customer_artifacts
        .get("published_manifest_url")
        .or_else(|| customer_artifacts.pointer("/publish/manifest_url"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url);
    let published = customer_artifacts
        .get("published")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && primary_url.is_some();
    let published_artifact_count = customer_artifacts
        .get("published_artifact_count")
        .or_else(|| customer_artifacts.pointer("/publish/artifact_count"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            artifacts
                .iter()
                .filter(|artifact| artifact.get("public_url").and_then(Value::as_str).is_some())
                .count() as u64
        });

    let primary_url_value = primary_url
        .as_ref()
        .map(|url| json!(url))
        .unwrap_or(Value::Null);
    let published_manifest_url_value = published_manifest_url
        .as_ref()
        .map(|url| json!(url))
        .unwrap_or(Value::Null);

    Some(json!({
        "schema": "v3.customer_codex_artifacts",
        "version": 1,
        "status": if published { "published".to_string() } else { status },
        "capability": capability,
        "manifest_path": manifest_path,
        "artifact_count": artifacts.len(),
        "artifacts": artifacts,
        "summary": summary,
        "title": title,
        "published": published,
        "published_artifact_count": published_artifact_count,
        "primary_url": primary_url_value.clone(),
        "public_url": primary_url_value,
        "published_manifest_url": published_manifest_url_value,
        "validation": {
            "workspace_scoped": true,
            "v3_product_repo_write_blocked": true,
            "absolute_paths_redacted": true,
            "published_to_generated_artifacts": published,
        },
    }))
}

fn assistant_run_safe_customer_codex_artifact_item(item: &Value) -> Option<Value> {
    let path = item
        .get("path")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_relative_path)?;
    let title = item
        .get("title")
        .or_else(|| item.get("name"))
        .or_else(|| item.get("label"))
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 160))
        .unwrap_or_default();
    let kind = item
        .get("kind")
        .or_else(|| item.get("type"))
        .or_else(|| item.get("artifact_kind"))
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_text(value, 80))
        .unwrap_or_else(|| "file".to_string());
    let mime_type = item
        .get("mime_type")
        .or_else(|| item.get("mimeType"))
        .or_else(|| item.get("content_type"))
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_text(value, 120))
        .unwrap_or_default();
    let bytes = item.get("bytes").and_then(Value::as_u64).unwrap_or(0);
    let sha256 = item
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|value| value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
        .unwrap_or_default();
    let public_url = item
        .get("public_url")
        .or_else(|| item.get("publicUrl"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url);
    let published = item
        .get("published")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && public_url.is_some();

    Some(json!({
        "path": path,
        "title": title,
        "kind": kind,
        "mime_type": mime_type,
        "bytes": bytes,
        "sha256": sha256,
        "published": published,
        "public_url": public_url,
    }))
}

fn assistant_run_safe_customer_codex_relative_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('~')
        || trimmed.starts_with("\\\\")
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.contains('\0')
    {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("/srv/aiv3/repo")
        || lower.contains("/users/")
        || lower.contains("node_modules")
        || lower.contains(".git")
        || lower.contains(".env")
        || lower.ends_with(".env")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("private_key")
        || lower.contains("password")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.contains("id_rsa")
        || lower.contains("id_dsa")
        || lower.contains("id_ecdsa")
        || lower.contains("id_ed25519")
        || lower.contains("authorized_keys")
    {
        return None;
    }
    if trimmed
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn assistant_run_safe_customer_codex_generated_artifact_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty()
        || trimmed.contains('\0')
        || trimmed.contains("/generated-artifacts/pending-")
        || trimmed.contains("/generated-artifacts/pending/")
        || trimmed.ends_with("/generated-artifacts/pending")
    {
        return None;
    }
    if trimmed.starts_with("/generated-artifacts/") {
        return Some(trimmed.to_string());
    }
    let default_base = "https://v3.elepcloud.com/generated-artifacts";
    if trimmed.starts_with(&format!("{default_base}/")) {
        return Some(trimmed.to_string());
    }
    let configured_base = external_channel_generated_artifact_public_base_url();
    if configured_base != default_base
        && trimmed.starts_with(&format!("{}/", configured_base.trim_end_matches('/')))
    {
        return Some(trimmed.to_string());
    }
    None
}

fn assistant_run_customer_codex_artifact_links(
    customer_artifacts: &Value,
    primary_url: Option<&str>,
) -> Vec<Value> {
    let mut links = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(url) = primary_url {
        if seen.insert(url.to_string()) {
            links.push(json!({"rel": "public", "url": url}));
        }
    }
    if let Some(url) = customer_artifacts
        .get("published_manifest_url")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url)
    {
        if seen.insert(url.clone()) {
            links.push(json!({"rel": "manifest", "url": url}));
        }
    }
    if let Some(artifacts) = customer_artifacts
        .get("artifacts")
        .and_then(Value::as_array)
    {
        for artifact in artifacts {
            let Some(url) = artifact
                .get("public_url")
                .and_then(Value::as_str)
                .and_then(assistant_run_safe_customer_codex_generated_artifact_url)
            else {
                continue;
            };
            if !seen.insert(url.clone()) {
                continue;
            }
            let path = artifact
                .get("path")
                .and_then(Value::as_str)
                .and_then(assistant_run_safe_customer_codex_relative_path);
            let kind = artifact
                .get("kind")
                .and_then(Value::as_str)
                .map(|value| assistant_run_safe_customer_codex_text(value, 80))
                .unwrap_or_else(|| "file".to_string());
            links.push(json!({
                "rel": "file",
                "url": url,
                "path": path,
                "kind": kind,
            }));
        }
    }
    links
}

fn assistant_run_safe_customer_codex_artifact_capability(value: &str) -> Option<String> {
    let trimmed = value.trim();
    match trimmed {
        CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
        | CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT
        | CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH => Some(trimmed.to_string()),
        _ => None,
    }
}

fn assistant_run_safe_customer_codex_artifact_route(value: &str) -> Option<String> {
    assistant_run_safe_customer_codex_artifact_capability(value)
}

fn assistant_run_customer_codex_default_route_for_capability(
    event_name: &str,
    capability: &str,
) -> String {
    match capability {
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT => "generated_static_page_edit",
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH => "generated_static_page_publish",
        CODEX_CAPABILITY_CUSTOMER_COMPLEX_REQUEST => "customer_complex_request",
        CODEX_CAPABILITY_V3_PRODUCT_CHANGE_REQUEST => "v3_product_change_request",
        _ if event_name == ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT => {
            "generated_static_page_edit"
        }
        _ => "customer_artifact_request",
    }
    .to_string()
}

fn assistant_run_customer_codex_text_looks_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("authorization")
        || lower.contains("bearer ")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("private_key")
        || lower.contains("secret")
        || lower.contains("cookie")
        || lower.contains("database_url")
        || lower.contains("mysql://")
        || lower.contains("postgres://")
        || lower.contains("postgresql://")
        || lower.contains("mongodb://")
        || lower.contains("/users/")
        || lower.contains("/srv/aiv3/repo")
        || lower.contains("/srv/aiv3/shared")
        || lower.contains("/private/var/")
        || lower.contains("\\.codex")
        || lower.contains("/.codex")
        || lower.contains(".env")
        || lower.contains("sk-")
        || lower.contains(":\\")
}

fn assistant_run_safe_customer_codex_display_text(value: &str, max_chars: usize) -> String {
    let text = assistant_run_safe_customer_codex_text(value, max_chars);
    if assistant_run_customer_codex_text_looks_sensitive(&text) {
        String::new()
    } else {
        text
    }
}

fn assistant_run_safe_customer_codex_text(value: &str, max_chars: usize) -> String {
    truncate_assistant_supply_text(value.trim(), max_chars)
}

fn assistant_run_safe_uuid_string(payload: &Value, key: &str) -> Value {
    payload
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(|uuid| json!(uuid.to_string()))
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_run_customer_codex_dedup_preserves_first_seen_artifacts() {
        let first = json!({"type": "artifact", "path": "reports/index.html"});
        let second = json!({"type": "artifact", "path": "reports/notes.md"});

        let merged = assistant_run_append_deduped_output_artifacts(
            vec![first.clone(), second.clone()],
            vec![first.clone()],
        );

        assert_eq!(merged, vec![first, second]);
    }
}
