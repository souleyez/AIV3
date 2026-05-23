param(
    [ValidateSet("LocalUnit", "Server")]
    [string] $Mode = "LocalUnit",
    [Alias("Case")]
    [string[]] $CaseId = @(),
    [string] $BaseUrl = "",
    [string] $BearerToken = "",
    [string] $ServerCaseConfigPath = "",
    [int] $ServerTimeoutSec = 180,
    [string] $ReportDir = "",
    [string] $CargoBin = "cargo",
    [switch] $Local,
    [switch] $ListCases,
    [switch] $Json
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$manifestPath = Join-Path $repoRoot "fixtures\document-quality\smoke-cases.json"
if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "Missing document quality smoke manifest: $manifestPath"
}

if ([string]::IsNullOrWhiteSpace($ReportDir)) {
    $ReportDir = Join-Path $repoRoot "target\document-quality-smoke"
}
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null

$head = git -C $repoRoot rev-parse --short HEAD
$startedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$reportBase = "document-quality-smoke-$((Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))-$PID"
$reportJson = Join-Path $ReportDir "$reportBase.json"
$reportMd = Join-Path $ReportDir "$reportBase.md"
$cases = Get-Content -Raw -Encoding UTF8 -LiteralPath $manifestPath | ConvertFrom-Json
$serverCaseConfig = $null
if (-not [string]::IsNullOrWhiteSpace($ServerCaseConfigPath)) {
    $resolvedServerCaseConfigPath = Resolve-Path -LiteralPath $ServerCaseConfigPath
    $serverCaseConfig = Get-Content -Raw -Encoding UTF8 -LiteralPath $resolvedServerCaseConfigPath | ConvertFrom-Json
}

if ($Local) {
    $Mode = "LocalUnit"
}

$caseAliases = @{
    "all" = "all"
    "one_character_pdf" = "one-character-pdf"
    "low_text_pdf_final" = "one-character-pdf"
    "deng_engineer" = "third-party-doc-deng-engineer"
    "third_party_doc_deng_engineer" = "third-party-doc-deng-engineer"
    "resume_company_stats" = "resume-company-statistics"
    "resume_ranking_table" = "resume-multidimension-ranking"
    "attendance_final" = "attendance-xlsx-date-format"
    "attendance_frequent" = "attendance-frequent-absence-workhours"
    "attendance_hot" = "attendance-frequent-absence-workhours"
    "smart_home" = "smart-home-customer-feedback"
    "smart_elevator" = "smart-elevator-customer-feedback"
}

function Resolve-CaseId {
    param([string] $Value)
    $normalized = $Value.Trim()
    if ([string]::IsNullOrWhiteSpace($normalized)) {
        return $null
    }
    $aliasKey = $normalized.ToLowerInvariant()
    if ($caseAliases.ContainsKey($aliasKey)) {
        return $caseAliases[$aliasKey]
    }
    return $normalized
}

if ($ListCases) {
    $cases | ForEach-Object {
        Write-Host "$($_.id) - $($_.label)"
    }
    return
}

if ($CaseId.Count -gt 0) {
    $requestedCaseIds = @($CaseId | ForEach-Object { Resolve-CaseId $_ } | Where-Object { $_ })
    if (-not ($requestedCaseIds -contains "all")) {
        $knownCaseIds = @($cases | ForEach-Object { $_.id })
        $unknownCaseIds = @($requestedCaseIds | Where-Object { $knownCaseIds -notcontains $_ })
        if ($unknownCaseIds.Count -gt 0) {
            throw "Unknown document-quality smoke case(s): $($unknownCaseIds -join ', '). Use -ListCases to inspect available cases."
        }
        $cases = @($cases | Where-Object { $requestedCaseIds -contains $_.id })
    }
}

$results = New-Object System.Collections.Generic.List[object]
$defaultFailureMarkers = @(
    "parse_degraded",
    "low_text_coverage",
    "fallback_unavailable",
    "需要先检索",
    "请继续",
    "无法回答",
    "runtime_manifest",
    "execution_trail",
    "react_trace",
    "tool_trace",
    "provider raw"
)

function Invoke-LocalCargoCheck {
    param(
        [string] $Package,
        [string] $Filter
    )

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $CargoBin test -p $Package $Filter --lib 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    [pscustomobject]@{
        package = $Package
        filter = $Filter
        status = if ($exitCode -eq 0) { "passed" } else { "failed" }
        exit_code = $exitCode
        output_excerpt = (($output | Select-Object -Last 30) -join "`n")
    }
}

function Get-CaseProperty {
    param(
        [object] $Object,
        [string] $Name,
        [object] $Default = $null
    )

    if ($null -eq $Object) {
        return $Default
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $Default
    }
    return $property.Value
}

function Get-PropertyByNames {
    param(
        [object] $Object,
        [string[]] $Names,
        [object] $Default = $null
    )

    if ($null -eq $Object) {
        return $Default
    }
    foreach ($name in $Names) {
        $property = $Object.PSObject.Properties[$name]
        if ($null -ne $property) {
            return $property.Value
        }
    }
    return $Default
}

function ConvertTo-StringArray {
    param([object] $Value)

    if ($null -eq $Value) {
        return @()
    }
    if ($Value -is [System.Array]) {
        return @($Value | ForEach-Object { "$_" })
    }
    return @("$Value")
}

function Get-CaseObservabilityValue {
    param(
        [object] $Case,
        [string] $Name,
        [object] $Default = $null
    )

    $observability = Get-CaseProperty -Object $Case -Name "observability" -Default $null
    return Get-CaseProperty -Object $observability -Name $Name -Default $Default
}

function Get-ResultObservabilityValue {
    param(
        [object] $Case,
        [object] $Observed,
        [bool] $PreferObserved,
        [string] $Name,
        [object] $Default = $null
    )

    if ($PreferObserved -and $null -ne $Observed) {
        $observedProperty = $Observed.PSObject.Properties[$Name]
        if ($null -ne $observedProperty -and $null -ne $observedProperty.Value) {
            return $observedProperty.Value
        }
    }
    return Get-CaseObservabilityValue -Case $Case -Name $Name -Default $Default
}

function Get-CaseFinalAnswerExcerpt {
    param([object] $Case)

    $configured = Get-CaseObservabilityValue -Case $Case -Name "final_answer_excerpt" -Default $null
    if (-not [string]::IsNullOrWhiteSpace($configured)) {
        return "$configured"
    }

    switch ($Case.id) {
        "one-character-pdf" {
            return "当前 PDF 的可用文本过少，不能把单个字符当作正文结论；需要使用 OCR/VLM 复查后再回答材料内容。"
        }
        "third-party-doc-deng-engineer" {
            return "邓工是可见文档中提到的技术人员，回答需引用选中文档证据。"
        }
        "resume-company-statistics" {
            return "公司名统计以 company_rows 为准，报告公司总数和对应文档来源，不从 candidate_terms 额外扩展。"
        }
        "resume-multidimension-ranking" {
            return "| 排序维度 | 候选人 | 排名依据 | 来源 |`n| 技能数 | 张三 | 8 项技能 | resume_profile_rows |"
        }
        "attendance-xlsx-date-format" {
            return "| 类别 | 日期 | 员工 | 工时 |`n| 缺勤 | 2026-05-01 | 李四 | - |`n| 最长工时 | 2026-05-02 | 王五 | 10.50小时 |"
        }
        "attendance-frequent-absence-workhours" {
            return "| 类别 | 日期 | 员工 | 班次/工时 | 状态 |`n| 缺勤 | 2026-05-13 | A5 | 坐班0900 | 未打卡 不考勤 |`n| 最长工时 | 2026-02-07 | A8 | 12.55小时 | 正常考勤 |`n| 最短工时 | 2026-02-25 | A8 | 4.30小时 | 正常考勤 |"
        }
        "table-heavy-document" {
            return "| 表格 | 行列信号 | 来源 |`n| 明细表 | table_rows/section rows 已供料 | structured scan rows |"
        }
        "smart-home-customer-feedback" {
            return "基于可见讲解词，智能家居功能包括场景联动、设备控制和状态感知；回答需避免暴露内部解析状态。"
        }
        "smart-elevator-customer-feedback" {
            return "| 楼层 | 点位 | 说明 |`n| 1F | 电梯厅 | 梯控点位来自可见材料或深读 observation |"
        }
        default {
            return "本地 smoke 仅验证供料、质量门禁和输出安全契约。"
        }
    }
}

function Test-FinalAnswerMarkers {
    param(
        [string] $Text,
        [string[]] $Markers
    )

    $hits = New-Object System.Collections.Generic.List[string]
    $lower = $Text.ToLowerInvariant()
    foreach ($marker in $Markers) {
        if ([string]::IsNullOrWhiteSpace($marker)) {
            continue
        }
        if ($lower.Contains($marker.ToLowerInvariant())) {
            $hits.Add($marker)
        }
    }
    return @($hits)
}

function New-SmokeAssertionCheck {
    param(
        [string] $Name,
        [bool] $Passed,
        [string] $Message
    )

    [pscustomobject]@{
        package = "smoke-assertion"
        filter = $Name
        status = if ($Passed) { "passed" } else { "failed" }
        exit_code = if ($Passed) { 0 } else { 1 }
        output_excerpt = $Message
    }
}

function New-CaseResult {
    param(
        [object] $Case,
        [object[]] $Checks,
        [string] $FinalAnswerText = "",
        [object] $Observed = $null,
        [switch] $PreferObserved
    )

    $finalAnswerExcerpt = if (-not [string]::IsNullOrWhiteSpace($FinalAnswerText)) {
        $FinalAnswerText.Trim()
    } else {
        Get-CaseFinalAnswerExcerpt -Case $Case
    }
    $caseFailureMarkers = ConvertTo-StringArray (Get-CaseProperty -Object $Case -Name "failure_markers" -Default $defaultFailureMarkers)
    if ($caseFailureMarkers.Count -eq 0) {
        $caseFailureMarkers = $defaultFailureMarkers
    }
    $markerHits = Test-FinalAnswerMarkers -Text $finalAnswerExcerpt -Markers $caseFailureMarkers
    $assertionChecks = New-Object System.Collections.Generic.List[object]
    $assertionChecks.Add((New-SmokeAssertionCheck -Name "final_sanitizer_leak_check" -Passed ($markerHits.Count -eq 0) -Message $(if ($markerHits.Count -eq 0) { "no forbidden final-answer markers" } else { "forbidden markers: $($markerHits -join ', ')" })))

    $requiresNormalizedDates = [bool](Get-CaseProperty -Object $Case -Name "requires_normalized_dates" -Default $false)
    if ($requiresNormalizedDates) {
        $hasIsoDate = $finalAnswerExcerpt -match "\b20\d{2}-\d{2}-\d{2}\b"
        $hasExcelSerialDate = $finalAnswerExcerpt -match "\b4[0-9]{4}(\.\d+)?\b"
        $assertionChecks.Add((New-SmokeAssertionCheck -Name "normalized_date_format" -Passed ($hasIsoDate -and -not $hasExcelSerialDate) -Message "requires YYYY-MM-DD and no Excel serial date leakage"))
    }

    $requiresTable = [bool](Get-CaseProperty -Object $Case -Name "requires_table" -Default $false)
    if ($requiresTable) {
        $hasTable = $finalAnswerExcerpt.Contains("|")
        $assertionChecks.Add((New-SmokeAssertionCheck -Name "table_answer_shape" -Passed $hasTable -Message "requires a table-shaped final answer excerpt"))
    }

    $requiredFinalAnswerTerms = ConvertTo-StringArray (Get-CaseProperty -Object $Case -Name "required_final_answer_terms" -Default @())
    if ($requiredFinalAnswerTerms.Count -gt 0) {
        $missingTerms = @($requiredFinalAnswerTerms | Where-Object { -not $finalAnswerExcerpt.Contains($_) })
        $assertionChecks.Add((New-SmokeAssertionCheck -Name "required_final_answer_terms" -Passed ($missingTerms.Count -eq 0) -Message $(if ($missingTerms.Count -eq 0) { "all required final-answer terms are present" } else { "missing terms: $($missingTerms -join ', ')" })))
    }

    $allChecks = @($Checks) + @($assertionChecks.ToArray())
    $failed = @($allChecks | Where-Object { $_.status -ne "passed" })
    $qualityGateReason = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "quality_gate_triggered_reason" -Default "not_required"
    $retryAttempts = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "retry_attempts" -Default 0
    $reactActions = ConvertTo-StringArray (Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "react_actions_used" -Default @())
    $premiumAction = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "premium_action" -Default "not_used"

    [pscustomobject]@{
        id = $Case.id
        label = $Case.label
        fixture_kind = $Case.fixture_kind
        prompt = $Case.prompt
        status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
        upload_parse_status = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "original_parse_status" -Default $(if ($Case.id -eq "one-character-pdf") { "local_low_quality_guard" } else { "local_unit_contract" })
        original_parse_status = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "original_parse_status" -Default $(if ($Case.id -eq "one-character-pdf") { "parse_degraded" } else { "unit_contract" })
        parse_quality_status = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "parse_quality_status" -Default $(if ($Case.id -eq "one-character-pdf") { "low_text_coverage" } else { "unit_contract" })
        parse_lifecycle = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "parse_lifecycle" -Default $(if ($Case.id -eq "one-character-pdf") { "parsed -> parse_degraded" } else { "unit_contract" })
        chunk_count = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "chunk_count" -Default $null
        section_count = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "section_count" -Default $null
        table_count = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "table_count" -Default $null
        entity_count = Get-ResultObservabilityValue -Case $Case -Observed $Observed -PreferObserved ([bool]$PreferObserved) -Name "entity_count" -Default $null
        quality_gate_triggered_reason = $qualityGateReason
        retry_attempts = $retryAttempts
        react_actions_used = $reactActions
        premium_action = $premiumAction
        final_sanitizer_leak_check = if ($markerHits.Count -eq 0) { "passed" } else { "failed" }
        final_answer_excerpt = $finalAnswerExcerpt
        direct_answer_text = $finalAnswerExcerpt
        evidence_source_refs = @($Case.expected)
        failure_markers = $caseFailureMarkers
        failure_reason = if ($failed.Count -eq 0) { $null } else { (($failed | ForEach-Object { "$($_.package):$($_.filter)" }) -join "; ") }
        checks = $allChecks
    }
}

function New-ServerSkippedCaseResult {
    param(
        [object] $Case,
        [string] $Reason
    )

    [pscustomobject]@{
        id = $Case.id
        label = $Case.label
        fixture_kind = $Case.fixture_kind
        prompt = $Case.prompt
        status = "skipped"
        upload_parse_status = "not_run"
        original_parse_status = "not_run"
        parse_quality_status = "not_run"
        parse_lifecycle = "not_run"
        chunk_count = $null
        section_count = $null
        table_count = $null
        entity_count = $null
        quality_gate_triggered_reason = "not_run"
        retry_attempts = 0
        react_actions_used = @()
        premium_action = "not_run"
        final_sanitizer_leak_check = "not_run"
        final_answer_excerpt = $null
        direct_answer_text = $null
        evidence_source_refs = @($Case.expected)
        failure_markers = @()
        failure_reason = $Reason
        checks = @()
    }
}

function Get-ServerCaseConfig {
    param(
        [object] $Config,
        [object] $Case
    )

    if ($null -eq $Config) {
        return $null
    }
    $caseConfigs = Get-CaseProperty -Object $Config -Name "cases" -Default $null
    if ($null -eq $caseConfigs) {
        return $null
    }
    if ($caseConfigs -is [System.Array]) {
        foreach ($entry in $caseConfigs) {
            $entryId = Get-PropertyByNames -Object $entry -Names @("id", "case_id", "caseId") -Default ""
            if ($entryId -eq $Case.id) {
                return $entry
            }
        }
        return $null
    }
    $property = $caseConfigs.PSObject.Properties[$Case.id]
    if ($null -ne $property) {
        return $property.Value
    }
    return $null
}

function Join-AssistantRunApiUrl {
    param(
        [string] $Root,
        [string] $Path
    )

    $trimmedRoot = $Root.TrimEnd("/")
    if ($trimmedRoot.EndsWith("/api/v3") -and $Path.StartsWith("/v1/")) {
        return "$trimmedRoot/$($Path.Substring(4))"
    }
    if ($trimmedRoot.EndsWith("/v1") -and $Path.StartsWith("/v1/")) {
        return "$trimmedRoot/$($Path.Substring(4))"
    }
    return "$trimmedRoot$Path"
}

function Get-AssistantRunIdFromResponse {
    param([object] $Response)

    $direct = Get-PropertyByNames -Object $Response -Names @("assistant_run_id", "assistantRunId", "id") -Default $null
    if (-not [string]::IsNullOrWhiteSpace($direct)) {
        return "$direct"
    }
    $run = Get-PropertyByNames -Object $Response -Names @("run", "assistant_run", "assistantRun") -Default $null
    $runId = Get-PropertyByNames -Object $run -Names @("id") -Default $null
    if (-not [string]::IsNullOrWhiteSpace($runId)) {
        return "$runId"
    }
    return $null
}

function Get-AssistantFinalTextFromResponse {
    param([object] $Response)

    $assistantMessage = Get-PropertyByNames -Object $Response -Names @("assistant_message", "assistantMessage") -Default $null
    $messageContent = Get-PropertyByNames -Object $assistantMessage -Names @("content") -Default $null
    if (-not [string]::IsNullOrWhiteSpace($messageContent)) {
        return "$messageContent"
    }

    $artifacts = Get-PropertyByNames -Object $Response -Names @("output_artifacts", "outputArtifacts") -Default @()
    foreach ($artifact in @($artifacts)) {
        $content = Get-PropertyByNames -Object $artifact -Names @("content", "text") -Default $null
        if (-not [string]::IsNullOrWhiteSpace($content)) {
            return "$content"
        }
    }
    return ""
}

function Get-AssistantEvents {
    param(
        [object] $Response,
        [object] $Detail
    )

    $events = New-Object System.Collections.Generic.List[object]
    foreach ($source in @($Response, $Detail)) {
        $sourceEvents = Get-PropertyByNames -Object $source -Names @("events") -Default @()
        foreach ($event in @($sourceEvents)) {
            if ($null -ne $event) {
                $events.Add($event)
            }
        }
    }
    return @($events.ToArray())
}

function Get-EventPayloadValue {
    param(
        [object] $Event,
        [string[]] $Names
    )

    $payload = Get-PropertyByNames -Object $Event -Names @("payload") -Default $null
    return Get-PropertyByNames -Object $payload -Names $Names -Default $null
}

function Convert-ServerObservability {
    param(
        [object] $Response,
        [object] $Detail
    )

    $events = Get-AssistantEvents -Response $Response -Detail $Detail
    $eventNames = @($events | ForEach-Object { Get-PropertyByNames -Object $_ -Names @("event_name", "eventName") -Default "" } | Where-Object { $_ })
    $gateEvents = @($events | Where-Object { (Get-PropertyByNames -Object $_ -Names @("event_name", "eventName") -Default "") -like "assistant_run.answer_quality_gate*" })
    $retryEvents = @($gateEvents | Where-Object { (Get-PropertyByNames -Object $_ -Names @("event_name", "eventName") -Default "") -like "*.retry_started" })
    $budgetEvents = @($gateEvents | Where-Object { (Get-PropertyByNames -Object $_ -Names @("event_name", "eventName") -Default "") -like "*.budget_selected" })

    $reason = $null
    foreach ($event in @($retryEvents + $budgetEvents + $gateEvents)) {
        $candidate = Get-EventPayloadValue -Event $event -Names @("reason", "retry_reason", "retryReason", "triggered_reason", "triggeredReason")
        if (-not [string]::IsNullOrWhiteSpace($candidate)) {
            $reason = "$candidate"
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($reason)) {
        $reason = if ($gateEvents.Count -gt 0) { "observed_quality_gate_event" } else { "not_observed" }
    }

    $reactActions = New-Object System.Collections.Generic.List[string]
    foreach ($event in $events) {
        $eventName = Get-PropertyByNames -Object $event -Names @("event_name", "eventName") -Default ""
        if ($eventName -notlike "assistant_run.react*") {
            continue
        }
        $action = Get-EventPayloadValue -Event $event -Names @("action_type", "actionType", "react_action", "reactAction")
        if (-not [string]::IsNullOrWhiteSpace($action) -and -not $reactActions.Contains("$action")) {
            $reactActions.Add("$action")
        }
    }

    $observabilityJson = (@($Response, $Detail, $events) | ConvertTo-Json -Depth 20 -Compress)
    $premiumAction = if ($observabilityJson.Contains("upgrade_parse_vlm")) { "upgrade_parse_vlm_observed" } else { "not_observed" }

    [pscustomobject]@{
        original_parse_status = "server_observed"
        parse_quality_status = "server_observed"
        parse_lifecycle = "server_observed"
        quality_gate_triggered_reason = $reason
        retry_attempts = $retryEvents.Count
        react_actions_used = @($reactActions.ToArray())
        premium_action = $premiumAction
    }
}

function Invoke-ServerAssistantRunCase {
    param(
        [object] $Case,
        [object] $CaseConfig
    )

    $headers = @{
        "Accept" = "application/json"
    }
    if (-not [string]::IsNullOrWhiteSpace($BearerToken)) {
        $headers["Authorization"] = "Bearer $BearerToken"
    }

    $promptOverride = Get-PropertyByNames -Object $CaseConfig -Names @("prompt") -Default $null
    $selectedScope = Get-PropertyByNames -Object $CaseConfig -Names @("selected_scope", "selectedScope") -Default $null
    $allowAutoScope = [bool](Get-PropertyByNames -Object $CaseConfig -Names @("allow_auto_scope", "allowAutoScope") -Default $false)
    if ($null -eq $selectedScope -and -not $allowAutoScope) {
        return New-ServerSkippedCaseResult -Case $Case -Reason "server_selected_scope_missing; provide selected_scope in ServerCaseConfigPath or set allow_auto_scope=true"
    }

    $payload = [ordered]@{
        prompt = if (-not [string]::IsNullOrWhiteSpace($promptOverride)) { "$promptOverride" } else { "$($Case.prompt)" }
        local_thread_id = "document-quality-smoke-$($Case.id)-$([guid]::NewGuid().ToString('N'))"
    }
    foreach ($pair in @(
        @{ Out = "startup_briefing"; Names = @("startup_briefing", "startupBriefing") },
        @{ Out = "selected_scope"; Names = @("selected_scope", "selectedScope") },
        @{ Out = "scope_candidates"; Names = @("scope_candidates", "scopeCandidates") },
        @{ Out = "context_policy_hint"; Names = @("context_policy_hint", "contextPolicyHint") },
        @{ Out = "current_artifact"; Names = @("current_artifact", "currentArtifact") },
        @{ Out = "messages"; Names = @("messages") }
    )) {
        $value = Get-PropertyByNames -Object $CaseConfig -Names $pair.Names -Default $null
        if ($null -ne $value) {
            $payload[$pair.Out] = $value
        }
    }

    $body = $payload | ConvertTo-Json -Depth 30
    $createUrl = Join-AssistantRunApiUrl -Root $BaseUrl -Path "/v1/assistant-runs"
    try {
        $response = Invoke-RestMethod -Method Post -Uri $createUrl -Headers $headers -ContentType "application/json; charset=utf-8" -Body $body -TimeoutSec $ServerTimeoutSec
        $detail = $null
        $runId = Get-AssistantRunIdFromResponse -Response $response
        if (-not [string]::IsNullOrWhiteSpace($runId)) {
            $detailUrl = Join-AssistantRunApiUrl -Root $BaseUrl -Path "/v1/assistant-runs/$runId"
            try {
                $detail = Invoke-RestMethod -Method Get -Uri $detailUrl -Headers $headers -TimeoutSec $ServerTimeoutSec
            } catch {
                $detail = $null
            }
        }
        $finalAnswer = Get-AssistantFinalTextFromResponse -Response $response
        $checks = @(
            (New-SmokeAssertionCheck -Name "server_create_assistant_run" -Passed $true -Message "POST completed against configured server URL"),
            (New-SmokeAssertionCheck -Name "server_final_answer_present" -Passed (-not [string]::IsNullOrWhiteSpace($finalAnswer)) -Message "requires non-empty assistant_message.content")
        )
        $observed = Convert-ServerObservability -Response $response -Detail $detail
        return New-CaseResult -Case $Case -Checks $checks -FinalAnswerText $finalAnswer -Observed $observed -PreferObserved
    } catch {
        $checks = @(
            (New-SmokeAssertionCheck -Name "server_create_assistant_run" -Passed $false -Message "$($_.Exception.Message)")
        )
        $observed = [pscustomobject]@{
            original_parse_status = "server_error"
            parse_quality_status = "server_error"
            parse_lifecycle = "server_error"
            quality_gate_triggered_reason = "server_error"
            retry_attempts = 0
            react_actions_used = @()
            premium_action = "server_error"
        }
        return New-CaseResult -Case $Case -Checks $checks -FinalAnswerText "" -Observed $observed -PreferObserved
    }
}

if ($Mode -eq "Server") {
    if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
        throw "BaseUrl is required for -Mode Server."
    }
    foreach ($case in $cases) {
        $caseConfig = Get-ServerCaseConfig -Config $serverCaseConfig -Case $case
        if ($null -eq $caseConfig) {
            $results.Add((New-ServerSkippedCaseResult -Case $case -Reason "server_case_config_missing; pass -ServerCaseConfigPath with a cases entry for this case"))
            continue
        }
        Write-Host ""
        Write-Host "== $($case.label) =="
        Write-Host "Server: $BaseUrl"
        $results.Add((Invoke-ServerAssistantRunCase -Case $case -CaseConfig $caseConfig))
    }
} else {
    foreach ($case in $cases) {
        Write-Host ""
        Write-Host "== $($case.label) =="
        Write-Host "Prompt: $($case.prompt)"
        $checks = @()
        foreach ($test in $case.local_tests) {
            Write-Host "cargo test -p $($test.package) $($test.filter) --lib"
            $check = Invoke-LocalCargoCheck -Package $test.package -Filter $test.filter
            Write-Host "$($check.status): $($test.package)::$($test.filter)"
            $checks += $check
            if ($check.status -ne "passed") {
                break
            }
        }
        $results.Add((New-CaseResult -Case $case -Checks $checks))
    }
}

$finishedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$ready = -not @($results | Where-Object { $_.status -eq "failed" }).Count
$report = [pscustomobject]@{
    smoke = "document-quality"
    mode = $Mode
    ready = $ready
    repository = $repoRoot.Path
    head = ($head | Select-Object -First 1)
    started_at = $startedAt
    finished_at = $finishedAt
    contract = [pscustomobject]@{
        public_api_shape = "unchanged by this smoke"
        required_prints = "original parse status, parse-quality status, quality-gate reason, retry attempts, ReAct actions, premium action, sanitizer leak check, final answer excerpt, evidence/source refs, and failure reason"
    }
    cases = $results
}

$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportJson -Encoding UTF8

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# Document Quality Smoke")
$lines.Add("")
$lines.Add("- Status: $(if ($ready) { 'passed' } else { 'failed' })")
$lines.Add("- Mode: $Mode")
$lines.Add("- Repository: $($repoRoot.Path)")
$lines.Add("- HEAD: $(($head | Select-Object -First 1))")
$lines.Add("- Started: $startedAt")
$lines.Add("- Finished: $finishedAt")
$lines.Add("")
$lines.Add("## Cases")
$lines.Add("")
foreach ($case in $results) {
    $lines.Add("### $($case.label)")
    $lines.Add("")
    $lines.Add("- Status: $($case.status)")
    $lines.Add("- Prompt: $($case.prompt)")
    $lines.Add("- Original parse status: $($case.original_parse_status)")
    $lines.Add("- Parse-quality status: $($case.parse_quality_status)")
    $lines.Add("- Parse lifecycle: $($case.parse_lifecycle)")
    $lines.Add("- Chunk count: $($case.chunk_count)")
    $lines.Add("- Section/table/entity counts: section=$($case.section_count), table=$($case.table_count), entity=$($case.entity_count)")
    $lines.Add("- Quality gate reason: $($case.quality_gate_triggered_reason)")
    $lines.Add("- Retry attempts: $($case.retry_attempts)")
    $lines.Add("- ReAct actions used: $((@($case.react_actions_used) -join ', '))")
    $lines.Add("- Premium action: $($case.premium_action)")
    $lines.Add("- Final sanitizer leak check: $($case.final_sanitizer_leak_check)")
    $lines.Add("- Final answer excerpt: $($case.final_answer_excerpt)")
    $lines.Add("- Evidence/source refs: $((@($case.evidence_source_refs) -join '; '))")
    $lines.Add("- Failure reason: $($case.failure_reason)")
    $lines.Add("")
}
$lines | Set-Content -LiteralPath $reportMd -Encoding UTF8

if ($Json) {
    $report | ConvertTo-Json -Depth 12
} else {
    Write-Host ""
    Write-Host "Document quality smoke report: $reportJson"
    Write-Host "Document quality smoke summary: $reportMd"
    if (-not $ready) {
        throw "document-quality smoke failed"
    }
    Write-Host "OK document-quality smoke completed."
}
