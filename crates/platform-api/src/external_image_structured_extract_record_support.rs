use serde_json::{json, Value};

use crate::text_normalization::non_empty_trimmed_string;

pub(crate) fn external_image_structured_extract_record_text(
    record: &Value,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        record.get(*key).and_then(|value| match value {
            Value::String(text) => non_empty_trimmed_string(text),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
    })
}

fn external_image_structured_extract_record_number(record: &Value, key: &str) -> Option<f64> {
    record.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => external_image_structured_extract_amount_number(text),
        _ => None,
    })
}

pub(crate) fn external_image_structured_extract_record_number_any(
    record: &Value,
    keys: &[&str],
) -> Option<f64> {
    keys.iter()
        .find_map(|key| external_image_structured_extract_record_number(record, key))
}

pub(crate) fn external_image_structured_extract_amount_number(value: &str) -> Option<f64> {
    let cleaned = value
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == '-')
        .collect::<String>();
    if cleaned.trim().is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

pub(crate) fn external_image_structured_extract_status(value: &str) -> Option<&'static str> {
    let lower = value.trim().to_ascii_lowercase();
    if lower.contains("成功") || lower.contains("success") || lower.contains("succeed") {
        Some("success")
    } else if lower.contains("处理")
        || lower.contains("进行")
        || lower.contains("pending")
        || lower.contains("processing")
    {
        Some("processing")
    } else if lower.contains("失败") || lower.contains("fail") || lower.contains("error") {
        Some("failed")
    } else if lower.contains("取消") || lower.contains("cancel") {
        Some("cancelled")
    } else {
        None
    }
}

pub(crate) fn external_image_structured_extract_payment_method(
    value: &str,
) -> Option<&'static str> {
    let lower = value.trim().to_ascii_lowercase();
    if lower.contains("支付宝") || lower.contains("alipay") {
        Some("alipay")
    } else if lower.contains("微信") || lower.contains("wechat") || lower.contains("weixin") {
        Some("wechat_pay")
    } else if lower.contains("银联") || lower.contains("银行卡") || lower.contains("unionpay")
    {
        Some("bank_card")
    } else {
        None
    }
}

pub(crate) fn external_image_structured_extract_raw_records(raw_payload: &Value) -> Vec<Value> {
    if let Some(items) = raw_payload.get("records").and_then(Value::as_array) {
        return items.clone();
    }
    for key in ["orders", "rows", "items", "data"] {
        if let Some(items) = raw_payload.get(key).and_then(Value::as_array) {
            return items.clone();
        }
    }
    match raw_payload {
        Value::Array(items) => items.clone(),
        Value::Object(_) => vec![raw_payload.clone()],
        _ => Vec::new(),
    }
}

pub(crate) fn external_image_structured_extract_visible_row_count(
    payload: &Value,
) -> Option<usize> {
    payload
        .get("visible_row_count")
        .or_else(|| payload.get("row_count"))
        .or_else(|| payload.get("detected_row_count"))
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64().map(|value| value as usize),
            Value::String(text) => text.trim().parse::<usize>().ok(),
            _ => None,
        })
}

pub(crate) fn external_image_structured_extract_payload_needs_retry(payload: &Value) -> bool {
    let records = external_image_structured_extract_raw_records(payload);
    if records.is_empty() {
        return true;
    }
    if let Some(visible_row_count) = external_image_structured_extract_visible_row_count(payload) {
        if visible_row_count > records.len() {
            return true;
        }
    }
    records.iter().any(|record| {
        external_image_structured_extract_record_text(
            record,
            &["order_no", "order_id", "order_number", "订单号", "订单编号"],
        )
        .is_none()
            || external_image_structured_extract_record_text(
                record,
                &["status", "status_label", "状态"],
            )
            .is_none()
            || external_image_structured_extract_record_text(
                record,
                &[
                    "pay_amount",
                    "pay_amount_raw",
                    "payment_amount",
                    "支付金额",
                    "付款金额",
                ],
            )
            .is_none()
    })
}

pub(crate) fn external_image_structured_extract_payload_quality_score(payload: &Value) -> usize {
    let records = external_image_structured_extract_raw_records(payload);
    records
        .iter()
        .map(|record| {
            let mut score = 10;
            for keys in [
                &["order_no", "order_id", "order_number", "订单号", "订单编号"][..],
                &["status", "status_label", "状态"][..],
                &[
                    "pay_amount",
                    "pay_amount_raw",
                    "payment_amount",
                    "支付金额",
                    "付款金额",
                ][..],
                &[
                    "recharge_amount",
                    "recharge_amount_raw",
                    "充值额度",
                    "充值金额",
                ][..],
                &[
                    "payment_method",
                    "payment_method_label",
                    "pay_method",
                    "支付方式",
                ][..],
                &[
                    "created_at",
                    "create_time",
                    "created_time",
                    "创建时间",
                    "时间",
                ][..],
            ] {
                if external_image_structured_extract_record_text(record, keys).is_some() {
                    score += 2;
                }
            }
            score
        })
        .sum::<usize>()
}

pub(crate) fn external_image_structured_extract_normalize_record(record: &Value) -> Value {
    let row_index = external_image_structured_extract_record_number_any(
        record,
        &["row_index", "row", "index", "行号", "序号"],
    )
    .and_then(|value| {
        if value.is_finite() && value >= 0.0 {
            Some(value as u64)
        } else {
            None
        }
    });
    let recharge_amount_raw = external_image_structured_extract_record_text(
        record,
        &[
            "recharge_amount_raw",
            "recharge_amount",
            "topup_amount",
            "charge_amount",
            "recharge",
            "充值额度",
            "充值金额",
            "充值",
            "充值额",
        ],
    );
    let pay_amount_raw = external_image_structured_extract_record_text(
        record,
        &[
            "pay_amount_raw",
            "pay_amount",
            "payment_amount",
            "amount",
            "paid_amount",
            "payment",
            "支付金额",
            "支付额",
            "付款金额",
            "实付金额",
        ],
    );
    let payment_method_label = external_image_structured_extract_record_text(
        record,
        &[
            "payment_method_label",
            "payment_method",
            "pay_method",
            "method",
            "pay_type",
            "支付方式",
            "支付渠道",
        ],
    );
    let order_no = external_image_structured_extract_record_text(
        record,
        &[
            "order_no",
            "order_id",
            "order_number",
            "trade_no",
            "transaction_id",
            "订单号",
            "订单编号",
            "订单",
            "交易号",
            "流水号",
        ],
    );
    let status_label = external_image_structured_extract_record_text(
        record,
        &[
            "status_label",
            "status",
            "payment_status",
            "状态",
            "订单状态",
        ],
    );
    let created_at = external_image_structured_extract_record_text(
        record,
        &[
            "created_at",
            "create_time",
            "created_time",
            "pay_time",
            "创建时间",
            "支付时间",
            "订单时间",
            "时间",
        ],
    );
    let recharge_amount = external_image_structured_extract_record_number_any(
        record,
        &[
            "recharge_amount",
            "topup_amount",
            "charge_amount",
            "recharge",
        ],
    )
    .or_else(|| {
        recharge_amount_raw
            .as_deref()
            .and_then(external_image_structured_extract_amount_number)
    });
    let pay_amount = external_image_structured_extract_record_number_any(
        record,
        &[
            "pay_amount",
            "payment_amount",
            "amount",
            "paid_amount",
            "payment",
        ],
    )
    .or_else(|| {
        pay_amount_raw
            .as_deref()
            .and_then(external_image_structured_extract_amount_number)
    });
    json!({
        "row_index": row_index,
        "recharge_amount": recharge_amount,
        "recharge_amount_raw": recharge_amount_raw,
        "pay_amount": pay_amount,
        "pay_amount_raw": pay_amount_raw,
        "payment_method": payment_method_label
            .as_deref()
            .and_then(external_image_structured_extract_payment_method),
        "payment_method_label": payment_method_label,
        "order_no": order_no,
        "status": status_label
            .as_deref()
            .and_then(external_image_structured_extract_status),
        "status_label": status_label,
        "created_at": created_at,
        "raw": record,
    })
}

pub(crate) fn normalize_external_image_structured_extract_payload(
    extraction_id: &str,
    raw_payload: &Value,
    schema: &Value,
    attachments: Value,
    failure_reason: Option<String>,
) -> Value {
    let raw_records = external_image_structured_extract_raw_records(raw_payload);
    let records = raw_records
        .iter()
        .map(external_image_structured_extract_normalize_record)
        .collect::<Vec<_>>();
    let record_count = records.len();
    let mut needs_review = raw_payload
        .get("needs_review")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if record_count == 0 {
        needs_review = true;
    }
    let status = if failure_reason.is_none() && record_count > 0 {
        "answered"
    } else {
        "needs_review"
    };
    json!({
        "type": "v3_order_screenshot_extract",
        "status": status,
        "extraction_id": extraction_id,
        "schema": schema,
        "records": records,
        "record_count": record_count,
        "visible_row_count": external_image_structured_extract_visible_row_count(raw_payload)
            .map(|value| json!(value))
            .unwrap_or(Value::Null),
        "needs_review": needs_review,
        "confidence": raw_payload.get("confidence").cloned().unwrap_or(Value::Null),
        "notes": raw_payload.get("notes").cloned().unwrap_or(Value::Null),
        "failure_reason": failure_reason.map(Value::String).unwrap_or(Value::Null),
        "attachments": attachments,
        "source": "external_image_structured_extract",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_text_preserves_alias_order_trim_and_number_strings() {
        let record = json!({
            "missing_alias": null,
            "充值": "  $100  ",
            "amount": 700
        });

        assert_eq!(
            external_image_structured_extract_record_text(&record, &["missing", "充值", "amount"]),
            Some("$100".to_string())
        );
        assert_eq!(
            external_image_structured_extract_record_text(&record, &["amount"]),
            Some("700".to_string())
        );
    }

    #[test]
    fn record_number_any_parses_numbers_and_amount_strings() {
        let record = json!({
            "pay_amount": "$700",
            "recharge_amount": 100.0
        });

        assert_eq!(
            external_image_structured_extract_record_number_any(
                &record,
                &["missing", "pay_amount"]
            ),
            Some(700.0)
        );
        assert_eq!(
            external_image_structured_extract_record_number_any(&record, &["recharge_amount"]),
            Some(100.0)
        );
    }

    #[test]
    fn amount_number_keeps_existing_currency_cleanup_semantics() {
        assert_eq!(
            external_image_structured_extract_amount_number("￥1,234.50"),
            Some(1234.50)
        );
        assert_eq!(
            external_image_structured_extract_amount_number("no amount"),
            None
        );
    }

    #[test]
    fn status_and_payment_method_normalize_known_labels() {
        assert_eq!(
            external_image_structured_extract_status("处理中"),
            Some("processing")
        );
        assert_eq!(
            external_image_structured_extract_status("FAILED"),
            Some("failed")
        );
        assert_eq!(
            external_image_structured_extract_payment_method("支付宝"),
            Some("alipay")
        );
        assert_eq!(
            external_image_structured_extract_payment_method("WeChat Pay"),
            Some("wechat_pay")
        );
    }

    #[test]
    fn raw_records_preserve_existing_array_alias_and_object_fallbacks() {
        assert_eq!(
            external_image_structured_extract_raw_records(&json!({
                "orders": [{ "order_no": "A1" }]
            })),
            vec![json!({ "order_no": "A1" })]
        );
        assert_eq!(
            external_image_structured_extract_raw_records(&json!([{ "order_no": "A2" }])),
            vec![json!({ "order_no": "A2" })]
        );
        assert_eq!(
            external_image_structured_extract_raw_records(&json!({ "order_no": "A3" })),
            vec![json!({ "order_no": "A3" })]
        );
    }

    #[test]
    fn visible_row_count_preserves_aliases_and_string_parse() {
        assert_eq!(
            external_image_structured_extract_visible_row_count(&json!({ "row_count": "3" })),
            Some(3)
        );
        assert_eq!(
            external_image_structured_extract_visible_row_count(
                &json!({ "detected_row_count": 2 })
            ),
            Some(2)
        );
        assert_eq!(
            external_image_structured_extract_visible_row_count(&json!({ "row_count": "many" })),
            None
        );
    }

    #[test]
    fn payload_retry_preserves_missing_rows_and_required_field_checks() {
        assert!(external_image_structured_extract_payload_needs_retry(
            &json!({
                "visible_row_count": 2,
                "records": [{ "order_no": "A1", "status": "成功", "pay_amount": "$700" }]
            })
        ));
        assert!(external_image_structured_extract_payload_needs_retry(
            &json!({
                "records": [{ "order_no": "A1", "status": "成功" }]
            })
        ));
        assert!(!external_image_structured_extract_payload_needs_retry(
            &json!({
                "records": [{ "order_no": "A1", "status": "成功", "pay_amount": "$700" }]
            })
        ));
    }

    #[test]
    fn payload_quality_score_preserves_field_weighting() {
        assert_eq!(
            external_image_structured_extract_payload_quality_score(&json!({
                "records": [{
                    "order_no": "A1",
                    "status": "成功",
                    "pay_amount": "$700",
                    "recharge_amount": "$100",
                    "payment_method": "支付宝",
                    "created_at": "2026/5/14 11:48:54"
                }]
            })),
            22
        );
    }

    #[test]
    fn normalize_record_preserves_common_order_aliases() {
        let record = json!({
            "序号": "1",
            "充值": "$100",
            "amount": "$700",
            "method": "支付宝",
            "trade_no": "A1778730534",
            "订单状态": "成功",
            "订单时间": "2026/5/14 11:48:54"
        });

        let normalized = external_image_structured_extract_normalize_record(&record);

        assert_eq!(normalized["row_index"], json!(1));
        assert_eq!(normalized["recharge_amount"], json!(100.0));
        assert_eq!(normalized["recharge_amount_raw"], json!("$100"));
        assert_eq!(normalized["pay_amount"], json!(700.0));
        assert_eq!(normalized["pay_amount_raw"], json!("$700"));
        assert_eq!(normalized["payment_method"], json!("alipay"));
        assert_eq!(normalized["payment_method_label"], json!("支付宝"));
        assert_eq!(normalized["order_no"], json!("A1778730534"));
        assert_eq!(normalized["status"], json!("success"));
        assert_eq!(normalized["status_label"], json!("成功"));
        assert_eq!(normalized["created_at"], json!("2026/5/14 11:48:54"));
        assert_eq!(normalized["raw"], record);
    }

    #[test]
    fn normalize_payload_preserves_answered_status_and_metadata() {
        let payload = normalize_external_image_structured_extract_payload(
            "img-extract-test",
            &json!({
                "visible_row_count": "1",
                "records": [{
                    "序号": "1",
                    "充值": "$100",
                    "amount": "$700",
                    "method": "支付宝",
                    "trade_no": "A1778730534",
                    "订单状态": "成功",
                    "订单时间": "2026/5/14 11:48:54"
                }],
                "confidence": 0.91,
                "needs_review": false,
                "notes": "ok"
            }),
            &json!({ "kind": "order" }),
            json!([{ "url": "https://example.test/a.png" }]),
            None,
        );

        assert_eq!(payload["type"], json!("v3_order_screenshot_extract"));
        assert_eq!(payload["status"], json!("answered"));
        assert_eq!(payload["extraction_id"], json!("img-extract-test"));
        assert_eq!(payload["schema"], json!({ "kind": "order" }));
        assert_eq!(payload["record_count"], json!(1));
        assert_eq!(payload["visible_row_count"], json!(1));
        assert_eq!(payload["needs_review"], json!(false));
        assert_eq!(payload["confidence"], json!(0.91));
        assert_eq!(payload["notes"], json!("ok"));
        assert_eq!(payload["failure_reason"], Value::Null);
        assert_eq!(
            payload["attachments"][0]["url"],
            json!("https://example.test/a.png")
        );
        assert_eq!(payload["records"][0]["order_no"], json!("A1778730534"));
    }

    #[test]
    fn normalize_payload_marks_empty_or_failed_payload_for_review() {
        let empty_payload = normalize_external_image_structured_extract_payload(
            "img-empty",
            &json!({ "records": [], "needs_review": false }),
            &json!({}),
            json!([]),
            None,
        );
        assert_eq!(empty_payload["status"], json!("needs_review"));
        assert_eq!(empty_payload["record_count"], json!(0));
        assert_eq!(empty_payload["needs_review"], json!(true));

        let failed_payload = normalize_external_image_structured_extract_payload(
            "img-failed",
            &json!({ "records": [{ "order_no": "A1", "status": "成功", "pay_amount": "$700" }] }),
            &json!({}),
            json!([]),
            Some("runtime_failed".to_string()),
        );
        assert_eq!(failed_payload["status"], json!("needs_review"));
        assert_eq!(failed_payload["record_count"], json!(1));
        assert_eq!(failed_payload["failure_reason"], json!("runtime_failed"));
    }
}
