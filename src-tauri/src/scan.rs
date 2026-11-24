use crate::models::{ParsedFilename, ScanResult, StreamInfo, TrackRecord, TrackTags, VerificationInfo};
use crate::tag;
use regex::Regex;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Option<Vec<FfprobeStream>>,
    format: Option<FfprobeFormat>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_name: Option<String>,
    codec_type: Option<String>,
    sample_rate: Option<String>,
    bits_per_raw_sample: Option<String>,
    channels: Option<u32>,
    bit_rate: Option<String>,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    bit_rate: Option<String>,
    duration: Option<String>,
}

pub fn scan_folder(folder: &str) -> Result<ScanResult, String> {
    let root = PathBuf::from(folder);
    if !root.exists() {
        return Err(format!("Folder does not exist: {folder}"));
    }
    if !root.is_dir() {
        return Err(format!("Path is not a folder: {folder}"));
    }

    let mut tracks = Vec::new();
    for entry in WalkDir::new(&root).follow_links(false).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let supported = matches!(ext.as_str(), "flac" | "m4a" | "aac" | "mp3" | "wav" | "ogg" | "opus");
        if !supported {
            continue;
        }
        tracks.push(scan_file(path));
    }

    tracks.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    let flac_count = tracks.iter().filter(|t| t.is_flac).count();
    let lossy_count = tracks.iter().filter(|t| t.is_lossy).count();
    Ok(ScanResult {
        folder: folder.to_string(),
        scanned_count: tracks.len(),
        flac_count,
        lossy_count,
        tracks,
    })
}

pub fn scan_file(path: &Path) -> TrackRecord {
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
    let is_flac = extension == "flac";
    let is_lossy = matches!(extension.as_str(), "m4a" | "aac" | "mp3" | "ogg" | "opus");
    let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut warnings = Vec::new();
    let mut error = None;

    if is_lossy {
        warnings.push(format!("Lossy/non-FLAC file detected (.{extension})."));
    }

    let stream = match probe_stream(path) {
        Ok(s) => Some(s),
        Err(e) => {
            warnings.push(format!("ffprobe unavailable or failed: {e}"));
            None
        }
    };

    let verification = if is_flac {
        Some(verify_flac(path))
    } else {
        Some(VerificationInfo {
            ok: false,
            tool: "policy".to_string(),
            message: "Not a lossless FLAC file; excluded from Navidrome lossless workflow.".to_string(),
        })
    };

    let tags = if is_flac {
        match tag::read_tags(path) {
            Ok(t) => t,
            Err(e) => {
                error = Some(format!("Failed to read tags: {e}"));
                TrackTags::default()
            }
        }
    } else {
        TrackTags::default()
    };

    let parsed = parse_filename(&file_name);

    TrackRecord {
        id: path.to_string_lossy().to_string(),
        path: path.to_string_lossy().to_string(),
        file_name,
        extension,
        is_flac,
        is_lossy,
        size_bytes,
        stream,
        verification,
        tags,
        parsed,
        warnings,
        error,
    }
}

pub fn probe_stream(path: &Path) -> Result<StreamInfo, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,codec_type,sample_rate,bits_per_raw_sample,channels,bit_rate,duration",
            "-show_entries",
            "format=bit_rate,duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("ffprobe not runnable: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Invalid ffprobe JSON: {e}"))?;

    let stream = parsed
        .streams
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));

    let format = parsed.format.unwrap_or(FfprobeFormat { bit_rate: None, duration: None });

    let s = stream.unwrap_or(FfprobeStream {
        codec_name: None,
        codec_type: None,
        sample_rate: None,
        bits_per_raw_sample: None,
        channels: None,
        bit_rate: None,
        duration: None,
    });

    Ok(StreamInfo {
        codec_name: s.codec_name,
        sample_rate: s.sample_rate.and_then(|v| v.parse().ok()),
        bits_per_raw_sample: s.bits_per_raw_sample.and_then(|v| v.parse().ok()),
        channels: s.channels,
        bit_rate: s
            .bit_rate
            .or(format.bit_rate)
            .and_then(|v| v.parse().ok()),
        duration_seconds: s
            .duration
            .or(format.duration)
            .and_then(|v| v.parse().ok()),
    })
}

pub fn verify_flac(path: &Path) -> VerificationInfo {
    let ffmpeg = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "null", "-"])
        .output();

    match ffmpeg {
        Ok(out) if out.status.success() && out.stderr.is_empty() => VerificationInfo {
            ok: true,
            tool: "ffmpeg".to_string(),
            message: "Decode check passed (ffmpeg -f null).".to_string(),
        },
        Ok(out) if out.status.success() => VerificationInfo {
            ok: true,
            tool: "ffmpeg".to_string(),
            message: format!(
                "Decode completed with warnings: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        },
        Ok(out) => VerificationInfo {
            ok: false,
            tool: "ffmpeg".to_string(),
            message: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        },
        Err(_) => {
            // Fallback: if lofty can read tags and ffprobe saw FLAC, report partial verification.
            match probe_stream(path) {
                Ok(s) if s.codec_name.as_deref() == Some("flac") => VerificationInfo {
                    ok: true,
                    tool: "ffprobe+lofty-fallback".to_string(),
                    message: "ffmpeg unavailable; FLAC stream detected via ffprobe.".to_string(),
                },
                Ok(_) => VerificationInfo {
                    ok: false,
                    tool: "fallback".to_string(),
                    message: "ffmpeg unavailable and stream is not confirmed FLAC.".to_string(),
                },
                Err(e) => VerificationInfo {
                    ok: false,
                    tool: "fallback".to_string(),
                    message: format!("ffmpeg/ffprobe unavailable: {e}"),
                },
            }
        }
    }
}

pub fn parse_filename(file_name: &str) -> ParsedFilename {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);

    let id_re = Regex::new(r"\s*\[(\d{5,})\]\s*$").unwrap();
    let (without_id, stripped_id) = if let Some(caps) = id_re.captures(stem) {
        let id = caps.get(1).map(|m| m.as_str().to_string());
        (id_re.replace(stem, "").to_string(), id)
    } else {
        (stem.to_string(), None)
    };

    let normalized = without_id.replace(" _ ", " / ").trim().to_string();
    let (artist, title) = if let Some((a, t)) = normalized.split_once(" - ") {
        (Some(a.trim().to_string()), Some(t.trim().to_string()))
    } else {
        (None, Some(normalized.clone()))
    };

    ParsedFilename {
        artist,
        title,
        stripped_id,
        normalized_base: normalized,
    }
}
