use super::*;

#[test]
fn menu_bar_music_next() {
    let mut t = app();
    t.open_bar_menu(28.0);
    t.hover("Next");
    assert!(t.selected("Next"), "hover must highlight the row");
    assert!(!t.selected("Stop"));
    t.click_row("Next");
    assert_eq!(*t.menu_actions.borrow(), ["next"]);
}

#[test]
fn escape_closes_the_menu() {
    let mut t = app();
    t.open_bar_menu(28.0);
    let (x, y) = t.row("Next");
    t.hover("Next");
    assert!(t.selected("Next"), "menu must be open before Esc");
    t.key(Key::Escape);
    t.click(x, y, PointerEventButton::Left);
    assert!(
        t.menu_actions.borrow().is_empty(),
        "closed menu must not fire"
    );
}

#[test]
fn menu_bar_keyboard_enter() {
    let mut t = app();
    t.open_bar_menu(28.0);
    t.key(Key::DownArrow);
    t.key(Key::Return);
    assert_eq!(*t.menu_actions.borrow(), ["play-pause"]);
}

#[test]
fn row_menu_play() {
    let mut t = app();
    t.open_row_menu(500.0, 227.0); // queue row 3
    t.hover("Play");
    assert!(t.selected("Play"));
    t.click_row("Play");
    assert_eq!(*t.context.borrow(), ["queue:play:3"]);
}

#[test]
fn row_menu_go_to_artist() {
    let mut t = app();
    t.open_row_menu(500.0, 203.0); // queue row 2
    t.click_row("Go to artist");
    assert_eq!(*t.context.borrow(), ["queue:artist:2"]);
}

#[test]
fn row_menu_add_to_playlist_submenu() {
    let mut t = app();
    t.open_row_menu(500.0, 155.0); // queue row 0
    t.hover("Add to playlist");
    let (x, y) = t.row("Existing playlist…");
    // the submenu sits to the right of the 230 px panel, outside the root's popup origin
    assert!(x > t.origin.0 + 230.0);
    t.click(x, y, PointerEventButton::Left);
    assert_eq!(*t.context.borrow(), ["queue:pl:__existing__:0"]);
}

#[test]
fn row_menu_escape_and_outside_click() {
    let mut t = app();
    t.open_row_menu(500.0, 227.0);
    let (x, y) = t.row("Play");
    t.hover("Play");
    assert!(t.selected("Play"), "menu must be open before Esc");
    t.key(Key::Escape);
    t.click(x, y, PointerEventButton::Left);
    assert!(t.context.borrow().is_empty());
    t.open_row_menu(500.0, 227.0);
    let (x, y) = t.row("Play");
    t.click(900.0, 600.0, PointerEventButton::Left); // outside closes
    t.click(x, y, PointerEventButton::Left);
    assert!(t.context.borrow().is_empty());
}

/// Right-click down the list until a menu with `label` opens; returns the y that worked.
fn open_menu_somewhere(t: &mut Ui, x: f32, label: &str) -> f32 {
    for y in (60..700).step_by(14) {
        let y = y as f32;
        t.open_row_menu(x, y);
        if t.item(label).is_some() {
            return y;
        }
        t.key(Key::Escape);
    }
    panic!("no menu with \"{label}\" opened anywhere in the panel");
}

/// A menu must outlive 2 s of playback: position ticks, model patches and a list refresh must
/// not close it, and a click on an item still fires the command.
fn keep_busy(t: &Ui, patch: impl Fn()) {
    for n in 0..20 {
        t.ui.set_position(n as f32 / 20.0);
        patch();
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(100));
    }
}

#[test]
fn library_tree_menu_survives_updates() {
    let mut t = app_timed();
    crate::fixtures::apply(&t.ui, "library", "");
    t.ui.set_sidebar_tab(1);
    let got = Rc::new(RefCell::new(Vec::<String>::new()));
    let g = got.clone();
    t.ui.global::<crate::Panels>()
        .on_tree_context(move |a, i| g.borrow_mut().push(format!("{a}:{i}")));
    let y = open_menu_somewhere(&mut t, 150.0, "Play");
    let tree = t.ui.global::<crate::Panels>().get_tree();
    keep_busy(&t, || {
        use slint::Model;
        for i in 0..tree.row_count() {
            if let Some(mut r) = tree.row_data(i) {
                r.playing = !r.playing;
                tree.set_row_data(i, r);
            }
        }
    });
    assert!(
        t.item("Play").is_some(),
        "menu closed by updates (opened at y={y})"
    );
    t.hover("Play");
    assert!(t.selected("Play"));
    t.click_row("Play");
    assert_eq!(got.borrow().len(), 1, "{:?}", got.borrow());
    assert!(got.borrow()[0].starts_with("play:"));
}

#[test]
fn search_row_menu_survives_updates() {
    let mut t = app_timed();
    crate::fixtures::apply(&t.ui, "search", "");
    t.ui.set_sidebar_tab(2);
    let y = open_menu_somewhere(&mut t, 150.0, "Play");
    let rows = t.ui.global::<crate::Panels>().get_results();
    keep_busy(&t, || {
        use slint::Model;
        for i in 0..rows.row_count() {
            if let Some(mut r) = rows.row_data(i) {
                r.playing = !r.playing;
                rows.set_row_data(i, r);
            }
        }
    });
    assert!(
        t.item("Play").is_some(),
        "menu closed by updates (opened at y={y})"
    );
    t.click_row("Play");
    let c = t.context.borrow();
    assert_eq!(c.len(), 1, "{c:?}");
    assert!(c[0].starts_with("results:play:"), "{c:?}");
}

#[test]
fn table_row_menu_survives_updates() {
    let mut t = app_timed();
    t.open_row_menu(500.0, 227.0);
    let rows = t.ui.get_table_rows();
    keep_busy(&t, || {
        use slint::Model;
        for i in 0..rows.row_count() {
            if let Some(mut r) = rows.row_data(i) {
                r.playing = !r.playing;
                rows.set_row_data(i, r);
            }
        }
    });
    assert!(t.item("Play").is_some(), "menu closed by updates");
    t.click_row("Play");
    assert_eq!(*t.context.borrow(), ["queue:play:3"]);
}

#[test]
fn discover_card_menu_survives_updates() {
    let mut t = app();
    crate::fixtures::apply(&t.ui, "discover", "");
    let got = Rc::new(RefCell::new(Vec::<String>::new()));
    let g = got.clone();
    t.ui.global::<crate::Discover>()
        .on_card_context(move |a, s, i| g.borrow_mut().push(format!("{a}:{s}:{i}")));
    let mut found = None;
    'scan: for y in (80..700).step_by(20) {
        for x in (330..1100).step_by(60) {
            t.open_row_menu(x as f32, y as f32);
            if t.item("Start radio").is_some() {
                found = Some((x, y));
                break 'scan;
            }
            t.key(Key::Escape);
        }
    }
    assert!(found.is_some(), "no Discover card opened a menu");
    keep_busy(&t, || {});
    assert!(t.item("Start radio").is_some(), "menu closed by updates");
    t.click_row("Start radio");
    let g = got.borrow();
    assert_eq!(g.len(), 1, "{g:?}");
    assert!(g[0].starts_with("radio:"), "{g:?}");
}

#[test]
fn artist_page_song_menu() {
    let mut t = app();
    crate::fixtures::apply(&t.ui, "artist", "");
    let got = Rc::new(RefCell::new(Vec::<String>::new()));
    let g = got.clone();
    t.ui.global::<crate::ArtistPage>()
        .on_song_context(move |a, i| g.borrow_mut().push(format!("{a}:{i}")));
    let mut ok = false;
    'scan: for y in (150..700).step_by(15) {
        for x in [450.0f32, 700.0] {
            t.open_row_menu(x, y as f32);
            if t.item("Start radio").is_some() {
                ok = true;
                break 'scan;
            }
            t.key(Key::Escape);
        }
    }
    assert!(ok, "no artist song opened a menu");
    keep_busy(&t, || {});
    t.click_row("Add to queue");
    assert!(
        got.borrow().iter().any(|s| s.starts_with("enqueue:")),
        "{:?}",
        got.borrow()
    );
}

/// The whole model is replaced (delegates are recreated) while the menu is open: the pick still
/// reaches the right owner and row.
#[test]
fn library_menu_survives_model_replacement() {
    use slint::Model;
    let mut t = app();
    crate::fixtures::apply(&t.ui, "library", "");
    t.ui.set_sidebar_tab(1);
    let got = Rc::new(RefCell::new(Vec::<String>::new()));
    let g = got.clone();
    t.ui.global::<crate::Panels>()
        .on_tree_context(move |a, i| g.borrow_mut().push(format!("{a}:{i}")));
    open_menu_somewhere(&mut t, 150.0, "Play");
    let rows: Vec<_> = t.ui.global::<crate::Panels>().get_tree().iter().collect();
    t.ui.global::<crate::Panels>()
        .set_tree(slint::ModelRc::new(slint::VecModel::from(rows)));
    keep_busy(&t, || {});
    assert!(
        t.item("Play").is_some(),
        "menu closed by a model replacement"
    );
    t.click_row("Play");
    assert_eq!(got.borrow().len(), 1, "{:?}", got.borrow());
}
