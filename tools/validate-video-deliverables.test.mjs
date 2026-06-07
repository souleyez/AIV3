import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { validateVideoDeliverables } from "./validate-video-deliverables.mjs";

test("accepts a complete video deliverables directory", () => {
  const sessionDir = createCompleteDeliverables();
  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, true);
  assert.deepEqual(result.errors, []);
  assert.equal(result.files.length, 11);
  assert.ok(result.files.every((file) => file.exists));
  assert.deepEqual(result.summary, {
    frame_count: null,
    selected_count: 1,
    requested_selected_count: 1,
    pptx_slide_count: 1,
    markdown_slide_count: 1,
    slide_rectangle_count: 1,
    quality_slide_count: 1,
    subtitle_page_count: 1,
  });
});

test("accepts legacy deliverables without optional slide quality report", () => {
  const sessionDir = createCompleteDeliverables();
  fs.unlinkSync(path.join(sessionDir, "generated_artifacts", "slide_quality_report.json"));
  for (const manifestFileName of [
    "final_deliverables_manifest.json",
    "published_deliverable_manifest.json",
    "published_version_history.json",
    "extraction_artifacts_manifest.json",
  ]) {
    const manifestPath = path.join(sessionDir, "generated_artifacts", manifestFileName);
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    removeArtifactKind(manifest, "slide_quality_report");
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
  }

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, true);
  assert.equal(result.files.find((file) => file.kind === "slide_quality_report").exists, false);
  assert.equal(result.summary.selected_count, 1);
  assert.equal(result.summary.pptx_slide_count, 1);
  assert.equal(result.summary.markdown_slide_count, 1);
  assert.equal(result.summary.quality_slide_count, null);
});

test("accepts deliverables without subtitle page map when transcript alignment is unavailable", () => {
  const sessionDir = createCompleteDeliverables();
  fs.unlinkSync(path.join(sessionDir, "generated_artifacts", "subtitle_page_map.json"));
  for (const manifestFileName of [
    "final_deliverables_manifest.json",
    "published_deliverable_manifest.json",
    "published_version_history.json",
    "extraction_artifacts_manifest.json",
  ]) {
    const manifestPath = path.join(sessionDir, "generated_artifacts", manifestFileName);
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    removeArtifactKind(manifest, "subtitle_page_map");
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
  }

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, true);
  assert.deepEqual(result.errors, []);
  assert.equal(result.files.find((file) => file.kind === "subtitle_page_map").exists, false);
});

test("accepts detector-cropped slide rectangles", () => {
  const sessionDir = createCompleteDeliverables();
  const slideRectanglesPath = path.join(sessionDir, "generated_artifacts", "slide_rectangles_manifest.json");
  const slideRectangles = JSON.parse(fs.readFileSync(slideRectanglesPath, "utf8"));
  slideRectangles.status = "promoted_detector_crop";
  slideRectangles.rectangle_extraction_status = "promoted_detector_crop";
  slideRectangles.rectangle_extraction_mode = "edge_projection_v1";
  slideRectangles.rectangles[0].rectangle_source = "raw_frame_edge_projection";
  slideRectangles.rectangles[0].rectangle_extraction_status = "promoted_detector_crop";
  slideRectangles.rectangles[0].rectangle_extraction_mode = "edge_projection_v1";
  slideRectangles.rectangles[0].crop_box = { unit: "relative", x: 0.2, y: 0.125, width: 0.6, height: 0.625 };
  fs.writeFileSync(slideRectanglesPath, JSON.stringify(slideRectangles, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, true);
});

test("accepts selected slides manifest without a published download entry", () => {
  const sessionDir = createCompleteDeliverables();
  const artifactsDir = path.join(sessionDir, "generated_artifacts");
  const publishedManifestPath = path.join(artifactsDir, "published_deliverable_manifest.json");
  const publishedManifest = JSON.parse(fs.readFileSync(publishedManifestPath, "utf8"));
  publishedManifest.published_files = publishedManifest.published_files.filter(
    (file) => file.artifact_kind !== "selected_slides_manifest",
  );
  fs.writeFileSync(publishedManifestPath, JSON.stringify(publishedManifest, null, 2));

  const publishedVersionHistoryPath = path.join(artifactsDir, "published_version_history.json");
  const publishedVersionHistory = JSON.parse(fs.readFileSync(publishedVersionHistoryPath, "utf8"));
  publishedVersionHistory.versions[0].published_files = publishedVersionHistory.versions[0].published_files.filter(
    (file) => file.artifact_kind !== "selected_slides_manifest",
  );
  publishedVersionHistory.versions[0].artifact_kinds = publishedVersionHistory.versions[0].artifact_kinds.filter(
    (kind) => kind !== "selected_slides_manifest",
  );
  fs.writeFileSync(publishedVersionHistoryPath, JSON.stringify(publishedVersionHistory, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, true);
});

test("accepts foreground component and bright canvas detector modes", () => {
  for (const [mode, source] of [
    ["foreground_component_v1", "raw_frame_foreground_component"],
    ["bright_canvas_v1", "raw_frame_bright_canvas"],
  ]) {
    const sessionDir = createCompleteDeliverables();
    const slideRectanglesPath = path.join(sessionDir, "generated_artifacts", "slide_rectangles_manifest.json");
    const slideRectangles = JSON.parse(fs.readFileSync(slideRectanglesPath, "utf8"));
    slideRectangles.status = "promoted_detector_crop";
    slideRectangles.rectangle_extraction_status = "promoted_detector_crop";
    slideRectangles.rectangle_extraction_mode = mode;
    slideRectangles.rectangles[0].rectangle_source = source;
    slideRectangles.rectangles[0].rectangle_extraction_status = "promoted_detector_crop";
    slideRectangles.rectangles[0].rectangle_extraction_mode = mode;
    slideRectangles.rectangles[0].crop_box = { unit: "relative", x: 0.2, y: 0.125, width: 0.6, height: 0.625 };
    fs.writeFileSync(slideRectanglesPath, JSON.stringify(slideRectangles, null, 2));

    const result = validateVideoDeliverables(sessionDir);

    assert.equal(result.ok, true, `${mode} should be accepted`);
  }
});

test("rejects missing review files", () => {
  const sessionDir = createCompleteDeliverables();
  fs.unlinkSync(path.join(sessionDir, "generated_artifacts", "slide_notes.md"));
  fs.unlinkSync(path.join(sessionDir, "generated_artifacts", "slide_rectangles_manifest.json"));
  fs.unlinkSync(path.join(sessionDir, "generated_artifacts", "selected_slides_manifest.json"));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "missing_required_file" && error.kind === "slide_notes"));
  assert.ok(result.errors.some((error) => error.code === "missing_required_file" && error.kind === "slide_rectangles_manifest"));
  assert.ok(result.errors.some((error) => error.code === "missing_required_file" && error.kind === "selected_slides_manifest"));
});

test("rejects inconsistent final manifest status", () => {
  const sessionDir = createCompleteDeliverables();
  const finalManifestPath = path.join(sessionDir, "generated_artifacts", "final_deliverables_manifest.json");
  const finalManifest = JSON.parse(fs.readFileSync(finalManifestPath, "utf8"));
  finalManifest.deliverable_status.has_subtitle_page_map = false;
  fs.writeFileSync(finalManifestPath, JSON.stringify(finalManifest, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "deliverable_status_missing_flag" && error.kind === "subtitle_page_map"));
});

test("rejects manifest entries pointing at unexpected file names", () => {
  const sessionDir = createCompleteDeliverables();
  const finalManifestPath = path.join(sessionDir, "generated_artifacts", "final_deliverables_manifest.json");
  const finalManifest = JSON.parse(fs.readFileSync(finalManifestPath, "utf8"));
  finalManifest.evidence_outputs[0].file_name = "subtitle-page-map-wrong.json";
  fs.writeFileSync(finalManifestPath, JSON.stringify(finalManifest, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "final_manifest_group_missing_file" && error.kind === "subtitle_page_map"));
});

test("rejects extraction manifest entries pointing at unexpected file names", () => {
  const sessionDir = createCompleteDeliverables();
  const extractionManifestPath = path.join(sessionDir, "generated_artifacts", "extraction_artifacts_manifest.json");
  const extractionManifest = JSON.parse(fs.readFileSync(extractionManifestPath, "utf8"));
  extractionManifest.files.find((file) => file.artifact_kind === "slide_notes").file_name = "speaker-notes-wrong.md";
  fs.writeFileSync(extractionManifestPath, JSON.stringify(extractionManifest, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "extraction_manifest_missing_file" && error.kind === "slide_notes"));
});

test("rejects public manifest entries without redacted paths", () => {
  const sessionDir = createCompleteDeliverables();
  const finalManifestPath = path.join(sessionDir, "generated_artifacts", "final_deliverables_manifest.json");
  const finalManifest = JSON.parse(fs.readFileSync(finalManifestPath, "utf8"));
  finalManifest.final_outputs[0].path = "generated_artifacts/video_slides_screenshot_based.pptx";
  finalManifest.final_outputs[0].path_redacted = false;
  fs.writeFileSync(finalManifestPath, JSON.stringify(finalManifest, null, 2));

  const extractionManifestPath = path.join(sessionDir, "generated_artifacts", "extraction_artifacts_manifest.json");
  const extractionManifest = JSON.parse(fs.readFileSync(extractionManifestPath, "utf8"));
  const slideNotes = extractionManifest.files.find((file) => file.artifact_kind === "slide_notes");
  slideNotes.path = "generated_artifacts/slide_notes.md";
  delete slideNotes.path_redacted;
  fs.writeFileSync(extractionManifestPath, JSON.stringify(extractionManifest, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "manifest_file_path_not_redacted" && error.kind === "pptx"));
  assert.ok(result.errors.some((error) => error.code === "manifest_file_path_not_redacted" && error.kind === "slide_notes"));
});

test("rejects malformed pptx containers", () => {
  const sessionDir = createCompleteDeliverables();
  fs.writeFileSync(path.join(sessionDir, "generated_artifacts", "video_slides_screenshot_based.pptx"), "not a pptx");

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "pptx_zip_magic_missing" && error.kind === "pptx"));
});

test("rejects pptx containers without required OOXML entries", () => {
  const sessionDir = createCompleteDeliverables();
  fs.writeFileSync(path.join(sessionDir, "generated_artifacts", "video_slides_screenshot_based.pptx"), Buffer.from([0x50, 0x4b, 0x03, 0x04]));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "pptx_central_directory_missing" && error.kind === "pptx"));
});

test("rejects zip pptx containers missing required OOXML entries", () => {
  const sessionDir = createCompleteDeliverables();
  fs.writeFileSync(path.join(sessionDir, "generated_artifacts", "video_slides_screenshot_based.pptx"), minimalZipBytes(["foo.txt"]));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "pptx_required_entry_missing" && error.kind === "pptx"));
});

test("rejects unredacted token-like text inside pptx speaker notes XML", () => {
  const sessionDir = createCompleteDeliverables();
  fs.writeFileSync(
    path.join(sessionDir, "generated_artifacts", "video_slides_screenshot_based.pptx"),
    minimalPptxFixtureBytes(
      '<p:notes><p:txBody><a:t>OCR evidence https://private.example/video?token=secret</a:t></p:txBody></p:notes>',
    ),
  );

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "unredacted_local_path_or_token" && error.kind === "pptx_notes_xml"));
});

test("rejects output slide counts that do not match selected slides", () => {
  const sessionDir = createCompleteDeliverables();
  const artifactsDir = path.join(sessionDir, "generated_artifacts");
  fs.writeFileSync(
    path.join(artifactsDir, "video_slides_screenshot_based.pptx"),
    minimalPptxFixtureBytes({ slideCount: 2 }),
  );
  fs.writeFileSync(
    path.join(artifactsDir, "video_slides.md"),
    "# Video Slides\n\n## Slide 1\n\n## Slide 2\n",
  );

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "output_slide_count_mismatch" && error.kind === "pptx"));
  assert.ok(result.errors.some((error) => error.code === "output_slide_count_mismatch" && error.kind === "video_slides_markdown"));
});

test("rejects video slides markdown without slide headings", () => {
  const sessionDir = createCompleteDeliverables();
  fs.writeFileSync(
    path.join(sessionDir, "generated_artifacts", "video_slides.md"),
    "# Video Slides\n\nNo slide headings are present.\n",
  );

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "video_slides_markdown_slide_count_invalid"));
  assert.ok(result.errors.some((error) => error.code === "output_slide_count_mismatch" && error.kind === "video_slides_markdown"));
});

test("rejects unredacted local paths in public JSON", () => {
  const sessionDir = createCompleteDeliverables();
  const subtitleMapPath = path.join(sessionDir, "generated_artifacts", "subtitle_page_map.json");
  fs.writeFileSync(
    subtitleMapPath,
    JSON.stringify({ pages: [{ page: 1, frame_path: "C:\\private\\video\\frame_000001.jpg" }] }, null, 2),
  );

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "unredacted_local_path_or_token" && error.kind === "subtitle_page_map"));
});

test("rejects unmapped subtitle page maps", () => {
  const sessionDir = createCompleteDeliverables();
  const subtitleMapPath = path.join(sessionDir, "generated_artifacts", "subtitle_page_map.json");
  const subtitleMap = JSON.parse(fs.readFileSync(subtitleMapPath, "utf8"));
  subtitleMap.status = "unmapped";
  subtitleMap.page_count = 0;
  subtitleMap.pages = [];
  fs.writeFileSync(subtitleMapPath, JSON.stringify(subtitleMap, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "subtitle_page_map_not_mapped"));
  assert.ok(result.errors.some((error) => error.code === "subtitle_page_map_page_count_invalid"));
  assert.ok(result.errors.some((error) => error.code === "subtitle_page_map_transcript_segments_missing"));
});

test("rejects malformed slide rectangle manifests", () => {
  const sessionDir = createCompleteDeliverables();
  const slideRectanglesPath = path.join(sessionDir, "generated_artifacts", "slide_rectangles_manifest.json");
  const slideRectangles = JSON.parse(fs.readFileSync(slideRectanglesPath, "utf8"));
  slideRectangles.rectangle_extraction_status = "waiting_for_selection";
  slideRectangles.rectangle_extraction_mode = "visual_detector";
  slideRectangles.promoted_rectangle_count = 2;
  slideRectangles.rectangles[0].crop_box.width = 0.8;
  slideRectangles.rectangles[0].review_required = false;
  fs.writeFileSync(slideRectanglesPath, JSON.stringify(slideRectangles, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "slide_rectangles_not_promoted"));
  assert.ok(result.errors.some((error) => error.code === "slide_rectangles_mode_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_rectangles_count_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_rectangles_crop_invalid"));
});

test("rejects malformed selected slides manifests", () => {
  const sessionDir = createCompleteDeliverables();
  const selectedSlidesPath = path.join(sessionDir, "generated_artifacts", "selected_slides_manifest.json");
  const selectedSlides = JSON.parse(fs.readFileSync(selectedSlidesPath, "utf8"));
  selectedSlides.status = "waiting_for_selection";
  selectedSlides.dedupe_status = "unknown";
  selectedSlides.selected_count = 2;
  selectedSlides.requested_selected_count = 1;
  selectedSlides.selected_candidate_indices = [2, 3];
  selectedSlides.requested_selected_candidate_indices = [2];
  selectedSlides.deduped_candidate_count = 3;
  selectedSlides.exact_duplicate_count = 1;
  selectedSlides.visual_duplicate_count = 1;
  selectedSlides.visual_shape_duplicate_count = 2;
  selectedSlides.selected_candidates[0].selection_status = "candidate";
  fs.writeFileSync(selectedSlidesPath, JSON.stringify(selectedSlides, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "selected_slides_not_ready"));
  assert.ok(result.errors.some((error) => error.code === "selected_slides_dedupe_status_invalid"));
  assert.ok(result.errors.some((error) => error.code === "selected_slides_count_invalid"));
  assert.ok(result.errors.some((error) => error.code === "selected_slides_requested_count_invalid"));
  assert.ok(result.errors.some((error) => error.code === "selected_slides_indices_invalid"));
  assert.ok(result.errors.some((error) => error.code === "selected_slides_candidate_invalid"));
  assert.ok(result.errors.some((error) => error.code === "selected_slides_dedupe_count_invalid"));
});

test("rejects malformed slide quality reports", () => {
  const sessionDir = createCompleteDeliverables();
  const qualityReportPath = path.join(sessionDir, "generated_artifacts", "slide_quality_report.json");
  const qualityReport = JSON.parse(fs.readFileSync(qualityReportPath, "utf8"));
  qualityReport.schema = "wrong";
  qualityReport.quality_score = 101;
  qualityReport.slide_count = 2;
  qualityReport.risk_count = 9;
  qualityReport.summary.single_slide_output = true;
  qualityReport.slides[0].crop_risk = "unknown";
  qualityReport.slides[0].sharpness_status = "broken";
  qualityReport.risk_flags[0].severity = "critical";
  qualityReport.risk_flags[0].count = 0;
  delete qualityReport.risk_flags[0].review_action;
  qualityReport.summary.sharpness_low_count = -1;
  qualityReport.summary.full_frame_fallback_count = -1;
  fs.writeFileSync(qualityReportPath, JSON.stringify(qualityReport, null, 2));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_schema_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_score_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_slide_count_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_risk_count_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_slide_row_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_summary_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_risk_flags_invalid"));
  assert.ok(result.errors.some((error) => error.code === "slide_quality_report_sharpness_invalid"));
});

test("rejects unredacted local paths in slide notes", () => {
  const sessionDir = createCompleteDeliverables();
  fs.writeFileSync(
    path.join(sessionDir, "generated_artifacts", "slide_notes.md"),
    "Internal frame path: C:\\private\\video\\frame_000001.jpg",
  );

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "unredacted_local_path_or_token" && error.kind === "slide_notes"));
});

test("rejects unredacted local paths in video slides markdown", () => {
  const sessionDir = createCompleteDeliverables();
  fs.writeFileSync(
    path.join(sessionDir, "generated_artifacts", "video_slides.md"),
    "Source frame: C:\\private\\video\\frame_000001.jpg",
  );

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "unredacted_local_path_or_token" && error.kind === "video_slides_markdown"));
});

function createCompleteDeliverables() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "aidp-v3-video-deliverables-test-"));
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
    "selected_slides_manifest",
    "slide_quality_report",
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
          has_selected_slides_manifest: true,
          has_slide_quality_report: true,
          has_slide_notes: true,
          has_video_slides_markdown: true,
          has_subtitle_page_map: true,
        },
        manifest_outputs: files.filter((file) =>
          ["final_deliverables_manifest", "published_deliverable_manifest", "published_version_history", "extraction_artifacts_manifest"].includes(file.artifact_kind),
        ),
        final_outputs: files.filter((file) => ["pptx", "video_slides_markdown"].includes(file.artifact_kind)),
        review_outputs: files.filter((file) =>
          ["slide_rectangles_manifest", "selected_slides_manifest", "slide_quality_report", "slide_notes"].includes(file.artifact_kind),
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
    path.join(artifactsDir, "selected_slides_manifest.json"),
    JSON.stringify(
      {
        status: "ready_for_pptx_writer",
        source: "manual_keep_list",
        selection_source: "manual_keep_list",
        title: "Fixture video",
        candidate_manifest: "[redacted]",
        contact_sheet_html: "[redacted]",
        keep_list_template: "[redacted]",
        selected_candidate_indices: [2],
        requested_selected_candidate_indices: [2],
        requested_selected_count: 1,
        selected_count: 1,
        deduped_candidate_count: 0,
        exact_duplicate_count: 0,
        visual_duplicate_count: 0,
        visual_shape_duplicate_count: 0,
        rectangle_extraction_status: "promoted_full_frame_fallback",
        rectangle_extraction_mode: "full_frame_fallback",
        dedupe_status: "selected_keep_list_order_deduped",
        selected_candidates: [
          {
            candidate_index: 2,
            contact_sheet_anchor: "frame-000002",
            file_name: "frame_000002.jpg",
            frame_path: "[redacted]",
            selection_status: "selected",
            timestamp_label: "0:00",
            timestamp_seconds: 0.2,
            rectangle_extraction_status: "promoted_full_frame_fallback",
            slide_rectangle: {
              unit: "relative",
              x: 0,
              y: 0,
              width: 1,
              height: 1,
              source: "raw_frame_full_frame_fallback",
            },
            transcript_segments: [{ start_seconds: 0.2, end_seconds: 0.6, text: "Aligned transcript" }],
            ocr_snippets: [{ text: "Aligned OCR", confidence: 0.91 }],
          },
        ],
        rejected_duplicate_candidates: [],
      },
      null,
      2,
    ),
  );

  fs.writeFileSync(
    path.join(artifactsDir, "slide_quality_report.json"),
    JSON.stringify(
      {
        schema: "v3.video_ppt_slide_quality_report.v1",
        status: "review_required",
        quality_score: 55,
        slide_count: 1,
        risk_count: 3,
        summary: {
          full_frame_fallback_count: 1,
          detector_crop_count: 0,
          review_required_count: 1,
          subtitle_mapped_count: 1,
          subtitle_missing_count: 0,
          ocr_mapped_count: 1,
          ocr_missing_count: 0,
          sharpness_low_count: 1,
          sharpness_medium_count: 0,
          sharpness_high_count: 0,
          sharpness_unknown_count: 0,
          deduped_candidate_count: 0,
          exact_duplicate_count: 0,
          visual_duplicate_count: 0,
          single_slide_output: true,
        },
        risk_flags: [
          {
            code: "full_frame_rectangle_fallback",
            severity: "medium",
            count: 1,
            review_action: "review_or_replace_full_frame_crops",
          },
          {
            code: "single_slide_output_review_required",
            severity: "medium",
            count: 1,
            review_action: "confirm_video_contains_only_one_ppt_or_reprocess_with_more_coverage",
          },
          {
            code: "manual_review_required",
            severity: "medium",
            count: 1,
            review_action: "review_slide_quality_report",
          },
        ],
        slides: [
          {
            slide_number: 1,
            candidate_index: 2,
            source_frame: "frame_000002.jpg",
            timestamp_label: "0:00",
            rectangle_extraction_status: "promoted_full_frame_fallback",
            rectangle_extraction_mode: "full_frame_fallback",
            crop_risk: "high",
            subtitle_alignment_status: "pre_page_mapped",
            transcript_segment_count: 1,
            transcript_risk: "low",
            ocr_alignment_status: "window_mapped",
            ocr_snippet_count: 1,
            ocr_risk: "low",
            sharpness_status: "measured",
            sharpness_score: 82,
            sharpness_risk: "low",
            review_required: true,
            quality_score: 55,
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
          has_selected_slides_manifest: true,
          has_slide_quality_report: true,
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
    selected_slides_manifest: "selected_slides_manifest.json",
    slide_quality_report: "slide_quality_report.json",
    slide_notes: "slide_notes.md",
    video_slides_markdown: "video_slides.md",
    subtitle_page_map: "subtitle_page_map.json",
  }[kind];
}

function removeArtifactKind(value, artifactKind) {
  if (Array.isArray(value)) {
    for (let index = value.length - 1; index >= 0; index -= 1) {
      const item = value[index];
      if (item === artifactKind || item?.artifact_kind === artifactKind) {
        value.splice(index, 1);
      } else {
        removeArtifactKind(item, artifactKind);
      }
    }
    return;
  }
  if (!value || typeof value !== "object") {
    return;
  }
  if (value.deliverable_status && typeof value.deliverable_status === "object") {
    if (artifactKind === "slide_quality_report") {
      delete value.deliverable_status.has_slide_quality_report;
    } else if (artifactKind === "subtitle_page_map") {
      value.deliverable_status.has_subtitle_page_map = false;
    }
  }
  for (const nested of Object.values(value)) {
    removeArtifactKind(nested, artifactKind);
  }
}

function minimalPptxFixtureBytes(options = {}) {
  const normalizedOptions = typeof options === "string" ? { notesXml: options } : options;
  const slideCount = normalizedOptions.slideCount ?? 1;
  const notesXml = normalizedOptions.notesXml ?? "";
  const entries = [
    "[Content_Types].xml",
    "_rels/.rels",
    "ppt/presentation.xml",
    "ppt/_rels/presentation.xml.rels",
  ];
  for (let index = 1; index <= slideCount; index += 1) {
    entries.push(
      `ppt/slides/slide${index}.xml`,
      `ppt/slides/_rels/slide${index}.xml.rels`,
      { name: `ppt/notesSlides/notesSlide${index}.xml`, content: notesXml },
    );
  }
  return minimalZipBytes(entries);
}

function minimalZipBytes(entries) {
  const localParts = [];
  const centralParts = [];
  let offset = 0;
  for (const entry of entries) {
    const entryName = typeof entry === "string" ? entry : entry.name;
    const content = Buffer.from(typeof entry === "string" ? "" : (entry.content ?? ""), "utf8");
    const fileName = Buffer.from(entryName, "utf8");
    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);
    localHeader.writeUInt16LE(0, 6);
    localHeader.writeUInt16LE(0, 8);
    localHeader.writeUInt32LE(0, 10);
    localHeader.writeUInt32LE(0, 14);
    localHeader.writeUInt32LE(content.length, 18);
    localHeader.writeUInt32LE(content.length, 22);
    localHeader.writeUInt16LE(fileName.length, 26);
    localHeader.writeUInt16LE(0, 28);
    localParts.push(localHeader, fileName, content);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt16LE(0, 8);
    centralHeader.writeUInt16LE(0, 10);
    centralHeader.writeUInt32LE(0, 12);
    centralHeader.writeUInt32LE(0, 16);
    centralHeader.writeUInt32LE(content.length, 20);
    centralHeader.writeUInt32LE(content.length, 24);
    centralHeader.writeUInt16LE(fileName.length, 28);
    centralHeader.writeUInt16LE(0, 30);
    centralHeader.writeUInt16LE(0, 32);
    centralHeader.writeUInt16LE(0, 34);
    centralHeader.writeUInt16LE(0, 36);
    centralHeader.writeUInt32LE(0, 38);
    centralHeader.writeUInt32LE(offset, 42);
    centralParts.push(centralHeader, fileName);
    offset += localHeader.length + fileName.length + content.length;
  }

  const centralDirectory = Buffer.concat(centralParts);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(0, 4);
  end.writeUInt16LE(0, 6);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralDirectory.length, 12);
  end.writeUInt32LE(offset, 16);
  end.writeUInt16LE(0, 20);
  return Buffer.concat([...localParts, centralDirectory, end]);
}
