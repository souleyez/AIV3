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

  fs.writeFileSync(path.join(artifactsDir, "video_slides_screenshot_based.pptx"), minimalPptxFixtureBytes());
  fs.writeFileSync(path.join(artifactsDir, "slide_notes.md"), "# Slide Notes\n\nAligned transcript is ready.\n");
  fs.writeFileSync(path.join(artifactsDir, "video_slides.md"), "# Video Slides\n\n## Slide 1\n\nAligned transcript is ready.\n");
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
    "published_deliverable_manifest",
    "published_version_history",
    "extraction_artifacts_manifest",
    "slide_rectangles_manifest",
    "slide_notes",
    "video_slides_markdown",
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
          has_published_deliverable_manifest: true,
          has_published_version_history: true,
          has_extraction_artifacts_manifest: true,
          has_slide_rectangles_manifest: true,
          has_slide_notes: true,
          has_video_slides_markdown: true,
          has_subtitle_page_map: true,
        },
        manifest_outputs: files.filter((file) =>
          ["final_deliverables_manifest", "published_deliverable_manifest", "published_version_history", "extraction_artifacts_manifest"].includes(file.artifact_kind),
        ),
        final_outputs: files.filter((file) => ["pptx", "video_slides_markdown"].includes(file.artifact_kind)),
        review_outputs: files.filter((file) =>
          ["slide_rectangles_manifest", "slide_notes"].includes(file.artifact_kind),
        ),
        evidence_outputs: files.filter((file) => file.artifact_kind === "subtitle_page_map"),
      },
      null,
      2,
    ),
  );

  fs.writeFileSync(
    path.join(artifactsDir, "slide_rectangles_manifest.json"),
    JSON.stringify(
      {
        status: "promoted_full_frame_fallback",
        rectangle_extraction_status: "promoted_full_frame_fallback",
        rectangle_extraction_mode: "full_frame_fallback",
        promoted_rectangle_count: 1,
        dedupe_status: "selected_keep_list_order_deduped",
        rectangles: [
          {
            slide_number: 1,
            candidate_index: 2,
            source_frame: "frame_000002.jpg",
            rectangle_source: "raw_frame_full_frame_fallback",
            rectangle_extraction_status: "promoted_full_frame_fallback",
            rectangle_extraction_mode: "full_frame_fallback",
            crop_box: { unit: "relative", x: 0, y: 0, width: 1, height: 1 },
            review_required: true,
          },
        ],
      },
      null,
      2,
    ),
  );

  fs.writeFileSync(
    path.join(artifactsDir, "published_deliverable_manifest.json"),
    JSON.stringify(
      {
        manifest_type: "v3.video_ppt_published_deliverable.v1",
        status: "published_version_ready",
        lifecycle_state: "published_version_ready",
        published: true,
        immutable_version: true,
        version_no: 1,
        deliverable_status: {
          state: "final_pptx_ready",
          has_pptx: true,
          has_final_deliverables_manifest: true,
          has_published_deliverable_manifest: true,
          has_published_version_history: true,
          has_extraction_artifacts_manifest: true,
          has_slide_rectangles_manifest: true,
          has_slide_notes: true,
          has_video_slides_markdown: true,
          has_subtitle_page_map: true,
        },
        published_files: files,
      },
      null,
      2,
    ),
  );

  fs.writeFileSync(
    path.join(artifactsDir, "published_version_history.json"),
    JSON.stringify(
      {
        manifest_type: "v3.video_ppt_published_version_history.v1",
        status: "history_ready",
        history_scope: "generated_artifact_workspace",
        durable_history_status: "pending_storage_promotion",
        latest_version_no: 1,
        latest_version_label: "v1",
        version_count: 1,
        versions: [
          {
            version_no: 1,
            version_label: "v1",
            lifecycle_state: "published_version_ready",
            published: true,
            immutable_version: true,
            published_manifest_file_name: "published_deliverable_manifest.json",
            file_count: files.length,
            artifact_kinds: files.map((file) => file.artifact_kind),
            published_files: files,
          },
        ],
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
    published_deliverable_manifest: "published_deliverable_manifest.json",
    published_version_history: "published_version_history.json",
    extraction_artifacts_manifest: "extraction_artifacts_manifest.json",
    slide_rectangles_manifest: "slide_rectangles_manifest.json",
    slide_notes: "slide_notes.md",
    video_slides_markdown: "video_slides.md",
    subtitle_page_map: "subtitle_page_map.json",
  }[kind];
}

function minimalPptxFixtureBytes() {
  return minimalZipBytes([
    "[Content_Types].xml",
    "_rels/.rels",
    "ppt/presentation.xml",
    "ppt/_rels/presentation.xml.rels",
    "ppt/slides/slide1.xml",
    "ppt/slides/_rels/slide1.xml.rels",
    "ppt/notesSlides/notesSlide1.xml",
  ]);
}

function minimalZipBytes(entryNames) {
  const localParts = [];
  const centralParts = [];
  let offset = 0;
  for (const entryName of entryNames) {
    const fileName = Buffer.from(entryName, "utf8");
    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);
    localHeader.writeUInt16LE(0, 6);
    localHeader.writeUInt16LE(0, 8);
    localHeader.writeUInt32LE(0, 10);
    localHeader.writeUInt32LE(0, 14);
    localHeader.writeUInt32LE(0, 18);
    localHeader.writeUInt32LE(0, 22);
    localHeader.writeUInt16LE(fileName.length, 26);
    localHeader.writeUInt16LE(0, 28);
    localParts.push(localHeader, fileName);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt16LE(0, 8);
    centralHeader.writeUInt16LE(0, 10);
    centralHeader.writeUInt32LE(0, 12);
    centralHeader.writeUInt32LE(0, 16);
    centralHeader.writeUInt32LE(0, 20);
    centralHeader.writeUInt32LE(0, 24);
    centralHeader.writeUInt16LE(fileName.length, 28);
    centralHeader.writeUInt16LE(0, 30);
    centralHeader.writeUInt16LE(0, 32);
    centralHeader.writeUInt16LE(0, 34);
    centralHeader.writeUInt16LE(0, 36);
    centralHeader.writeUInt32LE(0, 38);
    centralHeader.writeUInt32LE(offset, 42);
    centralParts.push(centralHeader, fileName);
    offset += localHeader.length + fileName.length;
  }

  const centralDirectory = Buffer.concat(centralParts);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(0, 4);
  end.writeUInt16LE(0, 6);
  end.writeUInt16LE(entryNames.length, 8);
  end.writeUInt16LE(entryNames.length, 10);
  end.writeUInt32LE(centralDirectory.length, 12);
  end.writeUInt32LE(offset, 16);
  end.writeUInt16LE(0, 20);
  return Buffer.concat([...localParts, centralDirectory, end]);
}
'@

$validatorSource = Get-Content -Raw -LiteralPath $validatorPath
($validatorSource + "`n" + $runnerSource) |
    ssh $HostAlias "powershell" "-NoProfile" "-EncodedCommand" $encodedRemoteCommand

if ($LASTEXITCODE -ne 0) {
    throw "jump-host video deliverable smoke failed with exit code $LASTEXITCODE"
}
