//! Row context menu actions and the clipboard helper.
use super::super::state::{STATE, show_toast};
use super::current_items;
use super::refresh_marks;
use super::select::{activate, delete_selection, selected_items, set_selection};
use crate::AppWindow;
use crate::core::msg::Command;
use crate::core::runtime::CoreHandle;
use slint::ComponentHandle;

/// Copy text with whichever clipboard tool exists (off the UI thread); false when none does.
fn copy_text(text: String) -> bool {
    let tool = [
        ("wl-copy", &[][..]),
        ("xclip", &["-selection", "clipboard"][..]),
        ("pbcopy", &[][..]),
    ]
    .into_iter()
    .find(|(bin, _)| {
        std::env::var_os("PATH")
            .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
    });
    let Some((bin, args)) = tool else {
        return false;
    };
    std::thread::spawn(move || {
        use std::io::Write;
        if let Ok(mut child) = std::process::Command::new(bin)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    });
    true
}

/// Context-menu action on table row `idx` (acts on the whole selection when `idx` is part of it).
pub fn context(ui: &AppWindow, core: &CoreHandle, action: &str, idx: usize) {
    // the row the menu was opened on may have moved (list refreshed): follow the track
    let Some(idx) = super::gesture::resolve(idx) else {
        return;
    };
    super::gesture::touch();
    if !STATE.with(|s| s.borrow().sel.contains(&idx)) {
        set_selection(vec![idx], idx);
        refresh_marks(ui);
    }
    let items = selected_items();
    if items.is_empty() {
        return;
    }
    match action {
        "play" => activate(core, idx),
        "remove" => delete_selection(ui, core),
        _ => {
            track_action(ui, core, action, items);
        }
    }
}

/// The actions of the shared track menu, on any list of items. False when `action` is not one
/// of them (the caller handles its own extras).
pub fn track_action(
    ui: &AppWindow,
    core: &CoreHandle,
    action: &str,
    items: Vec<crate::core::model::Item>,
) -> bool {
    let Some(first) = items.first().cloned() else {
        return true;
    };
    match action {
        "play" => core.send(Command::AddAndPlay(first)),
        "play-next" => items
            .into_iter()
            .rev()
            .for_each(|i| core.send(Command::PlayNext(i))),
        "enqueue" => {
            show_toast(ui, "added to queue");
            items
                .into_iter()
                .for_each(|i| core.send(Command::Enqueue(i)));
        }
        "add-to-tab" => super::super::tabs::add_items(ui, items),
        a if a.starts_with("ar:") => super::super::artist::open(ui, core, a[3..].to_string()),
        a if a.starts_with("pl:") => super::super::playlist_add::choose(ui, core, &a[3..], items),
        "radio" => core.send(Command::StartRadio(first)),
        "like" => {
            if let Some(v) = first.video_id {
                core.send(Command::ToggleLikeFor(v));
            }
        }
        "copy-link" => copy_link(ui, &items),
        "artist" | "album" => super::super::wire_lib::open_page(ui, core, &first, action),
        _ => return false,
    }
    true
}

/// Put the watch links of `items` on the clipboard (or show them when no clipboard tool exists).
pub fn copy_link(ui: &AppWindow, items: &[crate::core::model::Item]) {
    let text = items
        .iter()
        .filter_map(|i| i.video_id.as_ref())
        .map(|v| format!("https://music.youtube.com/watch?v={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    if copy_text(text.clone()) {
        show_toast(ui, "link copied");
    } else {
        show_toast(ui, &text);
    }
}

/// A right click is about to open the row menu: list the artists of the row for "Go to artist".
pub fn prepare_row(ui: &AppWindow, idx: usize) {
    super::gesture::press(idx);
    let artists: Vec<(String, String)> = STATE.with(|s| {
        let s = s.borrow();
        current_items(&s)
            .get(idx)
            .map(|i| {
                i.artists
                    .iter()
                    .filter_map(|a| Some((a.name.clone(), a.id.clone()?)))
                    .collect()
            })
            .unwrap_or_default()
    });
    let rows: Vec<crate::SubItem> = artists
        .into_iter()
        .map(|(name, id)| crate::SubItem {
            label: name.into(),
            id: format!("ar:{id}").into(),
            enabled: true,
        })
        .collect();
    ui.global::<crate::Menus>()
        .set_artist_sub(slint::ModelRc::new(slint::VecModel::from(rows)));
}
