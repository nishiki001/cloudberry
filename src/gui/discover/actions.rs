//! Discover card actions: open, play, queue, radio, lazy covers.
use super::super::panels::open_item_tab;
use super::super::state::*;
use super::super::wire_lib::request_library;
use crate::core::model::{Item, Kind};
use crate::core::msg::{Command, LibraryKind};
use crate::core::runtime::CoreHandle;
use crate::{AppWindow, Discover};
use slint::ComponentHandle;

fn item(shelf: i32, i: i32) -> Option<Item> {
    STATE.with(|s| {
        s.borrow()
            .discover
            .shelves
            .get(shelf as usize)?
            .get(i as usize)
            .cloned()
    })
}

pub(crate) fn is_track(it: &Item) -> bool {
    matches!(it.kind, Kind::Song | Kind::Video) && it.video_id.is_some()
}

/// Play an album / playlist / artist: load it, and start it when it has arrived.
fn play_collection(core: &CoreHandle, it: &Item) {
    let Some(id) = it.browse_id.clone() else {
        return;
    };
    let kind = match it.kind {
        Kind::Album => LibraryKind::Album,
        Kind::Artist => LibraryKind::Artist,
        _ => LibraryKind::Playlist,
    };
    request_library(core, kind, Some(id.clone()));
    STATE.with(|s| s.borrow_mut().play_on_load = Some((kind, id)));
}

pub(crate) fn play(core: &CoreHandle, it: Item) {
    if is_track(&it) {
        core.send(Command::AddAndPlay(it));
    } else {
        play_collection(core, &it);
    }
}

pub fn wire(ui: &AppWindow, core: &CoreHandle) {
    let d = ui.global::<Discover>();
    let c = core.clone();
    d.on_card_clicked(move |s, i| {
        let Some(it) = item(s, i) else { return };
        if is_track(&it) {
            c.send(Command::AddAndPlay(it));
        } else {
            open_item_tab(&c, &it);
        }
    });
    let c = core.clone();
    d.on_card_play(move |s, i| {
        if let Some(it) = item(s, i) {
            play(&c, it);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    d.on_card_context(move |action, s, i| {
        let (Some(ui), Some(it)) = (weak.upgrade(), item(s, i)) else {
            return;
        };
        card_action(&ui, &c, action.as_str(), it);
    });
    let c = core.clone();
    d.on_card_shown(move |s, i| {
        if let Some(it) = item(s, i) {
            request_cover(&c, &it);
        }
    });
}

/// Context-menu action on a card (Discover and artist page share it).
pub(crate) fn card_action(ui: &AppWindow, c: &CoreHandle, action: &str, it: Item) {
    match action {
        "play" => play(c, it),
        "enqueue" if is_track(&it) => {
            show_toast(ui, "added to queue");
            c.send(Command::Enqueue(it));
        }
        "enqueue" => show_toast(ui, "open it in a tab to add its tracks"),
        "open-tab" if !is_track(&it) => open_item_tab(c, &it),
        "open-tab" => c.send(Command::StartRadio(it)),
        "radio" if is_track(&it) => c.send(Command::StartRadio(it)),
        "radio" => show_toast(ui, "radio starts from a song"),
        a if is_track(&it) => {
            super::super::table::track_action(ui, c, a, vec![it]);
        }
        _ => {}
    }
}

/// Ask for the full-size cover of a card once (it arrives as `<url>#card`).
pub(crate) fn request_cover(c: &CoreHandle, it: &Item) {
    if let Some(url) = it.thumbnail.clone() {
        let key = format!("{url}#card");
        let fresh = STATE.with(|s| {
            let mut s = s.borrow_mut();
            !s.thumbs.contains_key(&key) && s.thumb_pending.insert(key)
        });
        if fresh {
            c.send(Command::LoadCovers(vec![url]));
        }
    }
}
