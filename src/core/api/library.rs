//! Library: playlists (with continuations), liked songs, library playlists, history.
use super::nav::{nav, nav_str};
use super::parse::{best_thumbnail, parse_list_item};
use crate::core::model::{Item, Kind};
use serde_json::Value;

/// Items of a playlist shelf plus the continuation token, if any (works on first pages and
/// on `appendContinuationItemsAction` pages).
fn shelf_items(contents: &[Value]) -> (Vec<Item>, Option<String>) {
    let mut items = Vec::new();
    let mut token = None;
    for c in contents {
        if let Some(r) = c.get("musicResponsiveListItemRenderer") {
            match parse_list_item(r) {
                Some(i) if i.video_id.is_some() => items.push(i),
                _ => tracing::debug!("skipped unparseable playlist item"),
            }
        } else if let Some(t) = nav_str(
            c,
            &[
                "continuationItemRenderer",
                "continuationEndpoint",
                "continuationCommand",
                "token",
            ],
        ) {
            token = Some(t.to_string());
        }
    }
    (items, token)
}

const SHELF_PATHS: [&[&str]; 2] = [
    &[
        "contents",
        "twoColumnBrowseResultsRenderer",
        "secondaryContents",
        "sectionListRenderer",
        "contents",
        "0",
        "musicPlaylistShelfRenderer",
        "contents",
    ],
    &[
        "contents",
        "singleColumnBrowseResultsRenderer",
        "tabs",
        "0",
        "tabRenderer",
        "content",
        "sectionListRenderer",
        "contents",
        "0",
        "musicPlaylistShelfRenderer",
        "contents",
    ],
];

pub fn parse_playlist_page(v: &Value) -> (Vec<Item>, Option<String>) {
    for path in SHELF_PATHS {
        if let Some(arr) = nav(v, path).and_then(Value::as_array) {
            return shelf_items(arr);
        }
    }
    match nav(
        v,
        &[
            "onResponseReceivedActions",
            "0",
            "appendContinuationItemsAction",
            "continuationItems",
        ],
    )
    .and_then(Value::as_array)
    {
        Some(arr) => shelf_items(arr),
        None => (Vec::new(), None),
    }
}

/// Playlists from the "FEmusic_liked_playlists" grid (first entry is "New playlist").
pub fn parse_library_playlists(v: &Value) -> Vec<Item> {
    let grid = nav(
        v,
        &[
            "contents",
            "singleColumnBrowseResultsRenderer",
            "tabs",
            "0",
            "tabRenderer",
            "content",
            "sectionListRenderer",
            "contents",
            "0",
            "gridRenderer",
            "items",
        ],
    )
    .and_then(Value::as_array);
    grid.into_iter()
        .flatten()
        .filter_map(|c| {
            let r = c.get("musicTwoRowItemRenderer")?;
            let id = nav_str(r, &["navigationEndpoint", "browseEndpoint", "browseId"])?;
            let title = nav_str(r, &["title", "runs", "0", "text"])?.to_string();
            let subtitle: String = nav(r, &["subtitle", "runs"])
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.get("text")?.as_str()).collect())
                .unwrap_or_default();
            Some(Item {
                // the same grid shape serves playlists (VL…) and albums (MPRE…)
                kind: if id.starts_with("MPRE") {
                    Kind::Album
                } else {
                    Kind::Playlist
                },
                title,
                video_id: None,
                browse_id: Some(id.strip_prefix("VL").unwrap_or(id).to_string()),
                artists: vec![],
                album: None,
                album_id: None,
                duration: None,
                duration_secs: None,
                thumbnail: nav(
                    r,
                    &[
                        "thumbnailRenderer",
                        "musicThumbnailRenderer",
                        "thumbnail",
                        "thumbnails",
                    ],
                )
                .and_then(best_thumbnail),
                subtitle,
            })
        })
        .collect()
}

/// Artists from "FEmusic_library_corpus_artists": a shelf of list rows (title + "N songs").
pub fn parse_library_artists(v: &Value) -> Vec<Item> {
    let rows = nav(
        v,
        &[
            "contents",
            "singleColumnBrowseResultsRenderer",
            "tabs",
            "0",
            "tabRenderer",
            "content",
            "sectionListRenderer",
            "contents",
            "0",
            "musicShelfRenderer",
            "contents",
        ],
    )
    .and_then(Value::as_array);
    rows.into_iter()
        .flatten()
        .filter_map(|c| {
            let r = c.get("musicResponsiveListItemRenderer")?;
            let id = nav_str(r, &["navigationEndpoint", "browseEndpoint", "browseId"])?;
            let title = nav_str(
                r,
                &[
                    "flexColumns",
                    "0",
                    "musicResponsiveListItemFlexColumnRenderer",
                    "text",
                    "runs",
                    "0",
                    "text",
                ],
            )?
            .to_string();
            let subtitle = nav_str(
                r,
                &[
                    "flexColumns",
                    "1",
                    "musicResponsiveListItemFlexColumnRenderer",
                    "text",
                    "runs",
                    "0",
                    "text",
                ],
            )
            .unwrap_or_default()
            .to_string();
            Some(Item {
                kind: Kind::Artist,
                title,
                video_id: None,
                browse_id: Some(id.to_string()),
                artists: vec![],
                album: None,
                album_id: None,
                duration: None,
                duration_secs: None,
                thumbnail: nav(
                    r,
                    &[
                        "thumbnail",
                        "musicThumbnailRenderer",
                        "thumbnail",
                        "thumbnails",
                    ],
                )
                .and_then(best_thumbnail),
                subtitle,
            })
        })
        .collect()
}

/// Listening history: every shelf of the page, flattened (newest first).
pub fn parse_history(v: &Value) -> Vec<Item> {
    let sections = nav(
        v,
        &[
            "contents",
            "singleColumnBrowseResultsRenderer",
            "tabs",
            "0",
            "tabRenderer",
            "content",
            "sectionListRenderer",
            "contents",
        ],
    )
    .and_then(Value::as_array);
    sections
        .into_iter()
        .flatten()
        .filter_map(|s| nav(s, &["musicShelfRenderer", "contents"]).and_then(Value::as_array))
        .flatten()
        .filter_map(|c| {
            c.get("musicResponsiveListItemRenderer")
                .and_then(parse_list_item)
        })
        .filter(|i| i.video_id.is_some())
        .collect()
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
