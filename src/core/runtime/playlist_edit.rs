//! Controller: add tracks to (or create) the user's playlists, with duplicate check and undo.
use super::*;
use crate::core::listcache;

fn video_ids(items: &[Item]) -> Vec<String> {
    items.iter().filter_map(|i| i.video_id.clone()).collect()
}

impl Controller {
    /// The playlists tracks can be added to (for the "Add to playlist" submenu).
    pub(super) fn load_add_targets(&self) {
        let Some(client) = self.svc.client.clone().filter(|c| c.has_cookies()) else {
            return;
        };
        let sink = self.sink.clone();
        tokio::spawn(async move {
            match client.add_targets().await {
                Ok(t) => sink(Event::AddTargets(t)),
                Err(e) => {
                    tracing::debug!("add targets: {e:#}");
                    sink(Event::AddTargets(Vec::new())); // ends the dialog's loading state
                }
            }
        });
    }

    pub(super) fn add_to_playlist(
        &self,
        playlist: String,
        title: String,
        items: Vec<Item>,
        force: bool,
    ) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let items: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
        if items.is_empty() {
            return;
        }
        let sink = self.sink.clone();
        tokio::spawn(async move {
            let ids = video_ids(&items);
            let dir = crate::config::cache_dir().join("lists");
            if !force {
                // duplicates: the cached copy of the playlist, else a fresh fetch
                let known = {
                    let (dir, key) = (dir.clone(), playlist.clone());
                    tokio::task::spawn_blocking(move || listcache::load(&dir, &key))
                        .await
                        .ok()
                        .flatten()
                };
                let current = match known {
                    Some(k) => Some(k),
                    None => client.playlist(&playlist).await.ok(),
                };
                let dupes = current.map_or(0, |c| {
                    ids.iter()
                        .filter(|id| c.iter().any(|i| i.video_id.as_deref() == Some(id.as_str())))
                        .count()
                });
                if dupes > 0 {
                    sink(Event::PlaylistDuplicates {
                        playlist,
                        title,
                        items,
                        dupes,
                    });
                    return;
                }
            }
            match client.add_to_playlist(&playlist, &ids, force).await {
                Ok(entries) => {
                    let (d, p, added) = (dir.clone(), playlist.clone(), items.clone());
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Some(mut c) = listcache::load(&d, &p) {
                            c.extend(added);
                            listcache::save(&d, &p, &c);
                        }
                    })
                    .await;
                    sink(Event::PlaylistAdded {
                        playlist,
                        title,
                        items,
                        entries,
                        created: false,
                    });
                }
                Err(e) => sink(Event::Error(format!(
                    "could not add to the playlist: {e:#}"
                ))),
            }
        });
    }

    pub(super) fn create_playlist(&self, title: String, privacy: String, items: Vec<Item>) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let items: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
        let sink = self.sink.clone();
        tokio::spawn(async move {
            match client
                .create_playlist(&title, &video_ids(&items), &privacy)
                .await
            {
                Ok(id) => {
                    sink(Event::PlaylistAdded {
                        playlist: id,
                        title,
                        items,
                        entries: Vec::new(),
                        created: true,
                    });
                    if let Ok(t) = client.add_targets().await {
                        sink(Event::AddTargets(t));
                    }
                }
                Err(e) => sink(Event::Error(format!(
                    "could not create the playlist: {e:#}"
                ))),
            }
        });
    }

    pub(super) fn undo_add(&self, playlist: String, entries: Vec<(String, String)>) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let sink = self.sink.clone();
        tokio::spawn(async move {
            match client.remove_from_playlist(&playlist, &entries).await {
                Ok(()) => {
                    let removed: Vec<String> = entries.iter().map(|e| e.0.clone()).collect();
                    let (dir, p, r) = (
                        crate::config::cache_dir().join("lists"),
                        playlist.clone(),
                        removed.clone(),
                    );
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Some(mut c) = listcache::load(&dir, &p) {
                            // drop one occurrence per removed id (the ones that were just added)
                            for id in &r {
                                if let Some(pos) =
                                    c.iter().rposition(|i| i.video_id.as_deref() == Some(id))
                                {
                                    c.remove(pos);
                                }
                            }
                            listcache::save(&dir, &p, &c);
                        }
                    })
                    .await;
                    sink(Event::PlaylistRemoved {
                        playlist,
                        video_ids: removed,
                    });
                }
                Err(e) => sink(Event::Error(format!("could not undo: {e:#}"))),
            }
        });
    }
}
