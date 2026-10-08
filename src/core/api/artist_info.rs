//! The artist page: header (name, subscribers, description, banner, play / shuffle / radio),
//! top songs with play counts, and shelves of albums, singles, videos, featured-on and related
//! artists. Shelves reuse the Discover parsers. Never panics.
use super::discover_parse::{Shelf, ShelfKind, parse_shelves};
use super::nav::{nav, nav_str};
use super::parse::{best_thumbnail, parse_list_item};
use crate::core::model::Item;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtistInfo {
    /// Channel id (`UC…`).
    pub id: String,
    pub name: String,
    pub description: String,
    /// "7.19M subscribers" style text.
    pub subscribers: Option<String>,
    /// "79.6M monthly audience" style text.
    pub monthly: Option<String>,
    /// The wide header picture.
    pub image: Option<String>,
    pub songs: Vec<Item>,
    /// Play counts of `songs` ("1.1B plays"), same order.
    pub plays: Vec<String>,
    /// Playlist behind "show all" of the top songs.
    pub songs_playlist: Option<String>,
    /// Playlist ids for the Play / Shuffle and Radio buttons.
    pub play: Option<String>,
    pub radio: Option<String>,
    pub shelves: Vec<Shelf>,
}

fn playlist_of(button: &Value) -> Option<String> {
    let ep = nav(button, &["buttonRenderer", "navigationEndpoint"])?;
    nav_str(ep, &["watchPlaylistEndpoint", "playlistId"])
        .or_else(|| nav_str(ep, &["watchEndpoint", "playlistId"]))
        .map(String::from)
}

/// "1.1B plays" from the third column of a top-songs row.
fn plays_of(row: &Value) -> String {
    nav_str(
        row,
        &[
            "musicResponsiveListItemRenderer",
            "flexColumns",
            "2",
            "musicResponsiveListItemFlexColumnRenderer",
            "text",
            "runs",
            "0",
            "text",
        ],
    )
    .filter(|t| t.ends_with("plays") || t.ends_with("views"))
    .unwrap_or_default()
    .to_string()
}

/// `id` is the channel id the page was requested with.
pub fn parse_artist_info(v: &Value, id: &str) -> ArtistInfo {
    let mut a = ArtistInfo {
        id: id.to_string(),
        ..Default::default()
    };
    if let Some(h) = nav(v, &["header", "musicImmersiveHeaderRenderer"])
        .or_else(|| nav(v, &["header", "musicVisualHeaderRenderer"]))
    {
        a.name = nav_str(h, &["title", "runs", "0", "text"])
            .unwrap_or_default()
            .to_string();
        a.description = nav_str(h, &["description", "runs", "0", "text"])
            .unwrap_or_default()
            .to_string();
        a.monthly = nav_str(h, &["monthlyListenerCount", "runs", "0", "text"]).map(String::from);
        a.subscribers = nav_str(
            h,
            &[
                "subscriptionButton",
                "subscribeButtonRenderer",
                "subscriberCountText",
                "runs",
                "0",
                "text",
            ],
        )
        .map(|s| format!("{s} subscribers"));
        a.image = nav(
            h,
            &[
                "thumbnail",
                "musicThumbnailRenderer",
                "thumbnail",
                "thumbnails",
            ],
        )
        .and_then(best_thumbnail);
        a.play = h.get("playButton").and_then(playlist_of);
        a.radio = h.get("startRadioButton").and_then(playlist_of);
    }
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
    let sections: &[Value] = sections.map_or(&[], Vec::as_slice);
    for (sec, shelf) in sections.iter().zip(
        sections
            .iter()
            .map(|s| parse_shelves(std::slice::from_ref(s))),
    ) {
        let Some(shelf) = shelf.into_iter().next() else {
            continue;
        };
        if a.songs.is_empty()
            && shelf.kind == ShelfKind::Songs
            && sec.get("musicShelfRenderer").is_some()
        {
            // the top songs list (its title links to the full playlist)
            let body = &sec["musicShelfRenderer"];
            a.songs_playlist = nav_str(
                body,
                &[
                    "title",
                    "runs",
                    "0",
                    "navigationEndpoint",
                    "browseEndpoint",
                    "browseId",
                ],
            )
            .map(|b| b.strip_prefix("VL").unwrap_or(b).to_string());
            for row in body
                .get("contents")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(item) = row
                    .get("musicResponsiveListItemRenderer")
                    .and_then(parse_list_item)
                {
                    a.songs.push(item);
                    a.plays.push(plays_of(row));
                }
            }
        } else {
            a.shelves.push(shelf);
        }
    }
    a
}

impl super::client::Client {
    pub async fn artist_info(&self, id: &str) -> anyhow::Result<ArtistInfo> {
        let v = self
            .post("browse", serde_json::json!({"browseId": id}))
            .await?;
        Ok(parse_artist_info(&v, id))
    }

    /// Radio from a playlist id (`RD…`, as the artist page's radio button gives).
    pub async fn radio_from_playlist(&self, playlist_id: &str) -> anyhow::Result<Vec<Item>> {
        let body = serde_json::json!({
            "playlistId": playlist_id,
            "isAudioOnly": true,
            "enablePersistentPlaylistPanel": true,
        });
        Ok(super::watch::parse_watch(&self.post("next", body).await?))
    }
}

#[cfg(test)]
#[path = "artist_info_tests.rs"]
mod tests;
