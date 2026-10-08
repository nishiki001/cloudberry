//! Raw Discover endpoints (home feed, explore pages), parsed by `discover_parse`.
use super::client::Client;
use super::discover_parse::{Mood, Page, Shelf, parse_moods, parse_page};
use super::nav::{nav, nav_str};
use anyhow::Result;
use serde_json::{Value, json};

impl Client {
    /// First response of a Discover section: `home`, `explore`, `new`, `charts` (with a country
    /// code, "ZZ" = global), `moods` or `mood` (a category's `params`).
    pub async fn discover_raw(
        &self,
        section: &str,
        country: &str,
        params: Option<&str>,
    ) -> Result<Value> {
        let body = match section {
            "home" => json!({"browseId": "FEmusic_home"}),
            "explore" => json!({"browseId": "FEmusic_explore"}),
            "new" => json!({"browseId": "FEmusic_new_releases_albums"}),
            "charts" => {
                json!({"browseId": "FEmusic_charts", "formData": {"selectedValues": [country]}})
            }
            "moods" => json!({"browseId": "FEmusic_moods_and_genres"}),
            "mood" => json!({"browseId": "FEmusic_moods_and_genres_category", "params": params}),
            other => anyhow::bail!("unknown discover section {other}"),
        };
        self.post("browse", body).await
    }

    /// Shelves of a section. The home feed follows `home_pages` continuation pages (3 shelves
    /// each); the other sections are a single response.
    pub async fn discover(
        &self,
        section: &str,
        country: &str,
        params: Option<&str>,
        home_pages: usize,
    ) -> Result<Page> {
        let mut page = parse_page(&self.discover_raw(section, country, params).await?);
        if section == "new" {
            // the new-releases grid has no heading of its own
            for s in page.shelves.iter_mut().filter(|s| s.title.is_empty()) {
                s.title = "New releases".into();
            }
        }
        if section == "home" {
            for _ in 1..home_pages {
                let Some(tok) = page.continuation.take() else {
                    break;
                };
                let more = parse_page(&self.post("browse", json!({"continuation": tok})).await?);
                page.shelves.extend(more.shelves);
                page.continuation = more.continuation;
            }
        }
        Ok(page)
    }

    /// Moods & genres categories.
    pub async fn discover_moods(&self) -> Result<Vec<Mood>> {
        Ok(parse_moods(&self.discover_raw("moods", "ZZ", None).await?))
    }

    /// "Related" shelves of a track (similar artists, more from the artist…), from the Related
    /// tab of its `next` response.
    pub async fn related(&self, video_id: &str) -> Result<Vec<Shelf>> {
        let next = self
            .post("next", json!({"videoId": video_id, "isAudioOnly": true}))
            .await?;
        let Some(id) = related_browse_id(&next) else {
            return Ok(Vec::new());
        };
        Ok(parse_page(&self.post("browse", json!({"browseId": id})).await?).shelves)
    }
}

/// Browse id of the tab titled "Related" in a `next` response.
pub fn related_browse_id(next: &Value) -> Option<String> {
    let tabs = nav(
        next,
        &[
            "contents",
            "singleColumnMusicWatchNextResultsRenderer",
            "tabbedRenderer",
            "watchNextTabbedResultsRenderer",
            "tabs",
        ],
    )?
    .as_array()?;
    tabs.iter().find_map(|t| {
        let r = t.get("tabRenderer")?;
        (r.get("title")?.as_str()? == "Related")
            .then(|| nav_str(r, &["endpoint", "browseEndpoint", "browseId"]).map(String::from))?
    })
}
