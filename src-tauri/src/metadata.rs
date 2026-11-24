use crate::models::MusicBrainzResult;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct MbSearchResponse {
    recordings: Option<Vec<MbRecording>>,
}

#[derive(Debug, Deserialize)]
struct MbRecording {
    id: Option<String>,
    title: Option<String>,
    score: Option<i32>,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    releases: Option<Vec<MbRelease>>,
}

#[derive(Debug, Deserialize)]
struct MbArtistCredit {
    name: Option<String>,
    artist: Option<MbArtist>,
}

#[derive(Debug, Deserialize)]
struct MbArtist { name: Option<String> }

#[derive(Debug, Deserialize)]
struct MbRelease {
    id: Option<String>,
    title: Option<String>,
    date: Option<String>,
    #[serde(rename = "track-count")]
    track_count: Option<u32>,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    media: Option<Vec<MbMedium>>,
}

#[derive(Debug, Deserialize)]
struct MbMedium {
    #[serde(rename = "track-count")]
    track_count: Option<u32>,
    tracks: Option<Vec<MbTrack>>,
}

#[derive(Debug, Deserialize)]
struct MbTrack {
    number: Option<String>,
}

fn encode_q(s: &str) -> String {
    // minimal percent-encoding for query values
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b' ' => {
                if b == b' ' { out.push_str("%20"); } else { out.push(b as char); }
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn artist_credit_name(credits: &Option<Vec<MbArtistCredit>>) -> Option<String> {
    let credits = credits.as_ref()?;
    let mut out = String::new();
    for c in credits {
        if let Some(name) = c.name.as_ref().or(c.artist.as_ref().and_then(|a| a.name.as_ref())) {
            out.push_str(name);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

pub async fn search_musicbrainz(artist: Option<String>, title: Option<String>, album: Option<String>, limit: usize) -> Result<Vec<MusicBrainzResult>, String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = artist.as_ref().filter(|s| !s.trim().is_empty()) {
        parts.push(format!("artist:{}", a.trim()));
    }
    if let Some(t) = title.as_ref().filter(|s| !s.trim().is_empty()) {
        parts.push(format!("recording:{}", t.trim()));
    }
    if let Some(al) = album.as_ref().filter(|s| !s.trim().is_empty()) {
        parts.push(format!("release:{}", al.trim()));
    }
    if parts.is_empty() {
        return Err("Provide at least artist, title, or album for search".to_string());
    }
    let query = parts.join(" AND ");
    let url = format!(
        "https://musicbrainz.org/ws/2/recording/?query={}&fmt=json&limit={}&inc=releases+artists",
        encode_q(&query),
        limit.clamp(1, 25)
    );

    let client = reqwest::Client::builder()
        .user_agent("OctaveTaggingApp/0.1.0 (https://github.com/xotwodfr/octavetaggingapp)")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().await.map_err(|e| format!("MusicBrainz request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("MusicBrainz returned {}", resp.status()));
    }
    let body: MbSearchResponse = resp.json().await.map_err(|e| format!("MusicBrainz parse failed: {e}"))?;

    let mut results = Vec::new();
    for rec in body.recordings.unwrap_or_default() {
        let rec_title = rec.title.clone().unwrap_or_default();
        let rec_artist = artist_credit_name(&rec.artist_credit).unwrap_or_default();
        let releases = rec.releases.unwrap_or_default();
        if releases.is_empty() {
            results.push(MusicBrainzResult {
                recording_id: rec.id.clone(),
                release_id: None,
                title: rec_title,
                artist: rec_artist,
                album: String::new(),
                album_artist: None,
                date: None,
                track_number: None,
                track_total: None,
                cover_art_url: None,
                score: rec.score,
            });
            continue;
        }
        for rel in releases.into_iter().take(3) {
            let track_total = rel.track_count.or_else(|| rel.media.as_ref()?.first()?.track_count);
            let track_number = rel.media.as_ref()
                .and_then(|m| m.first())
                .and_then(|m| m.tracks.as_ref())
                .and_then(|t| t.first())
                .and_then(|t| t.number.as_ref())
                .and_then(|n| n.split('/').next())
                .and_then(|n| n.parse::<u32>().ok());
            let release_id = rel.id.clone();
            let cover_art_url = release_id.as_ref().map(|id| format!("https://coverartarchive.org/release/{id}/front-500"));
            results.push(MusicBrainzResult {
                recording_id: rec.id.clone(),
                release_id,
                title: rec_title.clone(),
                artist: rec_artist.clone(),
                album: rel.title.clone().unwrap_or_default(),
                album_artist: artist_credit_name(&rel.artist_credit),
                date: rel.date.clone(),
                track_number,
                track_total,
                cover_art_url,
                score: rec.score,
            });
        }
    }
    Ok(results)
}

pub async fn fetch_cover_art(url: &str) -> Result<(Vec<u8>, String), String> {
    let client = reqwest::Client::builder()
        .user_agent("OctaveTaggingApp/0.1.0")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| format!("Cover art fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Cover art fetch returned {}", resp.status()));
    }
    let mime = resp.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok((bytes, mime))
}
