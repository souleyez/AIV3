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
  assert.equal(result.files.length, 5);
  assert.ok(result.files.every((file) => file.exists));
});

test("rejects missing review files", () => {
  const sessionDir = createCompleteDeliverables();
  fs.unlinkSync(path.join(sessionDir, "generated_artifacts", "slide_notes.md"));

  const result = validateVideoDeliverables(sessionDir);

  assert.equal(result.ok, false);
  assert.ok(result.errors.some((error) => error.code === "missing_required_file" && error.kind === "slide_notes"));
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

function createCompleteDeliverables() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "aidp-v3-video-deliverables-test-"));
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
