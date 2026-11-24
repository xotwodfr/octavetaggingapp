use crate::models::AppSettings;
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to resolve config dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    Ok(dir.join("settings.json"))
}

pub fn default_settings() -> AppSettings {
    AppSettings {
        spicy_lyrics_api_key: String::new(),
        spotify_client_id: String::new(),
        spotify_client_secret: String::new(),
        source_folder: None,
        destination_folder: None,
        filename_template: "{track:02} - {title}".to_string(),
        folder_template: "{albumartist}/{album}".to_string(),
        embed_lyrics: true,
        save_lyrics_sidecar: false,
    }
}

pub fn load_settings(app: &AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(default_settings());
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {e}"))?;
    let settings: AppSettings =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse settings: {e}"))?;
    Ok(settings.normalized())
}

pub fn save_settings(app: &AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    let settings = settings.normalized();
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write settings: {e}"))?;
    Ok(settings)
}
