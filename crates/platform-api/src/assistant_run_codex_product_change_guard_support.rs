use crate::{
    assistant_run_codex_data_ingestion_sidecar_prompt_support::assistant_run_prompt_requests_data_ingestion_or_integration_sidecar,
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
};

pub(crate) fn assistant_run_prompt_requests_v3_product_change(prompt: &str) -> bool {
    let compact = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let lower = prompt.to_ascii_lowercase();
    let has_v3_signal =
        prompt_contains_any(
            prompt,
            &[
                "V3",
                "主站",
                "原功能",
                "产品功能",
                "登录",
                "鉴权",
                "接口",
                "部署",
                "发版",
            ],
        ) || ascii_prompt_contains_any(&lower, &["v3", "api", "auth", "deploy", "login"]);
    let has_change_signal =
        prompt_contains_any(
            prompt,
            &[
                "修改", "改造", "调整", "删除", "新增", "重构", "发版", "部署", "重启",
            ],
        ) || ascii_prompt_contains_any(&lower, &["modify", "change", "patch", "deploy", "restart"]);
    if !(has_v3_signal && has_change_signal) {
        return false;
    }
    if assistant_run_prompt_requests_data_ingestion_or_integration_sidecar(prompt) {
        return false;
    }
    if assistant_run_prompt_mentions_customer_codex_artifact_surface(&compact, &lower)
        && !assistant_run_prompt_mentions_v3_product_system_surface(&compact, &lower)
    {
        return false;
    }
    true
}

fn assistant_run_prompt_mentions_customer_codex_artifact_surface(
    compact_prompt: &str,
    lower_prompt: &str,
) -> bool {
    prompt_contains_any(
        compact_prompt,
        &[
            "V3生成的静态页",
            "V3生成静态页",
            "V3生成的页面",
            "V3生成页面",
            "生成的静态页",
            "生成静态页",
            "生成的页面",
            "当前生成静态页",
            "当前静态页",
            "当前报表页面",
            "当前报表页",
            "当前看板",
            "客户产物",
            "客户制品",
            "生成产物",
            "报表页面",
            "报表页",
            "静态页",
            "generated-artifact",
            "generatedartifact",
        ],
    ) || ascii_prompt_contains_any(
        lower_prompt,
        &[
            "artifact",
            "generatedartifact",
            "generated_artifact",
            "staticpage",
            "static_page",
        ],
    )
}

fn assistant_run_prompt_mentions_v3_product_system_surface(
    compact_prompt: &str,
    lower_prompt: &str,
) -> bool {
    prompt_contains_any(
        compact_prompt,
        &[
            "源码",
            "源代码",
            "产品源码",
            "代码库",
            "仓库",
            "主站",
            "原功能",
            "产品功能",
            "功能",
            "服务",
            "登录",
            "鉴权",
            "认证",
            "权限",
            "接口",
            "公开接口",
            "公开API",
            "数据库迁移",
            "迁移",
            "表结构",
            "模型配置",
            "provider配置",
            "环境变量",
            "部署",
            "发版",
            "重启",
            "提交",
            "合并代码",
            "systemd",
            "nginx",
        ],
    ) || ascii_prompt_contains_any(
        lower_prompt,
        &[
            "source",
            "repo",
            "service",
            "api",
            "auth",
            "login",
            "migration",
            "schema",
            "provider",
            "env",
            "deploy",
            "restart",
            "commit",
            "systemd",
            "nginx",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_v3_system_changes() {
        assert!(assistant_run_prompt_requests_v3_product_change(
            "帮我修改 V3 登录功能并部署。"
        ));
        assert!(assistant_run_prompt_requests_v3_product_change(
            "cc 帮我修改 V3 主站页面样式。"
        ));
        assert!(assistant_run_prompt_requests_v3_product_change(
            "patch v3 auth and deploy"
        ));
    }

    #[test]
    fn ignores_customer_generated_artifact_changes() {
        assert!(!assistant_run_prompt_requests_v3_product_change(
            "cc 修改 V3 生成的静态页：把取高风险模块提到最前面。"
        ));
        assert!(!assistant_run_prompt_requests_v3_product_change(
            "cc 修改当前报表页面：把取高风险模块提到最前面。"
        ));
    }

    #[test]
    fn ignores_safe_data_ingestion_sidecar_requests() {
        assert!(!assistant_run_prompt_requests_v3_product_change(
            "cc 帮我做数据库 API 对接，先分析字段映射和 staging plan。"
        ));
    }

    #[test]
    fn keeps_public_api_product_change_blocked_even_with_ingestion_words() {
        assert!(assistant_run_prompt_requests_v3_product_change(
            "cc 修改 DataMax 公开 API 请求字段，顺便做数据库 API 对接。"
        ));
    }

    #[test]
    fn ignores_plain_business_requests() {
        assert!(!assistant_run_prompt_requests_v3_product_change(
            "cc 做一下新百经营分析，给出管理层建议。"
        ));
        assert!(!assistant_run_prompt_requests_v3_product_change(
            "请解释这份报表口径。"
        ));
    }
}
