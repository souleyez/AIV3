param(
    [string] $HostAlias = "windows-jump",
    [string] $CodexJs = "C:\Users\soulz\AppData\Roaming\npm\node_modules\@openai\codex\bin\codex.js",
    [string] $NodeBin = "node",
    [string] $SmokeScript = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
if ([string]::IsNullOrWhiteSpace($SmokeScript)) {
    $SmokeScript = Join-Path $repoRoot "tools\codex-host-responses-shim-smoke.mjs"
}
$smokeScriptPath = Resolve-Path -LiteralPath $SmokeScript

function Escape-RemotePowerShellString([string] $Value) {
    return $Value.Replace("'", "''")
}

$remoteCommand = @"
`$env:CODEX_HOST_SHIM_CODEX_JS = '$(Escape-RemotePowerShellString $CodexJs)'
& '$(Escape-RemotePowerShellString $NodeBin)' --input-type=module -
"@
$encodedRemoteCommand = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($remoteCommand))

Get-Content -Raw -LiteralPath $smokeScriptPath |
    ssh $HostAlias "powershell" "-NoProfile" "-EncodedCommand" $encodedRemoteCommand

if ($LASTEXITCODE -ne 0) {
    throw "jump-host Codex shim smoke failed with exit code $LASTEXITCODE"
}
