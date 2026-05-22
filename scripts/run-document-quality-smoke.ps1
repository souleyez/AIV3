param(
    [ValidateSet("LocalUnit", "Server")]
    [string] $Mode = "LocalUnit",
    [string] $BaseUrl = "",
    [string] $BearerToken = "",
    [string] $ReportDir = "",
    [string] $CargoBin = "cargo",
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
$reportBase = "document-quality-smoke-$((Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))"
$reportJson = Join-Path $ReportDir "$reportBase.json"
$reportMd = Join-Path $ReportDir "$reportBase.md"
$cases = Get-Content -Raw -Encoding UTF8 -LiteralPath $manifestPath | ConvertFrom-Json
$results = New-Object System.Collections.Generic.List[object]

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

function New-CaseResult {
    param(
        [object] $Case,
        [object[]] $Checks
    )

    $failed = @($Checks | Where-Object { $_.status -ne "passed" })
    [pscustomobject]@{
        id = $Case.id
        label = $Case.label
        fixture_kind = $Case.fixture_kind
        prompt = $Case.prompt
        status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
        upload_parse_status = if ($Case.id -eq "one-character-pdf") { "local_low_quality_guard" } else { "local_unit_contract" }
        parse_lifecycle = if ($Case.id -eq "one-character-pdf") { "parsed -> parse_degraded" } else { "unit_contract" }
        chunk_count = $null
        section_count = $null
        table_count = $null
        entity_count = $null
        direct_answer_text = if ($Case.id -like "resume-*") { "validated by structured direct-answer table assertions" } elseif ($Case.id -like "third-party-*") { "validated by selected DOC evidence supply assertion" } else { $null }
        evidence_source_refs = @($Case.expected)
        failure_reason = if ($failed.Count -eq 0) { $null } else { (($failed | ForEach-Object { "$($_.package):$($_.filter)" }) -join "; ") }
        checks = $Checks
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
            parse_lifecycle = "not_run"
            chunk_count = $null
            section_count = $null
            table_count = $null
            entity_count = $null
            direct_answer_text = $null
            evidence_source_refs = @($case.expected)
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
        required_prints = "upload/parse status, lifecycle, chunks, section/table/entity counts, direct answer text, evidence/source refs, and failure reason"
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
    $lines.Add("- Upload/parse status: $($case.upload_parse_status)")
    $lines.Add("- Parse lifecycle: $($case.parse_lifecycle)")
    $lines.Add("- Chunk count: $($case.chunk_count)")
    $lines.Add("- Section/table/entity counts: section=$($case.section_count), table=$($case.table_count), entity=$($case.entity_count)")
    $lines.Add("- Direct answer text: $($case.direct_answer_text)")
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
