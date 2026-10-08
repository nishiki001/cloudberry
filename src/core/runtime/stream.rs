//! Streaming playlist / liked-songs loads (cached copy first, then pages as they arrive).
use super::library::MAX_LIST_PAGES;
use super::*;
use crate::core::listcache;

impl Controller {
    /// Playlists and liked songs: cached copy first (if any), then the network; pages are
    /// streamed to the UI as they arrive, or compared with the cached copy and replaced in
    /// place only when something changed.
    pub(super) fn stream_playlist(
        &self,
        client: Arc<Client>,
        kind: LibraryKind,
        id: Option<String>,
    ) {
        let (sink, liked_tx) = (self.sink.clone(), self.liked_tx.clone());
        static NEXT_STREAM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let stream = NEXT_STREAM.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tokio::spawn(async move {
            let key = id.clone().unwrap_or_else(|| "LM".into());
            let dir = crate::config::cache_dir().join("lists");
            let t0 = std::time::Instant::now();
            let cached = {
                let (dir, key) = (dir.clone(), key.clone());
                tokio::task::spawn_blocking(move || listcache::load(&dir, &key))
                    .await
                    .ok()
                    .flatten()
            };
            let chunk = |items: Vec<Item>, replace: bool, done: bool| Event::LibraryChunk {
                stream,
                kind,
                id: id.clone(),
                items,
                replace,
                done,
            };
            let mut all: Vec<Item> = Vec::new();
            let res = if let Some(old) = &cached {
                tracing::info!(
                    rows = old.len(),
                    ms = t0.elapsed().as_millis() as u64,
                    "playlist from cache"
                );
                sink(chunk(old.clone(), true, false));
                client
                    .playlist_stream(&key, MAX_LIST_PAGES, |p| all.extend(p))
                    .await
            } else {
                let mut first = true;
                client
                    .playlist_stream(&key, MAX_LIST_PAGES, |p| {
                        if first {
                            tracing::info!(
                                rows = p.len(),
                                ms = t0.elapsed().as_millis() as u64,
                                "playlist first rows ready"
                            );
                        }
                        all.extend(p.iter().cloned());
                        sink(chunk(p, first, false));
                        first = false;
                    })
                    .await
            };
            // nothing came back: a signed-out session answers like an empty library
            let res = match res {
                Ok(()) if all.is_empty() => match super::library::auth_problem(&client).await {
                    Some(why) => Err(anyhow::anyhow!(why)),
                    None => Ok(()),
                },
                other => other,
            };
            match res {
                Ok(()) => {
                    tracing::info!(
                        rows = all.len(),
                        ms = t0.elapsed().as_millis() as u64,
                        "playlist complete"
                    );
                    if kind == LibraryKind::Liked {
                        let _ = liked_tx.send(LikedMsg::Replace(
                            all.iter().filter_map(|i| i.video_id.clone()).collect(),
                        ));
                    }
                    let changed = cached
                        .as_ref()
                        .is_none_or(|c| !listcache::same_list(c, &all));
                    tracing::info!(
                        list = %key,
                        cached = cached.as_ref().map(Vec::len),
                        fetched = all.len(),
                        changed,
                        first_difference = ?cached.as_ref().and_then(|c| listcache::first_difference(c, &all)),
                        "playlist refresh"
                    );
                    if cached.is_some() && changed {
                        sink(chunk(all.clone(), true, true)); // replace the cached view in place
                    } else {
                        sink(chunk(Vec::new(), false, true));
                    }
                    // an empty list is never cached (it would show as the truth until refreshed)
                    if changed && !all.is_empty() {
                        let _ =
                            tokio::task::spawn_blocking(move || listcache::save(&dir, &key, &all))
                                .await;
                    }
                }
                Err(e) => {
                    sink(Event::LibraryFailed {
                        kind,
                        id: id.clone(),
                        message: format!("could not load library: {e:#}"),
                    });
                }
            }
        });
    }
}
