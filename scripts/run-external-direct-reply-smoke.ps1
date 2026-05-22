param(
    [string] $ReportDir = "",
    [string] $CargoBin = "cargo",
    [switch] $Json
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
if ([string]::IsNullOrWhiteSpace($ReportDir)) {
    $ReportDir = Join-Path $repoRoot "target\external-direct-reply-smoke"
}
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null

$head = git -C $repoRoot rev-parse --short HEAD
$startedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$reportBase = "external-direct-reply-smoke-$((Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))"
$reportJson = Join-Path $ReportDir "$reportBase.json"
$reportMd = Join-Path $ReportDir "$reportBase.md"

$checksToRun = @(
    @{ name = "ordinary external chat returns provider-authored answered text"; package = "platform-api"; filter = "generic_chat_page_event_returns_provider_model_text_when_configured" },
    @{ name = "ordinary external chat falls back when primary output is rejected"; package = "platform-api"; filter = "generic_chat_page_event_uses_fallback_when_primary_output_is_rejected" },
    @{ name = "ordinary external chat falls back when primary output is suppressed"; package = "platform-api"; filter = "generic_chat_page_event_uses_fallback_when_primary_output_is_suppressed" },
    @{ name = "direct reply guard rejects empty and suppressed output"; package = "platform-api"; filter = "external_channel_model_reply_rejects_empty_and_suppressed_output" },
    @{ name = "external provider input forbids orchestration acknowledgements"; package = "platform-api"; filter = "assistant_run_provider_input_enforces_external_channel_direct_reply_contract" },
    @{ name = "external temporary document scope keeps direct reply contract"; package = "platform-api"; filter = "external_channel_temporary_scope_keeps_direct_reply_contract" }
)

$results = New-Object System.Collections.Generic.List[object]
foreach ($check in $checksToRun) {
    Write-Host ""
    Write-Host "== $($check.name) =="
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $CargoBin test -p $check.package $check.filter --lib 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $result = [pscustomobject]@{
        name = $check.name
        package = $check.package
        filter = $check.filter
        status = if ($exitCode -eq 0) { "passed" } else { "failed" }
        exit_code = $exitCode
        output_excerpt = (($output | Select-Object -Last 30) -join "`n")
    }
    $results.Add($result)
    Write-Host "$($result.status): $($check.package)::$($check.filter)"
    if ($exitCode -ne 0) {
        break
    }
}

$finishedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$ready = -not @($results | Where-Object { $_.status -eq "failed" }).Count
$report = [pscustomobject]@{
    smoke = "external-direct-reply"
    ready = $ready
    repository = $repoRoot.Path
    head = ($head | Select-Object -First 1)
    started_at = $startedAt
    finished_at = $finishedAt
    contract = [pscustomobject]@{
        ordinary_external_chat = "Final user-visible ordinary chat replies must be provider-authored text with task_status=answered."
        no_orchestration_answer = "accepted, duplicate_accepted, model_unavailable, and model_output_suppressed are not valid ordinary-chat answers."
        fallback = "Timeouts, provider failures, empty output, and unsafe/internal output are retryable before final response."
    }
    checks = $results
}

$report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $reportJson -Encoding UTF8

$lines = @(
    "# External Direct Reply Smoke",
    "",
    "- Status: $(if ($ready) { 'passed' } else { 'failed' })",
    "- Repository: $($repoRoot.Path)",
    "- HEAD: $(($head | Select-Object -First 1))",
    "- Started: $startedAt",
    "- Finished: $finishedAt",
    "",
    "## Checks",
    ""
)
foreach ($check in $results) {
    $lines += "- $($check.status): ``$($check.name)``"
}
$lines | Set-Content -LiteralPath $reportMd -Encoding UTF8

if ($Json) {
    $report | ConvertTo-Json -Depth 10
} else {
    Write-Host ""
    Write-Host "External direct reply smoke report: $reportJson"
    Write-Host "External direct reply smoke summary: $reportMd"
    if (-not $ready) {
        throw "external-direct-reply smoke failed"
    }
    Write-Host "OK external-direct-reply smoke completed."
}
