use crate::models::TrackTags;
use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag, TagType};
use std::path::Path;

pub fn read_tags(path: &Path) -> Result<TrackTags, String> {
    let tagged = Probe::open(path)
        .map_err(|e| e.to_string())?
        .read()
        .map_err(|e| e.to_string())?;

    let tag = tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .cloned();

    let Some(tag) = tag else {
        return Ok(TrackTags::default());
    };

    let has_cover_art = tag
        .pictures()
        .iter()
        .any(|p| matches!(p.pic_type(), PictureType::CoverFront | PictureType::Other))
        || !tag.pictures().is_empty();

    let lyrics = tag.get_string(ItemKey::Lyrics).map(|s| s.to_string()).or_else(|| {
        tag.items().find_map(|item| {
            if item.key() == ItemKey::Lyrics {
                item.value().text().map(|s| s.to_string())
            } else {
                None
            }
        })
    });

    let date = tag
        .get_string(ItemKey::RecordingDate)
        .map(|s| s.to_string())
        .or_else(|| tag.get_string(ItemKey::Year).map(|s| s.to_string()))
        .or_else(|| tag.date().map(|d| d.year.to_string()));

    Ok(TrackTags {
        title: tag.title().map(|s| s.to_string()),
        artist: tag.artist().map(|s| s.to_string()),
        album: tag.album().map(|s| s.to_string()),
        album_artist: tag.get_string(ItemKey::AlbumArtist).map(|s| s.to_string()),
        date,
        track_number: tag.track(),
        track_total: tag.track_total(),
        disc_number: tag.disk(),
        disc_total: tag.disk_total(),
        lyrics,
        has_cover_art,
    })
}

pub fn write_tags(
    path: &Path,
    tags: &TrackTags,
    cover_art: Option<&[u8]>,
    cover_mime: Option<&str>,
) -> Result<(), String> {
    let tagged = Probe::open(path)
        .map_err(|e| e.to_string())?
        .read()
        .map_err(|e| e.to_string())?;

    let tag_type = tagged.primary_tag_type();
    let mut tag = match tagged.tag(tag_type).cloned() {
        Some(t) => t,
        None => Tag::new(TagType::VorbisComments),
    };

    if let Some(v) = tags.title.as_ref().filter(|s| !s.is_empty()) {
        tag.set_title(v.clone());
    }
    if let Some(v) = tags.artist.as_ref().filter(|s| !s.is_empty()) {
        tag.set_artist(v.clone());
    }
    if let Some(v) = tags.album.as_ref().filter(|s| !s.is_empty()) {
        tag.set_album(v.clone());
    }
    if let Some(v) = tags.album_artist.as_ref().filter(|s| !s.is_empty()) {
        tag.insert_text(ItemKey::AlbumArtist, v.clone());
    }
    if let Some(v) = tags.date.as_ref().filter(|s| !s.is_empty()) {
        tag.insert_text(ItemKey::RecordingDate, v.clone());
        if let Ok(year) = v.chars().take(4).collect::<String>().parse::<u32>() {
            tag.insert_text(ItemKey::Year, year.to_string());
        }
    }
    if let Some(v) = tags.track_number {
        tag.set_track(v);
    }
    if let Some(v) = tags.track_total {
        tag.set_track_total(v);
    }
    if let Some(v) = tags.disc_number {
        tag.set_disk(v);
    }
    if let Some(v) = tags.disc_total {
        tag.set_disk_total(v);
    }
    if let Some(v) = tags.lyrics.as_ref().filter(|s| !s.is_empty()) {
        tag.insert_text(ItemKey::Lyrics, v.clone());
    }

    if let Some(bytes) = cover_art {
        let mime = match cover_mime.unwrap_or("image/jpeg") {
            "image/png" => MimeType::Png,
            "image/gif" => MimeType::Gif,
            "image/bmp" => MimeType::Bmp,
            _ => MimeType::Jpeg,
        };
        let picture = Picture::unchecked(bytes.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(mime)
            .build();
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(picture);
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("Failed to write tags: {e}"))?;
    Ok(())
}
