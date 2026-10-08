//! The artist page tab: tab handling (reuse, history), the page models, lazy covers.
mod actions;
mod album;
mod page;
pub use album::{album_arrived, open_album, open_album_global};
pub use page::{arrived, fixture, fixture_album, patch_thumb, refresh_marks};

use super::discover::shelf_rows;
use super::state::*;
use crate::core::api::artist_info::ArtistInfo;
use crate::core::msg::Command;
use crate::core::runtime::CoreHandle;
use crate::{AppWindow, ArtistPage, CardData, Discover, ShelfData, SongRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::HashMap;
use std::rc::Rc;

/// What the artist page currently shows.
#[derive(Default)]
pub struct ArtistState {
    /// Channel id on screen (events for others are stale).
    pub current: String,
    pub info: Option<ArtistInfo>,
    /// The page shows an album (the info is a stand-in built from the album).
    pub album: bool,
    /// Artist of the shown album, for the byline link.
    pub byline_artist: Option<String>,
    /// The header picture of the page (for the banner and the round avatar).
    header: Option<(String, std::sync::Arc<image::RgbaImage>)>,
    songs: Option<Rc<VecModel<SongRow>>>,
    pub(super) shelves: Vec<Vec<crate::core::model::Item>>,
    pub(super) models: Vec<Rc<VecModel<CardData>>>,
    /// Per tab (by uid): visited channel ids and the position in them.
    hist: HashMap<u64, (Vec<String>, usize)>,
}

fn song_row(info: &ArtistInfo, i: usize) -> SongRow {
    let it = &info.songs[i];
    let thumb = it
        .thumbnail
        .as_ref()
        .and_then(|u| STATE.with(|s| s.borrow().thumbs.get(u).cloned()));
    SongRow {
        title: it.title.clone().into(),
        artist: it
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
            .into(),
        album: it.album.clone().unwrap_or_default().into(),
        plays: info.plays.get(i).cloned().unwrap_or_default().into(),
        time: it.duration.clone().unwrap_or_default().into(),
        playing: false,
        has_thumb: thumb.is_some(),
        thumb: thumb.unwrap_or_default(),
    }
}

/// Open the page of an artist: an existing tab for it is reused, otherwise a new tab.
pub fn open(ui: &AppWindow, core: &CoreHandle, id: String) {
    let _ = core;
    super::tabs::open_special(ui, "Artist", "artist", &id);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let uid = s.tabs.get(s.cur_tab.wrapping_sub(1)).map(|t| t.uid);
        if let Some(uid) = uid {
            s.artist
                .hist
                .entry(uid)
                .or_insert_with(|| (vec![id.clone()], 0));
        }
    });
}

/// Open an artist page from code that has no window handle at hand.
pub fn open_global(id: String) {
    let (ui, core) = STATE.with(|s| {
        let s = s.borrow();
        (s.ui.as_ref().and_then(|w| w.upgrade()), s.core.clone())
    });
    if let (Some(ui), Some(core)) = (ui, core) {
        open(&ui, &core, id);
    }
}

/// The current main tab changed: show the artist page if it is one.
pub fn on_select(ui: &AppWindow) {
    let Some(core) = STATE.with(|s| s.borrow().core.clone()) else {
        return;
    };
    let (id, kind) = STATE.with(|s| {
        let s = s.borrow();
        match s
            .tabs
            .get(s.cur_tab.wrapping_sub(1))
            .filter(|t| t.kind == "artist" || t.kind == "album")
        {
            Some(t) => (t.source.clone(), t.kind.clone()),
            None => (None, String::new()),
        }
    });
    let album_page = kind == "album";
    let page = ui.global::<ArtistPage>();
    let Some(id) = id else {
        page.set_active(false);
        return;
    };
    ui.global::<Discover>().set_active(false);
    page.set_active(true);
    page.set_album_mode(album_page);
    page.set_songs_title(if album_page { "Tracks" } else { "Top songs" }.into());
    let (same, uid_hist) = STATE.with(|s| {
        let s = s.borrow();
        let uid = s.tabs.get(s.cur_tab.wrapping_sub(1)).map(|t| t.uid);
        (
            s.artist.current == id && s.artist.info.is_some(),
            uid.and_then(|u| s.artist.hist.get(&u).cloned()),
        )
    });
    let (hist, pos) = uid_hist.unwrap_or((vec![id.clone()], 0));
    page.set_can_back(pos > 0);
    page.set_can_forward(pos + 1 < hist.len());
    if same {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.artist.current.clone_from(&id);
        s.artist.album = album_page;
        s.artist.info = None;
        s.artist.shelves.clear();
        s.artist.models.clear();
    });
    page.set_busy(true);
    page.set_name("".into());
    page.set_stats("".into());
    page.set_description("".into());
    page.set_has_banner(false);
    page.set_has_avatar(false);
    page.set_songs(ModelRc::new(VecModel::default()));
    page.set_shelves(ModelRc::new(VecModel::default()));
    page.set_byline("".into());
    page.set_expanded(false);
    if album_page {
        core.send(Command::LoadAlbumPage(id));
    } else {
        core.send(Command::LoadArtistPage { id, force: false });
    }
}

/// From an album page: the artist opens in a tab of its own; from an artist page it navigates.
pub fn navigate_or_open(ui: &AppWindow, id: String) {
    let album = STATE.with(|s| s.borrow().artist.album);
    if album {
        if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
            open(ui, &core, id);
        }
    } else {
        navigate(ui, id);
    }
}

/// Go to another artist inside the current artist tab (history grows).
pub fn navigate(ui: &AppWindow, id: String) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(uid) = s.tabs.get(s.cur_tab.wrapping_sub(1)).map(|t| t.uid) else {
            return;
        };
        let seed = s.tabs[s.cur_tab - 1]
            .source
            .clone()
            .into_iter()
            .collect::<Vec<_>>();
        let (hist, pos) = s.artist.hist.entry(uid).or_insert_with(|| (seed, 0));
        hist.truncate(*pos + 1);
        hist.push(id.clone());
        *pos = hist.len() - 1;
        let cur = s.cur_tab - 1;
        s.tabs[cur].source = Some(id);
    });
    on_select(ui);
}

fn step(ui: &AppWindow, back: bool) {
    let moved = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let uid = s.tabs.get(s.cur_tab.wrapping_sub(1)).map(|t| t.uid)?;
        let (hist, pos) = s.artist.hist.get_mut(&uid)?;
        let np = if back {
            pos.checked_sub(1)?
        } else {
            (*pos + 1 < hist.len()).then_some(*pos + 1)?
        };
        *pos = np;
        let id = hist[np].clone();
        let cur = s.cur_tab - 1;
        s.tabs[cur].source = Some(id);
        Some(())
    });
    if moved.is_some() {
        on_select(ui);
    }
}

pub fn wire(ui: &AppWindow, core: &CoreHandle) {
    let page = ui.global::<ArtistPage>();
    let weak = ui.as_weak();
    page.on_back(move || {
        if let Some(ui) = weak.upgrade() {
            step(&ui, true);
        }
    });
    let weak = ui.as_weak();
    page.on_forward(move || {
        if let Some(ui) = weak.upgrade() {
            step(&ui, false);
        }
    });
    actions::wire(ui, core);
}
