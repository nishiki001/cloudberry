//! "Add to playlist": the row menu's submenu (New / Existing playlist…), the two dialogs, adding
//! / creating / undoing, duplicate warnings, and keeping open tabs of the edited playlist in step.
use super::state::*;
use crate::core::api::playlist_edit::AddTarget;
use crate::core::model::Item;
use crate::core::msg::Command;
use crate::core::runtime::CoreHandle;
use crate::{AddPlaylist, AppWindow, Dialogs, PlaylistChoice};
use slint::{ComponentHandle, ModelRc, VecModel};

pub use super::playlist_result::{added, duplicates, removed};

/// Marker ids of the "New playlist…" and "Existing playlist…" menu entries.
pub const NEW: &str = "__new__";
pub const EXISTING: &str = "__existing__";

/// Most recently used first (config), then the server's order.
fn ordered(targets: &[AddTarget], mru: &[String]) -> Vec<AddTarget> {
    let mut out: Vec<AddTarget> = mru
        .iter()
        .filter_map(|id| targets.iter().find(|t| &t.id == id).cloned())
        .collect();
    out.extend(targets.iter().filter(|t| !mru.contains(&t.id)).cloned());
    out
}

pub fn targets_arrived(ui: &AppWindow, targets: Vec<AddTarget>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.add_targets = targets;
        s.add_targets_loaded = true;
    });
    refresh_dialog(ui);
}

/// Rebuild the rows of the "Existing playlist" dialog from the cached list and the filter, and
/// ask for the covers that are not there yet.
pub(super) fn refresh_dialog(ui: &AppWindow) {
    let a = ui.global::<AddPlaylist>();
    let filter = a.get_filter().to_lowercase();
    let (shown, urls) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let shown: Vec<AddTarget> = ordered(&s.add_targets, &s.cfg.playlist_mru)
            .into_iter()
            .filter(|t| filter.is_empty() || t.title.to_lowercase().contains(&filter))
            .collect();
        let mut urls = Vec::new();
        for u in shown.iter().filter_map(|t| t.thumbnail.clone()) {
            if !s.thumbs.contains_key(&u) && s.thumb_pending.insert(u.clone()) {
                urls.push(u);
            }
        }
        s.add_shown.clone_from(&shown);
        (shown, urls)
    });
    let rows: Vec<PlaylistChoice> = STATE.with(|s| {
        let s = s.borrow();
        shown
            .iter()
            .map(|t| {
                let thumb = t.thumbnail.as_ref().and_then(|u| s.thumbs.get(u)).cloned();
                PlaylistChoice {
                    id: t.id.clone().into(),
                    title: t.title.clone().into(),
                    detail: t.detail.clone().into(),
                    has_thumb: thumb.is_some(),
                    thumb: thumb.unwrap_or_default(),
                }
            })
            .collect()
    });
    a.set_rows(ModelRc::new(VecModel::from(rows)));
    a.set_loading(!STATE.with(|s| s.borrow().add_targets_loaded) && a.get_existing_visible());
    if !urls.is_empty()
        && let Some(core) = STATE.with(|s| s.borrow().core.clone())
    {
        core.send(Command::LoadThumbs(urls));
    }
}

/// A cover arrived: patch the dialog rows that use it.
pub fn patch_thumb(ui: &AppWindow, url: &str, image: &slint::Image) {
    use slint::Model;
    let rows = ui.global::<AddPlaylist>().get_rows();
    let hits: Vec<usize> = STATE.with(|s| {
        s.borrow()
            .add_shown
            .iter()
            .enumerate()
            .filter(|(_, t)| t.thumbnail.as_deref() == Some(url))
            .map(|(i, _)| i)
            .collect()
    });
    for n in hits {
        if let Some(mut r) = rows.row_data(n) {
            (r.thumb, r.has_thumb) = (image.clone(), true);
            rows.set_row_data(n, r);
        }
    }
}

/// Menu action `pl:<id>` on `items`.
pub fn choose(ui: &AppWindow, core: &CoreHandle, playlist: &str, items: Vec<Item>) {
    let a = ui.global::<AddPlaylist>();
    if playlist == NEW {
        STATE.with(|s| s.borrow_mut().pending_add = Some(items));
        a.set_new_name("".into());
        a.set_privacy("PRIVATE".into());
        a.set_new_visible(true);
        return;
    }
    if playlist == EXISTING {
        a.set_count(items.len() as i32);
        a.set_filter("".into());
        STATE.with(|s| s.borrow_mut().pending_add = Some(items));
        a.set_existing_visible(true);
        // ask again when nothing is cached (an empty answer may be a failed start-up prefetch)
        let reload = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let reload = !s.add_targets_loaded || s.add_targets.is_empty();
            if reload {
                s.add_targets_loaded = false;
            }
            reload
        });
        if reload {
            core.send(Command::LoadAddTargets);
        }
        refresh_dialog(ui);
        return;
    }
    add_to(core, playlist, items);
}

fn add_to(core: &CoreHandle, playlist: &str, items: Vec<Item>) {
    let title = STATE.with(|s| {
        s.borrow()
            .add_targets
            .iter()
            .find(|t| t.id == playlist)
            .map(|t| t.title.clone())
    });
    core.send(Command::AddToPlaylist {
        playlist: playlist.to_string(),
        title: title.unwrap_or_else(|| "the playlist".into()),
        items,
        force: false,
    });
}

fn wire_dialogs(ui: &AppWindow, core: &CoreHandle) {
    let a = ui.global::<AddPlaylist>();
    let (weak, c) = (ui.as_weak(), core.clone());
    a.on_create(move |name, privacy| {
        let items = STATE.with(|s| s.borrow_mut().pending_add.take());
        if name.trim().is_empty() {
            // keep the dialog (and the songs) for another try
            STATE.with(|s| s.borrow_mut().pending_add = items);
            return;
        }
        if let Some(ui) = weak.upgrade() {
            ui.global::<AddPlaylist>().set_new_visible(false);
        }
        if let Some(items) = items {
            c.send(Command::CreatePlaylist {
                title: name.trim().to_string(),
                privacy: privacy.to_string(),
                items,
            });
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    a.on_add(move |id| {
        let items = STATE.with(|s| s.borrow_mut().pending_add.take());
        if let Some(ui) = weak.upgrade() {
            ui.global::<AddPlaylist>().set_existing_visible(false);
        }
        if let Some(items) = items {
            add_to(&c, &id, items);
        }
    });
    let weak = ui.as_weak();
    a.on_cancel(move || {
        STATE.with(|s| s.borrow_mut().pending_add = None);
        if let Some(ui) = weak.upgrade() {
            let a = ui.global::<AddPlaylist>();
            a.set_new_visible(false);
            a.set_existing_visible(false);
        }
    });
    let weak = ui.as_weak();
    a.on_filter_changed(move |t| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<AddPlaylist>().set_filter(t);
            refresh_dialog(&ui);
        }
    });
}

pub fn wire(ui: &AppWindow, core: &CoreHandle) {
    let c = core.clone();
    ui.global::<Dialogs>().on_status_action_clicked(move || {
        match STATE.with(|s| s.borrow_mut().toast_action.take()) {
            Some(ToastAction::Undo { playlist, entries }) => {
                c.send(Command::UndoAdd { playlist, entries })
            }
            Some(ToastAction::AddAnyway {
                playlist,
                title,
                items,
            }) => c.send(Command::AddToPlaylist {
                playlist,
                title,
                items,
                force: true,
            }),
            None => {}
        }
    });
    wire_dialogs(ui, core);
    core.send(Command::LoadAddTargets);
}
