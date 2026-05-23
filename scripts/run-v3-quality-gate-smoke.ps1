param(
    [string[]] $Case = @("all"),
    [switch] $Local,
    [string] $BaseUrl = "",
    [string] $BearerToken = "",
    [string] $ReportDir = "",
    [string] $CargoBin = "cargo",
    [switch] $ListCases,
    [switch] $Json
)

$ErrorActionPreference = "Stop"

$mode = if ($Local -or [string]::IsNullOrWhiteSpace($BaseUrl)) { "LocalUnit" } else { "Server" }
$script = Join-Path $PSScriptRoot "run-document-quality-smoke.ps1"

$arguments = @{
    Mode = $mode
    CaseId = $Case
    BaseUrl = $BaseUrl
    BearerToken = $BearerToken
    ReportDir = $ReportDir
    CargoBin = $CargoBin
    ListCases = $ListCases
    Json = $Json
}

& $script @arguments
