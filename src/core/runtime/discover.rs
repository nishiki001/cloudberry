//! Controller: Discover sections (cached copy first, refreshed in the background).
use super::*;
use crate::core::api::discover_parse::DiscoverData;
use crate::core::discover_cache;

/// Cache / event key of a section.
pub fn discover_key(section: &str, country: &str, params: Option<&str>) -> String {
    format!("{section}-{country}-{}", params.unwrap_or(""))
}

impl Controller {
    pub(super) fn load_discover(
        &self,
        section: String,
        country: String,
        params: Option<String>,
        force: bool,
    ) {
        let Some(client) = self.svc.client.clone() else {
            (self.sink)(Event::Error("could not load Discover".into()));
            return;
        };
        let sink = self.sink.clone();
        tokio::spawn(async move {
            let key = discover_key(&section, &country, params.as_deref());
            let dir = crate::config::cache_dir().join("discover");
            let t0 = std::time::Instant::now();
            let cached = {
                let (dir, key) = (dir.clone(), key.clone());
                tokio::task::spawn_blocking(move || {
                    discover_cache::load::<DiscoverData>(&dir, &key)
                })
                .await
                .ok()
                .flatten()
            };
            if let Some((data, age)) = &cached {
                tracing::info!(
                    key,
                    age,
                    ms = t0.elapsed().as_millis() as u64,
                    "discover from cache"
                );
                sink(Event::Discover {
                    key: key.clone(),
                    data: data.clone(),
                });
                if !force && !discover_cache::is_stale(*age) {
                    return;
                }
            }
            let fetched: anyhow::Result<DiscoverData> = if section == "moods" {
                client.discover_moods().await.map(|moods| DiscoverData {
                    shelves: Vec::new(),
                    moods,
                })
            } else {
                client
                    .discover(&section, &country, params.as_deref(), 3)
                    .await
                    .map(|p| DiscoverData {
                        shelves: p.shelves,
                        moods: Vec::new(),
                    })
            };
            match fetched {
                Ok(data) => {
                    tracing::info!(
                        key,
                        ms = t0.elapsed().as_millis() as u64,
                        shelves = data.shelves.len(),
                        "discover fetched"
                    );
                    if cached.as_ref().is_none_or(|(c, _)| *c != data) {
                        sink(Event::Discover {
                            key: key.clone(),
                            data: data.clone(),
                        });
                    }
                    let _ = tokio::task::spawn_blocking(move || {
                        discover_cache::save(&dir, &key, &data)
                    })
                    .await;
                }
                Err(e) => {
                    tracing::warn!("discover {key}: {e:#}");
                    if cached.is_none() {
                        sink(Event::Error(format!("could not load Discover: {e:#}")));
                    }
                }
            }
        });
    }

    pub(super) fn load_related(&self, video_id: String) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let sink = self.sink.clone();
        tokio::spawn(async move {
            match client.related(&video_id).await {
                Ok(shelves) if !shelves.is_empty() => sink(Event::Related { video_id, shelves }),
                Ok(_) => {}
                Err(e) => tracing::debug!("related {video_id}: {e:#}"),
            }
        });
    }

    /// An artist page: cached copy at once, a fresh one when the cache is older than 30 min
    /// (or on `force`); radio buttons need no extra state.
    pub(super) fn load_artist_page(&self, id: String, force: bool) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let sink = self.sink.clone();
        tokio::spawn(async move {
            use crate::core::api::artist_info::ArtistInfo;
            let dir = crate::config::cache_dir().join("artists");
            let cached = {
                let (dir, id) = (dir.clone(), id.clone());
                tokio::task::spawn_blocking(move || discover_cache::load::<ArtistInfo>(&dir, &id))
                    .await
                    .ok()
                    .flatten()
            };
            if let Some((info, age)) = &cached {
                sink(Event::Artist {
                    id: id.clone(),
                    info: info.clone(),
                });
                if !force && !discover_cache::is_stale(*age) {
                    return;
                }
            }
            match client.artist_info(&id).await {
                Ok(info) => {
                    if cached.as_ref().is_none_or(|(c, _)| *c != info) {
                        sink(Event::Artist {
                            id: id.clone(),
                            info: info.clone(),
                        });
                    }
                    let _ =
                        tokio::task::spawn_blocking(move || discover_cache::save(&dir, &id, &info))
                            .await;
                }
                Err(e) => {
                    if cached.is_none() {
                        sink(Event::Error(format!("could not load the artist: {e:#}")));
                    }
                }
            }
        });
    }
}
