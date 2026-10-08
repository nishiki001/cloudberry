//! Playlist tabs: Queue (tab 0, owned by the core) plus opened lists, radios and user-made
//! empty tabs. Open tabs are saved with their items and restored at the next start.
use super::state::{STATE, TabState, show_toast};
use super::table;
use crate::AppWindow;
use crate::Dialogs;
use crate::config;
use crate::core::model::Item;
use crate::core::tabs::{TabSave, TabsFile};
use slint::ComponentHandle;

fn path() -> std::path::PathBuf {
    config::data_dir().join("tabs.json")
}

/// Persist the open tabs (cheap: a few KB of JSON).
pub fn save() {
    if crate::config::NO_PERSIST.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let file = STATE.with(|s| {
        let s = s.borrow();
        TabsFile {
            tabs: s
                .tabs
                .iter()
                .map(|t| TabSave {
                    source: t.source.clone(),
                    name: t.name.clone(),
                    kind: t.kind.clone(),
                    items: t.items.clone(),
                })
                .collect(),
            current: s.cur_tab.checked_sub(1),
        }
    });
    // serialize here, write on a thread (tabs hold every item, and the UI must not wait on disk)
    static WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    std::thread::spawn(move || {
        let _guard = WRITE.lock();
        let _ = crate::core::tabs::save(&path(), &file);
    });
}

/// Load the saved tabs into the UI state (before the first refresh).
pub fn restore() {
    let f = crate::core::tabs::load(&path());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tabs = f
            .tabs
            .into_iter()
            .map(|t| TabState {
                source: t.source,
                uid: super::state::new_uid(),
                name: t.name,
                kind: t.kind,
                items: t.items,
            })
            .collect();
        s.cur_tab = f.current.map_or(0, |c| (c + 1).min(s.tabs.len()));
        s.last_list_tab = s.cur_tab.max(1).min(s.tabs.len());
    });
}

/// Tabs that show a page (artist / album), not a track list.
pub fn is_page_kind(kind: &str) -> bool {
    kind == "artist" || kind == "album"
}

pub(super) fn select(ui: &AppWindow, i: usize) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.cur_tab = i.min(s.tabs.len());
        s.sel.clear();
        s.sort = None;
        if s.cur_tab > 0 {
            s.last_list_tab = s.cur_tab;
        }
    });
    ui.global::<crate::Discover>().set_active(false);
    table::refresh_columns(ui);
    table::refresh(ui);
    save();
    super::artist::on_select(ui);
}

/// Open a list in a new tab and show it.
pub fn open(ui: &AppWindow, name: String, kind: &str, items: Vec<Item>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tabs.push(TabState {
            source: None,
            uid: super::state::new_uid(),
            name,
            kind: kind.into(),
            items,
        });
        s.cur_tab = s.tabs.len();
    });
    select(ui, STATE.with(|s| s.borrow().cur_tab));
}

/// A tab that is not a track list (an artist page): an existing one for the same `source` is
/// shown, otherwise a new one is added.
pub fn open_special(ui: &AppWindow, name: &str, kind: &str, source: &str) {
    let existing = STATE.with(|s| {
        s.borrow()
            .tabs
            .iter()
            .position(|t| t.kind == kind && t.source.as_deref() == Some(source))
    });
    match existing {
        Some(i) => {
            STATE.with(|s| s.borrow_mut().cur_tab = i + 1);
            select(ui, i + 1);
        }
        None => {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.tabs.push(TabState {
                    source: Some(source.to_string()),
                    uid: super::state::new_uid(),
                    name: name.into(),
                    kind: kind.into(),
                    items: Vec::new(),
                });
                s.cur_tab = s.tabs.len();
            });
            select(ui, STATE.with(|s| s.borrow().cur_tab));
        }
    }
}

/// "Add to playlist tab": append to the last used list tab, or start a new one.
pub fn add_items(ui: &AppWindow, items: Vec<Item>) {
    let n = items.len();
    let target = STATE.with(|s| {
        let s = s.borrow();
        (s.last_list_tab > 0
            && s.last_list_tab <= s.tabs.len()
            && !is_page_kind(&s.tabs[s.last_list_tab - 1].kind))
        .then_some(s.last_list_tab)
    });
    match target {
        Some(t) => {
            STATE.with(|s| s.borrow_mut().tabs[t - 1].items.extend(items));
            save();
            table::refresh(ui);
            show_toast(ui, &format!("added {n} tracks"));
        }
        None => {
            open(ui, "Playlist 1".into(), "empty", items);
            show_toast(ui, &format!("added {n} tracks to a new tab"));
        }
    }
}

pub fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_tab_selected(move |i| {
        if let Some(ui) = weak.upgrade() {
            select(&ui, i.max(0) as usize);
        }
    });
    let weak = ui.as_weak();
    ui.on_tab_add(move || {
        if let Some(ui) = weak.upgrade() {
            let n = STATE.with(|s| s.borrow().tabs.len()) + 1;
            open(&ui, format!("Playlist {n}"), "empty", Vec::new());
        }
    });
    let weak = ui.as_weak();
    ui.on_tab_close(move |i| {
        let i = i as usize;
        if i == 0 {
            return;
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if i <= s.tabs.len() {
                s.tabs.remove(i - 1);
            }
            s.cur_tab = if s.cur_tab == i {
                i.saturating_sub(1).min(s.tabs.len())
            } else if s.cur_tab > i {
                s.cur_tab - 1
            } else {
                s.cur_tab
            };
            s.sel.clear();
        });
        if let Some(ui) = weak.upgrade() {
            select(&ui, STATE.with(|s| s.borrow().cur_tab));
        }
    });
    let weak = ui.as_weak();
    ui.on_tab_rename(move |i| {
        let name = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.rename_tab = (i > 0).then_some(i as usize);
            s.tabs
                .get((i as usize).wrapping_sub(1))
                .map(|t| t.name.clone())
        });
        if let (Some(name), Some(ui)) = (name, weak.upgrade()) {
            ui.global::<Dialogs>()
                .set_rename_heading("Rename playlist".into());
            ui.global::<Dialogs>().set_rename_button("Rename".into());
            ui.global::<Dialogs>().set_rename_text(name.into());
            ui.global::<Dialogs>().set_rename_visible(true);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_rename_accepted(move |text| {
        let text = text.trim().to_string();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let (Some(i), false) = (s.rename_tab.take(), text.is_empty())
                && let Some(t) = s.tabs.get_mut(i - 1)
            {
                t.name = text;
            }
        });
        save();
        if let Some(ui) = weak.upgrade() {
            table::refresh(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_tab_move(move |i, dir| {
        let i = i as usize;
        let ok = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let j = i as i32 + dir;
            if i == 0 || i > s.tabs.len() || j < 1 || j as usize > s.tabs.len() {
                return false;
            }
            s.tabs.swap(i - 1, j as usize - 1);
            if s.last_list_tab == i {
                s.last_list_tab = j as usize;
            } else if s.last_list_tab == j as usize {
                s.last_list_tab = i;
            }
            if s.cur_tab == i {
                s.cur_tab = j as usize;
            } else if s.cur_tab == j as usize {
                s.cur_tab = i;
            }
            true
        });
        if let (true, Some(ui)) = (ok, weak.upgrade()) {
            table::refresh(&ui);
            save();
        }
    });
}
