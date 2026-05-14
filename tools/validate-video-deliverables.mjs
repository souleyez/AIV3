#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REQUIRED_FILES = [
  {
    kind: "pptx",
    fileName: "video_slides_screenshot_based.pptx",
    statusFlag: "has_pptx",
    group: "final_outputs",
  },
  {
    kind: "final_deliverables_manifest",
    fileName: "final_deliverables_manifest.json",
    statusFlag: "has_final_deliverables_manifest",
    group: "manifest_outputs",
  },
  {
    kind: "published_deliverable_manifest",
    fileName: "published_deliverable_manifest.json",
    statusFlag: "has_published_deliverable_manifest",
    group: "manifest_outputs",
  },
  {
    kind: "published_version_history",
    fileName: "published_version_history.json",
    statusFlag: "has_published_version_history",
    group: "manifest_outputs",
  },
  {
    kind: "extraction_artifacts_manifest",
    fileName: "extraction_artifacts_manifest.json",
    statusFlag: "has_extraction_artifacts_manifest",
    group: "manifest_outputs",
  },
  {
    kind: "slide_rectangles_manifest",
    fileName: "slide_rectangles_manifest.json",
    statusFlag: "has_slide_rectangles_manifest",
    group: "review_outputs",
  },
  {
    kind: "slide_notes",
    fileName: "slide_notes.md",
    statusFlag: "has_slide_notes",
    group: "review_outputs",
  },
  {
    kind: "subtitle_page_map",
    fileName: "subtitle_page_map.json",
    statusFlag: "has_subtitle_page_map",
    group: "evidence_outputs",
  },
];

const LOCAL_PATH_PATTERNS = [
  /[A-Za-z]:[\\/]/,
  /[\\/]Users[\\/]/,
  /[\\/]home[\\/]/,
  /token=/i,
];

const REQUIRED_PPTX_ENTRIES = [
  "[Content_Types].xml",
  "_rels/.rels",
  "ppt/presentation.xml",
  "ppt/_rels/presentation.xml.rels",
  "ppt/slides/slide1.xml",
  "ppt/slides/_rels/slide1.xml.rels",
  "ppt/notesSlides/notesSlide1.xml",
];

const ZIP_EOCD_SIGNATURE = 0x06054b50;
const ZIP_CENTRAL_FILE_HEADER_SIGNATURE = 0x02014b50;

export function resolveVideoDeliverablesPath(inputPath) {
  const absolutePath = path.resolve(inputPath || ".");
  const stat = fs.statSync(absolutePath);
  if (stat.isFile()) {
    if (path.basename(absolutePath) !== "final_deliverables_manifest.json") {
      throw new Error("file input must be final_deliverables_manifest.json");
    }
    return path.dirname(absolutePath);
  }
  const generatedArtifactsDir = path.join(absolutePath, "generated_artifacts");
  if (fs.existsSync(generatedArtifactsDir) && fs.statSync(generatedArtifactsDir).isDirectory()) {
    return generatedArtifactsDir;
  }
  return absolutePath;
}

export function validateVideoDeliverables(inputPath) {
  const errors = [];
  const warnings = [];
  let artifactsDir = "";
  try {
    artifactsDir = resolveVideoDeliverablesPath(inputPath);
  } catch (error) {
    return {
      ok: false,
      artifactsDir: path.resolve(inputPath || "."),
      errors: [issue("invalid_input", error.message)],
      warnings,
      files: [],
    };
  }

  const files = REQUIRED_FILES.map((file) => {
    const filePath = path.join(artifactsDir, file.fileName);
    const exists = fs.existsSync(filePath) && fs.statSync(filePath).isFile();
    const size = exists ? fs.statSync(filePath).size : 0;
    if (!exists) {
      errors.push(issue("missing_required_file", `${file.kind} file is missing`, file.kind));
    } else if (size === 0) {
      errors.push(issue("empty_required_file", `${file.kind} file is empty`, file.kind));
    }
    return {
      ...file,
      path: filePath,
      exists,
      size,
    };
  });

  const finalManifest = readJsonFile(
    path.join(artifactsDir, "final_deliverables_manifest.json"),
    "final_deliverables_manifest",
    errors,
  );
  const extractionManifest = readJsonFile(
    path.join(artifactsDir, "extraction_artifacts_manifest.json"),
    "extraction_artifacts_manifest",
    errors,
  );
  const publishedManifest = readJsonFile(
    path.join(artifactsDir, "published_deliverable_manifest.json"),
    "published_deliverable_manifest",
    errors,
  );
  const publishedVersionHistory = readJsonFile(
    path.join(artifactsDir, "published_version_history.json"),
    "published_version_history",
    errors,
  );
  const subtitlePageMap = readJsonFile(
    path.join(artifactsDir, "subtitle_page_map.json"),
    "subtitle_page_map",
    errors,
  );
  const slideRectanglesManifest = readJsonFile(
    path.join(artifactsDir, "slide_rectangles_manifest.json"),
    "slide_rectangles_manifest",
    errors,
  );

  const pptx = files.find((file) => file.kind === "pptx");
  if (pptx?.exists && !fileStartsWithZipMagic(pptx.path)) {
    errors.push(issue("pptx_zip_magic_missing", "PPTX does not start with ZIP magic bytes", "pptx"));
  } else if (pptx?.exists) {
    const { invalidZip, missingEntries } = checkRequiredPptxEntries(pptx.path);
    if (invalidZip) {
      errors.push(issue("pptx_central_directory_missing", "PPTX ZIP central directory could not be read", "pptx"));
    }
    if (missingEntries.length) {
      errors.push(issue(
        "pptx_required_entry_missing",
        `PPTX is missing required OOXML entries: ${missingEntries.join(", ")}`,
        "pptx",
      ));
    }
  }

  if (finalManifest) {
    validateFinalManifest(finalManifest, errors, warnings);
  }
  if (extractionManifest) {
    validateExtractionManifest(extractionManifest, errors);
  }
  if (publishedManifest) {
    validatePublishedManifest(publishedManifest, errors);
  }
  if (publishedVersionHistory) {
    validatePublishedVersionHistory(publishedVersionHistory, errors);
  }
  if (subtitlePageMap) {
    validateSubtitlePageMap(subtitlePageMap, errors);
    validateRedactedJson(subtitlePageMap, "subtitle_page_map", errors);
  }
  if (slideRectanglesManifest) {
    validateSlideRectanglesManifest(slideRectanglesManifest, errors);
    validateRedactedJson(slideRectanglesManifest, "slide_rectangles_manifest", errors);
  }
  const slideNotes = files.find((file) => file.kind === "slide_notes");
  if (slideNotes?.exists) {
    validateRedactedTextFile(slideNotes.path, "slide_notes", errors);
  }

  return {
    ok: errors.length === 0,
    artifactsDir,
    errors,
    warnings,
    files: files.map(({ kind, fileName, path: filePath, exists, size }) => ({
      kind,
      fileName,
      path: filePath,
      exists,
      size,
    })),
  };
}

function validateFinalManifest(manifest, errors, warnings) {
  const status = manifest.deliverable_status || {};
  for (const file of REQUIRED_FILES) {
    if (status[file.statusFlag] !== true) {
      errors.push(issue("deliverable_status_missing_flag", `final manifest ${file.statusFlag} is not true`, file.kind));
    }
  }
  const state = status.state || manifest.status || "";
  if (state !== "final_pptx_ready") {
    warnings.push(issue("deliverable_state_not_final", `deliverable state is ${state || "missing"}`));
  }
  for (const file of REQUIRED_FILES) {
    const entry = findFileEntry(manifest[file.group], file);
    if (!entry) {
      errors.push(issue("final_manifest_group_missing_file", `${file.group} does not include ${file.kind} ${file.fileName}`, file.kind));
    } else {
      validatePublicManifestFileEntry(entry, file, "final_deliverables_manifest", errors);
    }
  }
  validateRedactedJson(manifest, "final_deliverables_manifest", errors);
}

function validateExtractionManifest(manifest, errors) {
  const files = Array.isArray(manifest.files) ? manifest.files : [];
  for (const file of REQUIRED_FILES) {
    const entry = findFileEntry(files, file);
    if (!entry) {
      errors.push(issue("extraction_manifest_missing_file", `extraction manifest does not include ${file.kind} ${file.fileName}`, file.kind));
    } else {
      validatePublicManifestFileEntry(entry, file, "extraction_artifacts_manifest", errors);
    }
  }
  validateRedactedJson(manifest, "extraction_artifacts_manifest", errors);
}

function validatePublishedManifest(manifest, errors) {
  if (manifest.manifest_type !== "v3.video_ppt_published_deliverable.v1") {
    errors.push(issue("published_manifest_type_invalid", "published manifest type is invalid", "published_deliverable_manifest"));
  }
  if (manifest.lifecycle_state !== "published_version_ready" || manifest.status !== "published_version_ready") {
    errors.push(issue("published_manifest_not_ready", "published manifest lifecycle state is not published_version_ready", "published_deliverable_manifest"));
  }
  if (manifest.immutable_version !== true || manifest.published !== true || manifest.version_no !== 1) {
    errors.push(issue("published_manifest_version_invalid", "published manifest immutable version metadata is invalid", "published_deliverable_manifest"));
  }
  const publishedFiles = Array.isArray(manifest.published_files) ? manifest.published_files : [];
  for (const file of REQUIRED_FILES) {
    const entry = findFileEntry(publishedFiles, file);
    if (!entry) {
      errors.push(issue("published_manifest_missing_file", `published manifest does not include ${file.kind} ${file.fileName}`, file.kind));
    } else {
      validatePublicManifestFileEntry(entry, file, "published_deliverable_manifest", errors);
    }
  }
  validateRedactedJson(manifest, "published_deliverable_manifest", errors);
}

function validatePublishedVersionHistory(manifest, errors) {
  if (manifest.manifest_type !== "v3.video_ppt_published_version_history.v1") {
    errors.push(issue("published_history_type_invalid", "published version history manifest type is invalid", "published_version_history"));
  }
  if (manifest.status !== "history_ready") {
    errors.push(issue("published_history_not_ready", "published version history status is not history_ready", "published_version_history"));
  }
  if (manifest.history_scope !== "generated_artifact_workspace") {
    errors.push(issue("published_history_scope_invalid", "published version history scope is invalid", "published_version_history"));
  }
  if (manifest.latest_version_no !== 1 || !Number.isInteger(manifest.version_count) || manifest.version_count < 1) {
    errors.push(issue("published_history_version_invalid", "published version history latest version metadata is invalid", "published_version_history"));
  }
  const versions = Array.isArray(manifest.versions) ? manifest.versions : [];
  const latest = versions.find((version) => version?.version_no === 1);
  if (!latest || latest.immutable_version !== true || latest.published !== true) {
    errors.push(issue("published_history_latest_missing", "published version history does not include immutable published v1", "published_version_history"));
  } else {
    if (latest.published_manifest_file_name !== "published_deliverable_manifest.json") {
      errors.push(issue("published_history_manifest_pointer_invalid", "published version history does not point to the published manifest", "published_version_history"));
    }
    const publishedFiles = Array.isArray(latest.published_files) ? latest.published_files : [];
    for (const file of REQUIRED_FILES) {
      const entry = findFileEntry(publishedFiles, file);
      if (!entry) {
        errors.push(issue("published_history_missing_file", `published version history does not include ${file.kind} ${file.fileName}`, file.kind));
      } else {
        validatePublicManifestFileEntry(entry, file, "published_version_history", errors);
      }
    }
  }
  validateRedactedJson(manifest, "published_version_history", errors);
}

function validateSubtitlePageMap(map, errors) {
  const pages = Array.isArray(map.pages) ? map.pages : [];
  if (map.status !== "mapped") {
    errors.push(issue("subtitle_page_map_not_mapped", "subtitle_page_map status is not mapped", "subtitle_page_map"));
  }
  if (map.assignment_rule !== "pre_page_previous_to_current") {
    errors.push(issue("subtitle_page_map_assignment_rule_invalid", "subtitle_page_map assignment rule is invalid", "subtitle_page_map"));
  }
  if (!Number.isInteger(map.page_count) || map.page_count < 1 || map.page_count !== pages.length) {
    errors.push(issue("subtitle_page_map_page_count_invalid", "subtitle_page_map page_count does not match pages", "subtitle_page_map"));
  }
  if (!pages.some((page) => Array.isArray(page?.transcript_segments) && page.transcript_segments.length > 0)) {
    errors.push(issue("subtitle_page_map_transcript_segments_missing", "subtitle_page_map has no transcript segments", "subtitle_page_map"));
  }
}

function validateSlideRectanglesManifest(manifest, errors) {
  const rectangles = Array.isArray(manifest.rectangles) ? manifest.rectangles : [];
  const validStatuses = new Set([
    "promoted_full_frame_fallback",
    "promoted_detector_crop",
    "mixed_detector_and_full_frame_fallback",
  ]);
  const validModes = new Set([
    "full_frame_fallback",
    "simple_background_contrast_v1",
    "border_background_contrast_v2",
    "edge_projection_v1",
    "mixed_detector_modes",
    "mixed_detector_and_full_frame_fallback",
  ]);
  if (!validStatuses.has(manifest.rectangle_extraction_status)) {
    errors.push(issue("slide_rectangles_not_promoted", "slide rectangle manifest is not promoted", "slide_rectangles_manifest"));
  }
  if (!validModes.has(manifest.rectangle_extraction_mode)) {
    errors.push(issue("slide_rectangles_mode_invalid", "slide rectangle extraction mode is invalid", "slide_rectangles_manifest"));
  }
  if (!Number.isInteger(manifest.promoted_rectangle_count) || manifest.promoted_rectangle_count < 1 || manifest.promoted_rectangle_count !== rectangles.length) {
    errors.push(issue("slide_rectangles_count_invalid", "slide rectangle count does not match rectangles", "slide_rectangles_manifest"));
  }
  if (!rectangles.every(isValidSlideRectangle)) {
    errors.push(issue("slide_rectangles_crop_invalid", "slide rectangles must use a valid relative crop and require review", "slide_rectangles_manifest"));
  }
}

function isValidSlideRectangle(rectangle) {
  const box = rectangle?.crop_box || {};
  if (
    box.unit !== "relative"
    || !isRelativeNumber(box.x)
    || !isRelativeNumber(box.y)
    || !isRelativeNumber(box.width)
    || !isRelativeNumber(box.height)
    || box.width <= 0
    || box.height <= 0
    || box.x + box.width > 1.0001
    || box.y + box.height > 1.0001
    || rectangle.review_required !== true
  ) {
    return false;
  }
  if (rectangle.rectangle_extraction_status === "promoted_full_frame_fallback") {
    return box.x === 0 && box.y === 0 && box.width === 1 && box.height === 1;
  }
  return rectangle.rectangle_extraction_status === "promoted_detector_crop";
}

function isRelativeNumber(value) {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 1;
}

function validateRedactedJson(value, kind, errors) {
  validateRedactedText(JSON.stringify(value), kind, errors);
}

function validateRedactedTextFile(filePath, kind, errors) {
  try {
    validateRedactedText(fs.readFileSync(filePath, "utf8"), kind, errors);
  } catch (error) {
    errors.push(issue("text_file_read_failed", `${kind} text read failed: ${error.message}`, kind));
  }
}

function validateRedactedText(text, kind, errors) {
  for (const pattern of LOCAL_PATH_PATTERNS) {
    if (pattern.test(text)) {
      errors.push(issue("unredacted_local_path_or_token", `${kind} contains an unredacted local path or token-like URL`, kind));
      return;
    }
  }
}

function validatePublicManifestFileEntry(entry, file, manifestKind, errors) {
  if (entry?.path !== "[redacted]" || entry?.path_redacted !== true) {
    errors.push(issue("manifest_file_path_not_redacted", `${manifestKind} ${file.kind} path is not redacted`, file.kind));
  }
}

function findFileEntry(entries, file) {
  return Array.isArray(entries) ? entries.find((entry) => entryMatchesFile(entry, file)) : undefined;
}

function entryMatchesFile(entry, file) {
  return entry?.artifact_kind === file.kind && entry?.file_name === file.fileName;
}

function readJsonFile(filePath, kind, errors) {
  if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
    return null;
  }
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    errors.push(issue("invalid_json", `${kind} JSON parse failed: ${error.message}`, kind));
    return null;
  }
}

function fileStartsWithZipMagic(filePath) {
  const fd = fs.openSync(filePath, "r");
  try {
    const buffer = Buffer.alloc(2);
    const bytesRead = fs.readSync(fd, buffer, 0, 2, 0);
    return bytesRead === 2 && buffer[0] === 0x50 && buffer[1] === 0x4b;
  } finally {
    fs.closeSync(fd);
  }
}

function checkRequiredPptxEntries(filePath) {
  const entryNames = readZipCentralDirectoryEntryNames(filePath);
  if (!entryNames) {
    return {
      invalidZip: true,
      missingEntries: REQUIRED_PPTX_ENTRIES,
    };
  }
  const names = new Set(entryNames);
  return {
    invalidZip: false,
    missingEntries: REQUIRED_PPTX_ENTRIES.filter((entry) => !names.has(entry)),
  };
}

function readZipCentralDirectoryEntryNames(filePath) {
  const bytes = fs.readFileSync(filePath);
  const eocdOffset = findEndOfCentralDirectory(bytes);
  if (eocdOffset < 0 || eocdOffset + 22 > bytes.length) {
    return null;
  }
  const entryCount = bytes.readUInt16LE(eocdOffset + 10);
  const centralDirectorySize = bytes.readUInt32LE(eocdOffset + 12);
  const centralDirectoryOffset = bytes.readUInt32LE(eocdOffset + 16);
  if (centralDirectoryOffset + centralDirectorySize > bytes.length) {
    return null;
  }

  const names = [];
  let offset = centralDirectoryOffset;
  const centralDirectoryEnd = centralDirectoryOffset + centralDirectorySize;
  for (let index = 0; index < entryCount; index += 1) {
    if (offset + 46 > centralDirectoryEnd || bytes.readUInt32LE(offset) !== ZIP_CENTRAL_FILE_HEADER_SIGNATURE) {
      return null;
    }
    const fileNameLength = bytes.readUInt16LE(offset + 28);
    const extraFieldLength = bytes.readUInt16LE(offset + 30);
    const fileCommentLength = bytes.readUInt16LE(offset + 32);
    const fileNameStart = offset + 46;
    const fileNameEnd = fileNameStart + fileNameLength;
    if (fileNameEnd > centralDirectoryEnd) {
      return null;
    }
    names.push(bytes.toString("utf8", fileNameStart, fileNameEnd));
    offset = fileNameEnd + extraFieldLength + fileCommentLength;
  }
  return names;
}

function findEndOfCentralDirectory(bytes) {
  for (let offset = bytes.length - 22; offset >= 0; offset -= 1) {
    if (bytes.readUInt32LE(offset) === ZIP_EOCD_SIGNATURE) {
      return offset;
    }
  }
  return -1;
}

function issue(code, message, kind = "") {
  return { code, message, ...(kind ? { kind } : {}) };
}

function printHumanResult(result) {
  const prefix = result.ok ? "OK" : "FAIL";
  console.log(`${prefix} video deliverables: ${result.artifactsDir}`);
  for (const file of result.files) {
    console.log(`${file.exists ? "ok" : "missing"} ${file.kind} ${file.fileName} ${file.size}b`);
  }
  for (const warning of result.warnings) {
    console.log(`warning ${warning.code}: ${warning.message}`);
  }
  for (const error of result.errors) {
    console.error(`error ${error.code}: ${error.message}`);
  }
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMainModule()) {
  const args = process.argv.slice(2);
  const jsonOutput = args.includes("--json");
  const target = args.find((arg) => arg !== "--json");
  const result = validateVideoDeliverables(target || ".");
  if (jsonOutput) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHumanResult(result);
  }
  process.exitCode = result.ok ? 0 : 1;
}
