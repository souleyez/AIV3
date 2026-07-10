use domain_model::TenantId;
use serde_json::{json, Value};

use crate::platform_env_flag;

pub(crate) const MAIN_SITE_ASSET_IMPORT_ENABLED_ENV: &str = "MAIN_SITE_ASSET_IMPORT_ENABLED";
pub(crate) const MAIN_SITE_ASSET_IMPORT_TENANT_ALLOWLIST_ENV: &str =
    "MAIN_SITE_ASSET_IMPORT_TENANT_ALLOWLIST";
pub(crate) const ASSET_PROFILE_SUPPLY_ENABLED_ENV: &str = "ASSET_PROFILE_SUPPLY_ENABLED";
pub(crate) const EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED_ENV: &str =
    "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainSiteAssetImportAccess {
    Enabled,
    FeatureDisabled,
    TenantNotAllowlisted,
}

pub(crate) fn main_site_asset_import_access(tenant_id: TenantId) -> MainSiteAssetImportAccess {
    main_site_asset_import_access_from_values(
        platform_env_flag(MAIN_SITE_ASSET_IMPORT_ENABLED_ENV, false),
        std::env::var(MAIN_SITE_ASSET_IMPORT_TENANT_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        &tenant_id.0.to_string(),
    )
}

pub(crate) fn main_site_asset_import_readiness(tenant_id: TenantId) -> Value {
    let access = main_site_asset_import_access(tenant_id);
    json!({
        "enabled": access == MainSiteAssetImportAccess::Enabled,
        "reason_code": match access {
            MainSiteAssetImportAccess::Enabled => "enabled",
            MainSiteAssetImportAccess::FeatureDisabled => "feature_disabled",
            MainSiteAssetImportAccess::TenantNotAllowlisted => "tenant_not_allowlisted",
        },
        "single_supported": true,
        "batch_supported": true,
        "zip_supported": true,
    })
}

pub(crate) fn asset_profile_supply_enabled() -> bool {
    platform_env_flag(ASSET_PROFILE_SUPPLY_ENABLED_ENV, false)
}

pub(crate) fn external_image_structured_extract_enabled() -> bool {
    platform_env_flag(EXTERNAL_IMAGE_STRUCTURED_EXTRACT_ENABLED_ENV, false)
}

fn main_site_asset_import_access_from_values(
    enabled: bool,
    allowlist: Option<&str>,
    tenant_id: &str,
) -> MainSiteAssetImportAccess {
    if !enabled {
        return MainSiteAssetImportAccess::FeatureDisabled;
    }
    let expected = tenant_id.trim();
    if expected.is_empty() {
        return MainSiteAssetImportAccess::TenantNotAllowlisted;
    }
    if allowlist
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .any(|value| !value.is_empty() && value.eq_ignore_ascii_case(expected))
    {
        MainSiteAssetImportAccess::Enabled
    } else {
        MainSiteAssetImportAccess::TenantNotAllowlisted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_site_asset_import_is_default_deny() {
        assert_eq!(
            main_site_asset_import_access_from_values(false, None, "tenant-a"),
            MainSiteAssetImportAccess::FeatureDisabled
        );
        assert_eq!(
            main_site_asset_import_access_from_values(true, None, "tenant-a"),
            MainSiteAssetImportAccess::TenantNotAllowlisted
        );
        assert_eq!(
            main_site_asset_import_access_from_values(true, Some("tenant-b"), "tenant-a"),
            MainSiteAssetImportAccess::TenantNotAllowlisted
        );
    }

    #[test]
    fn main_site_asset_import_requires_exact_non_empty_tenant_match() {
        assert_eq!(
            main_site_asset_import_access_from_values(
                true,
                Some(" tenant-a, TENANT-B "),
                "tenant-b",
            ),
            MainSiteAssetImportAccess::Enabled
        );
        assert_eq!(
            main_site_asset_import_access_from_values(true, Some("tenant-a"), ""),
            MainSiteAssetImportAccess::TenantNotAllowlisted
        );
    }
}
