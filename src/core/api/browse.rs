//! Artist and album pages.
use super::client::Client;
use super::nav::{nav, nav_str};
use super::parse::{best_thumbnail, parse_list_item};
use crate::core::model::{Item, Kind};
use anyhow::Result;
use serde_json::{Value, json};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ArtistPage {
    pub name: String,
    pub thumbnail: Option<String>,
    pub songs: Vec<Item>,
    /// Albums, singles and EPs.
    pub albums: Vec<Item>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AlbumPage {
    pub title: String,
    pub artist: String,
    pub artist_id: Option<String>,
    pub year: Option<String>,
    pub thumbnail: Option<String>,
    pub tracks: Vec<Item>,
}

fn two_row_album(r: &Value) -> Option<Item> {
    let id = nav_str(r, &["navigationEndpoint", "browseEndpoint", "browseId"])?;
    if !id.starts_with("MPRE") {
        return None;
    }
    let subtitle: String = nav(r, &["subtitle", "runs"])
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get("text")?.as_str()).collect())
        .unwrap_or_default();
    Some(Item {
        kind: Kind::Album,
        title: nav_str(r, &["title", "runs", "0", "text"])?.to_string(),
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
                "thumbnailRenderer",
                "musicThumbnailRenderer",
                "thumbnail",
                "thumbnails",
            ],
        )
        .and_then(best_thumbnail),
        subtitle,
    })
}

pub fn parse_artist(v: &Value) -> ArtistPage {
    let mut page = ArtistPage {
        name: ["musicImmersiveHeaderRenderer", "musicVisualHeaderRenderer"]
            .iter()
            .find_map(|h| nav_str(v, &["header", h, "title", "runs", "0", "text"]))
            .unwrap_or_default()
            .to_string(),
        thumbnail: nav(
            v,
            &[
                "header",
                "musicImmersiveHeaderRenderer",
                "thumbnail",
                "musicThumbnailRenderer",
                "thumbnail",
                "thumbnails",
            ],
        )
        .and_then(best_thumbnail),
        ..Default::default()
    };
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
    for sec in sections.into_iter().flatten() {
        if let Some(shelf) = sec.get("musicShelfRenderer") {
            for c in shelf
                .get("contents")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(i) = c
                    .get("musicResponsiveListItemRenderer")
                    .and_then(parse_list_item)
                    .filter(|i| i.video_id.is_some())
                {
                    page.songs.push(i);
                }
            }
        } else if let Some(car) = sec.get("musicCarouselShelfRenderer") {
            for c in car
                .get("contents")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(a) = c.get("musicTwoRowItemRenderer").and_then(two_row_album) {
                    page.albums.push(a);
                }
            }
        }
    }
    page
}

pub fn parse_album(v: &Value, browse_id: &str) -> AlbumPage {
    let hdr = nav(
        v,
        &[
            "contents",
            "twoColumnBrowseResultsRenderer",
            "tabs",
            "0",
            "tabRenderer",
            "content",
            "sectionListRenderer",
            "contents",
            "0",
            "musicResponsiveHeaderRenderer",
        ],
    );
    let title = hdr
        .and_then(|h| nav_str(h, &["title", "runs", "0", "text"]))
        .unwrap_or_default()
        .to_string();
    let artist = hdr
        .and_then(|h| nav_str(h, &["straplineTextOne", "runs", "0", "text"]))
        .unwrap_or_default()
        .to_string();
    let artist_id = hdr
        .and_then(|h| {
            nav_str(
                h,
                &[
                    "straplineTextOne",
                    "runs",
                    "0",
                    "navigationEndpoint",
                    "browseEndpoint",
                    "browseId",
                ],
            )
        })
        .map(String::from);
    let year = hdr
        .and_then(|h| nav(h, &["subtitle", "runs"]))
        .and_then(Value::as_array)
        .and_then(|a| a.last())
        .and_then(|r| r.get("text")?.as_str())
        .filter(|t| t.len() == 4 && t.bytes().all(|b| b.is_ascii_digit()))
        .map(String::from);
    let thumbnail = hdr
        .and_then(|h| {
            nav(
                h,
                &[
                    "thumbnail",
                    "musicThumbnailRenderer",
                    "thumbnail",
                    "thumbnails",
                ],
            )
        })
        .and_then(best_thumbnail);
    let rows = nav(
        v,
        &[
            "contents",
            "twoColumnBrowseResultsRenderer",
            "secondaryContents",
            "sectionListRenderer",
            "contents",
            "0",
            "musicShelfRenderer",
            "contents",
        ],
    )
    .and_then(Value::as_array);
    let mut tracks = Vec::new();
    for c in rows.into_iter().flatten() {
        if let Some(mut i) = c
            .get("musicResponsiveListItemRenderer")
            .and_then(parse_list_item)
            .filter(|i| i.video_id.is_some())
        {
            // rows don't repeat album/artist/cover: take them from the header
            i.album = Some(title.clone());
            i.album_id = Some(browse_id.to_string());
            if i.artists.is_empty() && !artist.is_empty() {
                i.artists.push(crate::core::model::Artist {
                    name: artist.clone(),
                    id: artist_id.clone(),
                });
            }
            if i.thumbnail.is_none() {
                i.thumbnail.clone_from(&thumbnail);
            }
            tracks.push(i);
        }
    }
    AlbumPage {
        title,
        artist,
        artist_id,
        year,
        thumbnail,
        tracks,
    }
}

impl Client {
    pub async fn artist(&self, browse_id: &str) -> Result<ArtistPage> {
        Ok(parse_artist(
            &self.post("browse", json!({"browseId": browse_id})).await?,
        ))
    }

    pub async fn album(&self, browse_id: &str) -> Result<AlbumPage> {
        Ok(parse_album(
            &self.post("browse", json!({"browseId": browse_id})).await?,
            browse_id,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(name: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(format!("tests/fixtures/{name}.json")).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn artist_page() {
        let a = parse_artist(&load("artist_page"));
        assert_eq!(a.name, "Daft Punk");
        assert_eq!(a.songs.len(), 3);
        assert!(a.songs[0].video_id.is_some());
        assert!(a.albums.len() >= 2);
        assert!(
            a.albums[0]
                .browse_id
                .as_deref()
                .unwrap()
                .starts_with("MPRE")
        );
    }

    #[test]
    fn album_page() {
        let a = parse_album(&load("album_page"), "MPREb_test");
        assert_eq!(a.title, "Random Access Memories");
        assert_eq!(a.artist, "Daft Punk");
        assert_eq!(a.year.as_deref(), Some("2013"));
        assert_eq!(a.tracks.len(), 3);
        assert_eq!(a.tracks[0].title, "Give Life Back to Music");
        assert_eq!(a.tracks[0].album.as_deref(), Some("Random Access Memories"));
        assert_eq!(a.tracks[0].duration_secs, Some(276));
        assert_eq!(a.tracks[0].artists[0].name, "Daft Punk");
    }

    #[test]
    fn garbage() {
        assert!(parse_artist(&json!({})).songs.is_empty());
        assert!(parse_album(&json!([1]), "x").tracks.is_empty());
    }
}
