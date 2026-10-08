//! Table interaction: selection, sorting, multi-move, delete, activation, panel drops.
use super::super::state::STATE;
use super::{current_items, refresh, refresh_columns, refresh_marks};
use crate::AppWindow;
use crate::core::columns::Col;
use crate::core::model::Item;
use crate::core::msg::Command;
use crate::core::playlist::{SortKey, apply_order, move_selection, remove_indices, sort_indices};
use crate::core::runtime::CoreHandle;
use slint::ComponentHandle;

pub fn selected_items() -> Vec<Item> {
    STATE.with(|s| {
        let s = s.borrow();
        let items = current_items(&s);
        s.sel
            .iter()
            .filter_map(|&i| items.get(i).cloned())
            .collect()
    })
}

pub fn set_selection(sel: Vec<usize>, anchor: usize) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.sel = sel;
        s.anchor = anchor;
    });
}

/// Apply a new row order (permutation of old indices) to the current tab.
fn apply_permutation(core: &CoreHandle, order: Vec<usize>) {
    let queue = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let q = s.cur_tab == 0;
        if !q {
            let t = s.cur_tab - 1;
            if let Some(tab) = s.tabs.get_mut(t) {
                tab.items = apply_order(&tab.items, &order);
            }
        }
        q
    });
    if queue {
        core.send(Command::QueueReorder(order));
    } else {
        super::super::tabs::save();
    }
}

/// Positions of the previously selected rows after `order` was applied.
fn remap_selection(sel: &[usize], order: &[usize]) -> Vec<usize> {
    let mut v: Vec<usize> = order
        .iter()
        .enumerate()
        .filter(|(_, old)| sel.contains(old))
        .map(|(n, _)| n)
        .collect();
    v.sort_unstable();
    v
}

fn after_reorder(ui: &AppWindow) {
    refresh_columns(ui);
    // the queue's new order arrives as an Event::Queue; local tabs change right here
    if STATE.with(|s| s.borrow().cur_tab) != 0 {
        refresh(ui);
    }
}

pub fn sort_by(ui: &AppWindow, core: &CoreHandle, col: Col) {
    let Some(key) = col.sort_key() else { return };
    let (asc, items, sel) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let asc = !matches!(s.sort, Some((c, true)) if c == col);
        s.sort = Some((col, asc));
        (asc, current_items(&s), s.sel.clone())
    });
    let order = sort_indices(&items, key, asc);
    let new_sel = remap_selection(&sel, &order);
    apply_permutation(core, order);
    set_selection(new_sel, 0);
    after_reorder(ui);
}

pub fn move_rows(ui: &AppWindow, core: &CoreHandle, to: usize) {
    let (len, sel) = STATE.with(|s| {
        let s = s.borrow();
        (current_items(&s).len(), s.sel.clone())
    });
    if sel.is_empty() {
        return;
    }
    let order = move_selection(len, &sel, to);
    let new_sel = remap_selection(&sel, &order);
    apply_permutation(core, order);
    STATE.with(|s| s.borrow_mut().sort = None); // a manual order replaces any sort
    set_selection(new_sel, 0);
    after_reorder(ui);
}

pub fn delete_selection(ui: &AppWindow, core: &CoreHandle) {
    let (sel, queue) = STATE.with(|s| {
        let s = s.borrow();
        (s.sel.clone(), s.cur_tab == 0)
    });
    if sel.is_empty() {
        return;
    }
    if queue {
        core.send(Command::QueueRemoveMany(sel));
    } else {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let t = s.cur_tab - 1;
            if let Some(tab) = s.tabs.get_mut(t) {
                tab.items = remove_indices(&tab.items, &sel);
            }
        });
        super::super::tabs::save();
    }
    set_selection(Vec::new(), 0);
    if !queue {
        refresh(ui);
    }
}

pub fn activate(core: &CoreHandle, i: usize) {
    let (queue, items) = STATE.with(|s| {
        let s = s.borrow();
        (s.cur_tab == 0, current_items(&s))
    });
    if queue {
        core.send(Command::QueueJump(i));
    } else if i < items.len() {
        core.send(Command::PlayItems { items, index: i });
    }
}

/// Insert dragged-in tracks at `pos` of the current tab.
pub fn insert_items(ui: &AppWindow, core: &CoreHandle, items: Vec<Item>, pos: usize) {
    let items: Vec<Item> = items.into_iter().filter(|i| i.video_id.is_some()).collect();
    if items.is_empty() {
        return;
    }
    let on_page = STATE.with(|s| {
        let s = s.borrow();
        s.tabs
            .get(s.cur_tab.wrapping_sub(1))
            .is_some_and(|t| super::super::tabs::is_page_kind(&t.kind))
    });
    if on_page {
        return; // an artist / album page has no list to drop into
    }
    let queue = STATE.with(|s| s.borrow().cur_tab == 0);
    if queue {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let at = pos.min(s.queue_items.len());
            let n = items.len();
            s.sel.iter_mut().filter(|i| **i >= at).for_each(|i| *i += n);
            if s.anchor >= at {
                s.anchor += n;
            }
        });
        core.send(Command::QueueInsert { items, at: pos });
    } else {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let t = s.cur_tab - 1;
            if let Some(tab) = s.tabs.get_mut(t) {
                let at = pos.min(tab.items.len());
                let n = items.len();
                tab.items.splice(at..at, items);
                s.sel.iter_mut().filter(|i| **i >= at).for_each(|i| *i += n);
                s.anchor = if s.anchor >= at {
                    s.anchor + n
                } else {
                    s.anchor
                };
            }
        });
        super::super::tabs::save();
        refresh(ui);
    }
}

pub fn wire(ui: &AppWindow, core: &CoreHandle) {
    let weak = ui.as_weak();
    ui.on_table_row_clicked(move |i, ctrl, shift| {
        let i = i as usize;
        super::gesture::press(i);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.click_mod = ctrl || shift;
            if shift {
                let (a, b) = (s.anchor.min(i), s.anchor.max(i));
                s.sel = (a..=b).collect();
            } else if ctrl {
                match s.sel.binary_search(&i) {
                    Ok(p) => {
                        s.sel.remove(p);
                    }
                    Err(p) => s.sel.insert(p, i),
                }
                s.anchor = i;
            } else if !s.sel.contains(&i) {
                // a plain press on a selected row keeps the selection: it may start a drag
                s.sel = vec![i];
                s.anchor = i;
            }
        });
        if let Some(ui) = weak.upgrade() {
            refresh_marks(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_table_row_plain_click(move |i| {
        // released without dragging: a plain click on one row of a multi-selection collapses it
        let collapse = STATE.with(|s| !s.borrow().click_mod && s.borrow().sel.len() > 1);
        if collapse {
            set_selection(vec![i as usize], i as usize);
            if let Some(ui) = weak.upgrade() {
                refresh_marks(&ui);
            }
        }
    });
    let c = core.clone();
    ui.on_table_row_activated(move |i| {
        // the row may have moved since the first click of the double-click: use the track
        super::gesture::touch();
        if let Some(i) = super::gesture::resolve(i as usize) {
            activate(&c, i);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.on_table_rows_dropped(move |pos| {
        if let Some(ui) = weak.upgrade() {
            move_rows(&ui, &c, pos.max(0) as usize);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.on_table_header_clicked(move |id| {
        if let (Some(ui), Some(col)) = (weak.upgrade(), Col::from_id(&id)) {
            sort_by(&ui, &c, col);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.on_table_delete(move || {
        if let Some(ui) = weak.upgrade() {
            delete_selection(&ui, &c);
        }
    });
    let weak = ui.as_weak();
    ui.on_table_select_all(move || {
        let n = STATE.with(|s| current_items(&s.borrow()).len());
        set_selection((0..n).collect(), 0);
        if let Some(ui) = weak.upgrade() {
            refresh_marks(&ui);
        }
    });
    let c = core.clone();
    ui.on_table_activate_selected(move || {
        if let Some(i) = STATE.with(|s| s.borrow().sel.first().copied()) {
            activate(&c, i);
        }
    });
    let weak = ui.as_weak();
    ui.on_table_nav(move |d| {
        let n = STATE.with(|s| current_items(&s.borrow()).len());
        if n == 0 {
            return;
        }
        let next = match STATE.with(|s| s.borrow().sel.first().copied()) {
            Some(c) => (c as i32 + d).clamp(0, n as i32 - 1) as usize,
            None => 0,
        };
        set_selection(vec![next], next);
        if let Some(ui) = weak.upgrade() {
            refresh_marks(&ui);
            ui.invoke_table_ensure_visible(next as i32);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    ui.on_table_shift(move |d| {
        let sel = STATE.with(|s| s.borrow().sel.clone());
        let (Some(&lo), Some(&hi), Some(ui)) = (sel.first(), sel.last(), weak.upgrade()) else {
            return;
        };
        move_rows(&ui, &c, if d < 0 { lo.saturating_sub(1) } else { hi + 2 });
    });
}

#[allow(dead_code)]
const _: Option<SortKey> = None; // SortKey is part of the module's vocabulary
