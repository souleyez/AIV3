param(
    [switch] $SkipDatabase,
    [switch] $SkipWeb,
    [switch] $SkipCargoUnit
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$webRoot = Join-Path $repoRoot "apps\web"

function Invoke-SmokeStep {
    param(
        [string] $Name,
        [string] $WorkingDirectory,
        [string] $Command,
        [string[]] $Arguments
    )

    Write-Host ""
    Write-Host "==> $Name"
    Push-Location -LiteralPath $WorkingDirectory
    try {
        & $Command @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$Name failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
}

$steps = @()

if (-not $SkipCargoUnit) {
    $steps += @{
        Name = "external artifact publish is low-risk"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "assistant-runtime", "codex_executor_external_artifact_publish_is_low_risk_write", "--lib")
    }
    $steps += @{
        Name = "external artifact revoke requires confirmation"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "assistant-runtime", "codex_executor_external_artifact_revoke_requires_high_risk_confirmation", "--lib")
    }
    $steps += @{
        Name = "external business action requires cross-system confirmation"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "assistant-runtime", "codex_executor_external_business_action_requires_cross_system_confirmation", "--lib")
    }
    $steps += @{
        Name = "external artifact observability summary"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "platform-api", "external_artifact_summary_prioritizes_publish_revoke_status", "--lib")
    }
}

if (-not $SkipDatabase) {
    $steps += @{
        Name = "external ACL filters same question by principal"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "platform-api", "external_channel_acl_filters_same_question_by_principal", "--lib", "--", "--nocapture")
    }
    $steps += @{
        Name = "generic external source sync records workflow"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "platform-api", "external_source_sync_endpoint_enqueues_workflow_and_records_run", "--lib", "--", "--nocapture")
    }
    $steps += @{
        Name = "Feishu callback normalizes into channel ingress"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "platform-api", "feishu_callback_endpoint_decrypts_event_body_and_uses_normalized_ingestion", "--lib", "--", "--nocapture")
    }
    $steps += @{
        Name = "WeCom callback normalizes into channel ingress"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "platform-api", "wecom_callback_endpoint_decrypts_xml_and_uses_normalized_ingestion", "--lib", "--", "--nocapture")
    }
    $steps += @{
        Name = "external channel high-risk action pauses for confirmation"
        WorkingDirectory = $repoRoot
        Command = "cargo"
        Arguments = @("test", "-p", "platform-api", "external_channel_action_message_persists_pending_action_and_confirms_it", "--lib", "--", "--nocapture")
    }
}

if (-not $SkipWeb) {
    $steps += @{
        Name = "external integrations panel contract"
        WorkingDirectory = $webRoot
        Command = "node"
        Arguments = @("--test", "app/lib/external-integrations.test.mjs")
    }
}

if ($steps.Count -eq 0) {
    throw "No smoke steps selected."
}

$startedAt = Get-Date
Write-Host "External bot and third-party smoke started at $($startedAt.ToString("s"))"
Write-Host "Repository: $repoRoot"

foreach ($step in $steps) {
    Invoke-SmokeStep `
        -Name $step.Name `
        -WorkingDirectory $step.WorkingDirectory `
        -Command $step.Command `
        -Arguments $step.Arguments
}

$finishedAt = Get-Date
$elapsed = New-TimeSpan -Start $startedAt -End $finishedAt
Write-Host ""
Write-Host "OK external bot and third-party smoke completed in $([Math]::Round($elapsed.TotalSeconds, 1))s."
