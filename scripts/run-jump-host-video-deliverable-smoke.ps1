param(
    [string] $HostAlias = "windows-jump",
    [string] $NodeBin = "node",
    [string] $RemoteDeliverablesPath = "",
    [switch] $Json,
    [switch] $SelfTest
)

$ErrorActionPreference = "Stop"

if (-not $SelfTest -and [string]::IsNullOrWhiteSpace($RemoteDeliverablesPath)) {
    throw "RemoteDeliverablesPath is required unless -SelfTest is provided."
}
if ($SelfTest -and -not [string]::IsNullOrWhiteSpace($RemoteDeliverablesPath)) {
    throw "RemoteDeliverablesPath cannot be combined with -SelfTest."
}

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$validatorPath = Resolve-Path -LiteralPath (Join-Path $repoRoot "tools\validate-video-deliverables.mjs")

function Escape-RemotePowerShellString([string] $Value) {
    return $Value.Replace("'", "''")
}

$remoteTarget = Escape-RemotePowerShellString $RemoteDeliverablesPath
$remoteNode = Escape-RemotePowerShellString $NodeBin
$jsonLiteral = if ($Json) { "`$true" } else { "`$false" }
$selfTestLiteral = if ($SelfTest) { "`$true" } else { "`$false" }

$remoteCommand = @"
`$ErrorActionPreference = 'Stop'
`$target = '$remoteTarget'
`$jsonOutput = $jsonLiteral
`$selfTest = $selfTestLiteral
`$nodeArgs = @('--input-type=module', '-', '--')
if (`$selfTest) {
    `$nodeArgs += '--self-test'
} else {
    `$nodeArgs += `$target
}
if (`$jsonOutput) {
    `$nodeArgs += '--json'
}
& '$remoteNode' @nodeArgs
exit `$LASTEXITCODE
"@
$encodedRemoteCommand = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($remoteCommand))

$runnerSource = @'
const __rawArgs = process.argv.slice(2).filter((arg) => arg !== "--");
const __jsonOutput = __rawArgs.includes("--json");
const __selfTest = __rawArgs.includes("--self-test");
let __target = __rawArgs.find((arg) => arg !== "--json" && arg !== "--self-test") || ".";

if (__selfTest) {
  const os = await import("node:os");
  __target = createJumpHostVideoDeliverablesFixture(os.tmpdir());
}

const __result = validateVideoDeliverables(__target);
if (__selfTest && !__jsonOutput) {
  console.log(`self-test fixture: ${__target}`);
}
if (__jsonOutput) {
  console.log(JSON.stringify(__result, null, 2));
} else {
  printHumanResult(__result);
}
process.exitCode = __result.ok ? 0 : 1;

function createJumpHostVideoDeliverablesFixture(tempRoot) {
  const root = fs.mkdtempSync(path.join(tempRoot, "aidp-v3-video-deliverables-jump-smoke-"));
  const artifactsDir = path.join(root, "generated_artifacts");
  fs.mkdirSync(artifactsDir, { recursive: true });

  fs.writeFileSync(path.join(artifactsDir, "video_slides_screenshot_based.pptx"), Buffer.from([0x50, 0x4b, 0x03, 0x04]));
  fs.writeFileSync(path.join(artifactsDir, "slide_notes.md"), "# Slide Notes\n\nAligned transcript is ready.\n");
  fs.writeFileSync(
    path.join(artifactsDir, "subtitle_page_map.json"),
    JSON.stringify(
      {
        status: "mapped",
        assignment_rule: "pre_page_previous_to_current",
        page_count: 1,
        pages: [
          {
            slide_number: 1,
            candidate_index: 2,
            source_frame: "frame_000002.jpg",
            transcript_segments: [{ start_seconds: 0.2, end_seconds: 0.6, text: "Aligned transcript" }],
          },
        ],
      },
      null,
      2,
    ),
  );

  const files = [
    "pptx",
    "final_deliverables_manifest",
    "extraction_artifacts_manifest",
    "slide_notes",
    "subtitle_page_map",
  ].map((kind) => ({
    artifact_kind: kind,
    file_name: fileNameForKind(kind),
    path: "[redacted]",
    path_redacted: true,
  }));

  fs.writeFileSync(
    path.join(artifactsDir, "final_deliverables_manifest.json"),
    JSON.stringify(
      {
        status: "final_pptx_ready",
        deliverable_status: {
          state: "final_pptx_ready",
          has_pptx: true,
          has_final_deliverables_manifest: true,
          has_extraction_artifacts_manifest: true,
          has_slide_notes: true,
          has_subtitle_page_map: true,
        },
        manifest_outputs: files.filter((file) =>
          ["final_deliverables_manifest", "extraction_artifacts_manifest"].includes(file.artifact_kind),
        ),
        final_outputs: files.filter((file) => file.artifact_kind === "pptx"),
        review_outputs: files.filter((file) => file.artifact_kind === "slide_notes"),
        evidence_outputs: files.filter((file) => file.artifact_kind === "subtitle_page_map"),
      },
      null,
      2,
    ),
  );

  fs.writeFileSync(
    path.join(artifactsDir, "extraction_artifacts_manifest.json"),
    JSON.stringify({ status: "completed", files }, null, 2),
  );

  return root;
}

function fileNameForKind(kind) {
  return {
    pptx: "video_slides_screenshot_based.pptx",
    final_deliverables_manifest: "final_deliverables_manifest.json",
    extraction_artifacts_manifest: "extraction_artifacts_manifest.json",
    slide_notes: "slide_notes.md",
    subtitle_page_map: "subtitle_page_map.json",
  }[kind];
}
'@

$validatorSource = Get-Content -Raw -LiteralPath $validatorPath
($validatorSource + "`n" + $runnerSource) |
    ssh $HostAlias "powershell" "-NoProfile" "-EncodedCommand" $encodedRemoteCommand

if ($LASTEXITCODE -ne 0) {
    throw "jump-host video deliverable smoke failed with exit code $LASTEXITCODE"
}
