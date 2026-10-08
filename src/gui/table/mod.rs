//! Playlist table glue. Tab 0 is the Queue (rows live in the core); other tabs are UI-owned.
//!
//! The Slint repeaters are fed from *persistent* models and updated in place: replacing a model
//! while the mouse button is down would destroy the row under the cursor and with it the drag /
//! double-click / resize gesture. So selection and playing marks only touch the rows that
//! changed (`refresh_marks`); the whole content is replaced (`refresh`) only when the list
//! itself changed.
mod cols;
mod ctx;
pub(super) mod gesture;
mod select;

use super::state::{STATE, UiState, fmt_time, row};
use crate::core::model::Item;
use crate::{AppWindow, TrackData};
use slint::ComponentHandle;
use slint::{Model, ModelRc, SharedString, VecModel};

pub use cols::{refresh_columns, wire_width};
pub use ctx::{context, prepare_row, track_action};
pub use select::insert_items;

/// Items shown by the current tab.
pub fn current_items(s: &UiState) -> Vec<Item> {
    if s.cur_tab == 0 {
        s.queue_items.clone()
    } else {
        s.tabs
            .get(s.cur_tab - 1)
            .map(|t| t.items.clone())
            .unwrap_or_default()
    }
}

/// (playing, liked) for row `n`, from the current UI state.
fn marks(s: &UiState, n: usize, item: &Item) -> (bool, bool) {
    let playing = if s.cur_tab == 0 {
        s.queue_current == Some(n)
    } else {
        s.snap.video_id.is_some() && item.video_id == s.snap.video_id
    };
    (playing, playing && s.snap.liked)
}

/// Install the persistent models into the window (once, before the first refresh).
pub fn install_models(ui: &AppWindow) {
    STATE.with(|s| {
        let s = s.borrow();
        ui.set_table_rows(ModelRc::from(s.rows_model.clone()));
        ui.set_columns(ModelRc::from(s.cols_model.clone()));
        ui.set_all_columns(ModelRc::from(s.all_cols_model.clone()));
    });
}

/// Bring the table in line with the current tab: rows, summary, tab names. The persistent row
/// model is updated in place — a different tab replaces it, the same tab is diffed by track key
/// (only rows that changed are touched, an identical list touches nothing), so open menus,
/// double-clicks and drags on the rows that stay are not disturbed.
pub fn refresh(ui: &AppWindow) {
    let (rows, keys, tab, summary, names, cur) = STATE.with(|s| {
        let s = s.borrow();
        let items = current_items(&s);
        let rows: Vec<TrackData> = items
            .iter()
            .enumerate()
            .map(|(n, i)| {
                let (playing, liked) = marks(&s, n, i);
                let mut r = row(i, playing);
                r.selected = s.sel.binary_search(&n).is_ok();
                r.liked = liked;
                r
            })
            .collect();
        let keys: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(n, i)| gesture::key(i, n))
            .collect();
        let tab = match s.cur_tab {
            0 => 0,
            n => s.tabs.get(n - 1).map_or(0, |t| t.uid),
        };
        let total: u32 = items.iter().filter_map(|i| i.duration_secs).sum();
        let summary = if items.is_empty() {
            String::new()
        } else {
            format!("{} tracks · {}", items.len(), fmt_time(total as f64))
        };
        let names: Vec<SharedString> = std::iter::once("Queue".to_string())
            .chain(s.tabs.iter().map(|t| t.name.clone()))
            .map(Into::into)
            .collect();
        (rows, keys, tab, summary, names, s.cur_tab)
    });
    let touched = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let model = s.rows_model.clone();
        let same_tab = s.shown_tab == Some(tab) && s.row_keys.len() == model.row_count();
        let old_keys = std::mem::replace(&mut s.row_keys, keys.clone());
        s.shown_tab = Some(tab);
        (model, same_tab.then_some(old_keys))
    });
    // the borrow is released: model changes run Slint code
    let (model, old_keys) = touched;
    let changed = match old_keys {
        Some(old) => diff_rows(&model, &old, &keys, rows),
        None => {
            model.set_vec(rows);
            true
        }
    };
    tracing::debug!(rows = keys.len(), changed, "table refresh");
    ui.set_status_summary(summary.into());
    if names.len() != ui.get_playlist_tabs().row_count()
        || (0..names.len()).any(|n| ui.get_playlist_tabs().row_data(n).as_ref() != names.get(n))
    {
        ui.set_playlist_tabs(ModelRc::new(VecModel::from(names)));
    }
    if ui.get_playlist_tab() != cur as i32 {
        ui.set_playlist_tab(cur as i32);
    }
}

/// Make `model` equal to `new` touching as little as possible: rows whose key is unchanged and
/// whose content is equal are left alone; the changed middle part is rewritten (rows updated,
/// extra ones inserted or removed). Returns whether anything changed.
fn diff_rows(
    model: &VecModel<TrackData>,
    old: &[String],
    new_keys: &[String],
    new: Vec<TrackData>,
) -> bool {
    let mut new = new;
    // a row replaces another one only when something visible differs; a cover already loaded
    // stays (covers arrive separately and are patched into the model)
    let differs =
        |m: &VecModel<TrackData>, at: usize, new: &mut TrackData, same_track: bool| -> bool {
            let Some(o) = m.row_data(at) else {
                return true;
            };
            if same_track && o.has_thumb && !new.has_thumb {
                new.thumb = o.thumb.clone();
                new.has_thumb = true;
            }
            (&o.title, &o.artist, &o.album, &o.year, &o.time, &o.kind)
                != (
                    &new.title,
                    &new.artist,
                    &new.album,
                    &new.year,
                    &new.time,
                    &new.kind,
                )
                || (o.playing, o.liked, o.selected, o.has_thumb)
                    != (new.playing, new.liked, new.selected, new.has_thumb)
        };
    let (mut head, mut changed) = (0, false);
    while head < old.len().min(new_keys.len()) && old[head] == new_keys[head] {
        if differs(model, head, &mut new[head], true) {
            model.set_row_data(head, new[head].clone());
            changed = true;
        }
        head += 1;
    }
    let mut tail = 0;
    while tail < old.len().min(new_keys.len()) - head
        && old[old.len() - 1 - tail] == new_keys[new_keys.len() - 1 - tail]
    {
        tail += 1;
    }
    for t in 0..tail {
        let (o, n) = (old.len() - 1 - t, new_keys.len() - 1 - t);
        if differs(model, o, &mut new[n], true) {
            model.set_row_data(o, new[n].clone());
            changed = true;
        }
    }
    let (old_mid, new_mid) = (head..old.len() - tail, head..new_keys.len() - tail);
    let common = old_mid.len().min(new_mid.len());
    for k in 0..common {
        if differs(model, head + k, &mut new[head + k], false) {
            model.set_row_data(head + k, new[head + k].clone());
            changed = true;
        }
    }
    for _ in common..old_mid.len() {
        model.remove(head + common);
        changed = true;
    }
    for k in common..new_mid.len() {
        model.insert(head + k, new[head + k].clone());
        changed = true;
    }
    changed
}

/// Rows for items `start..` were appended to the current list: add them to the model in one
/// batch (one notification) instead of rebuilding the whole table.
pub fn append_rows(ui: &AppWindow, start: usize) {
    let (rows, summary) = STATE.with(|s| {
        let s = s.borrow();
        let items = current_items(&s);
        if s.rows_model.row_count() != start || start > items.len() {
            return (None, String::new()); // out of step: the caller falls back to `refresh`
        }
        let rows: Vec<TrackData> = items[start..]
            .iter()
            .enumerate()
            .map(|(k, i)| {
                let n = start + k;
                let (playing, liked) = marks(&s, n, i);
                let mut r = row(i, playing);
                r.selected = s.sel.binary_search(&n).is_ok();
                r.liked = liked;
                r
            })
            .collect();
        let total: u32 = items.iter().filter_map(|i| i.duration_secs).sum();
        (
            Some(rows),
            format!("{} tracks · {}", items.len(), fmt_time(total as f64)),
        )
    });
    match rows {
        Some(rows) => {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                let keys: Vec<String> = current_items(&s)
                    .iter()
                    .enumerate()
                    .skip(start)
                    .map(|(n, i)| gesture::key(i, n))
                    .collect();
                s.row_keys.extend(keys);
            });
            STATE.with(|s| s.borrow().rows_model.extend(rows));
            ui.set_status_summary(summary.into());
        }
        None => refresh(ui),
    }
}

/// Update selection / playing / liked marks in place (safe during a mouse gesture).
pub fn refresh_marks(_ui: &AppWindow) {
    STATE.with(|s| {
        let s = s.borrow();
        let items = current_items(&s);
        let model = &s.rows_model;
        if model.row_count() != items.len() {
            return; // content is about to be replaced by `refresh`
        }
        for (n, it) in items.iter().enumerate() {
            let Some(mut r) = model.row_data(n) else {
                continue;
            };
            let (playing, liked) = marks(&s, n, it);
            let selected = s.sel.binary_search(&n).is_ok();
            if (r.playing, r.liked, r.selected) != (playing, liked, selected) {
                (r.playing, r.liked, r.selected) = (playing, liked, selected);
                model.set_row_data(n, r);
            }
        }
    });
}

pub fn wire(ui: &AppWindow, core: &crate::core::runtime::CoreHandle) {
    let weak = ui.as_weak();
    ui.global::<crate::Menus>().on_prepare_row(move |i| {
        if let Some(ui) = weak.upgrade() {
            prepare_row(&ui, i.max(0) as usize);
        }
    });
    install_models(ui);
    select::wire(ui, core);
    cols::wire(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(t: &str) -> TrackData {
        TrackData {
            title: t.into(),
            ..Default::default()
        }
    }
    fn keys(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    fn titles(m: &VecModel<TrackData>) -> Vec<String> {
        (0..m.row_count())
            .map(|n| m.row_data(n).unwrap().title.to_string())
            .collect()
    }

    #[test]
    fn identical_list_touches_nothing() {
        let m = VecModel::from(vec![row("a"), row("b")]);
        assert!(!diff_rows(
            &m,
            &keys(&["a", "b"]),
            &keys(&["a", "b"]),
            vec![row("a"), row("b")]
        ));
    }

    #[test]
    fn only_changed_rows_are_rewritten() {
        let m = VecModel::from(vec![row("a"), row("b"), row("c")]);
        assert!(diff_rows(
            &m,
            &keys(&["a", "b", "c"]),
            &keys(&["a", "b", "c"]),
            vec![row("a"), row("B"), row("c")]
        ));
        assert_eq!(titles(&m), ["a", "B", "c"]);
    }

    #[test]
    fn inserts_removes_and_reorders_end_up_equal() {
        let cases: [(&[&str], &[&str]); 6] = [
            (&["a", "b", "c"], &["x", "a", "b", "c"]),
            (&["a", "b", "c"], &["a", "c"]),
            (&["a", "b", "c"], &["a", "b", "c", "d", "e"]),
            (&["a", "b", "c", "d"], &["d", "c", "b", "a"]),
            (&["a", "b", "c"], &[]),
            (&[], &["a", "b"]),
        ];
        for (old, new) in cases {
            let m = VecModel::from(old.iter().map(|k| row(k)).collect::<Vec<_>>());
            diff_rows(
                &m,
                &keys(old),
                &keys(new),
                new.iter().map(|k| row(k)).collect(),
            );
            assert_eq!(
                titles(&m),
                new.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "{old:?} -> {new:?}"
            );
        }
    }
}
