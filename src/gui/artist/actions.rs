//! Artist page actions: play / shuffle / radio, song and card clicks, menus.
use super::super::discover::{is_track, play_card};
use super::super::panels::{open_item_tab, request_thumbs};
use super::super::state::*;
use super::super::wire_lib::open_as_tab;
use crate::core::model::{Item, Kind};
use crate::core::msg::{Command, LibraryKind};
use crate::core::runtime::CoreHandle;
use crate::{AppWindow, ArtistPage};
use slint::ComponentHandle;

fn songs() -> Vec<Item> {
    STATE.with(|s| {
        s.borrow()
            .artist
            .info
            .as_ref()
            .map(|i| i.songs.clone())
            .unwrap_or_default()
    })
}

fn card(shelf: i32, i: i32) -> Option<Item> {
    STATE.with(|s| {
        s.borrow()
            .artist
            .shelves
            .get(shelf as usize)?
            .get(i as usize)
            .cloned()
    })
}

pub fn wire(ui: &AppWindow, core: &CoreHandle) {
    let p = ui.global::<ArtistPage>();
    let c = core.clone();
    p.on_play(move || {
        let items = songs();
        if !items.is_empty() {
            c.send(Command::PlayItems { items, index: 0 });
        }
    });
    let c = core.clone();
    p.on_shuffle(move || {
        // the artist's own shuffle mix, else the top songs
        let (mix, name) = STATE.with(|s| {
            let s = s.borrow();
            let i = s.artist.info.as_ref();
            (
                i.and_then(|i| i.play.clone()),
                i.map(|i| i.name.clone()).unwrap_or_default(),
            )
        });
        match mix {
            Some(playlist) => c.send(Command::MixFromPlaylist {
                playlist,
                name,
                play: true,
            }),
            None => {
                let items = songs();
                if !items.is_empty() {
                    c.send(Command::ShuffleAll(items));
                }
            }
        }
    });
    let c = core.clone();
    p.on_radio(move || {
        let (radio, name) = STATE.with(|s| {
            let s = s.borrow();
            let i = s.artist.info.as_ref();
            (
                i.and_then(|i| i.radio.clone()),
                i.map(|i| i.name.clone()).unwrap_or_default(),
            )
        });
        if let Some(playlist) = radio {
            c.send(Command::MixFromPlaylist {
                playlist,
                name,
                play: false,
            });
        }
    });
    let c = core.clone();
    p.on_show_all_songs(move || {
        let (id, name) = STATE.with(|s| {
            let s = s.borrow();
            let i = s.artist.info.as_ref();
            (
                i.and_then(|i| i.songs_playlist.clone()),
                i.map(|i| i.name.clone()).unwrap_or_default(),
            )
        });
        if let Some(id) = id {
            open_as_tab(&c, LibraryKind::Playlist, id, format!("{name} – songs"));
        }
    });
    let c = core.clone();
    p.on_song_activated(move |i| {
        let items = songs();
        if (i as usize) < items.len() {
            c.send(Command::PlayItems {
                items,
                index: i as usize,
            });
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    p.on_song_context(move |action, i| {
        let (Some(ui), Some(it)) = (weak.upgrade(), songs().get(i as usize).cloned()) else {
            return;
        };
        super::super::table::track_action(&ui, &c, action.as_str(), vec![it]);
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    p.on_card_clicked(move |s, i| {
        let (Some(ui), Some(it)) = (weak.upgrade(), card(s, i)) else {
            return;
        };
        match it.kind {
            Kind::Artist => {
                if let Some(id) = it.browse_id {
                    super::navigate(&ui, id);
                }
            }
            _ if is_track(&it) => c.send(Command::AddAndPlay(it)),
            _ => open_item_tab(&c, &it),
        }
    });
    let c = core.clone();
    p.on_card_play(move |s, i| {
        if let Some(it) = card(s, i) {
            play_card(&c, it);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    p.on_card_context(move |action, s, i| {
        let (Some(ui), Some(it)) = (weak.upgrade(), card(s, i)) else {
            return;
        };
        super::super::discover::card_action(&ui, &c, action.as_str(), it);
    });
    let c = core.clone();
    p.on_card_shown(move |s, i| {
        if let Some(it) = card(s, i) {
            super::super::discover::request_cover(&c, &it);
        }
    });
    let _ = request_thumbs;
    let weak = ui.as_weak();
    p.on_byline_clicked(move || {
        let id = STATE.with(|s| s.borrow().artist.byline_artist.clone());
        if let (Some(id), Some(ui)) = (id, weak.upgrade()) {
            super::navigate_or_open(&ui, id);
        }
    });
}
