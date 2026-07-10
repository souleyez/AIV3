use serde_json::{json, Value};

use crate::external_image_structured_extract_record_support::{
    external_image_structured_extract_payload_quality_score,
    external_image_structured_extract_raw_records,
    external_image_structured_extract_visible_row_count,
};

pub(crate) fn external_image_structured_extract_prompt(
    prompt: &str,
    schema: &Value,
    extraction_id: &str,
) -> String {
    let schema_text = serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string());
    format!(
        "本轮抽取编号：{extraction_id}\n用户要求：{}\n字段 schema：{schema_text}\n抽取步骤必须遵守：\n1. 先识别图片中的表头/列名和可见数据行数量。\n2. 只抽取本轮图片真实可见的订单、充值、支付记录，不要使用示例值、历史结果或字段 schema 猜测。\n3. records 必须按图片从上到下逐行输出；看到几行有效记录就输出几条，不要只输出第一行，不要合并多行。\n4. 每条记录增加 row_index，从 1 开始，对应图片中的可见行序号。\n5. 输出 visible_row_count，表示你实际看到的有效数据行数量；如果 records 数量少于 visible_row_count，必须继续补齐。\n6. 金额保留原文和数字：例如 recharge_amount_raw=\"$100\"，recharge_amount=100；pay_amount_raw=\"$700\"，pay_amount=700。\n7. 订单号要逐字符抄写，不要改写相近字符；时间按图片原文输出。\n若你无法实际看到图片、图片不可访问、图片内容与订单/充值记录无关，必须输出 {{\"records\":[],\"visible_row_count\":0,\"confidence\":0,\"needs_review\":true,\"notes\":\"image_not_visible_or_not_order_screenshot\"}}。\n输出严格 JSON：{{\"records\":[...] , \"visible_row_count\":0, \"confidence\":0-1, \"needs_review\":false, \"notes\":\"\"}}。每条记录字段优先包含 row_index、recharge_amount、recharge_amount_raw、pay_amount、pay_amount_raw、payment_method、payment_method_label、order_no、status、status_label、created_at。状态中文成功归一为 success，处理中归一为 processing；支付方式支付宝归一为 alipay。无法确认的字段填 null。",
        prompt.trim()
    )
}

pub(crate) fn external_image_structured_extract_retry_prompt(
    prompt: &str,
    schema: &Value,
    extraction_id: &str,
    previous_payload: &Value,
) -> String {
    let previous_summary = json!({
        "visible_row_count": external_image_structured_extract_visible_row_count(previous_payload),
        "record_count": external_image_structured_extract_raw_records(previous_payload).len(),
        "quality_score": external_image_structured_extract_payload_quality_score(previous_payload),
        "notes": previous_payload.get("notes").cloned().unwrap_or(Value::Null),
    });
    format!(
        "{}\n\n上一轮抽取疑似不完整，请重新看图复核。上一轮摘要：{}。\n这次必须重点检查是否漏行、漏订单号、漏状态、漏支付方式或把多行合并成一行。最终仍只输出一份完整 JSON，不要解释。",
        external_image_structured_extract_prompt(prompt, schema, extraction_id),
        previous_summary
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_id_trimmed_prompt_schema_and_strict_json_rules() {
        let prompt = external_image_structured_extract_prompt(
            "  请识别充值记录截图  ",
            &json!({
                "record_type": "recharge_order",
                "fields": [{ "name": "order_no" }]
            }),
            "img-extract-test",
        );

        assert!(prompt.contains("本轮抽取编号：img-extract-test"));
        assert!(prompt.contains("用户要求：请识别充值记录截图"));
        assert!(prompt.contains(r#""record_type":"recharge_order""#));
        assert!(prompt.contains("records 必须按图片从上到下逐行输出"));
        assert!(prompt.contains("不要使用示例值、历史结果或字段 schema 猜测"));
        assert!(prompt.contains("输出严格 JSON"));
    }

    #[test]
    fn retry_prompt_includes_previous_summary_and_reuses_base_prompt() {
        let retry = external_image_structured_extract_retry_prompt(
            "请识别订单",
            &json!({ "fields": [{ "name": "order_no" }] }),
            "img-extract-retry",
            &json!({
                "visible_row_count": 3,
                "records": [{ "row_index": 1, "order_no": "A1" }],
                "notes": "maybe_missing_rows"
            }),
        );

        assert!(retry.contains("本轮抽取编号：img-extract-retry"));
        assert!(retry.contains("上一轮抽取疑似不完整"));
        assert!(retry.contains(r#""visible_row_count":3"#));
        assert!(retry.contains(r#""record_count":1"#));
        assert!(retry.contains("maybe_missing_rows"));
        assert!(retry.contains("最终仍只输出一份完整 JSON"));
    }
}
