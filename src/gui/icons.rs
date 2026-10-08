//! Icon sets: answers the `Icons.lookup` callback from the SVGs of the active set.

use crate::core::icons;
use crate::{AppWindow, Icons};
use slint::{ComponentHandle, Image};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CACHE: RefCell<HashMap<(String, String), Image>> = RefCell::new(HashMap::new());
}

fn load(set: &str, name: &str) -> Image {
    let key = (set.to_string(), name.to_string());
    if let Some(i) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return i;
    }
    let img = icons::svg(set, name)
        .and_then(|b| Image::load_from_svg_data(b).ok())
        .unwrap_or_default();
    CACHE.with(|c| c.borrow_mut().insert(key, img.clone()));
    img
}

pub fn init(ui: &AppWindow, set: &str) {
    let g = ui.global::<Icons>();
    g.on_lookup(|name, set| load(&set, &name));
    apply(ui, set);
}

/// Switch the active set; every `Icon` re-evaluates through the `set` dependency.
pub fn apply(ui: &AppWindow, set: &str) {
    let set = icons::parse(set);
    let g = ui.global::<Icons>();
    g.set_mono(icons::mono(set));
    g.set_set(set.into());
}
