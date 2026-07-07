use crate::{
    assistant_run_codex_forward_prompt_support::assistant_run_prompt_without_codex_forward_prefix,
    external_channel_message_requests_data_ingestion_analysis,
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
};

pub(crate) fn assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
    prompt: &str,
) -> bool {
    let forwarded_prompt = assistant_run_prompt_without_codex_forward_prefix(prompt);
    let compact = forwarded_prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return false;
    }
    let lower = compact.to_ascii_lowercase();
    let has_data_ingestion_intent =
        external_channel_message_requests_data_ingestion_analysis(forwarded_prompt)
            || prompt_contains_any(
                &compact,
                &[
                    "数据库接入",
                    "数据库对接",
                    "数据库API",
                    "数据库接口",
                    "业务库接入",
                    "业务库对接",
                    "建库",
                    "建数据表",
                    "数据表设计",
                    "表结构设计",
                    "字段清洗",
                    "字段规范",
                    "字段口径",
                    "数据同步",
                    "同步入库",
                    "数据入库",
                    "接口接入",
                    "接口对接",
                    "API接入",
                    "API对接",
                    "第三方系统对接",
                    "业务系统对接",
                    "OA对接",
                    "ERP对接",
                    "CRM对接",
                    "MCP对接",
                    "连接器",
                    "connector",
                ],
            )
            || ascii_prompt_contains_any(
                &lower,
                &[
                    "databaseintegration",
                    "databaseapi",
                    "apiintegration",
                    "apiconnector",
                    "connector",
                    "webhook",
                    "etl",
                    "schema",
                ],
            );
    if !has_data_ingestion_intent {
        return false;
    }
    let unsafe_product_change = prompt_contains_any(
        &compact,
        &[
            "修改DataMax",
            "改DataMax",
            "修改V3",
            "改V3",
            "主站接口",
            "公开接口",
            "公开API",
            "鉴权",
            "认证",
            "登录",
            "部署",
            "重启",
            "发版",
            "提交代码",
            "数据库迁移",
            "生产表写入",
            "生产库写入",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "datamaxapi",
            "v3api",
            "publicapi",
            "auth",
            "login",
            "deploy",
            "restart",
            "migration",
            "productionwrite",
        ],
    );
    !unsafe_product_change
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_safe_database_integration_requests_after_cc_prefix() {
        assert!(
            assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "cc 帮我做数据库 API 对接，先分析字段映射和 staging plan。"
            )
        );
        assert!(
            assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "cc 数据库接口怎么接入，先做表结构设计。"
            )
        );
        assert!(
            assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "cc build an api connector and staging plan for database integration"
            )
        );
    }

    #[test]
    fn detects_ingestion_without_cc_prefix_for_internal_callers() {
        assert!(
            assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "请设计数据源字段映射和数据同步方案。"
            )
        );
    }

    #[test]
    fn rejects_product_or_public_api_changes() {
        assert!(
            !assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "cc 修改 DataMax 公开 API 请求字段，顺便做数据库 API 对接。"
            )
        );
        assert!(
            !assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "cc 修改 V3 主站接口并部署。"
            )
        );
        assert!(
            !assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "cc update auth and database integration"
            )
        );
    }

    #[test]
    fn rejects_plain_analysis_and_empty_prompts() {
        assert!(
            !assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(
                "cc 做一下新百经营分析，给出管理层建议。"
            )
        );
        assert!(!assistant_run_prompt_requests_data_ingestion_or_integration_sidecar("cc"));
        assert!(!assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(""));
    }
}
