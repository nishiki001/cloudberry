//! Interaction tests: drive the real UI with pointer and key events on Slint's testing backend.
//! Screenshots prove looks, not clicks; every interactive component needs a test here.
//! Menu rows are found by accessible label (they are `list-item`s); elements inside a popup
//! report positions relative to the popup, so `Menu` adds the popup's origin.
use crate::AppWindow;

use i_slint_backend_testing::{ElementHandle, ElementQuery};

use slint::platform::{Key, PointerEventButton, WindowEvent};

use slint::{ComponentHandle, LogicalPosition, PhysicalSize, SharedString};

use std::cell::RefCell;

use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

struct Ui {
    ui: AppWindow,
    menu_actions: Log,
    context: Log,
    origin: (f32, f32),
}

thread_local!(static READY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });

fn app() -> Ui {
    if !READY.with(|r| r.replace(true)) {
        i_slint_backend_testing::init_no_event_loop();
    }
    build()
}

/// Same, with a mock clock so timers can be advanced.
fn app_timed() -> Ui {
    app()
}

fn build() -> Ui {
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(PhysicalSize::new(1200, 760));
    ui.show().unwrap();
    crate::fixtures::apply(&ui, "main", "");
    let menu_actions = Log::default();
    let context = Log::default();
    let m = menu_actions.clone();
    ui.on_menu_action(move |id| m.borrow_mut().push(id.to_string()));
    let c = context.clone();
    ui.on_context_action(move |kind, id, row| {
        c.borrow_mut().push(format!("{kind}:{id}:{row}"));
    });
    Ui {
        ui,
        menu_actions,
        context,
        origin: (0.0, 0.0),
    }
}

impl Ui {
    fn send(&self, e: WindowEvent) {
        self.ui.window().dispatch_event(e);
    }
    fn at(&self, x: f32, y: f32) {
        self.send(WindowEvent::PointerMoved {
            position: LogicalPosition::new(x, y),
        });
    }
    fn click(&self, x: f32, y: f32, button: PointerEventButton) {
        self.at(x, y);
        let position = LogicalPosition::new(x, y);
        self.send(WindowEvent::PointerPressed { position, button });
        self.send(WindowEvent::PointerReleased { position, button });
    }
    fn key(&self, k: Key) {
        let text: SharedString = k.into();
        self.send(WindowEvent::KeyPressed { text: text.clone() });
        self.send(WindowEvent::KeyReleased { text });
    }
    /// Centre of the menu row labelled `label`, in window coordinates.
    fn row(&self, label: &str) -> (f32, f32) {
        let hit = self
            .item(label)
            .unwrap_or_else(|| panic!("no menu row \"{label}\""));
        let (p, s) = (hit.absolute_position(), hit.size());
        (
            self.origin.0 + p.x + s.width / 2.0,
            self.origin.1 + p.y + s.height / 2.0,
        )
    }
    fn item(&self, label: &str) -> Option<ElementHandle> {
        let l = label.to_string();
        ElementQuery::from_root(&self.ui)
            .match_accessible_role(i_slint_backend_testing::AccessibleRole::ListItem)
            .match_predicate(move |e| e.accessible_label().is_some_and(|x| x == l.as_str()))
            .find_first()
    }
    fn selected(&self, label: &str) -> bool {
        self.item(label)
            .and_then(|e| e.accessible_item_selected())
            .unwrap_or(false)
    }
    fn hover(&self, label: &str) {
        let (x, y) = self.row(label);
        self.at(x, y);
    }
    fn click_row(&self, label: &str) {
        let (x, y) = self.row(label);
        self.click(x, y, PointerEventButton::Left);
    }
    fn open_bar_menu(&mut self, x: f32) {
        self.click(x, 12.0, PointerEventButton::Left);
        self.origin = (4.0, 24.0);
    }
    /// Right-click the first queue row at (x, y); the menu opens at the pointer.
    fn open_row_menu(&mut self, x: f32, y: f32) {
        self.click(x, y, PointerEventButton::Right);
        self.origin = (x, y);
        // the track menu (15 rows, ThemedMenu est-h = 15 * 24 + 12) flips up at the bottom edge
        if y + 372.0 > 760.0 && self.item("Add to playlist").is_some() {
            self.origin.1 = y - 372.0;
        }
    }
}

impl Ui {
    fn by_label(&self, label: &str) -> ElementHandle {
        ElementHandle::find_by_accessible_label(&self.ui, label)
            .next()
            .unwrap_or_else(|| panic!("no element \"{label}\""))
    }
    fn click_label(&self, label: &str) {
        let e = self.by_label(label);
        let (p, s) = (e.absolute_position(), e.size());
        self.click(
            p.x + s.width / 2.0,
            p.y + s.height / 2.0,
            PointerEventButton::Left,
        );
    }
    fn type_text(&self, s: &str) {
        for c in s.chars() {
            let text: SharedString = c.to_string().into();
            self.send(WindowEvent::KeyPressed { text: text.clone() });
            self.send(WindowEvent::KeyReleased { text });
        }
    }
    fn rows_visible(&self, labels: &[&str]) -> Vec<bool> {
        labels.iter().map(|l| self.item(l).is_some()).collect()
    }
}

mod add_playlist;
mod library;
mod menus;
mod playlist_refresh;
mod toolbar;
mod tools;
mod track_menu;
