//! `--smoke`: a scripted run that drives the UI callbacks the way a user would (search → play,
//! Discover → open/play a card, open a tab → play a row, "More like this") so re-entrancy bugs
//! (RefCell double borrows) show up as a panic. Meant for `APP_AO=null` runs.
use crate::{AppWindow, ArtistPage, Discover, Panels};
use slint::Model;
use slint::{ComponentHandle, Timer, TimerMode};
use std::time::Duration;

type Step = fn(&AppWindow);

const STEPS: [(u64, &str, Step); 15] = [
    (0, "expand the library sections", |ui| {
        ui.set_sidebar_tab(1);
        for row in [3, 2, 1, 0] {
            ui.global::<Panels>().invoke_tree_clicked(row);
        }
    }),
    (1, "search", |ui| {
        ui.set_sidebar_tab(2);
        ui.global::<Panels>()
            .invoke_submit_search("daft punk get lucky".into());
    }),
    (5, "play from search", |ui| {
        ui.global::<Panels>().invoke_search_activate(0)
    }),
    (7, "go to artist from the row menu", |ui| {
        ui.invoke_context_action("queue".into(), "artist".into(), 0);
    }),
    (12, "check the artist page", |ui| {
        let p = ui.global::<ArtistPage>();
        assert!(
            p.get_active() && !p.get_album_mode(),
            "smoke: artist page not shown"
        );
        assert!(!p.get_name().is_empty(), "smoke: artist page has no name");
        eprintln!(
            "smoke: artist page \"{}\" with {} songs",
            p.get_name(),
            p.get_songs().row_count()
        );
    }),
    (13, "go to album from the row menu", |ui| {
        ui.invoke_tab_selected(0); // back to the queue (its first row is a search result)
        ui.invoke_context_action("queue".into(), "album".into(), 0);
    }),
    (18, "check the album page", |ui| {
        let p = ui.global::<ArtistPage>();
        assert!(
            p.get_active() && p.get_album_mode(),
            "smoke: album page not shown"
        );
        assert!(
            p.get_songs().row_count() > 0,
            "smoke: album page has no tracks"
        );
        eprintln!(
            "smoke: album page \"{}\" with {} tracks",
            p.get_name(),
            p.get_songs().row_count()
        );
    }),
    (23, "discover home", |ui| {
        ui.global::<Discover>().invoke_choose_section("home".into())
    }),
    (27, "play a discover card", |ui| {
        ui.global::<Discover>().invoke_card_play(0, 0)
    }),
    (31, "open a discover card", |ui| {
        ui.global::<Discover>().invoke_card_clicked(1, 0)
    }),
    (35, "play a row of the queue", |ui| {
        ui.invoke_tab_selected(0);
        ui.invoke_table_row_activated(0);
    }),
    (39, "more like this", |ui| ui.invoke_more_like_this()),
    (43, "discover charts", |ui| {
        ui.global::<Discover>()
            .invoke_choose_section("charts".into())
    }),
    (8, "library counts", |ui| {
        let labels: Vec<String> = ui
            .global::<Panels>()
            .get_tree()
            .iter()
            .filter(|r| r.kind == "section")
            .map(|r| r.label.to_string())
            .collect();
        eprintln!("smoke: library sections {labels:?}");
        ui.set_sidebar_tab(1);
        assert!(
            labels
                .iter()
                .take(4)
                .all(|l| !l.contains("(0") && l.contains('(')),
            "smoke: library sections without content: {labels:?}"
        );
    }),
    (46, "done", |_| {
        let _ = slint::quit_event_loop();
    }),
];

/// Schedule the script; the timers live as long as the returned vector.
pub fn run(ui: &AppWindow) -> Vec<Timer> {
    STEPS
        .iter()
        .map(|&(secs, name, step)| {
            let t = Timer::default();
            let weak = ui.as_weak();
            t.start(
                TimerMode::SingleShot,
                Duration::from_secs(secs),
                move || {
                    if let Some(ui) = weak.upgrade() {
                        eprintln!("smoke: {name}");
                        step(&ui);
                    }
                },
            );
            t
        })
        .collect()
}
