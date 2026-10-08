//! Screenshot fixtures: what each `--view` additionally sets up (dialogs, panels, menus…).
use crate::fixtures::{TRACKS, model, row};
use crate::fixtures_extra::thumbed;
use crate::{AppWindow, Dialogs, Panels, TrackData};
use slint::{ComponentHandle, ModelRc, VecModel};

pub fn view(ui: &AppWindow, view: &str, name: &str) {
    match view {
        "search" => {
            ui.set_sidebar_tab(2);
            ui.global::<Panels>().set_panel_selected(1);
            let mut rows: Vec<TrackData> = TRACKS
                .iter()
                .take(7)
                .enumerate()
                .map(|(n, t)| thumbed(row(t, false, false), n as f32 * 37.0))
                .collect();
            for (n, (title, sub, kind)) in [
                ("Discovery", "Album • Daft Punk • 2001", "album"),
                ("Daft Punk", "Artist • 12M monthly listeners", "artist"),
                ("Daft Punk essentials", "Playlist • 42 songs", "playlist"),
            ]
            .into_iter()
            .enumerate()
            {
                rows.push(thumbed(
                    TrackData {
                        title: title.into(),
                        artist: sub.into(),
                        album: "".into(),
                        year: "".into(),
                        time: "".into(),
                        playing: false,
                        liked: false,
                        selected: false,
                        kind: kind.into(),
                        thumb: Default::default(),
                        has_thumb: false,
                    },
                    200.0 + n as f32 * 40.0,
                ));
            }
            ui.global::<Panels>().set_results(model(rows));
        }
        "library" => {
            ui.set_sidebar_tab(1);
            crate::gui::fixture_tree(ui);
        }
        "page" => {
            ui.set_sidebar_tab(1);
            ui.global::<Panels>().set_library_mode("artist".into());
            ui.global::<Panels>().set_library_title("Daft Punk".into());
            ui.global::<Panels>().set_library(model(
                TRACKS
                    .iter()
                    .take(6)
                    .enumerate()
                    .map(|(n, t)| thumbed(row(t, false, false), n as f32 * 41.0))
                    .collect(),
            ));
        }
        "lists" => {
            ui.set_sidebar_tab(3);
            ui.global::<Panels>().set_library_mode("playlists".into());
            ui.global::<Panels>()
                .set_library_title("Your playlists".into());
            let lists = [
                "Road trip 「ドライブ」",
                "Король и Шут mix",
                "Focus",
                "Daft Punk essentials",
            ];
            ui.global::<Panels>().set_library(model(
                lists
                    .iter()
                    .map(|n| TrackData {
                        title: (*n).into(),
                        artist: "Playlist • 42 songs".into(),
                        album: "".into(),
                        year: "".into(),
                        time: "".into(),
                        playing: false,
                        liked: false,
                        selected: false,
                        kind: "song".into(),
                        thumb: Default::default(),
                        has_thumb: false,
                    })
                    .collect(),
            ));
        }
        "setup" => {
            ui.global::<Dialogs>().set_setup_visible(true);
            ui.global::<Dialogs>()
                .set_setup_status("Pick the browser you are signed in to.".into());
        }
        "discover" => crate::fixtures_discover::apply(ui, name),
        "menu" => {
            let sub = |t: &str, id: &str| crate::SubItem {
                label: t.into(),
                id: id.into(),
                enabled: true,
            };
            ui.global::<crate::Menus>()
                .set_playlists(ModelRc::new(VecModel::from(vec![
                    sub("Road trip", "pl:1"),
                    sub("Focus", "pl:2"),
                    sub("New playlist…", "pl:__new__"),
                ])));
            ui.global::<Dialogs>()
                .set_menu_demo(if name.starts_with("menu-bar") { 2 } else { 1 });
        }
        "artist" => crate::fixtures_artist::apply(ui, false),
        "album" => crate::fixtures_artist::apply(ui, true),
        "picker" => ui.global::<Dialogs>().set_picker_demo(true),
        "mini" => {
            ui.set_mini(true);
        }
        "analyzer" => {
            ui.global::<Dialogs>().set_settings_visible(true);
            ui.global::<Dialogs>().set_settings_page(2);
        }
        "account" | "cache" | "playback" => {
            ui.global::<Dialogs>().set_settings_visible(true);
            ui.global::<Dialogs>().set_settings_page(match view {
                "playback" => 0,
                "account" => 5,
                _ => 6,
            });
        }
        "addplaylist" | "newplaylist" => {
            let a = ui.global::<crate::AddPlaylist>();
            if view == "newplaylist" {
                a.set_new_name("Road trip".into());
                a.set_new_visible(true);
            } else {
                let row = |t: &str, d: &str| crate::PlaylistChoice {
                    id: t.into(),
                    title: t.into(),
                    detail: d.into(),
                    thumb: Default::default(),
                    has_thumb: false,
                };
                a.set_count(2);
                a.set_rows(slint::ModelRc::new(slint::VecModel::from(vec![
                    row("Road trip", "Playlist • 48 songs"),
                    row("Chill", "Playlist • 12 songs"),
                    row("Workout", "Playlist • 31 songs"),
                    row("Фонк и Шансон", "Playlist • 7 songs"),
                    row("日本語のプレイリスト", "Playlist • 120 songs"),
                ])));
                a.set_existing_visible(true);
            }
        }
        "fonts" => {
            ui.global::<Dialogs>().set_settings_visible(true);
            ui.global::<Dialogs>().set_settings_page(4);
        }
        "background" => {
            ui.global::<Dialogs>().set_settings_visible(true);
            ui.global::<Dialogs>().set_settings_page(3);
        }
        "appearance" => {
            ui.global::<Dialogs>().set_settings_visible(true);
            ui.global::<Dialogs>().set_settings_page(1);
        }
        "settings" => {
            ui.global::<Dialogs>().set_settings_visible(true);
            ui.global::<Dialogs>().set_cache_size("12.4 MB".into());
            ui.global::<Dialogs>()
                .set_setup_status("Signed in ✓".into());
        }
        _ => {}
    }
}
