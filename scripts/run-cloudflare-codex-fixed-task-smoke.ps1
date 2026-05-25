param(
    [string[]] $Case = @("all"),
    [switch] $Local,
    [switch] $PlanOnly,
    [string] $BaseUrl = "",
    [string] $ReportDir = "",
    [string] $CargoBin = "cargo",
    [switch] $AllowServerMutation,
    [switch] $Json
)

$ErrorActionPreference = "Stop"

$AllCases = @(
    "static-page-no-confirm",
    "data-ingestion-analysis",
    "static_page_plan_only",
    "static_page_new_artifact",
    "static_page_overwrite_rejected",
    "answer_quality_autofix",
    "human_exception",
    "runtime_summary"
)

$CaseAliases = @{
    "static_page_no_confirm" = "static-page-no-confirm"
    "static-page-image2-data-publish" = "static-page-no-confirm"
    "static_page_image2_data_publish" = "static-page-no-confirm"
    "data_ingestion_analysis" = "data-ingestion-analysis"
    "data-ingestion" = "data-ingestion-analysis"
}

function Resolve-SmokeCases {
    param([string[]] $Requested)
    if ($Requested.Count -eq 0 -or $Requested -contains "all") {
        return $AllCases
    }
    $resolved = New-Object System.Collections.Generic.List[string]
    foreach ($item in $Requested) {
        $caseId = if ($CaseAliases.ContainsKey($item)) { $CaseAliases[$item] } else { $item }
        if ($AllCases -notcontains $caseId) {
            throw "Unknown fixed-task smoke case '$item'. Known cases: $($AllCases -join ', ')"
        }
        if (-not $resolved.Contains($caseId)) {
            $resolved.Add($caseId) | Out-Null
        }
    }
    return $resolved.ToArray()
}

function New-SmokeResult {
    param(
        [string] $CaseId,
        [string] $Status,
        [string] $Message,
        [object] $Details = $null
    )
    [ordered]@{
        case = $CaseId
        status = $Status
        message = $Message
        details = $Details
    }
}

function Invoke-CheckedCommand {
    param(
        [string] $FilePath,
        [string[]] $Arguments
    )
    $output = & $FilePath @Arguments 2>&1
    $exitCode = $LASTEXITCODE
    [ordered]@{
        exit_code = $exitCode
        output = ($output -join "`n")
    }
}

function New-CommandDetail {
    param(
        [string] $Command,
        [object] $Result
    )
    $excerpt = ""
    if ($Result.exit_code -ne 0 -and -not [string]::IsNullOrWhiteSpace($Result.output)) {
        $excerpt = $Result.output.Substring(0, [Math]::Min(4000, $Result.output.Length))
    }
    [ordered]@{
        command = $Command
        exit_code = $Result.exit_code
        failure_excerpt = $excerpt
    }
}

function Invoke-LocalCaseSmoke {
    param(
        [string] $CaseId,
        [string] $CargoBin
    )

    $commands = New-Object System.Collections.Generic.List[object]
    switch ($CaseId) {
        "static-page-no-confirm" {
            $commands.Add([string[]]@("test", "-p", "platform-api", "external_channel_static_page", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "platform-api", "codex_host_fixed_task", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "codex-host-agent", "static_page", "--lib")) | Out-Null
        }
        "answer_quality_autofix" {
            $commands.Add([string[]]@("test", "-p", "platform-api", "answer_quality_autofix", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "codex-host-agent", "answer_quality", "--lib")) | Out-Null
        }
        "data-ingestion-analysis" {
            $commands.Add([string[]]@("test", "-p", "contracts", "data_ingestion", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "codex-host-agent", "data_ingestion", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "platform-api", "data_ingestion", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "platform-api", "codex_host_fixed_task", "--lib")) | Out-Null
        }
        default {
            $commands.Add([string[]]@("test", "-p", "platform-api", "codex_host_fixed_task", "--lib")) | Out-Null
        }
    }

    $details = New-Object System.Collections.Generic.List[object]
    $failed = $false
    foreach ($args in $commands.ToArray()) {
        $result = Invoke-CheckedCommand -FilePath $CargoBin -Arguments $args
        $commandText = "$CargoBin $($args -join ' ')"
        $details.Add((New-CommandDetail -Command $commandText -Result $result)) | Out-Null
        if ($result.exit_code -ne 0) {
            $failed = $true
        }
    }

    [ordered]@{
        status = if ($failed) { "failed" } else { "passed" }
        details = [ordered]@{
            plan_only = $true
            no_server_writes = $true
            commands = $details.ToArray()
        }
    }
}

function Write-SmokeReports {
    param(
        [object[]] $Results,
        [string] $Mode,
        [string] $OutputDir
    )
    if ([string]::IsNullOrWhiteSpace($OutputDir)) {
        $OutputDir = Join-Path (Get-Location) "target\cloudflare-codex-fixed-task-smoke"
    }
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
    $stamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $jsonPath = Join-Path $OutputDir "cloudflare-codex-fixed-task-smoke-$stamp.json"
    $mdPath = Join-Path $OutputDir "cloudflare-codex-fixed-task-smoke-$stamp.md"
    $passed = @($Results | Where-Object { $_.status -eq "passed" }).Count
    $failed = @($Results | Where-Object { $_.status -eq "failed" }).Count
    $skipped = @($Results | Where-Object { $_.status -eq "skipped" }).Count
    $payload = [ordered]@{
        generated_at = (Get-Date).ToUniversalTime().ToString("o")
        mode = $Mode
        passed = $passed
        failed = $failed
        skipped = $skipped
        results = $Results
    }
    $payload | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 -Path $jsonPath

    $lines = @()
    $lines += "# Cloudflare Codex Fixed Task Smoke"
    $lines += ""
    $lines += "- Mode: $Mode"
    $lines += "- Passed: $passed"
    $lines += "- Failed: $failed"
    $lines += "- Skipped: $skipped"
    $lines += ""
    foreach ($result in $Results) {
        $lines += "## $($result.case)"
        $lines += ""
        $lines += "- Status: $($result.status)"
        $lines += "- Message: $($result.message)"
        $lines += ""
    }
    $lines | Set-Content -Encoding UTF8 -Path $mdPath
    [ordered]@{
        json = $jsonPath
        markdown = $mdPath
        failed = $failed
    }
}

$selectedCases = @(Resolve-SmokeCases -Requested $Case)
$mode = if ($Local -or [string]::IsNullOrWhiteSpace($BaseUrl)) { "local_plan_only" } else { "server" }
$results = New-Object System.Collections.Generic.List[object]

if ($mode -eq "local_plan_only") {
    foreach ($caseId in $selectedCases) {
        $caseSmoke = Invoke-LocalCaseSmoke -CaseId $caseId -CargoBin $CargoBin
        $message = switch ($caseId) {
            "static-page-no-confirm" { "Image2-first static-page path exposes a no-confirm pending card, queues Codex Host only after preview-ready evidence, validates fixed output, and returns a final artifact-link reply." }
            "data-ingestion-analysis" { "Data-ingestion analysis path detects customer source-analysis requests, packages only V3-selected scope, validates read-only/staging outputs, and routes unsafe credential/schema/API/write requests to human review." }
            "static_page_plan_only" { "Fixed static-page queued audit package is safe and bounded." }
            "static_page_new_artifact" { "New generated-artifact output validation is accepted." }
            "static_page_overwrite_rejected" { "Non generated-artifact URL is rejected for human review." }
            "answer_quality_autofix" { "Low-risk allowlisted answer-quality patch output is accepted." }
            "human_exception" { "needs_human output is redacted and marked for exception notification." }
            "runtime_summary" { "Runtime diagnostics summarize fixed-task status without raw prompt data." }
            default { "Fixed-task smoke case ran." }
        }
        $results.Add((New-SmokeResult -CaseId $caseId -Status $caseSmoke.status -Message $message -Details $caseSmoke.details))
    }
} else {
    $base = $BaseUrl.TrimEnd("/")
    $healthUrl = "$base/healthz"
    try {
        $health = Invoke-WebRequest -Method Get -Uri $healthUrl -TimeoutSec 20 -UseBasicParsing
        $contentExcerpt = if ($null -eq $health.Content) { "" } else { $health.Content.ToString().Substring(0, [Math]::Min(160, $health.Content.ToString().Length)) }
        $results.Add((New-SmokeResult -CaseId "server_health" -Status "passed" -Message "Server health endpoint responded." -Details @{ url = $healthUrl; status_code = [int]$health.StatusCode; content_excerpt = $contentExcerpt }))
    } catch {
        $results.Add((New-SmokeResult -CaseId "server_health" -Status "failed" -Message "Server health endpoint failed: $($_.Exception.Message)" -Details @{ url = $healthUrl }))
    }

    if ($PlanOnly -or -not $AllowServerMutation) {
        foreach ($caseId in $selectedCases) {
            $guardDetails = @{
                base_url = $BaseUrl
                plan_only = [bool]$PlanOnly
                customer_confirmation_required = $false
            }
            if ($caseId -eq "static-page-no-confirm") {
                $guardDetails.expected_final_url_prefix = "$base/generated-artifacts/"
            } elseif ($caseId -eq "data-ingestion-analysis") {
                $guardDetails.expected_output_statuses = @("analysis_ready", "staging_spec_ready", "needs_human", "failed")
                $guardDetails.production_writes_allowed = $false
            }
            $results.Add((New-SmokeResult -CaseId $caseId -Status "skipped" -Message "Server mutation smoke is guarded. Re-run on an approved host with -AllowServerMutation after deployment review." -Details $guardDetails))
        }
    } else {
        foreach ($caseId in $selectedCases) {
            $results.Add((New-SmokeResult -CaseId $caseId -Status "failed" -Message "Server mutation execution is not wired to a public smoke endpoint; run this script on the approved V3 host with -Local -PlanOnly or add a private server case config before enabling." -Details @{ base_url = $BaseUrl }))
        }
    }
}

$resultArray = $results.ToArray()
$report = Write-SmokeReports -Results $resultArray -Mode $mode -OutputDir $ReportDir
if ($Json) {
    [ordered]@{
        mode = $mode
        report = $report
        results = $resultArray
    } | ConvertTo-Json -Depth 8
} else {
    Write-Host "Cloudflare Codex fixed-task smoke complete."
    Write-Host "JSON: $($report.json)"
    Write-Host "Markdown: $($report.markdown)"
}

if ($report.failed -gt 0) {
    exit 1
}
