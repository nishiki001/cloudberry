//! Parsers for InnerTube list items. Never panic; unparseable items are skipped.
use super::nav::{nav, nav_str};
use crate::core::model::{Artist, Item, Kind, parse_duration};
use serde_json::Value;

const SEP: &str = " • ";

/// Largest thumbnail URL, upscaled for googleusercontent hosts (REFERENCE §thumbnails).
pub fn best_thumbnail(thumbs: &Value) -> Option<String> {
    let url = thumbs.as_array()?.last()?.get("url")?.as_str()?;
    Some(upscale(url))
}

pub fn upscale(url: &str) -> String {
    let host_ok = url.contains("googleusercontent.com") || url.contains("ggpht.com");
    match url.rfind('=') {
        Some(i) if host_ok && url[i + 1..].starts_with('w') => {
            format!("{}=w544-h544-l90-rj", &url[..i])
        }
        _ => url.to_string(),
    }
}

fn run_page_type(run: &Value) -> Option<&str> {
    nav_str(
        run,
        &[
            "navigationEndpoint",
            "browseEndpoint",
            "browseEndpointContextSupportedConfigs",
            "browseEndpointContextMusicConfig",
            "pageType",
        ],
    )
}

fn run_browse_id(run: &Value) -> Option<String> {
    nav_str(run, &["navigationEndpoint", "browseEndpoint", "browseId"]).map(String::from)
}

fn flex_runs(item: &Value, col: usize) -> Vec<&Value> {
    let col = col.to_string();
    nav(
        item,
        &[
            "flexColumns",
            &col,
            "musicResponsiveListItemFlexColumnRenderer",
            "text",
            "runs",
        ],
    )
    .and_then(Value::as_array)
    .map(|a| a.iter().collect())
    .unwrap_or_default()
}

fn video_id(item: &Value) -> Option<String> {
    let cands = [
        nav_str(item, &["playlistItemData", "videoId"]),
        nav_str(
            item,
            &[
                "overlay",
                "musicItemThumbnailOverlayRenderer",
                "content",
                "musicPlayButtonRenderer",
                "playNavigationEndpoint",
                "watchEndpoint",
                "videoId",
            ],
        ),
        flex_runs(item, 0)
            .first()
            .and_then(|r| nav_str(r, &["navigationEndpoint", "watchEndpoint", "videoId"])),
    ];
    cands.into_iter().flatten().next().map(String::from)
}

fn music_video_type(item: &Value) -> Option<&str> {
    nav_str(
        item,
        &[
            "overlay",
            "musicItemThumbnailOverlayRenderer",
            "content",
            "musicPlayButtonRenderer",
            "playNavigationEndpoint",
            "watchEndpoint",
            "watchEndpointMusicSupportedConfigs",
            "watchEndpointMusicConfig",
            "musicVideoType",
        ],
    )
}

/// Parse one `musicResponsiveListItemRenderer`.
pub fn parse_list_item(item: &Value) -> Option<Item> {
    let title = flex_runs(item, 0)
        .first()?
        .get("text")?
        .as_str()?
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let item_browse =
        nav_str(item, &["navigationEndpoint", "browseEndpoint", "browseId"]).map(String::from);
    let item_type = nav_str(
        item,
        &[
            "navigationEndpoint",
            "browseEndpoint",
            "browseEndpointContextSupportedConfigs",
            "browseEndpointContextMusicConfig",
            "pageType",
        ],
    );
    let vid = video_id(item);

    let mut artists: Vec<Artist> = Vec::new();
    let mut album = None;
    let mut album_id = None;
    let mut duration = None;
    let mut label: Option<String> = None;
    let mut subtitle = String::new();
    let mut loose: Vec<String> = Vec::new();
    for (i, run) in flex_runs(item, 1).into_iter().enumerate() {
        let Some(text) = run.get("text").and_then(Value::as_str) else {
            continue;
        };
        subtitle.push_str(text);
        if text == SEP || text.trim().is_empty() {
            continue;
        }
        match run_page_type(run) {
            Some("MUSIC_PAGE_TYPE_ARTIST") | Some("MUSIC_PAGE_TYPE_USER_CHANNEL") => {
                artists.push(Artist {
                    name: text.into(),
                    id: run_browse_id(run),
                })
            }
            Some("MUSIC_PAGE_TYPE_ALBUM") => {
                album = Some(text.to_string());
                album_id = run_browse_id(run);
            }
            _ if parse_duration(text).is_some() => duration = Some(text.to_string()),
            _ if i == 0
                && matches!(
                    text,
                    "Song" | "Video" | "Album" | "Artist" | "Playlist" | "EP" | "Single"
                ) =>
            {
                label = Some(text.to_string())
            }
            _ => loose.push(text.to_string()),
        }
    }

    let kind = if let Some(v) = &vid {
        let _ = v;
        match (music_video_type(item), label.as_deref()) {
            (Some("MUSIC_VIDEO_TYPE_ATV"), _) | (_, Some("Song")) => Kind::Song,
            _ => Kind::Video,
        }
    } else {
        match item_type {
            Some("MUSIC_PAGE_TYPE_ALBUM") => Kind::Album,
            Some("MUSIC_PAGE_TYPE_ARTIST") => Kind::Artist,
            Some("MUSIC_PAGE_TYPE_PLAYLIST") => Kind::Playlist,
            _ => return None,
        }
    };

    // Plain-text owner/artist (videos, playlists, albums by "Various Artists"): first loose run.
    if artists.is_empty()
        && kind != Kind::Artist
        && let Some(first) = loose.first().filter(|t| {
            !t.ends_with("views")
                && !t.ends_with("plays")
                && !(t.len() == 4 && t.bytes().all(|b| b.is_ascii_digit()))
        })
    {
        artists.push(Artist {
            name: first.clone(),
            id: None,
        });
    }

    // Playlist/history rows: [title][artists][album] + fixed duration column.
    if let Some(fixed) = nav(
        item,
        &[
            "fixedColumns",
            "0",
            "musicResponsiveListItemFixedColumnRenderer",
            "text",
            "runs",
            "0",
            "text",
        ],
    )
    .and_then(Value::as_str)
    {
        if duration.is_none() && parse_duration(fixed).is_some() {
            duration = Some(fixed.to_string());
        }
        if album.is_none()
            && let Some(r) = flex_runs(item, 2).first()
        {
            // album pages put the play count in this column
            album = r
                .get("text")
                .and_then(Value::as_str)
                .filter(|t| !t.ends_with("plays"))
                .map(String::from);
            album_id = run_browse_id(r);
        }
    }

    let thumb = nav(
        item,
        &[
            "thumbnail",
            "musicThumbnailRenderer",
            "thumbnail",
            "thumbnails",
        ],
    )
    .and_then(best_thumbnail);
    Some(Item {
        kind,
        title,
        video_id: vid,
        browse_id: item_browse.map(|b| {
            b.strip_prefix("VL")
                .map(String::from)
                .filter(|_| kind == Kind::Playlist)
                .unwrap_or(b)
        }),
        artists,
        album,
        album_id,
        duration_secs: duration.as_deref().and_then(parse_duration),
        duration,
        thumbnail: thumb,
        subtitle,
    })
}

#[cfg(test)]
pub use super::parse_search::parse_search;
pub use super::parse_search::{SearchPage, parse_search_page};

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
