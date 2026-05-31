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
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $FilePath @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
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

function Invoke-SmokeHttpRequest {
    param(
        [string] $Method,
        [string] $Uri,
        [string] $Body = "",
        [string] $ContentType = ""
    )

    $params = @{
        Method = $Method
        Uri = $Uri
        TimeoutSec = 20
        UseBasicParsing = $true
        MaximumRedirection = 0
    }
    if ((Get-Command Invoke-WebRequest).Parameters.ContainsKey("SkipHttpErrorCheck")) {
        $params.SkipHttpErrorCheck = $true
    }
    if (-not [string]::IsNullOrWhiteSpace($Body)) {
        $params.Body = $Body
    }
    if (-not [string]::IsNullOrWhiteSpace($ContentType)) {
        $params.ContentType = $ContentType
    }

    try {
        $response = Invoke-WebRequest @params
        $content = if ($null -eq $response.Content) { "" } else { $response.Content.ToString() }
        return [ordered]@{
            ok = $true
            status_code = [int]$response.StatusCode
            content = $content
            error = $null
        }
    } catch {
        $response = $_.Exception.Response
        if ($null -eq $response) {
            return [ordered]@{
                ok = $false
                status_code = $null
                content = ""
                error = $_.Exception.Message
            }
        }

        $content = ""
        if ($response -is [System.Net.HttpWebResponse]) {
            try {
                $stream = $response.GetResponseStream()
                if ($null -ne $stream) {
                    $reader = [System.IO.StreamReader]::new($stream)
                    try {
                        $content = $reader.ReadToEnd()
                    } finally {
                        $reader.Dispose()
                    }
                }
            } catch {
                $content = ""
            }
        } elseif ($response.PSObject.Properties.Name -contains "Content") {
            try {
                if ($null -eq $response.Content) {
                    $content = ""
                } elseif ($response.Content.PSObject.Methods.Name -contains "ReadAsStringAsync") {
                    $content = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
                } else {
                    $content = $response.Content.ToString()
                }
            } catch {
                $content = ""
            }
        }

        return [ordered]@{
            ok = $true
            status_code = [int]$response.StatusCode
            content = $content
            error = $_.Exception.Message
        }
    }
}

function New-ContentExcerpt {
    param([string] $Content)
    if ([string]::IsNullOrWhiteSpace($Content)) {
        return ""
    }
    return $Content.Substring(0, [Math]::Min(240, $Content.Length))
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

    $docsUrl = "$base/external-integrations/pure-third-party-integration-guide.zh-CN.html"
    $docs = Invoke-SmokeHttpRequest -Method "GET" -Uri $docsUrl
    $docsPassed = $docs.ok -and $docs.status_code -eq 200 -and $docs.content.Contains("第三方")
    $results.Add((New-SmokeResult `
        -CaseId "server_docs" `
        -Status $(if ($docsPassed) { "passed" } else { "failed" }) `
        -Message $(if ($docsPassed) { "Public third-party integration guide responded." } else { "Public third-party integration guide did not return the expected HTML content." }) `
        -Details @{
            url = $docsUrl
            status_code = $docs.status_code
            content_excerpt = New-ContentExcerpt -Content $docs.content
            error = $docs.error
        }))

    $authGuardUrl = "$base/v1/external/channels/generic-chat-main/events"
    $authGuard = Invoke-SmokeHttpRequest -Method "POST" -Uri $authGuardUrl -Body "{}" -ContentType "application/json"
    $authGuardPassed = $authGuard.ok -and $authGuard.status_code -eq 401 -and $authGuard.content.Contains("external_channel_auth_failed")
    $results.Add((New-SmokeResult `
        -CaseId "server_external_api_auth_guard" `
        -Status $(if ($authGuardPassed) { "passed" } else { "failed" }) `
        -Message $(if ($authGuardPassed) { "External events API reached platform-api and rejected missing bearer token without mutation." } else { "External events API did not return the expected missing-token guard." }) `
        -Details @{
            url = $authGuardUrl
            status_code = $authGuard.status_code
            content_excerpt = New-ContentExcerpt -Content $authGuard.content
            error = $authGuard.error
        }))

    $queueStatsUrl = "$base/v1/workflow-tasks/queue-stats"
    $queueStats = Invoke-SmokeHttpRequest -Method "GET" -Uri $queueStatsUrl
    $queueStatsParsed = $null
    if ($queueStats.ok -and $queueStats.status_code -eq 200) {
        try {
            $queueStatsParsed = $queueStats.content | ConvertFrom-Json
        } catch {
            $queueStatsParsed = $null
        }
    }
    $queueStatsPassed = $null -ne $queueStatsParsed -and -not [string]::IsNullOrWhiteSpace($queueStatsParsed.generated_at) -and $null -ne $queueStatsParsed.queues
    $results.Add((New-SmokeResult `
        -CaseId "server_queue_stats" `
        -Status $(if ($queueStatsPassed) { "passed" } else { "failed" }) `
        -Message $(if ($queueStatsPassed) { "Workflow queue stats endpoint returned JSON diagnostics." } else { "Workflow queue stats endpoint did not return the expected JSON diagnostics." }) `
        -Details @{
            url = $queueStatsUrl
            status_code = $queueStats.status_code
            queue_count = if ($queueStatsPassed) { @($queueStatsParsed.queues).Count } else { $null }
            task_count = if ($queueStatsPassed) { $queueStatsParsed.task_count } else { $null }
            content_excerpt = New-ContentExcerpt -Content $queueStats.content
            error = $queueStats.error
        }))

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
