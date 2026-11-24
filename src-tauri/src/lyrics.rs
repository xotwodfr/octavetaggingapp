use crate::models::LyricsResult;
use serde_json::Value;

fn encode_q(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn find_lyrics_in_json(v: &Value) -> Option<String> {
    match v {
        Value::Object(map) => {
            for key in ["ttml", "lyrics", "syncedLyrics", "plainLyrics", "content", "lrc"] {
                if let Some(Value::String(s)) = map.get(key) {
                    let t = s.trim();
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
            for val in map.values() {
                if let Some(found) = find_lyrics_in_json(val) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(find_lyrics_in_json),
        _ => None,
    }
}

fn looks_like_ttml(s: &str) -> bool {
    let t = s.trim_start().to_lowercase();
    t.contains("<tt") || t.contains("<tt:tt") || t.contains("xmlns=\"http://www.w3.org/ns/ttml")
}

pub async fn lookup_spicy_lyrics(
    api_key: &str,
    artist: &str,
    title: &str,
    album: Option<&str>,
) -> Result<LyricsResult, String> {
    if api_key.trim().is_empty() {
        return Ok(LyricsResult {
            status: "error".to_string(),
            lyrics: None,
            format: None,
            message: Some("Spicy Lyrics API key is not configured".to_string()),
        });
    }
    if artist.trim().is_empty() || title.trim().is_empty() {
        return Ok(LyricsResult {
            status: "error".to_string(),
            lyrics: None,
            format: None,
            message: Some("Artist and title are required for lyrics lookup".to_string()),
        });
    }

    let base = std::env::var("SPICY_LYRICS_API_BASE")
        .unwrap_or_else(|_| "https://api.spicylyrics.org".to_string());
    let base = base.trim_end_matches('/');

    // Try a few likely endpoint shapes; API shape may vary and is documented in README/response handling.
    let mut candidates = vec![
        format!("{base}/lyrics?artist={}&title={}", encode_q(artist), encode_q(title)),
        format!("{base}/v1/lyrics?artist={}&title={}", encode_q(artist), encode_q(title)),
        format!("{base}/search?artist={}&title={}", encode_q(artist), encode_q(title)),
    ];
    if let Some(al) = album.filter(|s| !s.trim().is_empty()) {
        candidates.push(format!(
            "{base}/lyrics?artist={}&title={}&album={}",
            encode_q(artist),
            encode_q(title),
            encode_q(al)
        ));
    }

    let client = reqwest::Client::builder()
        .user_agent("OctaveTaggingApp/0.1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let mut last_err: Option<String> = None;
    for url in candidates {
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key.trim()))
            .header("X-API-Key", api_key.trim())
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(format!("Request failed: {e}"));
                continue;
            }
        };

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.as_u16() == 404 {
            last_err = Some("Not found".to_string());
            continue;
        }
        if !status.is_success() {
            last_err = Some(format!("HTTP {}: {}", status, text.chars().take(200).collect::<String>()));
            continue;
        }

        // Some APIs may return raw TTML/text; others JSON.
        let trimmed = text.trim();
        if trimmed.is_empty() {
            last_err = Some("Empty response".to_string());
            continue;
        }
        if looks_like_ttml(trimmed) {
            return Ok(LyricsResult {
                status: "found".to_string(),
                lyrics: Some(trimmed.to_string()),
                format: Some("ttml".to_string()),
                message: None,
            });
        }

        if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
            if let Some(lyrics) = find_lyrics_in_json(&json) {
                let format = if looks_like_ttml(&lyrics) { "ttml" } else { "text" };
                return Ok(LyricsResult {
                    status: "found".to_string(),
                    lyrics: Some(lyrics),
                    format: Some(format.to_string()),
                    message: None,
                });
            }
            // JSON but no lyrics field - keep trying other endpoints
            last_err = Some("Response did not contain lyrics".to_string());
            continue;
        }

        // Plain text fallback
        return Ok(LyricsResult {
            status: "found".to_string(),
            lyrics: Some(trimmed.to_string()),
            format: Some("text".to_string()),
            message: None,
        });
    }

    let msg = last_err.unwrap_or_else(|| "No lyrics found".to_string());
    let status = if msg.to_lowercase().contains("not found") { "not_found" } else { "error" };
    Ok(LyricsResult {
        status: status.to_string(),
        lyrics: None,
        format: None,
        message: Some(msg),
    })
}
