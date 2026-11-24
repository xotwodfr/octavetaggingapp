export interface AppSettings {
  spicyLyricsApiKey: string;
  sourceFolder?: string | null;
  destinationFolder?: string | null;
  filenameTemplate: string;
  folderTemplate: string;
  embedLyrics: boolean;
  saveLyricsSidecar: boolean;
}

export interface StreamInfo {
  codecName?: string | null;
  sampleRate?: number | null;
  bitsPerRawSample?: number | null;
  channels?: number | null;
  bitRate?: number | null;
  durationSeconds?: number | null;
}

export interface VerificationInfo {
  ok: boolean;
  tool: string;
  message: string;
}

export interface TrackTags {
  title?: string | null;
  artist?: string | null;
  album?: string | null;
  albumArtist?: string | null;
  date?: string | null;
  trackNumber?: number | null;
  trackTotal?: number | null;
  discNumber?: number | null;
  discTotal?: number | null;
  lyrics?: string | null;
  hasCoverArt: boolean;
}

export interface ParsedFilename {
  artist?: string | null;
  title?: string | null;
  strippedId?: string | null;
  normalizedBase: string;
}

export interface TrackRecord {
  id: string;
  path: string;
  fileName: string;
  extension: string;
  isFlac: boolean;
  isLossy: boolean;
  sizeBytes: number;
  stream?: StreamInfo | null;
  verification?: VerificationInfo | null;
  tags: TrackTags;
  parsed: ParsedFilename;
  warnings: string[];
  error?: string | null;
}

export interface ScanResult {
  folder: string;
  tracks: TrackRecord[];
  scannedCount: number;
  flacCount: number;
  lossyCount: number;
}

export interface MusicBrainzResult {
  recordingId?: string | null;
  releaseId?: string | null;
  title: string;
  artist: string;
  album: string;
  albumArtist?: string | null;
  date?: string | null;
  trackNumber?: number | null;
  trackTotal?: number | null;
  coverArtUrl?: string | null;
  score?: number | null;
}

export interface LyricsResult {
  status: string;
  lyrics?: string | null;
  format?: string | null;
  message?: string | null;
}

export interface ApplyTrackChange {
  id: string;
  sourcePath: string;
  targetPath: string;
  tags: TrackTags;
  coverArtUrl?: string | null;
  lyrics?: string | null;
  embedLyrics: boolean;
  saveLyricsSidecar: boolean;
}

export interface ApplyResult {
  id: string;
  sourcePath: string;
  targetPath: string;
  ok: boolean;
  message: string;
  lyricsStatus?: string | null;
}

export interface TrackDraft {
  selected: boolean;
  tags: TrackTags;
  coverArtUrl?: string | null;
  lyricsStatus: "idle" | "loading" | "found" | "embedded" | "sidecar" | "embedded+sidecar" | "not_found" | "error";
  lyricsMessage?: string | null;
  targetPath: string;
  applyStatus?: "pending" | "ok" | "error";
  applyMessage?: string | null;
}
