//! A user's own playlist is opened as a tab: the cached copy is shown at once and the network
//! copy replaces it a moment later. That background refresh must not close an open menu, and a
//! double-click / menu action must play the track that was under the pointer.
use super::library::{Api, fake_api, fixture, write_fake_cookies};
use super::*;
use crate::core::model::Item;
use crate::core::msg::{Command, Event, LibraryKind};
use slint::Model;

/// The tracks the fake service serves for the playlist (both pages).
fn served() -> Vec<Item> {
    let page = |name: &str| {
        let v: serde_json::Value = serde_json::from_str(&fixture(name)).unwrap();
        crate::core::api::library::parse_playlist_page(&v).0
    };
    let mut all = page("playlist_page1");
    all.extend(page("playlist_page2"));
    all
}

struct Rig {
    t: Ui,
    core: crate::core::runtime::CoreHandle,
    events: std::sync::mpsc::Receiver<Event>,
}

impl Rig {
    /// A playlist tab backed by `cached` (what the disk cache held) and the fake service.
    fn open(list: &str, cached: &[Item]) -> Rig {
        write_fake_cookies();
        let dir = crate::config::cache_dir().join("lists");
        crate::core::listcache::save(&dir, list, cached);
        let (core, events) = crate::core::runtime::tests::start_with_api(Some(fake_api(Api::Ok)));
        let t = app();
        crate::gui::state::STATE.with(|s| s.borrow_mut().core = Some(core.clone()));
        crate::gui::table::wire(&t.ui, &core);
        crate::gui::table::refresh_columns(&t.ui);
        crate::gui::table::refresh(&t.ui);
        crate::gui::wire_lib::open_as_tab(&core, LibraryKind::Playlist, list.into(), "Mine".into());
        let mut rig = Rig { t, core, events };
        // the cached copy arrives first and opens the tab
        rig.pump_until(|r| r.rows() > 0);
        rig
    }
    fn rows(&self) -> usize {
        self.t.ui.get_table_rows().row_count()
    }
    fn title(&self, n: usize) -> String {
        self.t
            .ui
            .get_table_rows()
            .row_data(n)
            .unwrap()
            .title
            .to_string()
    }
    /// Handle events that have arrived (all of them, or until `stop` holds).
    fn pump_until(&mut self, stop: impl Fn(&Rig) -> bool) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !stop(self) && std::time::Instant::now() < end {
            while let Ok(ev) = self.events.try_recv() {
                crate::gui::handle_event(&self.t.ui, ev);
                if stop(self) {
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    /// Short idle period for the tests (real time).
    fn quiet(&self, ms: u64) {
        crate::gui::state::STATE.with(|s| s.borrow_mut().quiet_ms = ms);
    }
    /// Wait for the network copy to arrive and be handled: it is either waiting (deferred) or
    /// already applied.
    fn refresh_fires(&mut self) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let before = self.rows();
        while std::time::Instant::now() < end {
            while let Ok(ev) = self.events.try_recv() {
                crate::gui::handle_event(&self.t.ui, ev);
            }
            if self.waiting() || self.rows() != before {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("the network copy never arrived");
    }
    fn waiting(&self) -> bool {
        crate::gui::state::STATE.with(|s| !s.borrow().deferred_refresh.is_empty())
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.core.send(Command::Quit);
    }
}

/// The cached copy differs from the service in one title and one track missing (so the refresh
/// really replaces something).
fn stale(items: &[Item]) -> Vec<Item> {
    let mut v: Vec<Item> = items.to_vec();
    v[2].title = "An old title".into();
    v.remove(0);
    v
}

#[test]
fn menu_survives_the_background_refresh() {
    let served = served();
    let mut r = Rig::open("PLtest_menu_survive", &stale(&served));
    let before = r.rows();
    r.t.open_row_menu(500.0, 203.0); // row 2
    assert!(r.t.item("Play").is_some());
    r.quiet(300);
    // what a playing track does to the window every second
    r.t.ui.set_elapsed("0:07".into());
    r.t.ui.set_status_message("x".into());
    r.refresh_fires();
    assert_eq!(
        r.rows(),
        before,
        "the refresh must wait while the menu is open"
    );
    assert!(r.t.item("Play").is_some(), "menu closed by the refresh");
}

impl Rig {
    /// Video id of the track on table row `n` (from the tab's items).
    fn row_id(&self, n: usize) -> String {
        crate::gui::state::STATE.with(|s| {
            let s = s.borrow();
            crate::gui::table::current_items(&s)[n]
                .video_id
                .clone()
                .unwrap()
        })
    }
    /// Pump until the core reports a queue whose current track is `id`.
    fn playing(&mut self, id: &str) -> bool {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < end {
            while let Ok(ev) = self.events.try_recv() {
                if let Event::Queue { items, current } = &ev
                    && current
                        .and_then(|c| items.get(c))
                        .and_then(|i| i.video_id.as_deref())
                        == Some(id)
                {
                    return true;
                }
                crate::gui::handle_event(&self.t.ui, ev);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }
    /// The user is idle: the waiting refresh may be applied (tests use a short idle time).
    fn idle_and_refresh(&mut self) {
        self.quiet(1);
        std::thread::sleep(std::time::Duration::from_millis(20));
        self.refresh_fires();
    }
}

/// Once the menu is closed and the user is idle, the refresh arrives (and the list is the
/// service's).
#[test]
fn refresh_is_applied_when_the_user_is_idle() {
    let served = served();
    let mut r = Rig::open("PLtest_refresh_is_a", &stale(&served));
    let before = r.rows();
    r.t.open_row_menu(500.0, 203.0);
    r.quiet(200);
    r.refresh_fires();
    assert_eq!(r.rows(), before);
    r.t.key(Key::Escape);
    std::thread::sleep(std::time::Duration::from_millis(250));
    for _ in 0..4 {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
    }
    assert_eq!(
        r.rows(),
        served.len(),
        "the waiting refresh was never applied"
    );
    assert_eq!(r.title(2), served[2].title);
}

/// A refresh that adds a row on top moves every row: the double-click that started on a track
/// still plays that track.
#[test]
fn double_click_plays_the_track_not_the_row() {
    let served = served();
    let mut r = Rig::open("PLtest_double_click", &stale(&served));
    let pressed = r.row_id(3); // row 3 of the cached list
    // press on row 3 (y of row 3 = 227), release; the refresh then shifts the rows
    r.t.click(500.0, 227.0, PointerEventButton::Left);
    r.idle_and_refresh();
    assert_ne!(
        r.row_id(3),
        pressed,
        "the refresh should have moved the track"
    );
    r.t.ui.invoke_table_row_activated(3); // the second click of the double-click arrives
    assert!(r.playing(&pressed), "played another track than {pressed}");
}

#[test]
fn menu_action_plays_the_track_not_the_row() {
    let served = served();
    let mut r = Rig::open("PLtest_menu_action_", &stale(&served));
    let pressed = r.row_id(3);
    r.t.ui.global::<crate::Menus>().invoke_prepare_row(3);
    r.idle_and_refresh();
    assert_ne!(r.row_id(3), pressed);
    // what the row menu's "Play" ends in (the context-action wiring is not part of this rig)
    crate::gui::table::context(&r.t.ui, &r.core, "play", 3);
    assert!(r.playing(&pressed), "played another track than {pressed}");
}

/// An edit made while the copy waited wins: the removed track does not come back.
#[test]
fn local_edit_wins_over_a_waiting_refresh() {
    let served = served();
    let mut r = Rig::open("PLtest_edit", &stale(&served));
    r.t.open_row_menu(500.0, 203.0);
    r.quiet(100);
    r.refresh_fires();
    assert!(r.waiting());
    // the user removes a track (what "Remove" does to the tab's items)
    let removed = crate::gui::state::STATE.with(|s| s.borrow_mut().tabs[0].items.remove(1));
    r.t.key(Key::Escape);
    std::thread::sleep(std::time::Duration::from_millis(150));
    for _ in 0..4 {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
    }
    assert!(!r.waiting());
    let items = crate::gui::state::STATE.with(|s| s.borrow().tabs[0].items.clone());
    assert!(
        !items.contains(&removed),
        "the refresh brought the removed track back"
    );
}
