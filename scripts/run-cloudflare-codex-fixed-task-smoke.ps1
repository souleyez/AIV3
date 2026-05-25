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
    "static_page_plan_only",
    "static_page_new_artifact",
    "static_page_overwrite_rejected",
    "answer_quality_autofix",
    "human_exception",
    "runtime_summary"
)

function Resolve-SmokeCases {
    param([string[]] $Requested)
    if ($Requested.Count -eq 0 -or $Requested -contains "all") {
        return $AllCases
    }
    foreach ($item in $Requested) {
        if ($AllCases -notcontains $item) {
            throw "Unknown fixed-task smoke case '$item'. Known cases: $($AllCases -join ', ')"
        }
    }
    return $Requested
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
    $commandResult = Invoke-CheckedCommand -FilePath $CargoBin -Arguments @(
        "test",
        "-p",
        "platform-api",
        "codex_host_fixed_task",
        "--lib"
    )
    $status = if ($commandResult.exit_code -eq 0) { "passed" } else { "failed" }
    $details = [ordered]@{
        command = "$CargoBin test -p platform-api codex_host_fixed_task --lib"
        exit_code = $commandResult.exit_code
    }
    foreach ($caseId in $selectedCases) {
        $message = switch ($caseId) {
            "static_page_plan_only" { "Fixed static-page queued audit package is safe and bounded." }
            "static_page_new_artifact" { "New generated-artifact output validation is accepted." }
            "static_page_overwrite_rejected" { "Non generated-artifact URL is rejected for human review." }
            "answer_quality_autofix" { "Low-risk allowlisted answer-quality patch output is accepted." }
            "human_exception" { "needs_human output is redacted and marked for exception notification." }
            "runtime_summary" { "Runtime diagnostics summarize fixed-task status without raw prompt data." }
            default { "Fixed-task smoke case ran." }
        }
        $results.Add((New-SmokeResult -CaseId $caseId -Status $status -Message $message -Details $details))
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
            $results.Add((New-SmokeResult -CaseId $caseId -Status "skipped" -Message "Server mutation smoke is guarded. Re-run on an approved host with -AllowServerMutation after deployment review." -Details @{ base_url = $BaseUrl; plan_only = [bool]$PlanOnly }))
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
