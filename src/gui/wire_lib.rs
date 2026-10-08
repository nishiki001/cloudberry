//! Library / search helpers shared by the callbacks: requests, pages, "open as tab", context menus.
use super::state::*;
use crate::AppWindow;
use crate::core::model::{Item, Kind};
use crate::core::msg::{Command, LibraryKind};
use crate::core::runtime::CoreHandle;

pub fn request_library(core: &CoreHandle, kind: LibraryKind, id: Option<String>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.lib_request = Some((kind, id.clone()));
        s.play_on_load = None; // only the card play button sets it, right after this call
        s.open_as_tab = None; // a page request is not a tab request (open_as_tab sets it afterwards)
    });
    core.send(match (kind, id) {
        (LibraryKind::Playlist, Some(id)) => Command::LoadPlaylist(id),
        (LibraryKind::Artist, Some(id)) => Command::LoadArtist(id),
        (LibraryKind::Album, Some(id)) => Command::LoadAlbum(id),
        (kind, _) => Command::LoadLibrary(kind),
    });
}

/// "Go to artist/album" for an item.
pub fn open_page(ui: &AppWindow, c: &CoreHandle, item: &Item, what: &str) {
    if what == "artist" {
        if let Some(a) = item.artists.first() {
            match &a.id {
                Some(id) => super::artist::open(ui, c, id.clone()),
                // the artist is plain text here (some search results): look the name up
                None => c.send(crate::core::msg::Command::FindArtist(a.name.clone())),
            }
        }
    } else {
        let id = if item.kind == Kind::Album {
            item.browse_id.clone()
        } else {
            item.album_id.clone()
        };
        if let Some(id) = id {
            let _ = c;
            super::artist::open_album(ui, id);
        }
    }
}

/// Open a Library page (playlist / album / artist) as a new playlist tab.
pub fn open_as_tab(c: &CoreHandle, kind: LibraryKind, id: String, name: String) {
    request_library(c, kind, Some(id.clone()));
    STATE.with(|s| s.borrow_mut().open_as_tab = Some((kind, id, name)));
}

pub fn clicked_library_item(_ui: &AppWindow, core: &CoreHandle, i: usize) {
    let (items, mode) =
        STATE.with(|s| (s.borrow().library.clone(), s.borrow().library_mode.clone()));
    let Some(clicked) = items.get(i).cloned() else {
        return;
    };
    match clicked.kind {
        Kind::Playlist => {
            if let Some(id) = clicked.browse_id.clone() {
                open_as_tab(core, LibraryKind::Playlist, id, clicked.title.clone());
            }
        }
        Kind::Album => {
            if let Some(id) = clicked.browse_id.clone() {
                open_as_tab(core, LibraryKind::Album, id, clicked.title.clone());
            }
        }
        _ => {
            // artist pages mix songs and albums: play only the songs
            let songs: Vec<Item> = if mode == "artist" {
                items.into_iter().filter(|x| x.video_id.is_some()).collect()
            } else {
                items
            };
            let index = songs
                .iter()
                .position(|x| x.video_id == clicked.video_id)
                .unwrap_or(0);
            core.send(Command::PlayItems {
                items: songs,
                index,
            });
        }
    }
}

pub fn context(ui: &AppWindow, c: &CoreHandle, kind: &str, action: &str, idx: usize) {
    if kind == "queue" {
        super::table::context(ui, c, action, idx);
        return;
    }
    let item = STATE.with(|s| {
        let s = s.borrow();
        let list = match kind {
            "results" => &s.results,
            _ => &s.library,
        };
        list.get(idx).cloned()
    });
    let Some(item) = item else { return };
    match action {
        "add-play" | "play" => super::discover::play_card(c, item),
        "open-tab" => {
            if let Some(id) = item.browse_id.clone() {
                let kind = match item.kind {
                    Kind::Album => LibraryKind::Album,
                    Kind::Artist => LibraryKind::Artist,
                    _ => LibraryKind::Playlist,
                };
                open_as_tab(c, kind, id, item.title.clone());
            }
        }
        a => {
            super::table::track_action(ui, c, a, vec![item]);
        }
    }
}
