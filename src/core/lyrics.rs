//! Lyrics: LRCLIB (synced) first, YouTube Music lyrics (plain) as fallback.
use super::api::client::Client;
use super::api::nav::{nav, nav_str};
use super::lrc::parse_lrc;
use super::model::Item;
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;

const UA: &str = concat!(
    "cloudberry/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/nishiki001/cloudberry)"
);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    /// Seconds; 0.0 for unsynced text.
    pub t: f64,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Lyrics {
    pub synced: bool,
    pub source: String,
    pub lines: Vec<Line>,
}

impl Lyrics {
    #[allow(dead_code)] // used by the Lyrics tab
    /// Index of the line being sung at `pos` seconds (synced lyrics only).
    pub fn current(&self, pos: f64) -> Option<usize> {
        if !self.synced {
            return None;
        }
        self.lines.iter().rposition(|l| l.t <= pos)
    }
}

fn plain_lines(text: &str) -> Vec<Line> {
    text.lines()
        .map(|l| Line {
            t: 0.0,
            text: l.trim_end().to_string(),
        })
        .collect()
}

fn artist_of(i: &Item) -> String {
    i.artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub struct LyricsClient {
    http: reqwest::Client, // no cookies
    lrclib_enabled: bool,
}

impl LyricsClient {
    pub fn new(lrclib_enabled: bool) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .user_agent(UA)
                .build()?,
            lrclib_enabled,
        })
    }

    pub async fn lrclib(&self, item: &Item) -> Result<Option<Lyrics>> {
        if !self.lrclib_enabled {
            return Ok(None);
        }
        let artist = item
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default();
        // /api/get needs a duration; without one, or when it finds nothing usable, search
        if let Some(dur) = item.duration_secs.map(|d| d.to_string()) {
            let resp = self
                .http
                .get("https://lrclib.net/api/get")
                .query(&[
                    ("track_name", item.title.as_str()),
                    ("artist_name", artist.as_str()),
                    ("album_name", item.album.as_deref().unwrap_or("")),
                    ("duration", dur.as_str()),
                ])
                .send()
                .await
                .context("lrclib request")?;
            if resp.status().is_success() {
                let v: Value = resp.json().await?;
                if let Some(l) = from_lrclib(&v, item.duration_secs) {
                    return Ok(Some(l));
                }
            } else if !resp.status().is_client_error() {
                anyhow::bail!("lrclib HTTP {}", resp.status());
            }
        }
        Ok(self
            .lrclib_search(item)
            .await?
            .and_then(|v| from_lrclib(&v, item.duration_secs)))
    }

    async fn lrclib_search(&self, item: &Item) -> Result<Option<Value>> {
        let q = format!("{} {}", artist_of(item), item.title);
        let v: Value = self
            .http
            .get("https://lrclib.net/api/search")
            .query(&[("q", q.as_str())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let want = item.duration_secs.map(f64::from);
        let best = v
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| {
                c.get("syncedLyrics").is_some_and(|s| s.is_string())
                    || c.get("plainLyrics").is_some_and(|s| s.is_string())
            })
            .min_by(|a, b| {
                let d = |c: &Value| match (want, c.get("duration").and_then(Value::as_f64)) {
                    (Some(w), Some(x)) => (w - x).abs(),
                    _ => f64::MAX,
                };
                d(a).total_cmp(&d(b))
            });
        // reject wildly different tracks
        Ok(best
            .filter(
                |c| match (want, c.get("duration").and_then(Value::as_f64)) {
                    (Some(w), Some(x)) => (w - x).abs() <= 8.0,
                    (Some(_), None) => false,
                    _ => true,
                },
            )
            .cloned())
    }
}

/// Too little text for a real song: community entries are sometimes vandalised or placeholders.
fn looks_junk(lines: &[Line], track_secs: Option<u32>) -> bool {
    let non_empty = lines.iter().filter(|l| !l.text.trim().is_empty()).count();
    non_empty < 3 && track_secs.is_none_or(|d| d > 60)
}

pub fn from_lrclib(v: &Value, track_secs: Option<u32>) -> Option<Lyrics> {
    if let Some(s) = v.get("syncedLyrics").and_then(Value::as_str) {
        let lines = parse_lrc(s);
        if !looks_junk(&lines, track_secs) {
            return Some(Lyrics {
                synced: true,
                source: "LRCLIB".into(),
                lines,
            });
        }
    }
    let plain = v.get("plainLyrics").and_then(Value::as_str)?;
    let lines = plain_lines(plain);
    (!looks_junk(&lines, track_secs)).then_some(Lyrics {
        synced: false,
        source: "LRCLIB".into(),
        lines,
    })
}

/// Lyrics browse id from a `next` response (tab titled "Lyrics").
pub fn lyrics_browse_id(next: &Value) -> Option<String> {
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
        (r.get("title")?.as_str()? == "Lyrics")
            .then(|| nav_str(r, &["endpoint", "browseEndpoint", "browseId"]).map(String::from))?
    })
}

pub fn from_ytm(v: &Value) -> Option<Lyrics> {
    let text = nav_str(
        v,
        &[
            "contents",
            "sectionListRenderer",
            "contents",
            "0",
            "musicDescriptionShelfRenderer",
            "description",
            "runs",
            "0",
            "text",
        ],
    )?;
    let source = nav_str(
        v,
        &[
            "contents",
            "sectionListRenderer",
            "contents",
            "0",
            "musicDescriptionShelfRenderer",
            "footer",
            "runs",
            "0",
            "text",
        ],
    )
    .unwrap_or("YouTube Music");
    (!text.trim().is_empty()).then(|| Lyrics {
        synced: false,
        source: source.to_string(),
        lines: plain_lines(text),
    })
}

impl Client {
    pub async fn ytm_lyrics(&self, video_id: &str) -> Result<Option<Lyrics>> {
        let next = self
            .post("next", json!({"videoId": video_id, "isAudioOnly": true}))
            .await?;
        let Some(id) = lyrics_browse_id(&next) else {
            return Ok(None);
        };
        Ok(from_ytm(
            &self.post("browse", json!({"browseId": id})).await?,
        ))
    }
}

/// LRCLIB first, then YouTube Music.
pub async fn fetch(
    lc: &LyricsClient,
    client: Option<&Client>,
    item: &Item,
) -> Result<Option<Lyrics>> {
    match lc.lrclib(item).await {
        Ok(Some(l)) => return Ok(Some(l)),
        Ok(None) => {}
        Err(e) => tracing::debug!("lrclib failed: {e:#}"),
    }
    match (client, item.video_id.as_deref()) {
        (Some(c), Some(v)) => c.ytm_lyrics(v).await,
        _ => Ok(None),
    }
}

#[cfg(test)]
#[path = "lyrics_tests.rs"]
mod tests;
