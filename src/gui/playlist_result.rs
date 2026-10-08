//! "Add to playlist": what happens after the server answered (toasts with Undo / Add anyway,
//! open tabs of the edited playlist kept in step).
use super::state::*;
use super::{table, tabs};
use crate::AppWindow;
use crate::core::model::Item;

fn remember(playlist: &str) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.cfg.playlist_mru.retain(|p| p != playlist);
        s.cfg.playlist_mru.insert(0, playlist.to_string());
        s.cfg.playlist_mru.truncate(20);
        let _ = s.cfg.save();
    });
}

pub fn added(
    ui: &AppWindow,
    playlist: String,
    title: String,
    items: Vec<Item>,
    entries: Vec<(String, String)>,
    created: bool,
) {
    remember(&playlist);
    let n = items.len();
    // open tabs of this playlist show the new tracks right away
    let touched = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let cur = s.cur_tab;
        let mut touched = None;
        for (i, t) in s.tabs.iter_mut().enumerate() {
            if t.source.as_deref() == Some(playlist.as_str()) {
                t.items.extend(items.iter().cloned());
                touched = Some(touched.unwrap_or(false) || i + 1 == cur);
            }
        }
        touched
    });
    if touched.is_some() {
        tabs::save();
    }
    if touched == Some(true) {
        table::refresh(ui);
    }
    super::playlist_add::refresh_dialog(ui);
    let what = if n == 1 {
        "1 track".to_string()
    } else {
        format!("{n} tracks")
    };
    if created {
        show_toast(ui, &format!("Created “{title}” with {what}"));
    } else if entries.is_empty() {
        show_toast(ui, &format!("Added {what} to “{title}”"));
    } else {
        show_toast_action(
            ui,
            &format!("Added {what} to “{title}”"),
            "Undo",
            ToastAction::Undo { playlist, entries },
        );
    }
}

pub fn duplicates(ui: &AppWindow, playlist: String, title: String, items: Vec<Item>, dupes: usize) {
    let total = items.len();
    show_toast_action(
        ui,
        &format!("{dupes} of {total} already in “{title}”"),
        "Add anyway",
        ToastAction::AddAnyway {
            playlist,
            title,
            items,
        },
    );
}

pub fn removed(ui: &AppWindow, playlist: String, video_ids: Vec<String>) {
    let touched = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let cur = s.cur_tab;
        let mut touched = None;
        for (i, t) in s.tabs.iter_mut().enumerate() {
            if t.source.as_deref() == Some(playlist.as_str()) {
                for id in &video_ids {
                    if let Some(pos) = t
                        .items
                        .iter()
                        .rposition(|x| x.video_id.as_deref() == Some(id))
                    {
                        t.items.remove(pos);
                    }
                }
                touched = Some(touched.unwrap_or(false) || i + 1 == cur);
            }
        }
        touched
    });
    if touched.is_some() {
        tabs::save();
    }
    if touched == Some(true) {
        table::refresh(ui);
    }
    show_toast(ui, "Undone");
}
