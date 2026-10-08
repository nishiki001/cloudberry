//! Search results arriving in the sidebar list: the first page, then continuation pages.
use super::panels;
use super::state::*;
use crate::core::model::Item;
use crate::{AppWindow, Panels, TrackData};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

/// Mark the playing row in the results list.
pub fn refresh_playing_marks(ui: &AppWindow) {
    let vid = STATE.with(|s| s.borrow().snap.video_id.clone());
    let model = ui.global::<Panels>().get_results();
    STATE.with(|s| {
        let s = s.borrow();
        for (n, it) in s.results.iter().enumerate() {
            let p = vid.is_some() && it.video_id == vid;
            if let Some(mut r) = model.row_data(n)
                && r.playing != p
            {
                r.playing = p;
                model.set_row_data(n, r);
            }
        }
    });
}

pub fn first(ui: &AppWindow, items: Vec<Item>, more: bool) {
    // every kind is shown (albums / artists / playlists open as tabs); songs need an id
    let items: Vec<Item> = items
        .into_iter()
        .filter(|i| i.video_id.is_some() || i.browse_id.is_some())
        .collect();
    let rows: Vec<TrackData> = items.iter().map(|i| panel_row(i, false)).collect();
    if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
        panels::request_thumbs(&core, &items);
    }
    let any = !items.is_empty();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.results = items;
        s.search_more = more;
        s.search_loading = false;
    });
    let p = ui.global::<Panels>();
    p.set_search_loading(false);
    p.set_search_end(any && !more);
    ui.global::<Panels>().set_panel_selected(-1);
    ui.global::<Panels>()
        .set_results(ModelRc::new(VecModel::from(rows)));
    ui.global::<Panels>().set_busy(false);
    refresh_playing_marks(ui);
}

pub fn more(ui: &AppWindow, items: Vec<Item>, more: bool) {
    let items: Vec<Item> = items
        .into_iter()
        .filter(|i| i.video_id.is_some() || i.browse_id.is_some())
        .collect();
    let rows: Vec<TrackData> = items.iter().map(|i| panel_row(i, false)).collect();
    if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
        panels::request_thumbs(&core, &items);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.results.extend(items);
        s.search_more = more;
        s.search_loading = false;
    });
    let p = ui.global::<Panels>();
    if let Some(vm) = p
        .get_results()
        .as_any()
        .downcast_ref::<VecModel<TrackData>>()
    {
        vm.extend(rows);
    }
    p.set_search_loading(false);
    p.set_search_end(!more);
    refresh_playing_marks(ui);
}
