use contracts::ReportRenderOutputView;
use serde_json::Value;

pub(crate) fn report_render_output_has_asset_path(output: &ReportRenderOutputView) -> bool {
    report_render_output_asset_path(output).is_some()
}

pub(crate) fn report_render_output_asset_path(output: &ReportRenderOutputView) -> Option<String> {
    report_render_asset_manifest_path(&output.asset_manifest)
}

pub(crate) fn report_render_output_asset_kind(output: &ReportRenderOutputView) -> Option<String> {
    report_render_asset_manifest_kind(&output.asset_manifest)
}

fn report_render_asset_manifest_path(asset_manifest: &Value) -> Option<String> {
    asset_manifest
        .as_object()
        .and_then(|manifest| manifest.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn report_render_asset_manifest_kind(asset_manifest: &Value) -> Option<String> {
    asset_manifest
        .as_object()
        .and_then(|manifest| manifest.get("kind"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn report_render_asset_manifest_path_trims_and_rejects_blank_values() {
        assert_eq!(
            report_render_asset_manifest_path(&json!({ "path": " reports/demo/index.html " })),
            Some("reports/demo/index.html".to_string()),
        );
        assert_eq!(
            report_render_asset_manifest_path(&json!({ "path": "   " })),
            None,
        );
        assert_eq!(report_render_asset_manifest_path(&json!({})), None);
        assert_eq!(
            report_render_asset_manifest_path(&json!({ "path": 123 })),
            None,
        );
    }

    #[test]
    fn report_render_asset_manifest_kind_reads_string_kind_only() {
        assert_eq!(
            report_render_asset_manifest_kind(&json!({ "kind": "html_report" })),
            Some("html_report".to_string()),
        );
        assert_eq!(report_render_asset_manifest_kind(&json!({})), None);
        assert_eq!(
            report_render_asset_manifest_kind(&json!({ "kind": true })),
            None,
        );
    }
}
