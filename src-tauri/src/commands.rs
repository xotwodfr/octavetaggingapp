use crate::album;
use crate::lyrics;
use crate::metadata;
use crate::models::{AlbumRelease, ApplyResult, ApplyTrackChange, AppSettings, LyricsResult, MusicBrainzResult, ScanResult, TrackRecord};
use crate::scan;
use crate::settings;
use crate::tag;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder.map(|p| p.to_string()));
    });
    rx.recv().map_err(|e| format!("Folder dialog failed: {e}"))
}

#[tauri::command]
pub async fn scan_folder(folder: String) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scan::scan_folder(&folder))
        .await
        .map_err(|e| format!("Scan task failed: {e}"))?
}

#[tauri::command]
pub async fn read_track(path: String) -> Result<TrackRecord, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(scan::scan_file(Path::new(&path))))
        .await
        .map_err(|e| format!("Read task failed: {e}"))?
}

#[tauri::command]
pub async fn lookup_musicbrainz(
    artist: Option<String>,
    title: Option<String>,
    album: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<MusicBrainzResult>, String> {
    metadata::search_musicbrainz(artist, title, album, limit.unwrap_or(10)).await
}

#[tauri::command]
pub async fn resolve_album_url(
    input: String,
    spotify_client_id: Option<String>,
    spotify_client_secret: Option<String>,
) -> Result<AlbumRelease, String> {
    let trimmed = input.trim();
    match album::parse_album_url(trimmed) {
        Some(album::AlbumUrl::Deezer(id)) => album::resolve_deezer_album(&id).await,
        Some(album::AlbumUrl::Spotify(id)) => {
            album::resolve_spotify_album(
                &id,
                spotify_client_id.as_deref(),
                spotify_client_secret.as_deref(),
            )
            .await
        }
        None => Err(format!("Not a supported album URL: {trimmed}")),
    }
}

#[tauri::command]
pub async fn lookup_spicy_lyrics(
    api_key: String,
    artist: String,
    title: String,
    album: Option<String>,
) -> Result<LyricsResult, String> {
    lyrics::lookup_spicy_lyrics(&api_key, &artist, &title, album.as_deref()).await
}

#[tauri::command]
pub async fn load_settings(app: AppHandle) -> Result<AppSettings, String> {
    settings::load_settings(&app)
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    settings::save_settings(&app, settings)
}

fn sanitize_component(s: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut out: String = s
        .chars()
        .map(|c| if invalid.contains(&c) || c.is_control() { '_' } else { c })
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() { "_".to_string() } else { out }
}

fn render_template(template: &str, tags: &crate::models::TrackTags, fallback_name: &str) -> String {
    let title = tags.title.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| fallback_name.to_string());
    let artist = tags.artist.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "Unknown Artist".to_string());
    let album = tags.album.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "Unknown Album".to_string());
    let album_artist = tags.album_artist.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| artist.clone());
    let date = tags.date.clone().unwrap_or_default();
    let track = tags.track_number.unwrap_or(0);
    let track_total = tags.track_total.unwrap_or(0);
    let disc = tags.disc_number.unwrap_or(0);

    let mut out = template.to_string();
    out = out.replace("{title}", &sanitize_component(&title));
    out = out.replace("{artist}", &sanitize_component(&artist));
    out = out.replace("{album}", &sanitize_component(&album));
    out = out.replace("{albumartist}", &sanitize_component(&album_artist));
    out = out.replace("{date}", &sanitize_component(&date));
    out = out.replace("{track:02}", &format!("{:02}", track));
    out = out.replace("{track}", &track.to_string());
    out = out.replace("{tracktotal}", &track_total.to_string());
    out = out.replace("{disc}", &disc.to_string());
    out
}

fn resolve_target_path(change: &ApplyTrackChange, filename_template: &str, folder_template: &str) -> Result<PathBuf, String> {
    let source = PathBuf::from(&change.source_path);
    if !source.exists() {
        return Err(format!("Source file does not exist: {}", change.source_path));
    }
    let fallback = source.file_stem().and_then(|s| s.to_str()).unwrap_or("track");
    let file_base = render_template(filename_template, &change.tags, fallback);
    let folder_rel = render_template(folder_template, &change.tags, fallback);

    let target = PathBuf::from(&change.target_path);
    let final_path = if target.extension().is_none() {
        let mut p = target;
        for comp in folder_rel.split('/') {
            let c = comp.trim();
            if !c.is_empty() { p.push(sanitize_component(c)); }
        }
        p.push(format!("{}.flac", sanitize_component(&file_base)));
        p
    } else {
        // Treat provided target as destination root-ish when it points to dir; otherwise keep as explicit file path.
        if target.is_dir() {
            let mut p = target;
            for comp in folder_rel.split('/') {
                let c = comp.trim();
                if !c.is_empty() { p.push(sanitize_component(c)); }
            }
            p.push(format!("{}.flac", sanitize_component(&file_base)));
            p
        } else {
            target
        }
    };
    Ok(final_path)
}

fn ensure_unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("track");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("flac");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for i in 1..1000 {
        let candidate = parent.join(format!("{stem} ({i}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-{}.{}", chrono::Utc::now().timestamp_millis(), ext))
}

#[tauri::command]
pub async fn apply_changes(
    changes: Vec<ApplyTrackChange>,
    filename_template: String,
    folder_template: String,
) -> Result<Vec<ApplyResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut results = Vec::new();
        for change in changes {
            results.push(apply_one(change, &filename_template, &folder_template));
        }
        Ok(results)
    })
    .await
    .map_err(|e| format!("Apply task failed: {e}"))?
}

fn apply_one(change: ApplyTrackChange, filename_template: &str, folder_template: &str) -> ApplyResult {
    let source_path = change.source_path.clone();
    let mut lyrics_status = None;
    let result = (|| -> Result<(String, Option<String>), String> {
        let source = PathBuf::from(&change.source_path);
        let ext = source.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if ext != "flac" {
            return Err("Only FLAC files can be safely tagged/moved by this app".to_string());
        }

        let target = resolve_target_path(&change, filename_template, folder_template)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create target directory: {e}"))?;
        }

        let temp = target.with_extension("tmp.flac");
        if temp.exists() { let _ = fs::remove_file(&temp); }
        fs::copy(&source, &temp).map_err(|e| format!("Failed to copy source to temp: {e}"))?;

        let cover = if let Some(url) = change.cover_art_url.as_ref().filter(|s| !s.is_empty()) {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            match rt.block_on(metadata::fetch_cover_art(url)) {
                Ok((bytes, mime)) => Some((bytes, mime)),
                Err(e) => return Err(format!("Failed to fetch cover art: {e}")),
            }
        } else {
            None
        };

        let mut tags = change.tags.clone();
        if change.embed_lyrics {
            if let Some(lyrics) = change.lyrics.as_ref().filter(|s| !s.trim().is_empty()) {
                tags.lyrics = Some(lyrics.clone());
                lyrics_status = Some("embedded".to_string());
            }
        }

        tag::write_tags(
            &temp,
            &tags,
            cover.as_ref().map(|(b, _)| b.as_slice()),
            cover.as_ref().map(|(_, m)| m.as_str()),
        )?;

        let final_target = if source.canonicalize().ok() == target.canonicalize().ok() {
            target
        } else {
            ensure_unique_path(&target)
        };

        if final_target.exists() && source.canonicalize().ok() != final_target.canonicalize().ok() {
            return Err(format!("Target already exists: {}", final_target.display()));
        }

        if source.canonicalize().ok() == final_target.canonicalize().ok() {
            fs::rename(&temp, &final_target).map_err(|e| format!("Failed to replace file: {e}"))?;
        } else {
            fs::rename(&temp, &final_target).map_err(|e| format!("Failed to move file into place: {e}"))?;
            if source.exists() {
                fs::remove_file(&source).map_err(|e| format!("Failed to remove original: {e}"))?;
            }
        }

        if change.save_lyrics_sidecar {
            if let Some(lyrics) = change.lyrics.as_ref().filter(|s| !s.trim().is_empty()) {
                let sidecar = final_target.with_extension("ttml");
                fs::write(&sidecar, lyrics).map_err(|e| format!("Failed to write lyrics sidecar: {e}"))?;
                lyrics_status = Some(match lyrics_status.as_deref() {
                    Some("embedded") => "embedded+sidecar".to_string(),
                    _ => "sidecar".to_string(),
                });
            }
        }

        Ok((final_target.to_string_lossy().to_string(), lyrics_status.clone()))
    })();

    match result {
        Ok((target_path, lyrics_status)) => ApplyResult {
            id: change.id,
            source_path,
            target_path,
            ok: true,
            message: "Applied successfully".to_string(),
            lyrics_status,
        },
        Err(e) => ApplyResult {
            id: change.id,
            source_path,
            target_path: change.target_path,
            ok: false,
            message: e,
            lyrics_status,
        },
    }
}
