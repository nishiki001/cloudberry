//! UI → core callbacks.
use super::state::*;
use super::wire_lib::*;
use crate::AppWindow;
use crate::Dialogs;
use crate::Panels;
use crate::config::Config;
use crate::core::msg::{Command, LibraryKind};
use crate::core::runtime::CoreHandle;
use slint::{ComponentHandle, ModelRc, VecModel};

pub fn wire(ui: &AppWindow, core: &CoreHandle, _cfg: Config) {
    let c = core.clone();
    ui.on_toggle_play(move || c.send(Command::Toggle));
    let c = core.clone();
    ui.on_next(move || c.send(Command::Next));
    let c = core.clone();
    ui.on_prev(move || c.send(Command::Prev));
    let c = core.clone();
    ui.on_stop(move || c.send(Command::Stop));
    let c = core.clone();
    ui.on_seek(move |f| c.send(Command::SeekFrac(f as f64)));
    let c = core.clone();
    ui.on_seek_by(move |secs| {
        let snap = STATE.with(|s| s.borrow().snap.clone());
        if snap.duration > 0.0 {
            c.send(Command::SeekFrac(
                ((snap.pos + secs as f64) / snap.duration).clamp(0.0, 1.0),
            ));
        }
    });
    let c = core.clone();
    ui.on_set_volume(move |v| c.send(Command::SetVolume((v * 100.0).round() as u8)));
    let c = core.clone();
    ui.on_toggle_shuffle(move || c.send(Command::ToggleShuffle));
    let c = core.clone();
    ui.on_cycle_repeat(move || c.send(Command::CycleRepeat));
    let c = core.clone();
    ui.on_toggle_like(move || c.send(Command::ToggleLike));

    let weak = ui.as_weak();
    let c = core.clone();
    ui.global::<Panels>().on_submit_search(move |q| {
        let q = q.trim().to_string();
        if q.is_empty() {
            return;
        }
        if let Some(ui) = weak.upgrade() {
            ui.global::<Panels>().set_busy(true);
        }
        let filter = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.last_query = q.clone();
            s.search_filter.clone()
        });
        c.send(Command::Search {
            query: q,
            filter: Some(filter),
        });
    });
    let weak = ui.as_weak();
    let c = core.clone();
    ui.global::<Panels>().on_panel_activate(move |i| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_sidebar_tab() == 2 {
                if let Some(item) = STATE.with(|s| s.borrow().results.get(i as usize).cloned()) {
                    if matches!(
                        item.kind,
                        crate::core::model::Kind::Song | crate::core::model::Kind::Video
                    ) {
                        c.send(Command::AddAndPlay(item));
                    } else {
                        super::panels::open_item_tab(&c, &item);
                    }
                }
            } else {
                clicked_library_item(&ui, &c, i as usize);
            }
        }
    });
    let weak = ui.as_weak();
    let c = core.clone();
    ui.on_context_action(move |kind, action, idx| {
        if let Some(ui) = weak.upgrade() {
            context(&ui, &c, kind.as_str(), action.as_str(), idx as usize);
        }
    });

    let c = core.clone();
    ui.global::<Panels>().on_library_open(move |k| {
        let kind = match k.as_str() {
            "liked" => LibraryKind::Liked,
            "history" => LibraryKind::History,
            _ => LibraryKind::Playlists,
        };
        request_library(&c, kind, None);
    });
    let c = core.clone();
    let weak = ui.as_weak();
    ui.global::<Panels>().on_library_back(move || {
        let Some(ui) = weak.upgrade() else { return };
        if STATE.with(|s| s.borrow().library_mode.clone()) == "playlist" {
            request_library(&c, LibraryKind::Playlists, None);
        } else {
            // back to the menu
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.library.clear();
                s.library_mode = "menu".into();
                s.lib_request = None;
            });
            ui.global::<Panels>().set_library_mode("menu".into());
            ui.global::<Panels>().set_library_title("".into());
            ui.global::<Panels>()
                .set_library(ModelRc::new(VecModel::default()));
            ui.global::<Panels>().set_panel_selected(-1);
        }
    });
    let c = core.clone();
    ui.global::<Panels>().on_play_all(move || {
        let items = STATE.with(|s| s.borrow().library.clone());
        if !items.is_empty() {
            c.send(Command::PlayItems { items, index: 0 });
        }
    });
    let c = core.clone();
    ui.global::<Panels>().on_shuffle_all(move || {
        let items = STATE.with(|s| s.borrow().library.clone());
        if !items.is_empty() {
            c.send(Command::ShuffleAll(items));
        }
    });
    let weak = ui.as_weak();
    let c = core.clone();
    ui.on_open_artist(move || {
        let id = STATE.with(|s| s.borrow().snap.artist_id.clone());
        if let (Some(id), Some(ui)) = (id, weak.upgrade()) {
            super::artist::open(&ui, &c, id);
        }
    });
    let weak = ui.as_weak();
    ui.on_open_album(move || {
        let id = STATE.with(|s| s.borrow().snap.album_id.clone());
        if let (Some(id), Some(ui)) = (id, weak.upgrade()) {
            super::artist::open_album(&ui, id);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.on_add_to_playlist(move |id| {
        if let (Some(ui), Some(item)) = (weak.upgrade(), playing_item()) {
            super::table::track_action(&ui, &c, id.as_str(), vec![item]);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.on_now_context(move |action| {
        if let (Some(ui), Some(item)) = (weak.upgrade(), playing_item()) {
            super::table::track_action(&ui, &c, action.as_str(), vec![item]);
        }
    });
    let c = core.clone();
    ui.on_more_like_this(move || {
        let item = playing_item();
        if let Some(item) = item {
            c.send(Command::StartRadio(item));
        }
    });
    let c = core.clone();
    ui.on_sidebar_changed(move |i| {
        if i == 3 {
            request_library(&c, LibraryKind::Playlists, None); // "Lists" = your playlists
        }
    });

    let weak = ui.as_weak();
    let c = core.clone();
    ui.on_menu_action(move |id| {
        let Some(ui) = weak.upgrade() else { return };
        match id.as_str() {
            "play-pause" => c.send(Command::Toggle),
            "stop" => c.send(Command::Stop),
            "next" => c.send(Command::Next),
            "prev" => c.send(Command::Prev),
            "like" => c.send(Command::ToggleLike),
            "add-to-playlist" => ui.invoke_open_add_popup(),
            "quit" => {
                let _ = slint::quit_event_loop();
            }
            "queue-clear" => c.send(Command::ClearQueue),
            "tab-new" => ui.invoke_tab_add(),
            "tab-close" => ui.invoke_tab_close(ui.get_playlist_tab()),
            "shuffle" => c.send(Command::ToggleShuffle),
            "repeat" => c.send(Command::CycleRepeat),
            "theme-system" => ui.global::<Dialogs>().invoke_set_theme(0),
            "theme-light" => ui.global::<Dialogs>().invoke_set_theme(1),
            "theme-dark" => ui.global::<Dialogs>().invoke_set_theme(2),
            id if id.starts_with("an-") => ui.invoke_analyzer_menu(id[3..].into()),
            "mini" => toggle_mini(&ui),
            "sidebar" => ui.set_sidebar_visible(!ui.get_sidebar_visible()),
            "settings" => {
                ui.global::<Dialogs>().set_settings_visible(true);
                ui.global::<Dialogs>().invoke_settings_opened();
            }
            "signin" => ui.global::<Dialogs>().set_setup_visible(true),
            "clear-cache" => ui.global::<Dialogs>().invoke_clear_cache(),
            "shortcuts" => show_toast(&ui, "Space play/pause · ←/→ seek · Ctrl+←/→ prev/next · Ctrl+↑/↓ volume · Ctrl+F search · Ctrl+L like · Ctrl+M mini player · Ctrl+T/W new/close tab · Ctrl+A select all · Alt+1-4 sidebar · Ctrl+, settings"),
            "about" => show_toast(&ui, "Cloudberry — unofficial client for YouTube Music, not affiliated with Google (GPL-3.0)"),
            _ => show_toast(&ui, "not available yet"),
        }
    });
}

thread_local! {
    static FULL_SIZE: std::cell::Cell<Option<slint::PhysicalSize>> = const { std::cell::Cell::new(None) };
}

/// Switch between the full window and the mini player (the window is resized, not replaced).
pub fn toggle_mini(ui: &AppWindow) {
    let mini = !ui.get_mini();
    if mini {
        FULL_SIZE.with(|s| s.set(Some(ui.window().size())));
    }
    // the new minimum size must reach the window before it is resized
    ui.set_mini(mini);
    let weak = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(30), move || {
        let Some(ui) = weak.upgrade() else { return };
        let k = ui.global::<crate::Theme>().get_font_scale();
        if mini {
            ui.window()
                .set_size(slint::LogicalSize::new(440.0 * k, 120.0 * k));
        } else if let Some(size) = FULL_SIZE.with(|s| s.take()) {
            ui.window().set_size(size);
        }
    });
}

/// The playing song as an `Item` (the snapshot carries what menus and radio need).
pub fn playing_item() -> Option<crate::core::model::Item> {
    STATE.with(|s| {
        let s = s.borrow();
        let snap = &s.snap;
        let video_id = snap.video_id.clone()?;
        Some(crate::core::model::Item {
            kind: crate::core::model::Kind::Song,
            title: snap.title.clone(),
            video_id: Some(video_id),
            browse_id: None,
            artists: if snap.artist_name.is_empty() {
                vec![]
            } else {
                vec![crate::core::model::Artist {
                    name: snap.artist_name.clone(),
                    id: snap.artist_id.clone(),
                }]
            },
            album: (!snap.album_name.is_empty()).then(|| snap.album_name.clone()),
            album_id: snap.album_id.clone(),
            duration: None,
            duration_secs: None,
            thumbnail: snap.thumbnail.clone(),
            subtitle: String::new(),
        })
    })
}
