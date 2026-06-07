use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{DatasetId, Document, DocumentId, DocumentLifecycle, TenantId};
use media_worker::{
    extract_video_ppt_output_with_artifacts, run_video_frame_extraction,
    write_video_extraction_text_artifacts, FrameExtractionConfig,
};
use serde_json::{json, Value};

fn main() -> Result<()> {
    let args = Args::parse()?;
    if !args.input.is_file() {
        return Err(anyhow!("input video file does not exist"));
    }
    fs::create_dir_all(&args.output_root)?;

    let document = offline_document(&args.title);
    let frame_extraction = run_video_frame_extraction(
        &document,
        &args.input,
        &FrameExtractionConfig {
            enabled: true,
            ffmpeg_bin: args.ffmpeg_bin.clone(),
            output_root: args.output_root.clone(),
            interval_seconds: args.interval_seconds,
        },
    )
    .map_err(|error| anyhow!("frame extraction failed: {error}"))?;
    let generated_artifacts =
        write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &args.output_root)
            .map_err(|error| anyhow!("artifact generation failed: {error}"))?;
    let output = extract_video_ppt_output_with_artifacts(
        &document,
        &[],
        frame_extraction,
        generated_artifacts,
    );

    let summary = smoke_summary(&output);
    if let Some(path) = &args.json_output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(&summary)?)?;
    }
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

#[derive(Debug)]
struct Args {
    input: PathBuf,
    output_root: PathBuf,
    ffmpeg_bin: String,
    interval_seconds: f64,
    title: String,
    json_output: Option<PathBuf>,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut input = None;
        let mut output_root = Some(PathBuf::from("target/video-ppt-offline-smoke"));
        let mut ffmpeg_bin =
            std::env::var("MEDIA_FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".to_string());
        let mut interval_seconds = 15.0;
        let mut title = "Offline Public Video PPT Candidate".to_string();
        let mut json_output = None;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--input" => input = Some(PathBuf::from(next_value(&mut args, &arg)?)),
                "--output-root" => output_root = Some(PathBuf::from(next_value(&mut args, &arg)?)),
                "--ffmpeg-bin" => ffmpeg_bin = next_value(&mut args, &arg)?,
                "--interval-seconds" => {
                    interval_seconds = next_value(&mut args, &arg)?
                        .parse::<f64>()
                        .map_err(|_| anyhow!("--interval-seconds must be a number"))?;
                }
                "--title" => title = next_value(&mut args, &arg)?,
                "--json-output" => json_output = Some(PathBuf::from(next_value(&mut args, &arg)?)),
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                value => return Err(anyhow!("unknown argument {value}")),
            }
        }

        if interval_seconds <= 0.0 {
            return Err(anyhow!("--interval-seconds must be greater than zero"));
        }

        Ok(Self {
            input: input.ok_or_else(|| anyhow!("--input is required"))?,
            output_root: output_root.ok_or_else(|| anyhow!("--output-root is required"))?,
            ffmpeg_bin,
            interval_seconds,
            title,
            json_output,
        })
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String> {
    args.next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("{name} requires a value"))
}

fn print_help() {
    println!(
        r#"Usage:
  cargo run -p media-worker --bin video_ppt_offline_smoke -- \
    --input target/video.mp4 \
    --output-root target/video-ppt-offline-smoke \
    --ffmpeg-bin /opt/homebrew/bin/ffmpeg \
    --interval-seconds 15 \
    --json-output target/video-ppt-offline-smoke/summary.json

This local smoke uses a local public video file and media-worker's normal frame
extraction/artifact generation functions. It does not use the database queue,
upload files, call the main site, call third-party APIs, record the browser, or
deploy services. Generated videos, frames, PPTX files, and reports must stay
under target/ and out of Git."#
    );
}

fn offline_document(title: &str) -> Document {
    Document {
        id: DocumentId::new(),
        tenant_id: TenantId::new(),
        dataset_id: DatasetId::new(),
        owner_user_id: None,
        title: title.to_string(),
        object_key: "offline-public-video-candidate.mp4".to_string(),
        content_type: "video/mp4".to_string(),
        lifecycle: DocumentLifecycle::Extracted,
        secret_binding_ids: Vec::new(),
        metadata: BTreeMap::from_iter([(
            "offline_public_sample".to_string(),
            json!({
                "source_url_redacted": true,
                "object_path_redacted": true,
                "production_write_allowed": false
            }),
        )]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn smoke_summary(output: &Value) -> Value {
    let generated = &output["generated_artifacts"];
    let files = generated
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ready_file_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let selected_count = files
        .iter()
        .find(|file| {
            file.get("artifact_kind").and_then(Value::as_str) == Some("selected_slides_manifest")
        })
        .and_then(|file| file.get("selected_count"))
        .cloned()
        .unwrap_or(Value::Null);

    json!({
        "schema": "v3.video_ppt_offline_smoke.v1",
        "status": "completed",
        "deliverable_state": output["deliverable_status"]["state"],
        "frame_extraction_status": output["frame_extraction"]["status"],
        "frame_count": output["frame_extraction"]["frame_count"],
        "selected_count": selected_count,
        "generated_artifacts_status": generated["status"],
        "generated_artifacts_session_dir": generated["session_dir"],
        "generated_artifacts_manifest_path": generated["manifest_path"],
        "has_pptx": output["deliverable_status"]["has_pptx"],
        "has_video_slides_markdown": output["deliverable_status"]["has_video_slides_markdown"],
        "has_slide_quality_report": output["deliverable_status"]["has_slide_quality_report"],
        "has_subtitle_page_map": output["deliverable_status"]["has_subtitle_page_map"],
        "ready_file_kinds": ready_file_kinds,
        "safety": {
            "production_write_allowed": false,
            "main_site_called": false,
            "third_party_event_sent": false,
            "browser_recording_used": false,
            "service_deployment_used": false,
            "source_url_included": false
        }
    })
}
