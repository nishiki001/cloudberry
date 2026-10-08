//! The one track menu: every place that lists tracks offers "Add to playlist ▸ Existing playlist…".
use super::*;

/// Right-click over the grid until a menu with "Add to playlist" opens whose "Existing playlist…"
/// reaches `log` (disabled entries do nothing); true when it did.
fn add_to_existing(t: &mut Ui, xs: &[i32], ys: std::ops::Range<i32>, log: &Log) -> bool {
    for y in ys.step_by(14) {
        for &x in xs {
            t.open_row_menu(x as f32, y as f32);
            if t.item("Add to playlist").is_some() {
                t.hover("Add to playlist");
                let (px, py) = t.row("Existing playlist…");
                t.click(px, py, PointerEventButton::Left);
                if !log.borrow().is_empty() {
                    return true;
                }
            }
            t.key(Key::Escape);
        }
    }
    false
}

fn new_log() -> Log {
    Log::default()
}

#[test]
fn queue_and_search_rows_add_to_playlist() {
    let mut t = app();
    t.open_row_menu(500.0, 155.0);
    t.hover("Add to playlist");
    let (x, y) = t.row("Existing playlist…");
    t.click(x, y, PointerEventButton::Left);
    assert_eq!(*t.context.borrow(), ["queue:pl:__existing__:0"]);

    crate::fixtures::apply(&t.ui, "search", "");
    t.ui.set_sidebar_tab(2);
    t.context.borrow_mut().clear();
    let log = t.context.clone();
    assert!(add_to_existing(&mut t, &[150], 60..700, &log));
    assert!(t.context.borrow()[0].starts_with("results:pl:__existing__:"));
}

#[test]
fn library_tree_add_to_playlist() {
    let mut t = app();
    crate::fixtures::apply(&t.ui, "library", "");
    t.ui.set_sidebar_tab(1);
    let log = new_log();
    let g = log.clone();
    t.ui.global::<crate::Panels>()
        .on_tree_context(move |a, i| g.borrow_mut().push(format!("{a}:{i}")));
    assert!(add_to_existing(&mut t, &[150], 60..700, &log));
    assert!(
        log.borrow()[0].starts_with("pl:__existing__:"),
        "{:?}",
        log.borrow()
    );
}

#[test]
fn artist_song_add_to_playlist() {
    let mut t = app();
    crate::fixtures::apply(&t.ui, "artist", "");
    let log = new_log();
    let g = log.clone();
    t.ui.global::<crate::ArtistPage>()
        .on_song_context(move |a, i| g.borrow_mut().push(format!("{a}:{i}")));
    assert!(add_to_existing(&mut t, &[450, 700], 150..700, &log));
    assert!(
        log.borrow()[0].starts_with("pl:__existing__:"),
        "{:?}",
        log.borrow()
    );
}

#[test]
fn discover_cards_and_songs_add_to_playlist() {
    let mut t = app();
    crate::fixtures::apply(&t.ui, "discover", "");
    let log = new_log();
    let g = log.clone();
    t.ui.global::<crate::Discover>()
        .on_card_context(move |a, s, i| g.borrow_mut().push(format!("{a}:{s}:{i}")));
    let xs: Vec<i32> = (330..1100).step_by(60).collect();
    assert!(add_to_existing(&mut t, &xs, 80..700, &log));
    assert!(
        log.borrow()[0].starts_with("pl:__existing__:"),
        "{:?}",
        log.borrow()
    );
}

/// The shared handler: the choice reaches the real dialog code and the fake core gets the add.
#[test]
fn track_action_adds_to_the_chosen_playlist() {
    use crate::core::msg::Command;
    let t = app();
    let (core, mut rx) = crate::core::runtime::CoreHandle::fake();
    crate::gui::playlist_add::wire(&t.ui, &core);
    while rx.try_recv().is_ok() {}
    let item = super::add_playlist::song("a");
    assert!(crate::gui::table::track_action(
        &t.ui,
        &core,
        "pl:__existing__",
        vec![item]
    ));
    assert!(t.ui.global::<crate::AddPlaylist>().get_existing_visible());
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok()).any(|c| matches!(c, Command::LoadAddTargets))
    );
}

#[test]
fn now_panel_add_to_playlist() {
    let mut t = app();
    t.ui.set_sidebar_tab(0);
    let log = new_log();
    let g = log.clone();
    t.ui.on_now_context(move |a| g.borrow_mut().push(a.to_string()));
    assert!(add_to_existing(&mut t, &[60, 120], 60..400, &log));
    assert_eq!(*log.borrow(), ["pl:__existing__"]);
}

fn watch_add(t: &Ui) -> Log {
    let log = new_log();
    let g = log.clone();
    t.ui.on_add_to_playlist(move |id| g.borrow_mut().push(id.to_string()));
    log
}

/// The player bar's "Add to playlist" button: popup with both entries, disabled with no track.
#[test]
fn player_bar_add_button() {
    let mut t = app();
    let log = watch_add(&t);
    let b = t.by_label("Add to playlist");
    let (p, s) = (b.absolute_position(), b.size());
    // ThemedMenu keeps room for a submenu: x is clamped to win-w - 440
    t.origin = (p.x.min(1200.0 - 440.0), p.y + s.height);
    t.click_label("Add to playlist");
    assert!(t.item("New playlist…").is_some());
    t.click_row("Existing playlist…");
    assert_eq!(*log.borrow(), ["pl:__existing__"]);

    t.click_label("Add to playlist");
    t.click_row("New playlist…");
    assert_eq!(log.borrow()[1], "pl:__new__");

    t.ui.set_has_track(false);
    t.click_label("Add to playlist");
    assert!(
        t.item("New playlist…").is_none(),
        "disabled button must not open"
    );
}

/// Ctrl+P and the Music menu entry open the same popup.
#[test]
fn add_to_playlist_shortcut_and_menu() {
    let mut t = app();
    let log = watch_add(&t);
    let g = t.ui.global::<crate::PlayerAdd>();
    t.origin = (g.get_x(), g.get_y());
    t.click(600.0, 400.0, PointerEventButton::Left); // focus the window
    t.send(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    t.send(WindowEvent::KeyPressed { text: "p".into() });
    t.send(WindowEvent::KeyReleased { text: "p".into() });
    t.send(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    t.click_row("Existing playlist…");
    assert_eq!(*log.borrow(), ["pl:__existing__"]);

    t.open_bar_menu(28.0);
    t.click_row("Add to playlist…");
    assert_eq!(*t.menu_actions.borrow(), ["add-to-playlist"]);
}

/// The mini player has its own button for the same popup.
#[test]
fn mini_player_add_button() {
    let mut t = app();
    let log = watch_add(&t);
    t.ui.set_mini(true);
    let b = t.by_label("Add to playlist");
    let (p, s) = (b.absolute_position(), b.size());
    // the button sits at the window bottom: the 2-row popup flips up (est-h = 2 * 24 + 12)
    t.origin = (p.x, p.y + s.height - 60.0);
    t.click_label("Add to playlist");
    t.click_row("Existing playlist…");
    assert_eq!(*log.borrow(), ["pl:__existing__"]);
}
