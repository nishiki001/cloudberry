//! Core → UI event handling.
use super::state::*;
use crate::AppWindow;
use crate::Panels;
use crate::TrackData;
use crate::art;
use crate::core::model::Item;
use crate::core::msg::{Event, LibraryKind, Snapshot};
use slint::ComponentHandle;
use slint::{ModelRc, SharedString, VecModel};

fn apply_snapshot(ui: &AppWindow, snap: &Snapshot) {
    ui.set_playing(snap.playing);
    super::analyzer::set_playing(ui, snap.playing);
    ui.set_liked(snap.liked);
    ui.set_track_title(
        if snap.has_track {
            snap.title.clone()
        } else {
            "Nothing playing".into()
        }
        .into(),
    );
    ui.set_has_track(snap.has_track);
    ui.set_elapsed(fmt_time(snap.pos).into());
    ui.set_total(fmt_time(snap.duration).into());
    ui.set_status_format(snap.format.clone().into());
    let (pos, buf) = if snap.duration > 0.0 {
        (
            (snap.pos / snap.duration) as f32,
            (snap.buffered / snap.duration) as f32,
        )
    } else {
        (0.0, 0.0)
    };
    if !ui.get_seeking() {
        ui.set_position(pos.clamp(0.0, 1.0));
    }
    ui.set_buffered(buf.clamp(0.0, 1.0));
    ui.set_repeat_mode(snap.repeat.map(|r| r.as_str()).unwrap_or("off").into());
    ui.set_shuffle(snap.shuffle);
    let changed = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.progressed |= snap.pos > 1.0;
        s.snap = snap.clone();
        let changed = s.last_video != snap.video_id;
        if changed {
            s.last_video = snap.video_id.clone();
        }
        changed
    });
    if changed {
        ui.set_has_cover(false);
        STATE.with(|s| s.borrow_mut().lyrics = None);
        ui.global::<Panels>()
            .set_lyrics(ModelRc::new(VecModel::default()));
        ui.global::<Panels>().set_lyrics_current(-1);
        ui.global::<Panels>().set_lyrics_status(
            if snap.has_track {
                "Loading lyrics…"
            } else {
                ""
            }
            .into(),
        );
    }
    ui.set_artist_name(snap.artist_name.clone().into());
    ui.set_album_name(snap.album_name.clone().into());
    update_lyrics_line(ui, snap.pos);
    STATE.with(|s| {
        if let Some(m) = s.borrow_mut().media.as_mut() {
            m.update(snap, snap.thumbnail.as_deref());
        }
    });
    super::search_results::refresh_playing_marks(ui);
    if changed {
        super::table::refresh_marks(ui);
        super::discover::track_changed(ui);
        super::artist::refresh_marks();
        super::panels::refresh_tree(ui);
    }
}

fn update_lyrics_line(ui: &AppWindow, pos: f64) {
    let idx = STATE.with(|s| s.borrow().lyrics.as_ref().and_then(|l| l.current(pos)));
    let idx = idx.map_or(-1, |i| i as i32);
    if ui.global::<Panels>().get_lyrics_current() != idx {
        ui.global::<Panels>().set_lyrics_current(idx);
    }
}

pub fn handle_event(ui: &AppWindow, ev: Event) {
    match ev {
        Event::Results { items, more } => super::search_results::first(ui, items, more),
        Event::ResultsMore { items, more } => super::search_results::more(ui, items, more),
        Event::State(s) => {
            apply_snapshot(ui, &s);
        }
        Event::PlayNow(items) => {
            if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
                core.send(crate::core::msg::Command::PlayItems { items, index: 0 });
            }
        }
        Event::ArtistFound(id) => super::artist::open_global(id),
        Event::Album { id, page } => super::artist::album_arrived(ui, id, page),
        Event::Artist { id, info } => super::artist::arrived(ui, id, info),
        Event::AddTargets(t) => super::playlist_add::targets_arrived(ui, t),
        Event::PlaylistAdded {
            playlist,
            title,
            items,
            entries,
            created,
        } => super::playlist_add::added(ui, playlist, title, items, entries, created),
        Event::PlaylistDuplicates {
            playlist,
            title,
            items,
            dupes,
        } => super::playlist_add::duplicates(ui, playlist, title, items, dupes),
        Event::PlaylistRemoved {
            playlist,
            video_ids,
        } => super::playlist_add::removed(ui, playlist, video_ids),
        Event::Related { video_id, shelves } => {
            super::discover::related_arrived(ui, video_id, shelves)
        }
        Event::Discover { key, data } => super::discover::arrived(ui, key, data),
        Event::Thumb { url, img } => super::panels::thumb_arrived(ui, url, img),
        Event::Library {
            kind,
            id,
            title: page_title,
            mut items,
        } => {
            // a card's play button asked for this collection: start it, show nothing
            let want = STATE.with(|s| {
                let mut s = s.borrow_mut();
                match &s.play_on_load {
                    Some((k, i)) if *k == kind && id.as_ref() == Some(i) => s.play_on_load.take(),
                    _ => None,
                }
            });
            if want.is_some() {
                let songs: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
                if let (Some(core), false) =
                    (STATE.with(|s| s.borrow().core.clone()), songs.is_empty())
                {
                    core.send(crate::core::msg::Command::PlayItems {
                        items: songs,
                        index: 0,
                    });
                }
                ui.global::<Panels>().set_busy(false);
                return;
            }
            let core = STATE.with(|s| s.borrow().core.clone());
            // 1. a tree section the user expanded (the same page may also be wanted by the panel)
            let for_tree = id.is_none()
                && STATE.with(|s| s.borrow().tree.iter().any(|t| t.kind == kind && t.loading));
            let fresh = STATE.with(|s| s.borrow().lib_request == Some((kind, id.clone())));
            if for_tree && let Some(core) = &core {
                let copy = if fresh {
                    items.clone()
                } else {
                    std::mem::take(&mut items)
                };
                super::panels::tree_loaded(ui, core, kind, copy);
            }
            if !fresh {
                return; // superseded by a newer request (or only the tree wanted it)
            }
            // 2. "open as tab": only when this page is the one that was asked for
            let tab = STATE.with(|s| {
                let mut s = s.borrow_mut();
                match &s.open_as_tab {
                    Some((k, i, _)) if *k == kind && id.as_ref() == Some(i) => {
                        s.open_as_tab.take().map(|t| t.2)
                    }
                    _ => None,
                }
            });
            if let Some(name) = tab {
                let songs: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
                ui.global::<Panels>().set_busy(false);
                super::tabs::open(ui, page_title.clone().unwrap_or(name), "list", songs);
                return;
            }
            // 3. a page shown in the Library panel
            show_library_page(ui, kind, page_title, items);
        }
        Event::LibraryChunk {
            stream,
            kind,
            id,
            items,
            replace,
            done,
        } => super::library_stream::chunk(ui, stream, kind, id, items, replace, done),
        Event::Lyrics { video_id, lyrics } => {
            let current = STATE.with(|s| s.borrow().last_video.clone());
            if current.as_deref() != Some(video_id.as_str()) {
                return; // stale
            }
            match &lyrics {
                Some(l) => {
                    let rows: Vec<SharedString> =
                        l.lines.iter().map(|x| x.text.clone().into()).collect();
                    ui.global::<Panels>()
                        .set_lyrics(ModelRc::new(VecModel::from(rows)));
                    ui.global::<Panels>().set_lyrics_synced(l.synced);
                    ui.global::<Panels>()
                        .set_lyrics_source(l.source.clone().into());
                    ui.global::<Panels>().set_lyrics_status(SharedString::new());
                }
                None => {
                    ui.global::<Panels>()
                        .set_lyrics(ModelRc::new(VecModel::default()));
                    ui.global::<Panels>()
                        .set_lyrics_status("No lyrics found".into());
                }
            }
            let pos = STATE.with(|s| s.borrow().snap.pos);
            STATE.with(|s| s.borrow_mut().lyrics = lyrics);
            update_lyrics_line(ui, pos);
        }
        Event::Queue { items, current } => {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.queue_items = items;
                s.queue_current = current;
                // the selection may point past the end after removals
                let n = s.queue_items.len();
                if s.cur_tab == 0 {
                    s.sel.retain(|&i| i < n);
                }
            });
            super::table::refresh(ui);
        }
        Event::Tab { name, kind, items } => super::tabs::open(ui, name, &kind, items),
        Event::Cover(img) => {
            let cover = img.clone();
            ui.set_cover_flat(art::to_slint(&img));
            let mut img = (*img).clone();
            art::circle_mask(&mut img);
            ui.set_cover(art::to_slint(&img));
            ui.set_has_cover(true);
            super::backdrop::set_cover(ui, cover);
        }
        Event::Error(e) => {
            ui.global::<Panels>().set_busy(false);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.open_as_tab = None;
                s.play_on_load = None;
            });
            ui.global::<crate::Discover>().set_busy(false);
            show_toast(ui, &e);
        }
        Event::LibraryFailed { kind, id, message } => {
            super::library_stream::failed(kind, &id);
            super::panels::tree_failed(ui, kind);
            ui.global::<Panels>().set_busy(false);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                if s.lib_request == Some((kind, id.clone())) {
                    s.lib_request = None;
                }
                s.open_as_tab = None;
                s.play_on_load = None;
            });
            show_toast(ui, &message);
        }
    }
}

/// Show a list in the Library panel (liked songs, playlist, albums, an artist page…).
pub(super) fn show_library_page(
    ui: &AppWindow,
    kind: LibraryKind,
    page_title: Option<String>,
    items: Vec<Item>,
) {
    let core = STATE.with(|s| s.borrow().core.clone());
    ui.global::<Panels>().set_panel_selected(-1);
    let (mode, title) = match kind {
        LibraryKind::Liked => ("liked", "Liked songs".to_string()),
        LibraryKind::Playlists => ("playlists", "Your playlists".to_string()),
        LibraryKind::Albums => ("albums", "Your albums".to_string()),
        LibraryKind::Artists => ("artists", "Your artists".to_string()),
        LibraryKind::History => ("history", "History".to_string()),
        LibraryKind::Playlist => ("playlist", STATE.with(|s| s.borrow().pending_title.clone())),
        LibraryKind::Artist => ("artist", page_title.clone().unwrap_or_default()),
        LibraryKind::Album => ("album", page_title.clone().unwrap_or_default()),
    };
    let rows: Vec<TrackData> = items.iter().map(|i| panel_row(i, false)).collect();
    if let Some(core) = core {
        super::panels::request_thumbs(&core, &items);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.library = items;
        s.library_mode = mode.to_string();
    });
    ui.global::<Panels>().set_library_mode(mode.into());
    ui.global::<Panels>().set_library_title(title.into());
    ui.global::<Panels>()
        .set_library(ModelRc::new(VecModel::from(rows)));
    ui.global::<Panels>().set_busy(false);
}
