//! One Discover card (`musicTwoRowItemRenderer`).
use super::nav::{nav, nav_str};
use super::parse::best_thumbnail;
use crate::core::model::{Artist, Item, Kind};
use serde_json::Value;

fn text(v: &Value, path: &[&str]) -> Option<String> {
    nav_str(v, path).map(|s| s.trim().to_string())
}

/// A musicTwoRowItemRenderer: album, playlist, artist, video/song or mix.
pub fn parse_card(r: &Value) -> Option<Item> {
    let title = text(r, &["title", "runs", "0", "text"]).filter(|t| !t.is_empty())?;
    let mut artists = Vec::new();
    let mut subtitle = String::new();
    for run in nav(r, &["subtitle", "runs"])
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let t = run.get("text").and_then(Value::as_str).unwrap_or("");
        subtitle.push_str(t);
        let page = nav_str(
            run,
            &[
                "navigationEndpoint",
                "browseEndpoint",
                "browseEndpointContextSupportedConfigs",
                "browseEndpointContextMusicConfig",
                "pageType",
            ],
        );
        if matches!(
            page,
            Some("MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL")
        ) {
            artists.push(Artist {
                name: t.to_string(),
                id: nav_str(run, &["navigationEndpoint", "browseEndpoint", "browseId"])
                    .map(String::from),
            });
        }
    }
    let thumbnail = nav(
        r,
        &[
            "thumbnailRenderer",
            "musicThumbnailRenderer",
            "thumbnail",
            "thumbnails",
        ],
    )
    .and_then(best_thumbnail);
    let mut item = Item {
        kind: Kind::Song,
        title,
        video_id: None,
        browse_id: None,
        artists,
        album: None,
        album_id: None,
        duration: None,
        duration_secs: None,
        thumbnail,
        subtitle,
    };
    let ep = r.get("navigationEndpoint")?;
    if let Some(b) = ep.get("browseEndpoint") {
        let id = nav_str(b, &["browseId"])?.to_string();
        item.kind = match nav_str(
            b,
            &[
                "browseEndpointContextSupportedConfigs",
                "browseEndpointContextMusicConfig",
                "pageType",
            ],
        )? {
            "MUSIC_PAGE_TYPE_ALBUM" | "MUSIC_PAGE_TYPE_AUDIOBOOK" => Kind::Album,
            "MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL" => Kind::Artist,
            "MUSIC_PAGE_TYPE_PLAYLIST" => Kind::Playlist,
            _ => return None, // podcasts and other page types
        };
        item.browse_id = Some(match (item.kind, id.strip_prefix("VL")) {
            (Kind::Playlist, Some(rest)) => rest.to_string(),
            _ => id,
        });
    } else if let Some(w) = ep.get("watchEndpoint") {
        match nav_str(w, &["videoId"]) {
            Some(v) => {
                item.video_id = Some(v.to_string());
                let atv = nav_str(
                    w,
                    &[
                        "watchEndpointMusicSupportedConfigs",
                        "watchEndpointMusicConfig",
                        "musicVideoType",
                    ],
                ) == Some("MUSIC_VIDEO_TYPE_ATV");
                item.kind = if atv { Kind::Song } else { Kind::Video };
            }
            None => {
                // a radio / mix: opens as a playlist
                item.kind = Kind::Playlist;
                item.browse_id = Some(nav_str(w, &["playlistId"])?.to_string());
            }
        }
    } else {
        let w = ep.get("watchPlaylistEndpoint")?;
        item.kind = Kind::Playlist;
        item.browse_id = Some(nav_str(w, &["playlistId"])?.to_string());
    }
    Some(item)
}
