use std::collections::BTreeSet;

use crate::semantic_understanding::{SemanticStatus, MAX_EXAMPLES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticRole {
    Identifier,
    Name,
    Date,
    Amount,
    Quantity,
    Category,
    Status,
    Location,
    Text,
    Unknown,
}

impl SemanticRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::Name => "name",
            Self::Date => "date",
            Self::Amount => "amount",
            Self::Quantity => "quantity",
            Self::Category => "category",
            Self::Status => "status",
            Self::Location => "location",
            Self::Text => "text",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LabelCandidate {
    pub label: String,
    pub description: Option<String>,
    pub status: String,
    pub confidence: f64,
}

impl LabelCandidate {
    pub fn confirmed(label: &str, description: &str) -> Self {
        Self {
            label: label.to_string(),
            description: Some(description.to_string()),
            status: "confirmed".to_string(),
            confidence: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SemanticLabelInput {
    pub raw_field_key: String,
    pub confirmed_dictionary: Option<LabelCandidate>,
    pub source_comment: Option<String>,
    pub reviewed_template: Option<String>,
    pub model_suggestion: Option<LabelCandidate>,
    pub observed_values: Vec<String>,
    pub declared_value_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticLabelResolution {
    pub display_name: String,
    pub technical_name: String,
    pub description: Option<String>,
    pub semantic_role: SemanticRole,
    pub value_type: String,
    pub status: SemanticStatus,
    pub label_source: String,
    pub confidence: f64,
    pub examples: Vec<String>,
}

pub fn resolve_semantic_label(input: &SemanticLabelInput) -> SemanticLabelResolution {
    let raw_field_key = input.raw_field_key.trim();
    let (display_name, description, label_source, status, confidence) =
        if let Some(candidate) = input.confirmed_dictionary.as_ref().filter(|candidate| {
            candidate.status == "confirmed" && !candidate.label.trim().is_empty()
        }) {
            (
                candidate.label.trim().to_string(),
                candidate.description.clone(),
                "confirmed_dictionary".to_string(),
                SemanticStatus::Confirmed,
                candidate.confidence.clamp(0.0, 1.0),
            )
        } else if let Some(comment) = non_empty(input.source_comment.as_deref()) {
            (
                comment.to_string(),
                None,
                "source_comment".to_string(),
                SemanticStatus::Confirmed,
                1.0,
            )
        } else if let Some(label) = non_empty(input.reviewed_template.as_deref()) {
            (
                label.to_string(),
                None,
                "reviewed_template".to_string(),
                SemanticStatus::Confirmed,
                1.0,
            )
        } else if let Some(candidate) = input.model_suggestion.as_ref().filter(|candidate| {
            candidate.status == "suggested" && !candidate.label.trim().is_empty()
        }) {
            (
                candidate.label.trim().to_string(),
                candidate.description.clone(),
                "model_suggestion".to_string(),
                SemanticStatus::Inferred,
                candidate.confidence.clamp(0.0, 0.89),
            )
        } else {
            let split = deterministic_field_label(raw_field_key);
            let improved = !split.is_empty() && !split.eq_ignore_ascii_case(raw_field_key);
            (
                if split.is_empty() {
                    raw_field_key.to_string()
                } else {
                    split
                },
                None,
                if improved {
                    "deterministic_split".to_string()
                } else {
                    "unresolved_raw_key".to_string()
                },
                SemanticStatus::Unresolved,
                if improved { 0.25 } else { 0.0 },
            )
        };

    SemanticLabelResolution {
        display_name,
        technical_name: raw_field_key.to_string(),
        description,
        semantic_role: infer_semantic_role(raw_field_key),
        value_type: input
            .declared_value_type
            .as_deref()
            .and_then(|value| non_empty(Some(value)))
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| infer_value_type(&input.observed_values)),
        status,
        label_source,
        confidence,
        examples: safe_public_examples(&input.observed_values),
    }
}

pub fn infer_semantic_role(raw_field_key: &str) -> SemanticRole {
    let tokens = field_tokens(raw_field_key);
    let has = |needles: &[&str]| tokens.iter().any(|token| needles.contains(&token.as_str()));
    if has(&["id", "identifier", "code", "number", "no", "编号", "编码"]) {
        SemanticRole::Identifier
    } else if has(&["name", "title", "label", "名称", "姓名", "标题"]) {
        SemanticRole::Name
    } else if has(&["date", "time", "timestamp", "year", "month", "日期", "时间"]) {
        SemanticRole::Date
    } else if has(&[
        "amount",
        "price",
        "revenue",
        "sales",
        "rent",
        "cost",
        "fee",
        "金额",
        "租金",
        "销售额",
    ]) {
        SemanticRole::Amount
    } else if has(&["quantity", "qty", "count", "volume", "数量", "件数"]) {
        SemanticRole::Quantity
    } else if has(&["category", "type", "kind", "class", "分类", "类型"]) {
        SemanticRole::Category
    } else if has(&["status", "state", "phase", "状态", "阶段"]) {
        SemanticRole::Status
    } else if has(&[
        "location", "address", "city", "province", "country", "region", "位置", "地址", "城市",
        "区域",
    ]) {
        SemanticRole::Location
    } else if has(&[
        "text",
        "content",
        "description",
        "note",
        "remark",
        "summary",
        "正文",
        "描述",
        "备注",
        "摘要",
    ]) {
        SemanticRole::Text
    } else {
        SemanticRole::Unknown
    }
}

pub fn safe_public_examples(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty() && value.chars().count() <= 80)
        .filter(|value| !looks_sensitive(value))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_EXAMPLES)
        .collect()
}

fn deterministic_field_label(raw: &str) -> String {
    let mut output = String::new();
    let mut previous_lower_or_digit = false;
    for character in raw.trim().chars() {
        if matches!(character, '_' | '-' | '.' | '/' | '\\') {
            if !output.ends_with(' ') && !output.is_empty() {
                output.push(' ');
            }
            previous_lower_or_digit = false;
        } else {
            if character.is_ascii_uppercase() && previous_lower_or_digit && !output.ends_with(' ') {
                output.push(' ');
            }
            output.extend(character.to_lowercase());
            previous_lower_or_digit = character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn field_tokens(raw: &str) -> Vec<String> {
    let normalized = deterministic_field_label(raw);
    let mut tokens = normalized
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !normalized.is_empty() {
        tokens.push(normalized);
    }
    tokens
}

fn infer_value_type(values: &[String]) -> String {
    let safe = safe_public_examples(values);
    if safe.is_empty() {
        return "unknown".to_string();
    }
    if safe.iter().all(|value| value.parse::<f64>().is_ok()) {
        "number".to_string()
    } else if safe.iter().all(|value| {
        let lower = value.to_ascii_lowercase();
        matches!(lower.as_str(), "true" | "false" | "yes" | "no")
    }) {
        "boolean".to_string()
    } else if safe.iter().all(|value| looks_like_date(value)) {
        "date".to_string()
    } else {
        "text".to_string()
    }
}

fn looks_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let compact_digits = value
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let credential_marker = [
        "password=",
        "passwd=",
        "token=",
        "secret=",
        "api_key=",
        "apikey=",
        "bearer ",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let database_url = [
        "postgres://",
        "postgresql://",
        "mysql://",
        "oracle://",
        "jdbc:",
        "redis://",
    ]
    .iter()
    .any(|prefix| lower.contains(prefix));
    let email = !value.contains(char::is_whitespace)
        && value.contains('@')
        && value.rsplit_once('.').is_some();
    let phone = compact_digits.len() == 11 && compact_digits.starts_with('1');
    let identity = compact_digits.len() == 15
        || compact_digits.len() == 18
        || (value.chars().count() == 18
            && compact_digits.len() == 17
            && (value.ends_with('X') || value.ends_with('x')));
    let windows_path = value.len() > 3
        && value.as_bytes().get(1) == Some(&b':')
        && matches!(value.as_bytes().get(2), Some(b'\\') | Some(b'/'));
    let unix_path = ["/home/", "/users/", "/etc/", "/var/", "/root/"]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
    let token = lower.starts_with("sk-")
        || lower.starts_with("eyj")
        || (value.len() >= 32
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            }));
    credential_marker
        || database_url
        || email
        || phone
        || identity
        || windows_path
        || unix_path
        || token
}

fn looks_like_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 8
        && bytes
            .iter()
            .filter(|byte| matches!(byte, b'-' | b'/'))
            .count()
            >= 2
        && bytes.iter().all(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'-' | b'/' | b' ' | b':' | b'T' | b'Z')
        })
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_understanding::SemanticStatus;

    fn input() -> SemanticLabelInput {
        SemanticLabelInput {
            raw_field_key: "rent_amount".to_string(),
            confirmed_dictionary: None,
            source_comment: None,
            reviewed_template: None,
            model_suggestion: None,
            observed_values: vec!["1200.50".to_string()],
            declared_value_type: None,
        }
    }

    #[test]
    fn label_precedence_is_dictionary_comment_template_split_then_raw() {
        let mut value = input();
        value.confirmed_dictionary = Some(LabelCandidate::confirmed("合同租金", "dictionary"));
        value.source_comment = Some("租金金额".to_string());
        value.reviewed_template = Some("应付金额".to_string());
        let resolution = resolve_semantic_label(&value);
        assert_eq!(resolution.display_name, "合同租金");
        assert_eq!(resolution.label_source, "confirmed_dictionary");
        assert_eq!(resolution.status, SemanticStatus::Confirmed);

        value.confirmed_dictionary = None;
        assert_eq!(resolve_semantic_label(&value).display_name, "租金金额");
        value.source_comment = None;
        assert_eq!(resolve_semantic_label(&value).display_name, "应付金额");
        value.reviewed_template = None;
        assert_eq!(resolve_semantic_label(&value).display_name, "rent amount");
    }

    #[test]
    fn semantic_roles_cover_supported_business_field_classes() {
        for (key, expected) in [
            ("contract_id", SemanticRole::Identifier),
            ("store_name", SemanticRole::Name),
            ("signed_date", SemanticRole::Date),
            ("rent_amount", SemanticRole::Amount),
            ("item_quantity", SemanticRole::Quantity),
            ("product_category", SemanticRole::Category),
            ("approval_status", SemanticRole::Status),
            ("store_address", SemanticRole::Location),
            ("contract_note", SemanticRole::Text),
            ("x9z", SemanticRole::Unknown),
        ] {
            assert_eq!(infer_semantic_role(key), expected, "key={key}");
        }
    }

    #[test]
    fn suggested_model_label_is_never_projected_as_confirmed() {
        let mut value = input();
        value.model_suggestion = Some(LabelCandidate {
            label: "模型猜测租金".to_string(),
            description: None,
            status: "suggested".to_string(),
            confidence: 0.91,
        });
        let resolution = resolve_semantic_label(&value);

        assert_eq!(resolution.display_name, "模型猜测租金");
        assert_eq!(resolution.status, SemanticStatus::Inferred);
        assert_eq!(resolution.label_source, "model_suggestion");
    }

    #[test]
    fn public_examples_remove_credentials_identity_contacts_urls_and_paths() {
        let values = vec![
            "普通门店".to_string(),
            "postgres://user:pass@db/tenant".to_string(),
            "person@example.com".to_string(),
            "13800138000".to_string(),
            "11010519491231002X".to_string(),
            "C:\\secrets\\config.json".to_string(),
            "sk-abcdefghijklmnopqrstuvwxyz123456".to_string(),
            "普通门店".to_string(),
        ];

        assert_eq!(safe_public_examples(&values), vec!["普通门店"]);
    }

    #[test]
    fn random_code_is_not_translated_into_a_business_concept() {
        let mut value = input();
        value.raw_field_key = "BJBDS".to_string();
        value.observed_values = vec!["TJ1b".to_string()];
        let resolution = resolve_semantic_label(&value);

        assert_eq!(resolution.technical_name, "BJBDS");
        assert_eq!(resolution.status, SemanticStatus::Unresolved);
        assert_eq!(resolution.semantic_role, SemanticRole::Unknown);
    }
}
