#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";

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
    kind: "selected_slides_manifest",
    fileName: "selected_slides_manifest.json",
    statusFlag: "has_selected_slides_manifest",
    group: "review_outputs",
    publishedRequired: false,
  },
  {
    kind: "slide_notes",
    fileName: "slide_notes.md",
    statusFlag: "has_slide_notes",
    group: "review_outputs",
  },
  {
    kind: "video_slides_markdown",
    fileName: "video_slides.md",
    statusFlag: "has_video_slides_markdown",
    group: "final_outputs",
  },
];

const CONDITIONAL_FILES = [
  {
    kind: "subtitle_page_map",
    fileName: "subtitle_page_map.json",
    statusFlag: "has_subtitle_page_map",
    group: "evidence_outputs",
  },
];

const OPTIONAL_FILES = [
  {
    kind: "slide_quality_report",
    fileName: "slide_quality_report.json",
    group: "review_outputs",
  },
];

const LOCAL_PATH_PATTERNS = [
  /(^|[\s"'({\[])[A-Za-z]:[\\/]/,
  /[\\/]Users[\\/]/,
  /[\\/]home[\\/]/,
  /https?:\/\/[^\s<>"']*(?:token|cookie|authorization|bearer|provider_key|secret)[^\s<>"']*/i,
  /(?:token|cookie|authorization|provider_key|secret|password)\s*[:=]/i,
  /bearer\s+[A-Za-z0-9._~+/=-]+/i,
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
const ZIP_LOCAL_FILE_HEADER_SIGNATURE = 0x04034b50;
const ZIP_COMPRESSION_STORED = 0;
const ZIP_COMPRESSION_DEFLATED = 8;
const PPTX_NOTES_XML_PATTERN = /^ppt\/notesSlides\/notesSlide\d+\.xml$/;
const PPTX_SLIDE_XML_PATTERN = /^ppt\/slides\/slide\d+\.xml$/;
const MARKDOWN_SLIDE_HEADING_PATTERN = /^#{2,3}\s+Slide\s+\d+\b/gm;

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
      summary: {},
    };
  }

  const files = [
    ...REQUIRED_FILES.map((file) => ({ ...file, required: true })),
    ...CONDITIONAL_FILES.map((file) => ({ ...file, required: false })),
    ...OPTIONAL_FILES.map((file) => ({ ...file, required: false })),
  ].map((file) => {
    const filePath = path.join(artifactsDir, file.fileName);
    const exists = fs.existsSync(filePath) && fs.statSync(filePath).isFile();
    const size = exists ? fs.statSync(filePath).size : 0;
    if (!exists && file.required) {
      errors.push(issue("missing_required_file", `${file.kind} file is missing`, file.kind));
    } else if (exists && size === 0) {
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
  const selectedSlidesManifest = readJsonFile(
    path.join(artifactsDir, "selected_slides_manifest.json"),
    "selected_slides_manifest",
    errors,
  );
  const slideQualityReport = readJsonFile(
    path.join(artifactsDir, "slide_quality_report.json"),
    "slide_quality_report",
    errors,
  );
  const requiredFiles = [
    ...REQUIRED_FILES,
    ...CONDITIONAL_FILES.filter((file) =>
      conditionalFileIsRequired(file, files, [
        finalManifest,
        extractionManifest,
        publishedManifest,
        publishedVersionHistory,
      ]),
    ),
  ];

  let pptxSlideCount = null;
  const pptx = files.find((file) => file.kind === "pptx");
  if (pptx?.exists && !fileStartsWithZipMagic(pptx.path)) {
    errors.push(issue("pptx_zip_magic_missing", "PPTX does not start with ZIP magic bytes", "pptx"));
  } else if (pptx?.exists) {
    const {
      invalidZip,
      missingEntries,
      notesXmlEntries,
      unreadableNotesXmlEntries,
      slideXmlCount,
    } = checkRequiredPptxEntries(pptx.path);
    if (invalidZip) {
      errors.push(issue("pptx_central_directory_missing", "PPTX ZIP central directory could not be read", "pptx"));
    } else {
      pptxSlideCount = slideXmlCount;
      if (!Number.isInteger(slideXmlCount) || slideXmlCount < 1) {
        errors.push(issue("pptx_slide_count_invalid", "PPTX does not contain any slide XML entries", "pptx"));
      } else if (notesXmlEntries.length !== slideXmlCount) {
        errors.push(issue("pptx_notes_slide_count_mismatch", "PPTX notes XML count does not match slide XML count", "pptx_notes_xml"));
      }
    }
    if (missingEntries.length) {
      errors.push(issue(
        "pptx_required_entry_missing",
        `PPTX is missing required OOXML entries: ${missingEntries.join(", ")}`,
        "pptx",
      ));
    }
    for (const entry of unreadableNotesXmlEntries) {
      errors.push(issue(
        "pptx_notes_xml_read_failed",
        `PPTX notes XML could not be read: ${entry.name}: ${entry.reason}`,
        "pptx_notes_xml",
      ));
    }
    for (const entry of notesXmlEntries) {
      validateRedactedText(entry.text, "pptx_notes_xml", errors);
    }
  }

  if (finalManifest) {
    validateFinalManifest(finalManifest, errors, warnings, requiredFiles, CONDITIONAL_FILES);
  }
  if (extractionManifest) {
    validateExtractionManifest(extractionManifest, errors, requiredFiles);
  }
  if (publishedManifest) {
    validatePublishedManifest(publishedManifest, errors, requiredFiles);
  }
  if (publishedVersionHistory) {
    validatePublishedVersionHistory(publishedVersionHistory, errors, requiredFiles);
  }
  if (subtitlePageMap) {
    validateSubtitlePageMap(subtitlePageMap, errors);
    validateRedactedJson(subtitlePageMap, "subtitle_page_map", errors);
  }
  if (slideRectanglesManifest) {
    validateSlideRectanglesManifest(slideRectanglesManifest, errors);
    validateRedactedJson(slideRectanglesManifest, "slide_rectangles_manifest", errors);
  }
  if (selectedSlidesManifest) {
    validateSelectedSlidesManifest(selectedSlidesManifest, errors);
    validateRedactedJson(selectedSlidesManifest, "selected_slides_manifest", errors);
  }
  if (slideQualityReport) {
    validateSlideQualityReport(slideQualityReport, errors);
    validateRedactedJson(slideQualityReport, "slide_quality_report", errors);
  }
  validateSelectedSlideCounts(selectedSlidesManifest, slideRectanglesManifest, slideQualityReport, errors);
  const slideNotes = files.find((file) => file.kind === "slide_notes");
  if (slideNotes?.exists) {
    validateRedactedTextFile(slideNotes.path, "slide_notes", errors);
  }
  let markdownSlideCount = null;
  const videoSlidesMarkdown = files.find((file) => file.kind === "video_slides_markdown");
  if (videoSlidesMarkdown?.exists) {
    const text = readTextFile(videoSlidesMarkdown.path, "video_slides_markdown", errors);
    if (text !== null) {
      markdownSlideCount = countMarkdownSlideHeadings(text);
      if (markdownSlideCount < 1) {
        errors.push(issue("video_slides_markdown_slide_count_invalid", "video_slides Markdown has no slide headings", "video_slides_markdown"));
      }
      validateRedactedText(text, "video_slides_markdown", errors);
    }
  }
  validateOutputSlideCounts({ pptxSlideCount, markdownSlideCount, selectedSlidesManifest }, errors);

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
    summary: buildValidationSummary({
      finalManifest,
      pptxSlideCount,
      markdownSlideCount,
      selectedSlidesManifest,
      slideRectanglesManifest,
      slideQualityReport,
      subtitlePageMap,
    }),
  };
}

export function redactValidationResultForOutput(result) {
  return {
    ...result,
    outputRedacted: true,
    artifactsDir: "[redacted]",
    artifactsDirRedacted: true,
    errors: (result.errors || []).map(redactIssueForOutput),
    warnings: (result.warnings || []).map(redactIssueForOutput),
    files: (result.files || []).map((file) => ({
      ...file,
      path: "[redacted]",
      pathRedacted: true,
    })),
  };
}

function buildValidationSummary({
  finalManifest,
  pptxSlideCount,
  markdownSlideCount,
  selectedSlidesManifest,
  slideRectanglesManifest,
  slideQualityReport,
  subtitlePageMap,
}) {
  return {
    frame_count: numberOrNull(finalManifest?.frame_extraction?.frame_count),
    selected_count: numberOrNull(selectedSlidesManifest?.selected_count),
    requested_selected_count: numberOrNull(selectedSlidesManifest?.requested_selected_count),
    pptx_slide_count: numberOrNull(pptxSlideCount),
    markdown_slide_count: numberOrNull(markdownSlideCount),
    slide_rectangle_count: numberOrNull(slideRectanglesManifest?.promoted_rectangle_count),
    quality_slide_count: numberOrNull(slideQualityReport?.slide_count),
    subtitle_page_count: numberOrNull(subtitlePageMap?.page_count),
  };
}

function numberOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function conditionalFileIsRequired(file, files, manifests) {
  const generatedFile = files.find((candidate) => candidate.kind === file.kind);
  if (generatedFile?.exists) {
    return true;
  }
  return manifests.some((manifest) =>
    manifestStatusFlagIsTrue(manifest, file)
      || manifestReferencesArtifactKind(manifest, file.kind),
  );
}

function manifestStatusFlagIsTrue(manifest, file) {
  return manifest?.deliverable_status?.[file.statusFlag] === true;
}

function manifestReferencesArtifactKind(value, artifactKind) {
  if (Array.isArray(value)) {
    return value.some((item) => manifestReferencesArtifactKind(item, artifactKind));
  }
  if (!value || typeof value !== "object") {
    return false;
  }
  if (value.artifact_kind === artifactKind) {
    return true;
  }
  if (Array.isArray(value.artifact_kinds) && value.artifact_kinds.includes(artifactKind)) {
    return true;
  }
  if (Array.isArray(value.required_file_kinds) && value.required_file_kinds.includes(artifactKind)) {
    return true;
  }
  return Object.values(value).some((item) => manifestReferencesArtifactKind(item, artifactKind));
}

function validateFinalManifest(manifest, errors, warnings, requiredFiles, conditionalFiles) {
  const status = manifest.deliverable_status || {};
  for (const file of requiredFiles) {
    if (status[file.statusFlag] !== true) {
      errors.push(issue("deliverable_status_missing_flag", `final manifest ${file.statusFlag} is not true`, file.kind));
    }
  }
  for (const file of conditionalFiles) {
    if (!requiredFiles.some((requiredFile) => requiredFile.kind === file.kind)
      && Object.hasOwn(status, file.statusFlag)
      && status[file.statusFlag] !== false) {
      errors.push(issue("conditional_status_flag_invalid", `final manifest ${file.statusFlag} must be false when ${file.kind} is absent`, file.kind));
    }
  }
  const state = status.state || manifest.status || "";
  if (state !== "final_pptx_ready") {
    warnings.push(issue("deliverable_state_not_final", `deliverable state is ${state || "missing"}`));
  }
  for (const file of requiredFiles) {
    const entry = findFileEntry(manifest[file.group], file);
    if (!entry) {
      errors.push(issue("final_manifest_group_missing_file", `${file.group} does not include ${file.kind} ${file.fileName}`, file.kind));
    } else {
      validatePublicManifestFileEntry(entry, file, "final_deliverables_manifest", errors);
    }
  }
  validateRedactedJson(manifest, "final_deliverables_manifest", errors);
}

function validateExtractionManifest(manifest, errors, requiredFiles) {
  const files = Array.isArray(manifest.files) ? manifest.files : [];
  for (const file of requiredFiles) {
    const entry = findFileEntry(files, file);
    if (!entry) {
      errors.push(issue("extraction_manifest_missing_file", `extraction manifest does not include ${file.kind} ${file.fileName}`, file.kind));
    } else {
      validatePublicManifestFileEntry(entry, file, "extraction_artifacts_manifest", errors);
    }
  }
  validateRedactedJson(manifest, "extraction_artifacts_manifest", errors);
}

function validatePublishedManifest(manifest, errors, requiredFiles) {
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
  for (const file of filesRequiredForPublishedSurface(requiredFiles)) {
    const entry = findFileEntry(publishedFiles, file);
    if (!entry) {
      errors.push(issue("published_manifest_missing_file", `published manifest does not include ${file.kind} ${file.fileName}`, file.kind));
    } else {
      validatePublicManifestFileEntry(entry, file, "published_deliverable_manifest", errors);
    }
  }
  validateRedactedJson(manifest, "published_deliverable_manifest", errors);
}

function validatePublishedVersionHistory(manifest, errors, requiredFiles) {
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
    for (const file of filesRequiredForPublishedSurface(requiredFiles)) {
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

function filesRequiredForPublishedSurface(requiredFiles) {
  return requiredFiles.filter((file) => file.publishedRequired !== false);
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
    "foreground_component_v1",
    "bright_canvas_v1",
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

function validateSelectedSlidesManifest(manifest, errors) {
  const selectedCandidates = Array.isArray(manifest.selected_candidates) ? manifest.selected_candidates : [];
  const rejectedDuplicateCandidates = Array.isArray(manifest.rejected_duplicate_candidates)
    ? manifest.rejected_duplicate_candidates
    : [];
  const selectedCandidateIndices = Array.isArray(manifest.selected_candidate_indices)
    ? manifest.selected_candidate_indices
    : [];
  const requestedSelectedCandidateIndices = Array.isArray(manifest.requested_selected_candidate_indices)
    ? manifest.requested_selected_candidate_indices
    : [];
  const validDedupeStatuses = new Set([
    "selected_keep_list_order_deduped",
    "exact_frame_content_deduped",
    "visual_similarity_deduped",
    "exact_and_visual_similarity_deduped",
  ]);
  if (manifest.status !== "ready_for_pptx_writer") {
    errors.push(issue("selected_slides_not_ready", "selected slides manifest is not ready for the PPTX writer", "selected_slides_manifest"));
  }
  if (!validDedupeStatuses.has(manifest.dedupe_status)) {
    errors.push(issue("selected_slides_dedupe_status_invalid", "selected slides dedupe status is invalid", "selected_slides_manifest"));
  }
  if (
    !Number.isInteger(manifest.selected_count)
    || manifest.selected_count < 1
    || manifest.selected_count !== selectedCandidates.length
    || manifest.selected_count !== selectedCandidateIndices.length
  ) {
    errors.push(issue("selected_slides_count_invalid", "selected_count must match selected candidates and final candidate indices", "selected_slides_manifest"));
  }
  if (
    !Number.isInteger(manifest.requested_selected_count)
    || manifest.requested_selected_count < manifest.selected_count
    || manifest.requested_selected_count !== requestedSelectedCandidateIndices.length
  ) {
    errors.push(issue("selected_slides_requested_count_invalid", "requested_selected_count must match requested indices and be at least selected_count", "selected_slides_manifest"));
  }
  if (!isPositiveIntegerArray(selectedCandidateIndices) || !isPositiveIntegerArray(requestedSelectedCandidateIndices)) {
    errors.push(issue("selected_slides_indices_invalid", "selected slide candidate indices must be positive integers", "selected_slides_manifest"));
  }
  if (!selectedCandidateIndices.every((index) => requestedSelectedCandidateIndices.includes(index))) {
    errors.push(issue("selected_slides_indices_invalid", "final selected indices must be a subset of requested indices", "selected_slides_manifest"));
  }
  if (!selectedCandidates.every(isValidSelectedSlideCandidate)) {
    errors.push(issue("selected_slides_candidate_invalid", "selected candidate rows are invalid", "selected_slides_manifest"));
  }
  validateSelectedSlidesDedupeCounts(manifest, rejectedDuplicateCandidates, errors);
}

function validateSelectedSlidesDedupeCounts(manifest, rejectedDuplicateCandidates, errors) {
  for (const key of [
    "deduped_candidate_count",
    "exact_duplicate_count",
    "visual_duplicate_count",
    "visual_shape_duplicate_count",
  ]) {
    if (!Number.isInteger(manifest[key]) || manifest[key] < 0) {
      errors.push(issue("selected_slides_dedupe_count_invalid", `selected slides ${key} is invalid`, "selected_slides_manifest"));
      return;
    }
  }
  if (manifest.deduped_candidate_count !== rejectedDuplicateCandidates.length) {
    errors.push(issue("selected_slides_dedupe_count_invalid", "deduped candidate count does not match rejected duplicate rows", "selected_slides_manifest"));
    return;
  }
  if (manifest.visual_shape_duplicate_count > manifest.visual_duplicate_count) {
    errors.push(issue("selected_slides_dedupe_count_invalid", "visual shape duplicate count cannot exceed visual duplicate count", "selected_slides_manifest"));
    return;
  }
  if (manifest.exact_duplicate_count + manifest.visual_duplicate_count !== manifest.deduped_candidate_count) {
    errors.push(issue("selected_slides_dedupe_count_invalid", "exact plus visual duplicate counts must match deduped candidate count", "selected_slides_manifest"));
  }
}

function validateSelectedSlideCounts(selectedSlidesManifest, slideRectanglesManifest, slideQualityReport, errors) {
  if (!selectedSlidesManifest) {
    return;
  }
  const selectedCount = selectedSlidesManifest.selected_count;
  if (!Number.isInteger(selectedCount) || selectedCount < 1) {
    return;
  }
  if (
    slideRectanglesManifest
    && Number.isInteger(slideRectanglesManifest.promoted_rectangle_count)
    && slideRectanglesManifest.promoted_rectangle_count !== selectedCount
  ) {
    errors.push(issue("selected_slides_cross_count_mismatch", "selected_count does not match slide rectangle count", "selected_slides_manifest"));
  }
  if (
    slideQualityReport
    && Number.isInteger(slideQualityReport.slide_count)
    && slideQualityReport.slide_count !== selectedCount
  ) {
    errors.push(issue("selected_slides_cross_count_mismatch", "selected_count does not match slide quality report count", "selected_slides_manifest"));
  }
}

function validateOutputSlideCounts({ pptxSlideCount, markdownSlideCount, selectedSlidesManifest }, errors) {
  const selectedCount = selectedSlidesManifest?.selected_count;
  if (!Number.isInteger(selectedCount) || selectedCount < 1) {
    return;
  }
  if (Number.isInteger(pptxSlideCount) && pptxSlideCount !== selectedCount) {
    errors.push(issue("output_slide_count_mismatch", "PPTX slide count does not match selected_count", "pptx"));
  }
  if (Number.isInteger(markdownSlideCount) && markdownSlideCount !== selectedCount) {
    errors.push(issue("output_slide_count_mismatch", "Markdown slide count does not match selected_count", "video_slides_markdown"));
  }
}

function isPositiveIntegerArray(values) {
  return values.every((value) => Number.isInteger(value) && value >= 1);
}

function isValidSelectedSlideCandidate(candidate) {
  return Number.isInteger(candidate?.candidate_index)
    && candidate.candidate_index >= 1
    && typeof candidate?.file_name === "string"
    && candidate.file_name.length > 0
    && typeof candidate?.timestamp_label === "string"
    && candidate.timestamp_label.length > 0
    && Number.isFinite(candidate?.timestamp_seconds)
    && candidate.selection_status === "selected"
    && typeof candidate?.slide_rectangle === "object"
    && candidate.slide_rectangle !== null;
}

function validateSlideQualityReport(report, errors) {
  const slides = Array.isArray(report.slides) ? report.slides : [];
  const riskFlags = Array.isArray(report.risk_flags) ? report.risk_flags : [];
  if (report.schema !== "v3.video_ppt_slide_quality_report.v1") {
    errors.push(issue("slide_quality_report_schema_invalid", "slide quality report schema is invalid", "slide_quality_report"));
  }
  if (!["waiting_for_selection", "review_required", "review_ready"].includes(report.status)) {
    errors.push(issue("slide_quality_report_status_invalid", "slide quality report status is invalid", "slide_quality_report"));
  }
  if (!isScore(report.quality_score)) {
    errors.push(issue("slide_quality_report_score_invalid", "slide quality report quality_score is invalid", "slide_quality_report"));
  }
  if (!Number.isInteger(report.slide_count) || report.slide_count !== slides.length) {
    errors.push(issue("slide_quality_report_slide_count_invalid", "slide quality report slide_count does not match slides", "slide_quality_report"));
  }
  if (!Number.isInteger(report.risk_count) || report.risk_count !== riskFlags.length) {
    errors.push(issue("slide_quality_report_risk_count_invalid", "slide quality report risk_count does not match risk_flags", "slide_quality_report"));
  }
  if (report.status !== "waiting_for_selection" && slides.length < 1) {
    errors.push(issue("slide_quality_report_slides_missing", "slide quality report has no slide rows", "slide_quality_report"));
  }
  if (!slides.every(isValidSlideQualityRow)) {
    errors.push(issue("slide_quality_report_slide_row_invalid", "slide quality report slide rows are invalid", "slide_quality_report"));
  }
  const summary = report.summary || {};
  for (const key of [
    "full_frame_fallback_count",
    "detector_crop_count",
    "review_required_count",
    "subtitle_mapped_count",
    "subtitle_missing_count",
    "ocr_mapped_count",
    "ocr_missing_count",
    "deduped_candidate_count",
  ]) {
    if (!Number.isInteger(summary[key]) || summary[key] < 0) {
      errors.push(issue("slide_quality_report_summary_invalid", `slide quality report summary ${key} is invalid`, "slide_quality_report"));
      break;
    }
  }
  validateSlideQualityRiskFlags(report, summary, riskFlags, errors);
  validateOptionalSlideQualitySharpness(report, slides, summary, errors);
  validateOptionalSlideQualityReadability(report, slides, summary, errors);
}

function validateSlideQualityRiskFlags(report, summary, riskFlags, errors) {
  if (!riskFlags.every(isValidSlideQualityRiskFlag)) {
    errors.push(issue("slide_quality_report_risk_flags_invalid", "slide quality report risk flags are invalid", "slide_quality_report"));
    return;
  }
  const riskCodes = new Set(riskFlags.map((flag) => flag.code));
  if (
    Object.hasOwn(summary, "single_slide_output")
    && typeof summary.single_slide_output !== "boolean"
  ) {
    errors.push(issue("slide_quality_report_risk_flags_invalid", "slide quality report single-slide summary is invalid", "slide_quality_report"));
    return;
  }
  if (
    summary.single_slide_output === true
    && (report.slide_count !== 1 || !riskCodes.has("single_slide_output_review_required"))
  ) {
    errors.push(issue("slide_quality_report_risk_flags_invalid", "single-slide output must include the single-slide review risk", "slide_quality_report"));
    return;
  }
  if (
    riskCodes.has("single_slide_output_review_required")
    && summary.single_slide_output !== true
  ) {
    errors.push(issue("slide_quality_report_risk_flags_invalid", "single-slide review risk must match summary.single_slide_output", "slide_quality_report"));
  }
}

function isValidSlideQualityRiskFlag(flag) {
  return flag
    && typeof flag === "object"
    && typeof flag.code === "string"
    && flag.code.length > 0
    && ["low", "medium", "high"].includes(flag.severity)
    && Number.isInteger(flag.count)
    && flag.count >= 1
    && typeof flag.review_action === "string"
    && flag.review_action.length > 0;
}

function isValidSlideQualityRow(slide) {
  return Number.isInteger(slide?.slide_number)
    && slide.slide_number >= 1
    && Number.isInteger(slide?.candidate_index)
    && slide.candidate_index >= 1
    && typeof slide?.source_frame === "string"
    && slide.source_frame.length > 0
    && ["low", "medium", "high"].includes(slide.crop_risk)
    && ["low", "medium", "high"].includes(slide.transcript_risk)
    && ["low", "medium", "high"].includes(slide.ocr_risk)
    && Number.isInteger(slide.ocr_snippet_count)
    && slide.ocr_snippet_count >= 0
    && isValidOptionalSlideSharpnessRow(slide)
    && isValidOptionalSlideReadabilityRow(slide)
    && typeof slide.review_required === "boolean"
    && isScore(slide.quality_score);
}

function validateOptionalSlideQualitySharpness(report, slides, summary, errors) {
  const summaryKeys = [
    "sharpness_low_count",
    "sharpness_medium_count",
    "sharpness_high_count",
    "sharpness_unknown_count",
  ];
  const hasSharpnessSummary = summaryKeys.some((key) => Object.hasOwn(summary, key));
  const hasSharpnessRows = slides.some((slide) => (
    Object.hasOwn(slide ?? {}, "sharpness_status")
      || Object.hasOwn(slide ?? {}, "sharpness_score")
      || Object.hasOwn(slide ?? {}, "sharpness_risk")
  ));
  if (!hasSharpnessSummary && !hasSharpnessRows) {
    return;
  }
  if (!summaryKeys.every((key) => Number.isInteger(summary[key]) && summary[key] >= 0)) {
    errors.push(issue("slide_quality_report_sharpness_invalid", "slide quality report sharpness summary is invalid", "slide_quality_report"));
    return;
  }
  if (!slides.every(hasValidCompleteSlideSharpness)) {
    errors.push(issue("slide_quality_report_sharpness_invalid", "slide quality report sharpness rows are invalid", "slide_quality_report"));
    return;
  }
  const counts = {
    low: 0,
    medium: 0,
    high: 0,
    unknown: 0,
  };
  for (const slide of slides) {
    counts[slide.sharpness_risk] += 1;
  }
  if (
    counts.low !== summary.sharpness_low_count
    || counts.medium !== summary.sharpness_medium_count
    || counts.high !== summary.sharpness_high_count
    || counts.unknown !== summary.sharpness_unknown_count
  ) {
    errors.push(issue("slide_quality_report_sharpness_invalid", "slide quality report sharpness counts do not match slide rows", "slide_quality_report"));
  }
}

function isValidOptionalSlideSharpnessRow(slide) {
  const hasAnySharpnessField = Object.hasOwn(slide ?? {}, "sharpness_status")
    || Object.hasOwn(slide ?? {}, "sharpness_score")
    || Object.hasOwn(slide ?? {}, "sharpness_risk");
  if (!hasAnySharpnessField) {
    return true;
  }
  return hasValidCompleteSlideSharpness(slide);
}

function hasValidCompleteSlideSharpness(slide) {
  if (!["measured", "unavailable"].includes(slide?.sharpness_status)) {
    return false;
  }
  if (!["low", "medium", "high", "unknown"].includes(slide?.sharpness_risk)) {
    return false;
  }
  if (slide.sharpness_status === "measured") {
    return isScore(slide.sharpness_score) && slide.sharpness_risk !== "unknown";
  }
  return slide.sharpness_score === null && slide.sharpness_risk === "unknown";
}

function validateOptionalSlideQualityReadability(report, slides, summary, errors) {
  const summaryKeys = [
    "readability_low_count",
    "readability_medium_count",
    "readability_high_count",
    "readability_unknown_count",
  ];
  const hasReadabilitySummary = summaryKeys.some((key) => Object.hasOwn(summary, key));
  const hasReadabilityRows = slides.some((slide) => (
    Object.hasOwn(slide ?? {}, "readability_status")
      || Object.hasOwn(slide ?? {}, "readability_score")
      || Object.hasOwn(slide ?? {}, "readability_risk")
  ));
  if (!hasReadabilitySummary && !hasReadabilityRows) {
    return;
  }
  if (!summaryKeys.every((key) => Number.isInteger(summary[key]) && summary[key] >= 0)) {
    errors.push(issue("slide_quality_report_readability_invalid", "slide quality report readability summary is invalid", "slide_quality_report"));
    return;
  }
  if (!slides.every(hasValidCompleteSlideReadability)) {
    errors.push(issue("slide_quality_report_readability_invalid", "slide quality report readability rows are invalid", "slide_quality_report"));
    return;
  }
  const counts = {
    low: 0,
    medium: 0,
    high: 0,
    unknown: 0,
  };
  for (const slide of slides) {
    counts[slide.readability_risk] += 1;
  }
  if (
    counts.low !== summary.readability_low_count
    || counts.medium !== summary.readability_medium_count
    || counts.high !== summary.readability_high_count
    || counts.unknown !== summary.readability_unknown_count
  ) {
    errors.push(issue("slide_quality_report_readability_invalid", "slide quality report readability counts do not match slide rows", "slide_quality_report"));
  }
}

function isValidOptionalSlideReadabilityRow(slide) {
  const hasAnyReadabilityField = Object.hasOwn(slide ?? {}, "readability_status")
    || Object.hasOwn(slide ?? {}, "readability_score")
    || Object.hasOwn(slide ?? {}, "readability_risk");
  if (!hasAnyReadabilityField) {
    return true;
  }
  return hasValidCompleteSlideReadability(slide);
}

function hasValidCompleteSlideReadability(slide) {
  if (!["measured", "unavailable"].includes(slide?.readability_status)) {
    return false;
  }
  if (!["low", "medium", "high", "unknown"].includes(slide?.readability_risk)) {
    return false;
  }
  if (slide.readability_status === "measured") {
    return isScore(slide.readability_score) && slide.readability_risk !== "unknown";
  }
  return slide.readability_score === null && slide.readability_risk === "unknown";
}

function isScore(value) {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 100;
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
  const text = readTextFile(filePath, kind, errors);
  if (text !== null) {
    validateRedactedText(text, kind, errors);
  }
}

function readTextFile(filePath, kind, errors) {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch (error) {
    errors.push(issue("text_file_read_failed", `${kind} text read failed: ${error.message}`, kind));
    return null;
  }
}

function countMarkdownSlideHeadings(text) {
  return (text.match(MARKDOWN_SLIDE_HEADING_PATTERN) || []).length;
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
  const zip = readZipCentralDirectory(filePath);
  if (!zip) {
    return {
      invalidZip: true,
      missingEntries: REQUIRED_PPTX_ENTRIES,
      notesXmlEntries: [],
      unreadableNotesXmlEntries: [],
      slideXmlCount: 0,
    };
  }
  const names = new Set(zip.entries.map((entry) => entry.name));
  const slideXmlCount = zip.entries.filter((candidate) => PPTX_SLIDE_XML_PATTERN.test(candidate.name)).length;
  const notesXmlEntries = [];
  const unreadableNotesXmlEntries = [];
  for (const entry of zip.entries.filter((candidate) => PPTX_NOTES_XML_PATTERN.test(candidate.name))) {
    const content = readZipEntryContent(zip.bytes, entry);
    if (content.error) {
      unreadableNotesXmlEntries.push({ name: entry.name, reason: content.error });
    } else {
      notesXmlEntries.push({ name: entry.name, text: content.bytes.toString("utf8") });
    }
  }
  return {
    invalidZip: false,
    missingEntries: REQUIRED_PPTX_ENTRIES.filter((entry) => !names.has(entry)),
    notesXmlEntries,
    unreadableNotesXmlEntries,
    slideXmlCount,
  };
}

function readZipCentralDirectory(filePath) {
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

  const entries = [];
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
    entries.push({
      name: bytes.toString("utf8", fileNameStart, fileNameEnd),
      compressionMethod: bytes.readUInt16LE(offset + 10),
      compressedSize: bytes.readUInt32LE(offset + 20),
      uncompressedSize: bytes.readUInt32LE(offset + 24),
      localHeaderOffset: bytes.readUInt32LE(offset + 42),
    });
    offset = fileNameEnd + extraFieldLength + fileCommentLength;
  }
  return { bytes, entries };
}

function readZipEntryContent(bytes, entry) {
  if (
    entry.localHeaderOffset === 0xffffffff
    || entry.compressedSize === 0xffffffff
    || entry.uncompressedSize === 0xffffffff
  ) {
    return { error: "zip64 entries are not supported by this validator" };
  }
  const headerOffset = entry.localHeaderOffset;
  if (headerOffset + 30 > bytes.length || bytes.readUInt32LE(headerOffset) !== ZIP_LOCAL_FILE_HEADER_SIGNATURE) {
    return { error: "local file header is invalid" };
  }
  const fileNameLength = bytes.readUInt16LE(headerOffset + 26);
  const extraFieldLength = bytes.readUInt16LE(headerOffset + 28);
  const contentStart = headerOffset + 30 + fileNameLength + extraFieldLength;
  const contentEnd = contentStart + entry.compressedSize;
  if (contentEnd > bytes.length) {
    return { error: "entry content extends past the ZIP boundary" };
  }
  const compressed = bytes.subarray(contentStart, contentEnd);
  try {
    if (entry.compressionMethod === ZIP_COMPRESSION_STORED) {
      return { bytes: compressed };
    }
    if (entry.compressionMethod === ZIP_COMPRESSION_DEFLATED) {
      return { bytes: zlib.inflateRawSync(compressed) };
    }
    return { error: `unsupported compression method ${entry.compressionMethod}` };
  } catch (error) {
    return { error: error.message };
  }
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

function redactIssueForOutput(item) {
  if (!item || typeof item !== "object") {
    return item;
  }
  return {
    ...item,
    message: typeof item.message === "string"
      ? redactUnsafeSharedText(item.message)
      : item.message,
  };
}

function redactUnsafeSharedText(text) {
  return text
    .replace(/(^|[\s"'({\[])[A-Za-z]:[\\/][^\s"',)}\]]+/g, "$1[redacted]")
    .replace(/(^|[\s"'({\[])(?:\/Users|\/home|\/tmp|\/var|\/private|\/Volumes)\/[^\s"',)}\]]+/g, "$1[redacted]")
    .replace(/(^|[\s"'({\[])(?:\.\/)?target\/[^\s"',)}\]]*generated_artifacts[^\s"',)}\]]*/g, "$1[redacted]")
    .replace(/https?:\/\/[^\s<>"']*(?:token|cookie|authorization|bearer|provider_key|secret)[^\s<>"']*/gi, "[redacted]")
    .replace(/bearer\s+[A-Za-z0-9._~+/=-]+/gi, "bearer [redacted]");
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
    console.log(JSON.stringify(redactValidationResultForOutput(result), null, 2));
  } else {
    printHumanResult(result);
  }
  process.exitCode = result.ok ? 0 : 1;
}
