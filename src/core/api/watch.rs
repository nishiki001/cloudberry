//! `next` endpoint: radio / up-next lists.
use super::client::Client;
use super::nav::{nav, nav_str};
use super::parse::best_thumbnail;
use crate::core::model::{Artist, Item, Kind, parse_duration};
use anyhow::Result;
use serde_json::{Value, json};

fn runs_text(v: &Value) -> Option<String> {
    let runs = v.get("runs")?.as_array()?;
    Some(
        runs.iter()
            .filter_map(|r| r.get("text")?.as_str())
            .collect(),
    )
}

fn parse_panel_item(r: &Value) -> Option<Item> {
    let video_id = r.get("videoId")?.as_str()?.to_string();
    let title = runs_text(r.get("title")?)?;
    let mut artists = Vec::new();
    let mut album = None;
    let mut album_id = None;
    for run in nav(r, &["longBylineText", "runs"])
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let text = run.get("text").and_then(Value::as_str).unwrap_or("");
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
        let id =
            nav_str(run, &["navigationEndpoint", "browseEndpoint", "browseId"]).map(String::from);
        match page {
            Some("MUSIC_PAGE_TYPE_ARTIST") | Some("MUSIC_PAGE_TYPE_USER_CHANNEL") => {
                artists.push(Artist {
                    name: text.into(),
                    id,
                })
            }
            Some("MUSIC_PAGE_TYPE_ALBUM") => {
                album = Some(text.to_string());
                album_id = id;
            }
            _ => {}
        }
    }
    let duration = r.get("lengthText").and_then(runs_text);
    let is_song = nav_str(
        r,
        &[
            "navigationEndpoint",
            "watchEndpoint",
            "watchEndpointMusicSupportedConfigs",
            "watchEndpointMusicConfig",
            "musicVideoType",
        ],
    ) != Some("MUSIC_VIDEO_TYPE_UGC");
    Some(Item {
        kind: if is_song { Kind::Song } else { Kind::Video },
        title,
        video_id: Some(video_id),
        browse_id: None,
        artists,
        album,
        album_id,
        duration_secs: duration.as_deref().and_then(parse_duration),
        duration,
        thumbnail: nav(r, &["thumbnail", "thumbnails"]).and_then(best_thumbnail),
        subtitle: r
            .get("longBylineText")
            .and_then(runs_text)
            .unwrap_or_default(),
    })
}

/// Items of the playlist panel in a `next` response (wrapper entries are unwrapped).
pub fn parse_watch(v: &Value) -> Vec<Item> {
    let contents = nav(
        v,
        &[
            "contents",
            "singleColumnMusicWatchNextResultsRenderer",
            "tabbedRenderer",
            "watchNextTabbedResultsRenderer",
            "tabs",
            "0",
            "tabRenderer",
            "content",
            "musicQueueRenderer",
            "content",
            "playlistPanelRenderer",
            "contents",
        ],
    )
    .and_then(Value::as_array);
    contents
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let r = c.get("playlistPanelVideoRenderer").or_else(|| {
                nav(
                    c,
                    &[
                        "playlistPanelVideoWrapperRenderer",
                        "primaryRenderer",
                        "playlistPanelVideoRenderer",
                    ],
                )
            })?;
            let item = parse_panel_item(r);
            if item.is_none() {
                tracing::debug!("skipped unparseable radio item");
            }
            item
        })
        .collect()
}

impl Client {
    /// Radio ("automix") for a video; the first item is the seed itself.
    pub async fn radio(&self, video_id: &str) -> Result<Vec<Item>> {
        let body = json!({
            "videoId": video_id,
            "playlistId": format!("RDAMVM{video_id}"),
            "isAudioOnly": true,
            "enablePersistentPlaylistPanel": true,
            "tunerSettingValue": "AUTOMIX_SETTING_NORMAL",
        });
        Ok(parse_watch(&self.post("next", body).await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_radio_fixture() {
        let s = std::fs::read_to_string("tests/fixtures/next_radio.json").unwrap();
        let items = parse_watch(&serde_json::from_str(&s).unwrap());
        assert!(items.len() >= 4);
        assert!(
            items
                .iter()
                .all(|i| i.video_id.is_some() && !i.title.is_empty())
        );
        assert!(!items[1].artists.is_empty());
        assert!(items[1].duration_secs.is_some());
        assert!(items[1].thumbnail.is_some());
    }

    #[test]
    fn garbage_is_empty() {
        assert!(parse_watch(&json!({"contents": []})).is_empty());
    }
}
