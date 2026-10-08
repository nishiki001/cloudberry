//! Sidebar panels glue: library tree, search, list thumbnails, drag & drop into the table.
mod fixture;
mod tree;

use super::state::*;
use super::wire_lib::open_as_tab;
use crate::core::model::{Item, Kind};
use crate::core::msg::{Command, LibraryKind};
use crate::core::runtime::CoreHandle;
use crate::{AppWindow, Panels, TrackData};
use slint::{ComponentHandle, Model, ModelRc};

pub use fixture::fixture_tree;
use tree::{Target, install_models, target, toggle_section};
pub use tree::{init_tree, refresh_tree, tree_failed, tree_loaded};

/// Thumbnails requested per page (Liked/History can hold thousands of rows).
const MAX_THUMBS: usize = 100;

/// Ask for the small thumbnails of `items` that are not loaded yet.
pub fn request_thumbs(core: &CoreHandle, items: &[Item]) {
    let urls: Vec<String> = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut v = Vec::new();
        for u in items
            .iter()
            .take(MAX_THUMBS)
            .filter_map(|i| i.thumbnail.clone())
        {
            if !s.thumbs.contains_key(&u) && s.thumb_pending.insert(u.clone()) {
                v.push(u);
            }
        }
        v
    });
    if !urls.is_empty() {
        core.send(Command::LoadThumbs(urls));
    }
}

/// A thumbnail arrived: store it and patch the rows that use it, in place.
pub fn thumb_arrived(ui: &AppWindow, url: String, img: std::sync::Arc<image::RgbaImage>) {
    let image = crate::art::to_slint(&img);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.thumb_pending.remove(&url);
        s.thumbs.insert(url.clone(), image.clone());
    });
    let patch = |model: ModelRc<TrackData>, items: &[Item]| {
        for (n, it) in items.iter().enumerate() {
            if it.thumbnail.as_deref() == Some(url.as_str())
                && let Some(mut r) = model.row_data(n)
            {
                (r.thumb, r.has_thumb) = (image.clone(), true);
                model.set_row_data(n, r);
            }
        }
    };
    STATE.with(|s| {
        let s = s.borrow();
        patch(ui.global::<Panels>().get_results(), &s.results);
        patch(ui.global::<Panels>().get_library(), &s.library);
    });
    super::discover::patch_thumb(&url, &image);
    super::artist::patch_thumb(ui, &url, &image, &img);
    super::playlist_add::patch_thumb(ui, &url, &image);
    refresh_tree(ui);
}

/// Open a playlist / album / artist item as a tab.
pub fn open_item_tab(core: &CoreHandle, it: &Item) {
    let (Some(id), kind) = (it.browse_id.clone(), it.kind) else {
        return;
    };
    if kind == Kind::Artist {
        super::artist::open_global(id);
        return;
    }
    if kind == Kind::Album {
        super::artist::open_album_global(id);
        return;
    }
    let lk = match kind {
        Kind::Album => LibraryKind::Album,
        _ => LibraryKind::Playlist,
    };
    open_as_tab(core, lk, id, it.title.clone());
}

pub fn wire(ui: &AppWindow, core: &CoreHandle) {
    install_models(ui);
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.global::<Panels>().on_tree_clicked(move |i| {
        if let (Some(Target::Section(si)), Some(ui)) = (target(i as usize), weak.upgrade()) {
            toggle_section(&ui, &c, si);
        }
    });
    let c = core.clone();
    ui.global::<Panels>()
        .on_tree_activated(move |i| match target(i as usize) {
            Some(Target::Section(_)) | None => {} // sections toggle on the single click
            Some(Target::Item(it)) if matches!(it.kind, Kind::Song | Kind::Video) => {
                c.send(Command::AddAndPlay(*it))
            }
            Some(Target::Item(it)) => open_item_tab(&c, &it),
        });
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.global::<Panels>().on_tree_context(move |action, i| {
        let (Some(Target::Item(it)), Some(ui)) = (target(i as usize), weak.upgrade()) else {
            return;
        };
        match action.as_str() {
            "open-tab" => open_item_tab(&c, &it),
            "add-play" | "play" => super::discover::play_card(&c, *it),
            a => {
                crate::gui::table::track_action(&ui, &c, a, vec![*it]);
            }
        }
    });

    // search: infinite scroll
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.global::<Panels>().on_search_more(move || {
        let go = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let go = s.search_more && !s.search_loading;
            if go {
                s.search_loading = true;
            }
            go
        });
        if go {
            if let Some(ui) = weak.upgrade() {
                ui.global::<Panels>().set_search_loading(true);
            }
            c.send(Command::SearchMore);
        }
    });

    // search: filter chips + activation by kind
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.global::<Panels>().on_search_filter(move |f| {
        let q = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.search_filter = f.to_string();
            s.last_query.clone()
        });
        if !q.is_empty() {
            if let Some(ui) = weak.upgrade() {
                ui.global::<Panels>().set_busy(true);
            }
            c.send(Command::Search {
                query: q,
                filter: Some(f.to_string()),
            });
        }
    });
    let c = core.clone();
    ui.global::<Panels>().on_search_activate(move |i| {
        let Some(it) = STATE.with(|s| s.borrow().results.get(i as usize).cloned()) else {
            return;
        };
        if matches!(it.kind, Kind::Song | Kind::Video) {
            c.send(Command::AddAndPlay(it));
        } else {
            open_item_tab(&c, &it);
        }
    });

    // rows dragged from a sidebar list and dropped on the table
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.on_table_panel_dropped(move |data, pos| {
        let Some((list, idx)) = data.split_once(':') else {
            return;
        };
        let Ok(idx) = idx.parse::<usize>() else {
            return;
        };
        let item = STATE.with(|s| {
            let s = s.borrow();
            match list {
                "results" => s.results.get(idx).cloned(),
                _ => s.library.get(idx).cloned(),
            }
        });
        if let (Some(it), Some(ui)) = (item, weak.upgrade()) {
            super::table::insert_items(&ui, &c, vec![it], pos.max(0) as usize);
        }
    });
}
