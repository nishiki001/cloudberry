//! Controller: queue planning, prefetch, radio, track changes, player events.
use super::*;
use crate::core::model::Item;

impl Controller {
    /// Queue changed under a playing track: re-plan what comes next.
    pub(super) fn replan(&mut self) {
        if self.prefetched {
            let _ = self.player.clear_pending();
            self.prefetched = false;
        }
        if self.snap.has_track {
            self.ensure_prefetch();
        }
        self.emit_queue();
    }

    /// Append the next track to the player so playback is gapless; ask for radio at the tail.
    pub(super) fn ensure_prefetch(&mut self) {
        if self.prefetched {
            return;
        }
        if let Some(i) = self.queue.peek_next(true) {
            if let Some(vid) = self.queue.items()[i].video_id.clone()
                && self.player.load(&watch_url(&vid), LoadMode::Append).is_ok()
            {
                self.prefetched = true;
                tracing::info!(next = %vid, "prefetched next track");
            }
        } else if self.queue.repeat() == crate::core::queue::Repeat::Off {
            self.request_radio();
        }
    }

    pub(super) fn request_radio(&mut self) {
        let (Some(client), Some(seed)) = (self.svc.client.clone(), self.snap.video_id.clone())
        else {
            return;
        };
        if self.radio_for.as_deref() == Some(seed.as_str()) {
            return;
        }
        self.radio_for = Some(seed.clone());
        tracing::info!(%seed, "queue ended: fetching radio");
        let tx = self.radio_tx.clone();
        tokio::spawn(async move {
            match client.radio(&seed).await {
                Ok(items) => {
                    let _ = tx.send((seed, items));
                }
                Err(e) => {
                    tracing::debug!("radio failed: {e:#}");
                    let _ = tx.send((seed, Vec::new())); // clears awaiting/retry state
                }
            }
        });
    }

    pub(super) fn radio_arrived(&mut self, seed: String, items: Vec<Item>) {
        if self.snap.video_id.as_deref() != Some(seed.as_str()) && !self.awaiting_next {
            return;
        }
        let known: std::collections::HashSet<String> = self
            .queue
            .items()
            .iter()
            .filter_map(|i| i.video_id.clone())
            .collect();
        let fresh: Vec<Item> = items
            .into_iter()
            .filter(|i| i.video_id.as_ref().is_some_and(|v| !known.contains(v)))
            .take(25)
            .collect();
        if fresh.is_empty() {
            self.awaiting_next = false;
            self.radio_for = None; // allow a retry on the next track
            return;
        }
        for i in fresh {
            self.queue.add(i);
        }
        if self.awaiting_next {
            self.awaiting_next = false;
            if self.queue.next(true).is_some() {
                self.play_current();
            }
        } else {
            self.ensure_prefetch();
            self.emit_queue();
        }
    }

    pub(super) fn play_current(&mut self) {
        let Some(item) = self.queue.current().cloned() else {
            return;
        };
        let Some(vid) = item.video_id.clone() else {
            return;
        };
        self.prefetched = false;
        self.awaiting_next = false;
        if let Err(e) = self.player.load(&watch_url(&vid), LoadMode::Replace) {
            self.fail(&format!("playback failed: {e:#}"));
            return;
        }
        self.progressed = false;
        self.paused = false;
        self.idle = false;
        self.snap.has_track = true;
        self.snap.title = item.title.clone();
        self.snap.artist_line = artist_line(&item);
        self.snap.video_id = Some(vid);
        self.set_meta(&item);
        self.snap.pos = 0.0;
        self.snap.duration = item.duration_secs.map(f64::from).unwrap_or(0.0);
        self.snap.buffered = 0.0;
        self.emit_queue();
        self.fetch_cover(item.thumbnail);
    }

    pub(super) fn set_meta(&mut self, item: &Item) {
        self.snap.thumbnail = item.thumbnail.clone();
        self.snap.artist_id = item.artists.first().and_then(|a| a.id.clone());
        self.snap.artist_name = item
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default();
        self.snap.album_id = item.album_id.clone();
        self.snap.album_name = item.album.clone().unwrap_or_default();
        if self.last_lyrics_for != item.video_id {
            self.last_lyrics_for.clone_from(&item.video_id);
            self.fetch_lyrics(item);
        }
    }

    /// The player moved on to the prefetched entry by itself.
    pub(super) fn adopt_current(&mut self) {
        let Some(item) = self.queue.current().cloned() else {
            return;
        };
        tracing::info!(title = %item.title, "advance to prefetched track");
        self.snap.has_track = true;
        self.idle = false;
        self.paused = false;
        self.prefetched = false;
        self.progressed = false;
        self.snap.title = item.title.clone();
        self.snap.artist_line = artist_line(&item);
        self.snap.video_id = item.video_id.clone();
        self.set_meta(&item);
        self.snap.pos = 0.0;
        self.snap.duration = item.duration_secs.map(f64::from).unwrap_or(0.0);
        self.snap.buffered = 0.0;
        self.emit_queue();
        self.fetch_cover(item.thumbnail);
    }

    pub(super) fn fetch_cover(&mut self, url: Option<String>) {
        self.cover_url = url.clone();
        let (Some(url), Some(thumbs)) = (url, self.svc.thumbs.clone()) else {
            return;
        };
        let tx = self.cover_tx.clone();
        tokio::spawn(async move {
            match thumbs.get(&url).await {
                Ok(img) => {
                    let _ = tx.send((url, img));
                }
                Err(e) => tracing::debug!("cover fetch failed: {e:#}"),
            }
        });
    }

    pub(super) fn player_event(&mut self, ev: PlayerEvent) {
        match ev {
            PlayerEvent::TimePos(t) => {
                self.snap.pos = t.max(0.0);
                if t > 0.5 {
                    self.progressed = true;
                }
            }
            PlayerEvent::Duration(d) => self.snap.duration = d.max(0.0),
            PlayerEvent::Buffered(b) => self.snap.buffered = b.max(0.0),
            PlayerEvent::Paused(p) => self.paused = p,
            PlayerEvent::Idle(i) => self.idle = i,
            PlayerEvent::Volume(_) | PlayerEvent::StartFile => return,
            PlayerEvent::Format(f) => self.snap.format = f,
            PlayerEvent::FileLoaded => {
                self.ensure_prefetch();
                return;
            }
            PlayerEvent::EndFile(EndReason::Eof) => {
                if self.prefetched {
                    self.queue.next(true);
                    self.adopt_current();
                } else if self.queue.next(true).is_some() {
                    self.play_current();
                } else if self.radio_for.is_some() && self.radio_for == self.snap.video_id {
                    self.awaiting_next = true; // radio request in flight (or empty): resolve on arrival
                    self.snap.has_track = false;
                    self.snap.pos = 0.0;
                    self.emit();
                } else {
                    self.snap.has_track = false;
                    self.snap.pos = 0.0;
                    self.emit();
                }
                return;
            }
            PlayerEvent::EndFile(EndReason::Error) => {
                self.fail("playback failed — try `yt-dlp -U`");
                if self.prefetched {
                    // mpv moved on to the appended entry by itself
                    self.queue.next(true);
                    self.adopt_current();
                    return;
                }
                self.snap.has_track = false;
            }
            PlayerEvent::EndFile(_) => return,
            PlayerEvent::Error(e) => {
                tracing::debug!("player error: {e}");
                if self.progressed || !self.snap.has_track {
                    return;
                }
                self.fail("playback failed — try `yt-dlp -U`");
                self.snap.has_track = false;
            }
        }
        self.emit();
    }
}

fn artist_line(i: &Item) -> String {
    let artists: Vec<&str> = i.artists.iter().map(|a| a.name.as_str()).collect();
    match (&i.album, artists.is_empty()) {
        (Some(al), false) => format!("{} — {}", artists.join(", "), al),
        (None, false) => artists.join(", "),
        (Some(al), true) => al.clone(),
        _ => String::new(),
    }
}
