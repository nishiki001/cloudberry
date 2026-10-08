//! Editing the user's playlists: add / remove tracks, create a playlist, and the list of
//! playlists a track can be added to (the "Add to playlist" dialog of the web app).
//! Request bodies are built by pure functions so they can be unit-tested.
use super::client::Client;
use super::nav::{nav, nav_str};
use anyhow::{Result, bail};
use serde_json::{Value, json};

/// A playlist the user can add tracks to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTarget {
    pub id: String,
    pub title: String,
    pub thumbnail: Option<String>,
    /// "Playlist • 12 songs" or similar, when the library listing knows it.
    pub detail: String,
}

pub fn add_body(playlist: &str, video_ids: &[String], allow_duplicates: bool) -> Value {
    let actions: Vec<Value> = video_ids
        .iter()
        .map(|v| {
            let mut a = json!({"action": "ACTION_ADD_VIDEO", "addedVideoId": v});
            if allow_duplicates {
                // ytmusicapi: with duplicates allowed the dedupe check is skipped
                a["dedupeOption"] = json!("DEDUPE_OPTION_SKIP");
            }
            a
        })
        .collect();
    json!({"playlistId": playlist, "actions": actions})
}

pub fn remove_body(playlist: &str, entries: &[(String, String)]) -> Value {
    let actions: Vec<Value> = entries
        .iter()
        .map(|(video, set)| {
            json!({"action": "ACTION_REMOVE_VIDEO", "removedVideoId": video, "setVideoId": set})
        })
        .collect();
    json!({"playlistId": playlist, "actions": actions})
}

/// `privacy`: PRIVATE, UNLISTED or PUBLIC (anything else = PRIVATE).
pub fn create_body(title: &str, video_ids: &[String], privacy: &str) -> Value {
    let privacy = match privacy {
        "PUBLIC" | "UNLISTED" => privacy,
        _ => "PRIVATE",
    };
    json!({"title": title, "description": "", "privacyStatus": privacy, "videoIds": video_ids})
}

/// (videoId, setVideoId) of every track an edit added; the set ids are what an undo needs.
pub fn parse_added(v: &Value) -> Vec<(String, String)> {
    v.get("playlistEditResults")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let d = r.get("playlistEditVideoAddedResultData")?;
            Some((
                d.get("videoId")?.as_str()?.to_string(),
                d.get("setVideoId")?.as_str()?.to_string(),
            ))
        })
        .collect()
}

fn succeeded(v: &Value) -> bool {
    v.get("status")
        .and_then(Value::as_str)
        .is_some_and(|s| s.contains("SUCCEEDED"))
}

/// Playlists from a `playlist/get_add_to_playlist` response, in the order the server sent them.
pub fn parse_targets(v: &Value) -> Vec<AddTarget> {
    let mut out = Vec::new();
    let mut stack = vec![v];
    // the renderer sits at varying depths (action popups): find it, then read its list
    while let Some(x) = stack.pop() {
        match x {
            Value::Object(o) => {
                if let Some(r) = o.get("addToPlaylistRenderer") {
                    for p in r
                        .get("playlists")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let o = p.get("playlistAddToOptionRenderer").unwrap_or(p);
                        if let (Some(id), Some(title)) = (
                            nav_str(o, &["playlistId"]),
                            nav_str(o, &["title", "runs", "0", "text"])
                                .or_else(|| nav_str(o, &["title", "simpleText"])),
                        ) {
                            out.push(AddTarget {
                                id: id.to_string(),
                                title: title.trim().to_string(),
                                thumbnail: None,
                                detail: String::new(),
                            });
                        }
                    }
                    return out;
                }
                stack.extend(o.values());
            }
            Value::Array(a) => stack.extend(a.iter()),
            _ => {}
        }
    }
    out
}

/// Enrich `targets` with the library's thumbnail / detail line; with no targets at all the
/// library's playlists are used (the ones that are not albums).
pub fn merge_library(
    mut targets: Vec<AddTarget>,
    lib: Vec<crate::core::model::Item>,
) -> Vec<AddTarget> {
    let lib: Vec<_> = lib
        .into_iter()
        .filter(|i| i.kind == crate::core::model::Kind::Playlist)
        .collect();
    if targets.is_empty() {
        return lib
            .into_iter()
            .filter(|i| !matches!(i.browse_id.as_deref(), Some("LM" | "SE")))
            .filter_map(|i| {
                Some(AddTarget {
                    id: i.browse_id?,
                    title: i.title,
                    thumbnail: i.thumbnail,
                    detail: i.subtitle,
                })
            })
            .collect();
    }
    for t in &mut targets {
        if let Some(i) = lib.iter().find(|i| i.browse_id.as_deref() == Some(&t.id)) {
            t.thumbnail.clone_from(&i.thumbnail);
            t.detail.clone_from(&i.subtitle);
        }
    }
    targets
}

impl Client {
    pub async fn add_targets(&self) -> Result<Vec<AddTarget>> {
        let ask = |ids: Value| {
            self.post(
                "playlist/get_add_to_playlist",
                json!({"videoIds": ids, "excludeWatchLater": false}),
            )
        };
        // an empty list of ids is not accepted everywhere: retry with a dummy track
        let mut first_err = None;
        let mut targets = match ask(json!([])).await {
            Ok(v) => parse_targets(&v),
            Err(e) => {
                first_err = Some(e);
                Vec::new()
            }
        };
        if targets.is_empty() {
            match ask(json!(["dQw4w9WgXcQ"])).await {
                Ok(v) => targets = parse_targets(&v),
                Err(e) => first_err = Some(e),
            }
        }
        // thumbnails and track counts come from the library listing, which also stands in when
        // the add-to-playlist call gave nothing usable
        match self.library_playlists().await {
            Ok(lib) => Ok(merge_library(targets, lib)),
            Err(e) if targets.is_empty() => Err(first_err.unwrap_or(e)),
            Err(_) => Ok(targets),
        }
    }

    /// Add tracks; returns what was added (with the ids an undo needs).
    pub async fn add_to_playlist(
        &self,
        playlist: &str,
        video_ids: &[String],
        allow_duplicates: bool,
    ) -> Result<Vec<(String, String)>> {
        let v = self
            .post(
                "browse/edit_playlist",
                add_body(playlist, video_ids, allow_duplicates),
            )
            .await?;
        if !succeeded(&v) {
            bail!("YouTube Music did not accept the change");
        }
        Ok(parse_added(&v))
    }

    pub async fn remove_from_playlist(
        &self,
        playlist: &str,
        entries: &[(String, String)],
    ) -> Result<()> {
        let v = self
            .post("browse/edit_playlist", remove_body(playlist, entries))
            .await?;
        if !succeeded(&v) {
            bail!("YouTube Music did not accept the change");
        }
        Ok(())
    }

    /// Create a playlist with some tracks; returns its id.
    pub async fn create_playlist(
        &self,
        title: &str,
        video_ids: &[String],
        privacy: &str,
    ) -> Result<String> {
        let v = self
            .post("playlist/create", create_body(title, video_ids, privacy))
            .await?;
        match nav(&v, &["playlistId"]).and_then(Value::as_str) {
            Some(id) => Ok(id.to_string()),
            None => bail!("YouTube Music did not create the playlist"),
        }
    }
}

#[cfg(test)]
#[path = "playlist_edit_tests.rs"]
mod tests;
