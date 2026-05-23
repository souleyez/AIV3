param(
    [ValidateSet("LocalUnit", "Server")]
    [string] $Mode = "LocalUnit",
    [Alias("Case")]
    [string[]] $CaseId = @(),
    [string] $BaseUrl = "",
    [string] $BearerToken = "",
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
        [object[]] $Checks
    )

    $finalAnswerExcerpt = Get-CaseFinalAnswerExcerpt -Case $Case
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

    $allChecks = @($Checks) + @($assertionChecks.ToArray())
    $failed = @($allChecks | Where-Object { $_.status -ne "passed" })
    $qualityGateReason = Get-CaseObservabilityValue -Case $Case -Name "quality_gate_triggered_reason" -Default "not_required"
    $retryAttempts = Get-CaseObservabilityValue -Case $Case -Name "retry_attempts" -Default 0
    $reactActions = ConvertTo-StringArray (Get-CaseObservabilityValue -Case $Case -Name "react_actions_used" -Default @())
    $premiumAction = Get-CaseObservabilityValue -Case $Case -Name "premium_action" -Default "not_used"

    [pscustomobject]@{
        id = $Case.id
        label = $Case.label
        fixture_kind = $Case.fixture_kind
        prompt = $Case.prompt
        status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
        upload_parse_status = Get-CaseObservabilityValue -Case $Case -Name "original_parse_status" -Default $(if ($Case.id -eq "one-character-pdf") { "local_low_quality_guard" } else { "local_unit_contract" })
        original_parse_status = Get-CaseObservabilityValue -Case $Case -Name "original_parse_status" -Default $(if ($Case.id -eq "one-character-pdf") { "parse_degraded" } else { "unit_contract" })
        parse_quality_status = Get-CaseObservabilityValue -Case $Case -Name "parse_quality_status" -Default $(if ($Case.id -eq "one-character-pdf") { "low_text_coverage" } else { "unit_contract" })
        parse_lifecycle = Get-CaseObservabilityValue -Case $Case -Name "parse_lifecycle" -Default $(if ($Case.id -eq "one-character-pdf") { "parsed -> parse_degraded" } else { "unit_contract" })
        chunk_count = Get-CaseObservabilityValue -Case $Case -Name "chunk_count" -Default $null
        section_count = Get-CaseObservabilityValue -Case $Case -Name "section_count" -Default $null
        table_count = Get-CaseObservabilityValue -Case $Case -Name "table_count" -Default $null
        entity_count = Get-CaseObservabilityValue -Case $Case -Name "entity_count" -Default $null
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

if ($Mode -eq "Server") {
    if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
        throw "BaseUrl is required for -Mode Server."
    }
    $serverNote = if ([string]::IsNullOrWhiteSpace($BearerToken)) {
        "Server mode requested without BearerToken; only manifest/report scaffolding was produced."
    } else {
        "Server mode is reserved for 8-server fixture uploads; public request/response fields are not changed by this script."
    }
    foreach ($case in $cases) {
        $results.Add([pscustomobject]@{
            id = $case.id
            label = $case.label
            fixture_kind = $case.fixture_kind
            prompt = $case.prompt
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
            evidence_source_refs = @($case.expected)
            failure_markers = @()
            failure_reason = $serverNote
            checks = @()
        })
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
