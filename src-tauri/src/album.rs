use crate::models::{AlbumRelease, AlbumTrack};
use regex::Regex;
use serde::Deserialize;

#[derive(Debug)]
pub enum AlbumUrl {
    Deezer(String),
    Spotify(String),
}

pub fn parse_album_url(input: &str) -> Option<AlbumUrl> {
    let t = input.trim();
    let deezer = Regex::new(r"(?i)deezer\.com/(?:[a-z]{2}(?:-[a-z]{2})?/)?album/(\d+)").ok()?;
    if let Some(c) = deezer.captures(t) {
        return Some(AlbumUrl::Deezer(c[1].to_string()));
    }
    let spotify = Regex::new(r"(?i)open\.spotify\.com/album/([A-Za-z0-9]+)").ok()?;
    if let Some(c) = spotify.captures(t) {
        return Some(AlbumUrl::Spotify(c[1].to_string()));
    }
    let spotify_uri = Regex::new(r"(?i)^spotify:album:([A-Za-z0-9]+)$").ok()?;
    if let Some(c) = spotify_uri.captures(t) {
        return Some(AlbumUrl::Spotify(c[1].to_string()));
    }
    None
}

#[derive(Debug, Deserialize)]
struct DeezerAlbum {
    id: Option<u64>,
    title: Option<String>,
    release_date: Option<String>,
    cover_xl: Option<String>,
    cover_big: Option<String>,
    cover_medium: Option<String>,
    artist: Option<DeezerArtist>,
    tracks: Option<DeezerTracks>,
    error: Option<DeezerError>,
}
#[derive(Debug, Deserialize)]
struct DeezerArtist { name: Option<String> }
#[derive(Debug, Deserialize)]
struct DeezerTracks { data: Option<Vec<DeezerTrack>> }
#[derive(Debug, Deserialize)]
struct DeezerTrack {
    title: Option<String>,
    track_position: Option<u32>,
    artist: Option<DeezerArtist>,
}
#[derive(Debug, Deserialize)]
struct DeezerError { message: Option<String>, code: Option<i32>, #[serde(rename="type")] kind: Option<String> }

pub async fn resolve_deezer_album(id: &str) -> Result<AlbumRelease, String> {
    let url = format!("https://api.deezer.com/album/{}", id.trim());
    let client = reqwest::Client::builder().user_agent("OctaveTaggingApp/0.1.0").build().map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| format!("Deezer request failed: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Deezer read failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("Deezer HTTP {}: {}", status, body.chars().take(300).collect::<String>()));
    }
    let parsed: DeezerAlbum = serde_json::from_str(&body).map_err(|e| format!("Deezer parse failed: {e}; body: {}", body.chars().take(200).collect::<String>()))?;
    if let Some(err) = parsed.error {
        return Err(format!("Deezer API error: {} ({:?} {:?})", err.message.unwrap_or_default(), err.code, err.kind));
    }
    let album = parsed.title.unwrap_or_default();
    let album_artist = parsed.artist.and_then(|a| a.name).unwrap_or_default();
    let date = parsed.release_date.clone();
    let year = date.as_ref().and_then(|d| d.get(0..4)).map(|s| s.to_string());
    let cover_art_url = parsed.cover_xl.or(parsed.cover_big).or(parsed.cover_medium);
    let tracks: Vec<AlbumTrack> = parsed.tracks.and_then(|t| t.data).unwrap_or_default().into_iter().enumerate().map(|(i, t)| AlbumTrack {
        track_number: t.track_position.or(Some((i as u32) + 1)),
        title: t.title.unwrap_or_default(),
        artist: t.artist.and_then(|a| a.name).filter(|s| !s.trim().is_empty()).or_else(|| Some(album_artist.clone())),
        album: album.clone(),
        album_artist: Some(album_artist.clone()),
        date: date.clone(),
    }).collect();
    if tracks.is_empty() {
        return Err(format!("Deezer album {id} returned no tracks"));
    }
    Ok(AlbumRelease {
        source: "deezer".to_string(),
        source_id: parsed.id.map(|v| v.to_string()).unwrap_or_else(|| id.to_string()),
        album,
        album_artist: Some(album_artist),
        date,
        year,
        cover_art_url,
        track_total: Some(tracks.len() as u32),
        tracks,
        warnings: vec![],
    })
}

#[derive(Debug, Deserialize)]
struct SpotifyToken { access_token: String }
#[derive(Debug, Deserialize)]
struct SpotifyAlbum {
    id: Option<String>,
    name: Option<String>,
    release_date: Option<String>,
    images: Option<Vec<SpotifyImage>>,
    artists: Option<Vec<SpotifyArtist>>,
    tracks: Option<SpotifyTracksPage>,
}
#[derive(Debug, Deserialize, Clone)]
struct SpotifyImage { url: Option<String> }
#[derive(Debug, Deserialize, Clone)]
struct SpotifyArtist { name: Option<String> }
#[derive(Debug, Deserialize, Clone)]
struct SpotifyTracksPage { items: Option<Vec<SpotifyTrack>>, next: Option<String> }
#[derive(Debug, Deserialize, Clone)]
struct SpotifyTrack { name: Option<String>, track_number: Option<u32>, artists: Option<Vec<SpotifyArtist>> }

async fn spotify_token(client_id: &str, client_secret: &str) -> Result<String, String> {
    let client = reqwest::Client::builder().user_agent("OctaveTaggingApp/0.1.0").build().map_err(|e| e.to_string())?;
    let resp = client.post("https://accounts.spotify.com/api/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send().await.map_err(|e| format!("Spotify token request failed: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Spotify token HTTP {}: {}", status, body.chars().take(300).collect::<String>()));
    }
    let tok: SpotifyToken = serde_json::from_str(&body).map_err(|e| format!("Spotify token parse failed: {e}"))?;
    Ok(tok.access_token)
}

pub async fn resolve_spotify_album(id: &str, client_id: Option<&str>, client_secret: Option<&str>) -> Result<AlbumRelease, String> {
    let id = id.trim();
    let cid = client_id.filter(|s| !s.trim().is_empty());
    let sec = client_secret.filter(|s| !s.trim().is_empty());
    if let (Some(cid), Some(sec)) = (cid, sec) {
        return resolve_spotify_api(id, cid, sec).await;
    }
    resolve_spotify_oembed(id).await
}

async fn resolve_spotify_api(id: &str, client_id: &str, client_secret: &str) -> Result<AlbumRelease, String> {
    let token = spotify_token(client_id, client_secret).await?;
    let client = reqwest::Client::builder().user_agent("OctaveTaggingApp/0.1.0").build().map_err(|e| e.to_string())?;
    let url = format!("https://api.spotify.com/v1/albums/{}?market=US&limit=50", id);
    let resp = client.get(&url).bearer_auth(&token).send().await.map_err(|e| format!("Spotify album request failed: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Spotify album HTTP {}: {}", status, body.chars().take(300).collect::<String>()));
    }
    let album: SpotifyAlbum = serde_json::from_str(&body).map_err(|e| format!("Spotify parse failed: {e}; body: {}", body.chars().take(200).collect::<String>()))?;
    let mut items = album.tracks.as_ref().and_then(|t| t.items.clone()).unwrap_or_default();
    let mut next = album.tracks.as_ref().and_then(|t| t.next.clone());
    while let Some(nurl) = next {
        let r = client.get(&nurl).bearer_auth(&token).send().await.map_err(|e| format!("Spotify tracks page failed: {e}"))?;
        let st = r.status();
        let b = r.text().await.map_err(|e| e.to_string())?;
        if !st.is_success() { return Err(format!("Spotify tracks HTTP {}: {}", st, b.chars().take(200).collect::<String>())); }
        let page: SpotifyTracksPage = serde_json::from_str(&b).map_err(|e| format!("Spotify tracks parse failed: {e}"))?;
        if let Some(mut more) = page.items { items.append(&mut more); }
        next = page.next;
    }
    let album_name = album.name.unwrap_or_default();
    let album_artist = album.artists.as_ref().and_then(|a| a.first()).and_then(|a| a.name.clone()).unwrap_or_default();
    let date = album.release_date;
    let year = date.as_ref().and_then(|d| d.get(0..4)).map(|s| s.to_string());
    let cover_art_url = album.images.as_ref().and_then(|imgs| imgs.first()).and_then(|i| i.url.clone());
    let tracks: Vec<AlbumTrack> = items.into_iter().enumerate().map(|(i, t)| AlbumTrack {
        track_number: t.track_number.or(Some((i as u32) + 1)),
        title: t.name.unwrap_or_default(),
        artist: t.artists.as_ref().and_then(|a| a.first()).and_then(|a| a.name.clone()).or_else(|| Some(album_artist.clone())),
        album: album_name.clone(),
        album_artist: Some(album_artist.clone()),
        date: date.clone(),
    }).collect();
    if tracks.is_empty() { return Err("Spotify album returned no tracks".to_string()); }
    Ok(AlbumRelease {
        source: "spotify".to_string(),
        source_id: album.id.unwrap_or_else(|| id.to_string()),
        album: album_name,
        album_artist: Some(album_artist),
        date,
        year,
        cover_art_url,
        track_total: Some(tracks.len() as u32),
        tracks,
        warnings: vec!["Resolved via Spotify Web API".to_string()],
    })
}

#[derive(Debug, Deserialize)]
struct SpotifyOEmbed { title: Option<String>, thumbnail_url: Option<String>, author_name: Option<String> }

async fn resolve_spotify_oembed(id: &str) -> Result<AlbumRelease, String> {
    let album_url = format!("https://open.spotify.com/album/{}", id);
    let oembed = format!("https://open.spotify.com/oembed?url={}", urlencoding_simple(&album_url));
    let client = reqwest::Client::builder().user_agent("OctaveTaggingApp/0.1.0").build().map_err(|e| e.to_string())?;
    let resp = client.get(&oembed).send().await.map_err(|e| format!("Spotify oEmbed request failed: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Spotify oEmbed HTTP {}: {}. Configure Spotify Client ID/Secret in Settings for full tracklist resolution.", status, body.chars().take(200).collect::<String>()));
    }
    let oe: SpotifyOEmbed = serde_json::from_str(&body).map_err(|e| format!("Spotify oEmbed parse failed: {e}"))?;
    let title = oe.title.unwrap_or_else(|| "Unknown Album".to_string());
    let artist = oe.author_name;
    Ok(AlbumRelease {
        source: "spotify-oembed".to_string(),
        source_id: id.to_string(),
        album: title,
        album_artist: artist.clone(),
        date: None,
        year: None,
        cover_art_url: oe.thumbnail_url,
        track_total: None,
        tracks: vec![],
        warnings: vec![
            "Spotify unauthenticated fallback used (oEmbed): tracklist unavailable. Configure Spotify Client ID/Secret in Settings for full album+track resolution.".to_string(),
            format!("oEmbed status: HTTP {}", status),
        ],
    })
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b':' => out.push_str("%3A"),
            b'/' => out.push_str("%2F"),
            b'?' => out.push_str("%3F"),
            b'=' => out.push_str("%3D"),
            b'&' => out.push_str("%26"),
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
