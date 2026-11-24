mod album;
mod commands;
mod lyrics;
mod metadata;
mod models;
mod scan;
mod settings;
mod tag;

pub use models::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::scan_folder,
            commands::read_track,
            commands::lookup_musicbrainz,
            commands::resolve_album_url,
            commands::lookup_spicy_lyrics,
            commands::apply_changes,
            commands::load_settings,
            commands::save_settings,
            commands::pick_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
