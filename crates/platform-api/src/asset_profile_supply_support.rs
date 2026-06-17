use serde_json::Value;
use std::collections::BTreeSet;

const SUMMARY_LIMIT: usize = 240;
const TERMS_LIMIT: usize = 24;
const FACETS_LIMIT: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetProfileSupplyInput {
    pub asset_id: String,
    pub title: String,
    pub asset_kind: String,
    pub source_kind: String,
    pub profile_kind: String,
    pub attributes: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetProfileSupplyHint {
    pub asset_id: String,
    pub title: String,
    pub asset_kind: String,
    pub source_kind: String,
    pub profile_kind: String,
    pub summary: String,
    pub noun_terms: Vec<String>,
    pub facets: Vec<String>,
}

pub(crate) fn build_asset_profile_supply_hints(
    profiles: &[AssetProfileSupplyInput],
    limit: usize,
) -> Vec<AssetProfileSupplyHint> {
    let mut hints = profiles
        .iter()
        .filter_map(build_asset_profile_supply_hint)
        .collect::<Vec<_>>();
    hints.sort_by(|left, right| {
        left.asset_kind
            .cmp(&right.asset_kind)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.profile_kind.cmp(&right.profile_kind))
            .then_with(|| left.asset_id.cmp(&right.asset_id))
    });
    hints.truncate(limit.min(100));
    hints
}

fn build_asset_profile_supply_hint(
    profile: &AssetProfileSupplyInput,
) -> Option<AssetProfileSupplyHint> {
    let title = normalize_text(&profile.title);
    let asset_id = normalize_text(&profile.asset_id);
    if title.is_empty() || asset_id.is_empty() || !profile.attributes.is_object() {
        return None;
    }

    let noun_terms = collect_noun_terms(&profile.attributes);
    let facets = collect_facets(&profile.attributes);
    let summary = build_summary(&profile.attributes, &facets)
        .unwrap_or_else(|| title.clone())
        .chars()
        .take(SUMMARY_LIMIT)
        .collect::<String>();

    Some(AssetProfileSupplyHint {
        asset_id,
        title,
        asset_kind: normalize_text(&profile.asset_kind),
        source_kind: normalize_text(&profile.source_kind),
        profile_kind: normalize_text(&profile.profile_kind),
        summary,
        noun_terms,
        facets,
    })
}

fn build_summary(attributes: &Value, facets: &[String]) -> Option<String> {
    for key in [
        "summary",
        "description",
        "caption",
        "ocr_text",
        "transcript_summary",
    ] {
        if let Some(value) = attributes.get(key).and_then(Value::as_str) {
            let text = normalize_text(value);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    if facets.is_empty() {
        None
    } else {
        Some(facets.join("；"))
    }
}

fn collect_noun_terms(attributes: &Value) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for key in [
        "noun_terms",
        "nounTermHints",
        "tags",
        "keywords",
        "objects",
        "entities",
        "colors",
        "materials",
        "scenes",
        "styles",
    ] {
        collect_text_values(attributes.get(key), &mut terms);
    }
    for key in [
        "category",
        "season",
        "style",
        "silhouette",
        "material",
        "process",
        "theme",
    ] {
        if let Some(value) = attributes.get(key) {
            collect_text_values(Some(value), &mut terms);
        }
    }
    terms.into_iter().take(TERMS_LIMIT).collect()
}

fn collect_facets(attributes: &Value) -> Vec<String> {
    let mut facets = Vec::new();
    for (label, key) in [
        ("品类", "category"),
        ("季节", "season"),
        ("风格", "style"),
        ("版型", "silhouette"),
        ("颜色", "colors"),
        ("面料", "materials"),
        ("场景", "scenes"),
        ("状态", "status"),
    ] {
        let values = normalized_text_values(attributes.get(key));
        if !values.is_empty() {
            facets.push(format!("{label}: {}", values.join("、")));
        }
    }
    facets.truncate(FACETS_LIMIT);
    facets
}

fn collect_text_values(value: Option<&Value>, target: &mut BTreeSet<String>) {
    for text in normalized_text_values(value) {
        target.insert(text);
    }
}

fn normalized_text_values(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => {
            let text = normalize_text(text);
            if text.is_empty() {
                vec![]
            } else {
                vec![text]
            }
        }
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(normalize_text)
            .filter(|text| !text.is_empty())
            .collect(),
        _ => vec![],
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn asset_profile_supply_extracts_multimodal_terms_and_facets() {
        let hints = build_asset_profile_supply_hints(
            &[AssetProfileSupplyInput {
                asset_id: "asset-1".to_string(),
                title: "春夏连衣裙灵感图".to_string(),
                asset_kind: "image".to_string(),
                source_kind: "upload".to_string(),
                profile_kind: "image_semantic".to_string(),
                attributes: json!({
                    "category": "连衣裙",
                    "season": "春夏",
                    "style": "通勤",
                    "colors": ["绿色", "白色"],
                    "materials": ["棉麻"],
                    "noun_terms": ["泡泡袖", "碎花", "连衣裙"]
                }),
            }],
            10,
        );

        assert_eq!(hints.len(), 1);
        assert!(hints[0].summary.contains("品类: 连衣裙"));
        assert!(hints[0].summary.contains("颜色: 绿色、白色"));
        assert!(hints[0].noun_terms.contains(&"泡泡袖".to_string()));
        assert!(hints[0].noun_terms.contains(&"棉麻".to_string()));
        assert_eq!(hints[0].profile_kind, "image_semantic");
    }

    #[test]
    fn asset_profile_supply_prefers_existing_summaries_and_limits_output() {
        let hints = build_asset_profile_supply_hints(
            &[
                AssetProfileSupplyInput {
                    asset_id: "asset-b".to_string(),
                    title: "视频资产".to_string(),
                    asset_kind: "video".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "video_summary".to_string(),
                    attributes: json!({
                        "summary": "门店陈列视频，重点展示夏季女装区域和导购讲解。",
                        "entities": ["门店", "女装", "导购"]
                    }),
                },
                AssetProfileSupplyInput {
                    asset_id: "asset-a".to_string(),
                    title: "PPT资产".to_string(),
                    asset_kind: "presentation".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "slide_outline".to_string(),
                    attributes: json!({
                        "description": "季度经营复盘 PPT，包含销售趋势和库存风险。",
                        "keywords": ["销售趋势", "库存风险"]
                    }),
                },
            ],
            1,
        );

        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].asset_kind, "presentation");
        assert_eq!(
            hints[0].summary,
            "季度经营复盘 PPT，包含销售趋势和库存风险。"
        );
        assert!(hints[0].noun_terms.contains(&"库存风险".to_string()));
    }

    #[test]
    fn asset_profile_supply_drops_empty_or_non_object_profiles() {
        let hints = build_asset_profile_supply_hints(
            &[
                AssetProfileSupplyInput {
                    asset_id: "".to_string(),
                    title: "missing id".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "image_semantic".to_string(),
                    attributes: json!({"category": "连衣裙"}),
                },
                AssetProfileSupplyInput {
                    asset_id: "asset-2".to_string(),
                    title: "bad attrs".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "image_semantic".to_string(),
                    attributes: json!("raw text should not be used directly"),
                },
            ],
            10,
        );

        assert!(hints.is_empty());
    }
}
