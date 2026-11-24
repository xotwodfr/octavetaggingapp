import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  ApplyResult,
  ApplyTrackChange,
  LyricsResult,
  MusicBrainzResult,
  ScanResult,
  TrackRecord,
} from "./types";

export const api = {
  pickFolder: () => invoke<string | null>("pick_folder"),
  scanFolder: (folder: string) => invoke<ScanResult>("scan_folder", { folder }),
  readTrack: (path: string) => invoke<TrackRecord>("read_track", { path }),
  lookupMusicBrainz: (params: { artist?: string; title?: string; album?: string; limit?: number }) =>
    invoke<MusicBrainzResult[]>("lookup_musicbrainz", params),
  lookupSpicyLyrics: (params: { apiKey: string; artist: string; title: string; album?: string }) =>
    invoke<LyricsResult>("lookup_spicy_lyrics", params),
  applyChanges: (params: { changes: ApplyTrackChange[]; filenameTemplate: string; folderTemplate: string }) =>
    invoke<ApplyResult[]>("apply_changes", params),
  loadSettings: () => invoke<AppSettings>("load_settings"),
  saveSettings: (settings: AppSettings) => invoke<AppSettings>("save_settings", { settings }),
};
