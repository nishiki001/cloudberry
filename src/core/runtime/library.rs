//! Controller: liked ids, library/page loading, lyrics, search.
use super::*;
use crate::core::listcache;

pub(super) const MAX_LIST_PAGES: usize = 40;

/// Why a (suspiciously empty) answer may not be real: no cookies, or an expired session.
pub(super) async fn auth_problem(client: &Client) -> Option<String> {
    if !client.has_cookies() {
        return Some("not signed in (Settings → Account)".into());
    }
    match client.signed_in().await {
        Ok(false) => Some("the session has expired: sign in again (Settings → Account)".into()),
        _ => None,
    }
}

impl Controller {
    pub(super) fn liked_update(&mut self, msg: LikedMsg) {
        match msg {
            LikedMsg::Replace(ids) => {
                self.liked_ids = ids.into_iter().collect();
                for (id, liked) in &self.inflight {
                    if *liked {
                        self.liked_ids.insert(id.clone());
                    } else {
                        self.liked_ids.remove(id);
                    }
                }
            }
            LikedMsg::Done(id, liked, ok) => {
                if self.inflight.get(&id) == Some(&liked) {
                    self.inflight.remove(&id);
                    if !ok {
                        if liked {
                            self.liked_ids.remove(&id);
                        } else {
                            self.liked_ids.insert(id);
                        }
                    }
                }
            }
            LikedMsg::Set(id, liked) => {
                self.inflight.remove(&id);
                if liked {
                    self.liked_ids.insert(id);
                } else {
                    self.liked_ids.remove(&id);
                }
            }
        }
    }

    /// Fetch liked ids in the background so the heart is right from the first track.
    pub(super) fn preload_liked(&self) {
        let Some(client) = self.svc.client.clone().filter(|c| c.has_cookies()) else {
            return;
        };
        let tx = self.liked_tx.clone();
        tokio::spawn(async move {
            match client.playlist_pages("LM", MAX_LIST_PAGES).await {
                Ok(items) => {
                    // warm the disk cache too, so the Liked tab opens instantly
                    let (dir, list) = (crate::config::cache_dir().join("lists"), items.clone());
                    let _ = tokio::task::spawn_blocking(move || {
                        if listcache::load(&dir, "LM")
                            .is_none_or(|c| !listcache::same_list(&c, &list))
                        {
                            listcache::save(&dir, "LM", &list);
                        }
                    })
                    .await;
                    let _ = tx.send(LikedMsg::Replace(
                        items.into_iter().filter_map(|i| i.video_id).collect(),
                    ));
                }
                Err(e) => tracing::debug!("liked preload failed: {e:#}"),
            }
        });
    }

    pub(super) fn load_library(&self, kind: LibraryKind, id: Option<String>) {
        let Some(client) = self.svc.client.clone() else {
            (self.sink)(Event::LibraryFailed {
                kind,
                id,
                message: "not signed in (try `auth setup`)".into(),
            });
            return;
        };
        if matches!(
            (kind, &id),
            (LibraryKind::Liked, _) | (LibraryKind::Playlist, Some(_))
        ) {
            self.stream_playlist(client, kind, id);
            return;
        }
        let (sink, liked_tx) = (self.sink.clone(), self.liked_tx.clone());
        tokio::spawn(async move {
            let mut title = None;
            let res = match (kind, &id) {
                (LibraryKind::Liked, _) => client.liked().await,
                (LibraryKind::Playlists, _) => client.library_playlists().await,
                (LibraryKind::Albums, _) => client.library_albums().await,
                (LibraryKind::Artists, _) => client.library_artists().await,
                (LibraryKind::History, _) => client.history().await,
                (LibraryKind::Playlist, Some(id)) => client.playlist(id).await,
                (LibraryKind::Artist, Some(id)) => client.artist(id).await.map(|a| {
                    title = Some(a.name);
                    let mut v = a.songs;
                    v.extend(a.albums);
                    v
                }),
                (LibraryKind::Album, Some(id)) => client.album(id).await.map(|a| {
                    title = Some(if a.artist.is_empty() {
                        a.title
                    } else {
                        format!("{} — {}", a.artist, a.title)
                    });
                    a.tracks
                }),
                _ => return,
            };
            // an empty answer from a signed-out session looks exactly like an empty library
            let res = match res {
                Ok(items) if items.is_empty() => match auth_problem(&client).await {
                    Some(why) => Err(anyhow::anyhow!(why)),
                    None => Ok(items),
                },
                other => other,
            };
            match res {
                Ok(items) => {
                    if kind == LibraryKind::Liked {
                        let _ = liked_tx.send(LikedMsg::Replace(
                            items.iter().filter_map(|i| i.video_id.clone()).collect(),
                        ));
                    }
                    sink(Event::Library {
                        kind,
                        id,
                        title,
                        items,
                    });
                }
                Err(e) => sink(Event::LibraryFailed {
                    kind,
                    id,
                    message: format!("could not load library: {e:#}"),
                }),
            }
        });
    }

    pub(super) fn fetch_lyrics(&self, item: &Item) {
        let Some(vid) = item.video_id.clone() else {
            return;
        };
        let Some(lc) = self.svc.lyrics.clone() else {
            (self.sink)(Event::Lyrics {
                video_id: vid,
                lyrics: None,
            });
            return;
        };
        let (client, sink, item) = (self.svc.client.clone(), self.sink.clone(), item.clone());
        tokio::spawn(async move {
            let lyrics = lyrics::fetch(&lc, client.as_deref(), &item)
                .await
                .unwrap_or_else(|e| {
                    tracing::debug!("lyrics failed: {e:#}");
                    None
                });
            sink(Event::Lyrics {
                video_id: vid,
                lyrics,
            });
        });
    }

    pub(super) fn toggle_like(&mut self) {
        if let Some(vid) = self.snap.video_id.clone() {
            self.toggle_like_for(vid);
        }
    }

    /// Fetch the radio of `item` and hand it to the UI as a new playlist tab.
    pub(super) fn start_radio(&self, item: Item) {
        let (Some(client), Some(vid)) = (self.svc.client.clone(), item.video_id.clone()) else {
            return;
        };
        let sink = self.sink.clone();
        tokio::spawn(async move {
            match client.radio(&vid).await {
                Ok(items) if !items.is_empty() => sink(Event::Tab {
                    name: format!("Radio: {}", item.title),
                    kind: "radio".into(),
                    items,
                }),
                Ok(_) => sink(Event::Error("no radio for this track".into())),
                Err(e) => sink(Event::Error(format!("radio failed: {e:#}"))),
            }
        });
    }

    pub(super) fn toggle_like_for(&mut self, vid: String) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        if !client.has_cookies() {
            self.fail("sign in first: run `cloudberry auth setup`");
            return;
        }
        let like = !self.liked_ids.contains(&vid);
        // optimistic; rolled back through the channel if the request fails
        self.liked_update(LikedMsg::Set(vid.clone(), like));
        self.inflight.insert(vid.clone(), like);
        self.emit();
        let (sink, tx) = (self.sink.clone(), self.liked_tx.clone());
        tokio::spawn(async move {
            match client.set_like(&vid, like).await {
                Ok(()) => {
                    let _ = tx.send(LikedMsg::Done(vid, like, true));
                }
                Err(e) => {
                    sink(Event::Error(format!("like failed: {e:#}")));
                    let _ = tx.send(LikedMsg::Done(vid, like, false));
                }
            }
        });
    }

    /// Small list thumbnails, at most 6 at a time; each arrives as `Event::Thumb` (keyed by the
    /// url that was asked for).
    pub(super) fn load_thumbs(&self, urls: Vec<String>) {
        let Some(thumbs) = self.svc.thumbs.clone() else {
            return;
        };
        let gate = Arc::new(tokio::sync::Semaphore::new(6));
        for url in urls {
            let (thumbs, sink, gate) = (thumbs.clone(), self.sink.clone(), gate.clone());
            tokio::spawn(async move {
                let Ok(_permit) = gate.acquire().await else {
                    return;
                };
                let small = crate::core::thumbs::small_url(&url);
                if let Ok(img) = thumbs
                    .get_edge(&small, crate::core::thumbs::SMALL_EDGE)
                    .await
                {
                    sink(Event::Thumb { url, img });
                }
            });
        }
    }

    /// Covers for Discover cards (140 px, 2x on HiDPI): the url as given, decoded at full cover
    /// resolution, delivered under `<url>#card` so they never collide with the small thumbnails.
    pub(super) fn load_covers(&self, urls: Vec<String>) {
        let Some(thumbs) = self.svc.thumbs.clone() else {
            return;
        };
        let gate = Arc::new(tokio::sync::Semaphore::new(6));
        for url in urls {
            let (thumbs, sink, gate) = (thumbs.clone(), self.sink.clone(), gate.clone());
            tokio::spawn(async move {
                let Ok(_permit) = gate.acquire().await else {
                    return;
                };
                if let Ok(img) = thumbs.get(&url).await {
                    sink(Event::Thumb {
                        url: format!("{url}#card"),
                        img,
                    });
                }
            });
        }
    }

    /// Radio / mix from a playlist id: a "Radio: name" tab, or straight into the queue.
    pub(super) fn mix_from_playlist(&self, playlist: String, name: String, play: bool) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let sink = self.sink.clone();
        tokio::spawn(async move {
            match client.radio_from_playlist(&playlist).await {
                Ok(items) if !items.is_empty() && play => sink(Event::PlayNow(items)),
                Ok(items) if !items.is_empty() => sink(Event::Tab {
                    name: format!("Radio: {name}"),
                    kind: "radio".into(),
                    items,
                }),
                Ok(_) => sink(Event::Error("nothing to play for this one".into())),
                Err(e) => sink(Event::Error(format!("could not start the mix: {e:#}"))),
            }
        });
    }
}
