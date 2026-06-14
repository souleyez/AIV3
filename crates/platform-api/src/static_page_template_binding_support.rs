pub(crate) fn static_page_template_text_contains_any(
    text: &str,
    lower_text: &str,
    needles: &[&str],
) -> bool {
    needles.iter().any(|needle| {
        let needle = needle.trim();
        if needle.is_empty() {
            return false;
        }
        if text.contains(needle) {
            return true;
        }
        lower_text.contains(&needle.to_ascii_lowercase())
    })
}

pub(crate) fn static_page_template_request_needs_store_sales_binding(prompt: Option<&str>) -> bool {
    let prompt = prompt.unwrap_or_default().trim();
    if prompt.is_empty() {
        return false;
    }
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let lower = compact.to_ascii_lowercase();
    let has_metric = static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "近7日",
            "近七日",
            "销售",
            "营业额",
            "营收",
            "租金",
            "高分成",
            "取高",
            "sales",
            "revenue",
            "rent",
        ],
    );
    let has_partition = static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "门店", "分店", "店铺", "区域", "分区", "品牌", "客户", "store", "region", "area",
            "brand",
        ],
    );
    let has_binding = static_page_template_text_contains_any(
        &compact,
        &lower,
        &[
            "联动",
            "筛选",
            "切换",
            "刷新",
            "不会变",
            "不变化",
            "不更新",
            "绑定",
            "filter",
            "binding",
            "refresh",
            "change",
        ],
    );
    has_metric && (has_partition || has_binding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_contains_any_matches_original_or_ascii_lowercase() {
        let text = "近7日Revenue按门店切换";
        let lower = text.to_ascii_lowercase();

        assert!(static_page_template_text_contains_any(
            text,
            &lower,
            &["revenue"]
        ));
        assert!(static_page_template_text_contains_any(
            text,
            &lower,
            &["近7日"]
        ));
        assert!(!static_page_template_text_contains_any(
            text,
            &lower,
            &["   ", "traffic"]
        ));
    }

    #[test]
    fn store_sales_binding_detects_metric_with_partition_or_binding() {
        assert!(static_page_template_request_needs_store_sales_binding(
            Some("近7日销售切换区域和门店必须联动刷新")
        ));
        assert!(static_page_template_request_needs_store_sales_binding(
            Some("STORE revenue filter does not change")
        ));
        assert!(static_page_template_request_needs_store_sales_binding(
            Some("租金取高筛选不会变")
        ));
    }

    #[test]
    fn store_sales_binding_rejects_empty_or_unrelated_prompts() {
        assert!(!static_page_template_request_needs_store_sales_binding(
            None
        ));
        assert!(!static_page_template_request_needs_store_sales_binding(
            Some("   ")
        ));
        assert!(!static_page_template_request_needs_store_sales_binding(
            Some("销售趋势整体说明")
        ));
        assert!(!static_page_template_request_needs_store_sales_binding(
            Some("区域门店布局说明")
        ));
    }
}
