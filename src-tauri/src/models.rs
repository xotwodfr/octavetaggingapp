use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub spicy_lyrics_api_key: String,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
    pub source_folder: Option<String>,
    pub destination_folder: Option<String>,
    pub filename_template: String,
    pub folder_template: String,
    pub embed_lyrics: bool,
    pub save_lyrics_sidecar: bool,
}

impl AppSettings {
    pub fn normalized(self) -> Self {
        Self {
            filename_template: if self.filename_template.trim().is_empty() {
                "{track:02} - {title}".to_string()
            } else {
                self.filename_template
            },
            folder_template: if self.folder_template.trim().is_empty() {
                "{albumartist}/{album}".to_string()
            } else {
                self.folder_template
            },
            ..self
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamInfo {
    pub codec_name: Option<String>,
    pub sample_rate: Option<u32>,
    pub bits_per_raw_sample: Option<u32>,
    pub channels: Option<u32>,
    pub bit_rate: Option<u64>,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerificationInfo {
    pub ok: bool,
    pub tool: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub lyrics: Option<String>,
    pub has_cover_art: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParsedFilename {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub stripped_id: Option<String>,
    pub normalized_base: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrackRecord {
    pub id: String,
    pub path: String,
    pub file_name: String,
    pub extension: String,
    pub is_flac: bool,
    pub is_lossy: bool,
    pub size_bytes: u64,
    pub stream: Option<StreamInfo>,
    pub verification: Option<VerificationInfo>,
    pub tags: TrackTags,
    pub parsed: ParsedFilename,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub folder: String,
    pub tracks: Vec<TrackRecord>,
    pub scanned_count: usize,
    pub flac_count: usize,
    pub lossy_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicBrainzResult {
    pub recording_id: Option<String>,
    pub release_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub cover_art_url: Option<String>,
    pub score: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlbumTrack {
    pub track_number: Option<u32>,
    pub title: String,
    pub artist: Option<String>,
    pub album: String,
    pub album_artist: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRelease {
    pub source: String,
    pub source_id: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub year: Option<String>,
    pub cover_art_url: Option<String>,
    pub track_total: Option<u32>,
    pub tracks: Vec<AlbumTrack>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResult {
    pub status: String,
    pub lyrics: Option<String>,
    pub format: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTrackChange {
    pub id: String,
    pub source_path: String,
    pub target_path: String,
    pub tags: TrackTags,
    pub cover_art_url: Option<String>,
    pub lyrics: Option<String>,
    pub embed_lyrics: bool,
    pub save_lyrics_sidecar: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub id: String,
    pub source_path: String,
    pub target_path: String,
    pub ok: bool,
    pub message: String,
    pub lyrics_status: Option<String>,
}
