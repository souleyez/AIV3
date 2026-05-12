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
    kind: "extraction_artifacts_manifest",
    fileName: "extraction_artifacts_manifest.json",
    statusFlag: "has_extraction_artifacts_manifest",
    group: "manifest_outputs",
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
  const subtitlePageMap = readJsonFile(
    path.join(artifactsDir, "subtitle_page_map.json"),
    "subtitle_page_map",
    errors,
  );

  const pptx = files.find((file) => file.kind === "pptx");
  if (pptx?.exists && !fileStartsWithZipMagic(pptx.path)) {
    errors.push(issue("pptx_zip_magic_missing", "PPTX does not start with ZIP magic bytes", "pptx"));
  }

  if (finalManifest) {
    validateFinalManifest(finalManifest, errors, warnings);
  }
  if (extractionManifest) {
    validateExtractionManifest(extractionManifest, errors);
  }
  if (subtitlePageMap) {
    validateRedactedJson(subtitlePageMap, "subtitle_page_map", errors);
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
    if (!groupContainsKind(manifest[file.group], file.kind)) {
      errors.push(issue("final_manifest_group_missing_kind", `${file.group} does not include ${file.kind}`, file.kind));
    }
  }
  validateRedactedJson(manifest, "final_deliverables_manifest", errors);
}

function validateExtractionManifest(manifest, errors) {
  const files = Array.isArray(manifest.files) ? manifest.files : [];
  for (const file of REQUIRED_FILES) {
    if (!files.some((entry) => entry?.artifact_kind === file.kind)) {
      errors.push(issue("extraction_manifest_missing_kind", `extraction manifest does not include ${file.kind}`, file.kind));
    }
  }
  validateRedactedJson(manifest, "extraction_artifacts_manifest", errors);
}

function validateRedactedJson(value, kind, errors) {
  const text = JSON.stringify(value);
  for (const pattern of LOCAL_PATH_PATTERNS) {
    if (pattern.test(text)) {
      errors.push(issue("unredacted_local_path_or_token", `${kind} contains an unredacted local path or token-like URL`, kind));
      return;
    }
  }
}

function groupContainsKind(group, kind) {
  return Array.isArray(group) && group.some((entry) => entry?.artifact_kind === kind);
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
