# Octave Tagging

Cross-platform desktop app for verifying, tagging, organizing, and enriching lossless music exports (FLAC) for a Navidrome music library.

Built with **Tauri 2 + React + TypeScript + Tailwind CSS** (frontend) and **Rust** (backend).

## Features

- **Folder scan**: pick source and optional destination/Navidrome library folders with native dialogs.
- **Lossless verification**: scans `.flac` files and inspects streams via `ffprobe` (`codec_name`, `sample_rate`, `bits_per_raw_sample`, `channels`, `bit_rate`). Runs decode/integrity check via `ffmpeg -v error -i <file> -f null -`.
- **Lossy detection**: flags non-FLAC/lossy files (`.m4a`, `.aac`, `.mp3`, `.ogg`, `.opus`, `.wav`) and excludes them from safe FLAC apply workflow.
- **Tag reading**: reads existing Vorbis comments/tags and cover art presence with `lofty`.
- **Filename cleanup**: parses common exports like `Artist - Title [1234567890].flac`, strips trailing `[ID]`, normalizes ` _ ` to ` / `, and keeps original filename until apply.
- **Metadata lookup**: MusicBrainz search with manual release/result selection; batch-apply album metadata (`ALBUM`, `ALBUMARTIST`, `DATE`, `TRACKNUMBER`, `ARTIST`, `TITLE`) and front cover URL.
- **Spicy Lyrics**: configurable API key in Settings; per-track lookup; embed lyrics into FLAC tags and/or save `.ttml` sidecar; per-track status (found/embedded/sidecar/not found/error). Batch lookup continues on misses/errors.
- **Preview & safe apply**: shows target path preview using templates (`Artist/Album/01 - Title.flac` defaults). Apply writes to a temp copy, writes tags, moves into place, and removes original only after successful move.
- **Persistent settings**: stored locally in app config dir (`settings.json`) including Spicy Lyrics API key, folders, templates, and lyrics options.

## Spicy Lyrics API Key

1. Open **Settings** in the app.
2. Paste your Spicy Lyrics API key into **Spicy Lyrics API key**.
3. Save settings. The key is stored locally in the app config directory and is **never hardcoded**.

The app attempts a few likely endpoint shapes against `SPICY_LYRICS_API_BASE` (default `https://api.spicylyrics.org`):

- `/lyrics?artist=...&title=...`
- `/v1/lyrics?artist=...&title=...`
- `/search?artist=...&title=...`

It sends both `Authorization: Bearer <key>` and `X-API-Key: <key>` headers. Response parsing looks for `ttml`, `lyrics`, `syncedLyrics`, `plainLyrics`, `content`, or `lrc` fields, and also accepts raw TTML/text responses. **Adjust endpoint/field mapping in `src-tauri/src/lyrics.rs` once tested against the real API.**

## Requirements

### Runtime / verification tools
- `ffprobe` and `ffmpeg` available on PATH for stream inspection and decode verification. The app falls back gracefully when unavailable, but verification will be limited.

### Development
- Node.js 20+ and npm
- Rust stable toolchain
- Tauri 2 system dependencies

#### Linux (Debian/Ubuntu)
```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf \
  ffmpeg libasound2-dev pkg-config build-essential curl wget file \
  libssl-dev libgtk-3-dev libayatana-appindicator3-dev
```

#### Windows
- Microsoft Visual Studio C++ Build Tools
- WebView2 runtime (usually present on Windows 10/11)
- Rust `x86_64-pc-windows-msvc` target
- `ffmpeg`/`ffprobe` on PATH for verification features

## Local Development

```bash
npm install
npm run tauri dev
```

## Checks / Builds

```bash
npm run lint          # TypeScript typecheck
npm run build         # Frontend build
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri build   # Full desktop bundle (platform-specific deps required)
```

## Release / CI

GitHub Actions workflow: `.github/workflows/build.yml`

- Triggers on `push` (all branches), `v*` tags, and `workflow_dispatch`.
- Builds Linux (`ubuntu-24.04`) bundles: `deb,appimage`.
- Builds Windows (`windows-latest`, `x86_64-pc-windows-msvc`) bundles: `nsis,msi`.
- Uploads artifacts for every run.
- Attaches build assets to GitHub Release when pushing a `v*` tag.

To publish a release:
```bash
git tag v0.1.0
git push origin v0.1.0
```

## Security / Secrets

- Do not commit `.env`, API keys, or local test media.
- `.gitignore` excludes build outputs, secrets, and common audio/test media files.
- Spicy Lyrics API key is user-provided at runtime and stored in local app config.

## Project Structure

```
src/                    # React + TS frontend
src-tauri/src/          # Rust backend
  commands.rs           # Tauri commands
  scan.rs               # folder scan, ffprobe/ffmpeg verification
  tag.rs                # FLAC tag/cover read-write via lofty
  metadata.rs           # MusicBrainz + Cover Art Archive
  lyrics.rs             # Spicy Lyrics lookup/adapters
  settings.rs           # persistent local settings
.github/workflows/      # CI build workflow
```

## Notes / Assumptions

- Spicy Lyrics API response shape is treated as adaptable; real endpoint/field names may need adjustment.
- MusicBrainz rate limiting applies; heavy batch lookups should be paced in future iterations.
- Cover art is fetched from URL at apply time; if CAA URL is missing/inaccessible, apply reports the error for that track.
- Filename/folder template sanitization replaces invalid filesystem characters with `_`.
