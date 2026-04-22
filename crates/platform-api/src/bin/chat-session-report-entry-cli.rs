use contracts::{ChatSessionReportEntryActionView, UpdateChatSessionReportEntryRequest};
use domain_model::ChatSessionId;
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} <session_id> <request_confirmation|stay_material_service|enter_report_service> [--title <title>] [--objective <objective>] [--pretty]"
    )
}

fn take_option_value(args: &mut Vec<String>, flag: &str) -> anyhow::Result<Option<String>> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };
    if index + 1 >= args.len() {
        anyhow::bail!("{flag} requires a value");
    }

    let value = args.remove(index + 1);
    args.remove(index);
    Ok(Some(value))
}

fn parse_action(raw: &str) -> anyhow::Result<ChatSessionReportEntryActionView> {
    match raw {
        "request_confirmation" => Ok(ChatSessionReportEntryActionView::RequestConfirmation),
        "stay_material_service" => Ok(ChatSessionReportEntryActionView::StayMaterialService),
        "enter_report_service" => Ok(ChatSessionReportEntryActionView::EnterReportService),
        _ => anyhow::bail!("unsupported report entry action: {raw}"),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("chat_session_report_entry_cli")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "chat-session-report-entry-cli".to_string());
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };
    let title = take_option_value(&mut args, "--title")?;
    let objective = take_option_value(&mut args, "--objective")?;

    let (session_id_raw, action_raw) = match args.as_slice() {
        [session_id, action] => (session_id.clone(), action.clone()),
        _ => anyhow::bail!(usage(&program)),
    };

    let session_id = ChatSessionId(Uuid::from_str(&session_id_raw)?);
    let action = parse_action(&action_raw)?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = storage::PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    let response = platform_api::apply_chat_session_report_entry_update(
        storage,
        tenant.id,
        session_id,
        UpdateChatSessionReportEntryRequest {
            action,
            title,
            objective,
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    if pretty {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}
