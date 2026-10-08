//! Controller: UI command dispatch.
use super::*;

impl Controller {
    pub(super) fn command(&mut self, cmd: Command) {
        match cmd {
            Command::Search { query, filter } => self.search(query, filter),
            Command::PlayItems { items, index } => {
                let items: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
                // index refers to the unfiltered list in the caller; clamp after filtering
                let index = index.min(items.len().saturating_sub(1));
                self.queue.set(items, index);
                self.play_current();
            }
            Command::AddAndPlay(i) => {
                if i.video_id.is_some() {
                    self.queue.add(i);
                    if self.queue.jump(self.queue.len() - 1).is_some() {
                        self.play_current();
                    }
                }
            }
            Command::ClearQueue => {
                let _ = self.player.stop();
                self.queue.set(Vec::new(), 0);
                self.prefetched = false;
                self.snap.has_track = false;
                self.snap.pos = 0.0;
                self.idle = true;
                self.emit_queue();
            }
            Command::LoadLibrary(kind) => self.load_library(kind, None),
            Command::SearchMore => self.search_more(),
            Command::LoadArtistPage { id, force } => self.load_artist_page(id, force),
            Command::MixFromPlaylist {
                playlist,
                name,
                play,
            } => self.mix_from_playlist(playlist, name, play),
            Command::FindArtist(name) => {
                if let Some(client) = self.svc.client.clone() {
                    let sink = self.sink.clone();
                    tokio::spawn(async move {
                        match client.search_page(&name, Some("artists")).await {
                            Ok(p) => match p.items.into_iter().find_map(|i| i.browse_id) {
                                Some(id) => sink(Event::ArtistFound(id)),
                                None => {
                                    sink(Event::Error(format!("no artist page found for {name}")))
                                }
                            },
                            Err(e) => sink(Event::Error(format!("artist search failed: {e:#}"))),
                        }
                    });
                }
            }
            Command::LoadAlbumPage(id) => {
                if let Some(client) = self.svc.client.clone() {
                    let sink = self.sink.clone();
                    tokio::spawn(async move {
                        match client.album(&id).await {
                            Ok(page) => sink(Event::Album { id, page }),
                            Err(e) => {
                                sink(Event::Error(format!("could not load the album: {e:#}")))
                            }
                        }
                    });
                }
            }
            Command::LoadAddTargets => self.load_add_targets(),
            Command::AddToPlaylist {
                playlist,
                title,
                items,
                force,
            } => self.add_to_playlist(playlist, title, items, force),
            Command::CreatePlaylist {
                title,
                privacy,
                items,
            } => self.create_playlist(title, privacy, items),
            Command::UndoAdd { playlist, entries } => self.undo_add(playlist, entries),
            Command::LoadRelated(vid) => self.load_related(vid),
            Command::LoadDiscover {
                section,
                country,
                params,
                force,
            } => self.load_discover(section, country, params, force),
            Command::LoadPlaylist(id) => self.load_library(LibraryKind::Playlist, Some(id)),
            Command::LoadArtist(id) => self.load_library(LibraryKind::Artist, Some(id)),
            Command::LoadAlbum(id) => self.load_library(LibraryKind::Album, Some(id)),
            Command::ShuffleAll(items) => {
                let items: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
                if !items.is_empty() {
                    let start = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.subsec_nanos() as usize))
                        % items.len();
                    self.queue.set(items, start);
                    self.queue.set_shuffle(true);
                    self.play_current();
                }
            }
            Command::ToggleLike => self.toggle_like(),
            Command::ToggleLikeFor(v) => self.toggle_like_for(v),
            Command::StartRadio(i) => self.start_radio(i),
            Command::SetAudioFormat(f) => {
                let _ = self.player.set_audio_format(&f);
            }
            Command::ReloadCookies(p) => {
                let _ = self.player.set_cookies(p.as_deref());
                self.preload_liked();
                self.load_add_targets(); // signed in now: the "Add to playlist" list
            }
            Command::QueueJump(i) => {
                if self.queue.jump(i).is_some() {
                    self.play_current();
                }
            }
            Command::QueueReorder(order) => {
                self.queue.reorder(&order);
                self.replan();
            }
            Command::QueueInsert { items, at } => {
                let items: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
                let was_empty = self.queue.is_empty();
                self.queue.insert_at(at, items);
                if was_empty && !self.queue.is_empty() {
                    self.queue.jump(0);
                    self.play_current();
                } else {
                    self.replan();
                }
            }
            Command::LoadThumbs(urls) => self.load_thumbs(urls),
            Command::LoadCovers(urls) => self.load_covers(urls),
            Command::QueueRemoveMany(idx) => {
                let was_current = self.queue.current_index().is_some_and(|c| idx.contains(&c));
                // the track after the removed playing one, in the new numbering
                let first = self
                    .queue
                    .current_index()
                    .map_or(0, |c| c - idx.iter().filter(|&&r| r < c).count());
                self.queue.remove_many(&idx);
                if was_current {
                    // the playing track was removed: continue with whatever now sits at that slot
                    let at = first.min(self.queue.len().saturating_sub(1));
                    if !self.queue.is_empty() && self.queue.jump(at).is_some() {
                        self.play_current();
                    } else {
                        let _ = self.player.stop();
                        self.snap.has_track = false;
                        self.emit_queue();
                    }
                } else {
                    self.replan();
                }
            }
            Command::Enqueue(i) => {
                if i.video_id.is_some() {
                    let was_empty = self.queue.is_empty();
                    self.queue.add(i);
                    if was_empty {
                        self.queue.jump(0);
                        self.play_current();
                    } else {
                        self.replan();
                    }
                }
            }
            Command::PlayNext(i) => {
                if i.video_id.is_some() {
                    self.queue.play_next(i);
                    self.replan();
                }
            }
            Command::Toggle => {
                if self.snap.has_track && !self.idle {
                    let _ = self.player.toggle();
                } else if self.queue.current().is_some() {
                    self.play_current();
                }
            }
            Command::Play => {
                if self.snap.has_track && !self.idle {
                    let _ = self.player.play();
                } else if self.queue.current().is_some() {
                    self.play_current();
                }
            }
            Command::Pause => {
                if self.snap.has_track && !self.idle {
                    let _ = self.player.pause();
                }
            }
            Command::Stop => {
                let _ = self.player.stop();
                self.snap.has_track = false;
                self.snap.pos = 0.0;
                self.idle = true;
                self.emit();
            }
            Command::Next => {
                if self.queue.next(false).is_some() {
                    self.play_current();
                }
            }
            Command::Prev => {
                if self.snap.pos > 3.0 {
                    let _ = self.player.seek(0.0);
                } else if self.queue.prev().is_some() {
                    self.play_current();
                }
            }
            Command::SeekFrac(f) => {
                if self.snap.duration > 0.0 {
                    let _ = self.player.seek(f.clamp(0.0, 1.0) * self.snap.duration);
                }
            }
            Command::SeekBy(d) => {
                let t = (self.snap.pos + d).max(0.0);
                let _ = self.player.seek(if self.snap.duration > 0.0 {
                    t.min(self.snap.duration)
                } else {
                    t
                });
            }
            Command::SeekTo(t) => {
                let _ = self.player.seek(t.max(0.0));
            }
            Command::SetVolume(v) => {
                self.snap.volume = v.min(100);
                let _ = self.player.set_volume(self.snap.volume);
                self.emit();
            }
            Command::CycleRepeat => {
                let r = self.queue.repeat().cycle();
                self.queue.set_repeat(r);
                self.replan();
            }
            Command::ToggleShuffle => {
                let s = !self.queue.shuffle();
                self.queue.set_shuffle(s);
                self.replan();
            }
            Command::Quit => {}
        }
    }
}
