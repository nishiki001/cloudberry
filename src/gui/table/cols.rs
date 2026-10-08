//! Table columns: layout from `core/columns.rs`, header callbacks (sort, resize, reorder, toggle).
use super::super::state::STATE;
use crate::core::columns::{ALL, Col, Placed};
use crate::{AppWindow, ColumnData};
use slint::{ComponentHandle, Model, SharedString};

fn col_data(p: &Placed, sort: Option<(Col, bool)>) -> ColumnData {
    ColumnData {
        id: p.col.id().into(),
        title: if p.col == Col::Like {
            SharedString::from("")
        } else {
            p.col.label().into()
        },
        x: p.x,
        w: p.w,
        right: p.col.right_aligned(),
        sort: match sort {
            Some((c, true)) if c == p.col => 1,
            Some((c, false)) if c == p.col => 2,
            _ => 0,
        },
        shown: true,
    }
}

/// Recompute the column geometry. Same column set → rows are updated in place (a resize drag
/// must not recreate the header cell under the cursor); otherwise the model is replaced.
pub fn refresh_columns(ui: &AppWindow) {
    STATE.with(|s| {
        let s = s.borrow();
        let new: Vec<ColumnData> = s
            .cfg
            .table
            .layout(s.table_w.max(300.0))
            .iter()
            .map(|p| col_data(p, s.sort))
            .collect();
        let model = &s.cols_model;
        let same_set = model.row_count() == new.len()
            && new
                .iter()
                .enumerate()
                .all(|(i, c)| model.row_data(i).is_some_and(|m| m.id == c.id));
        if same_set {
            for (i, c) in new.into_iter().enumerate() {
                if model
                    .row_data(i)
                    .is_some_and(|m| (m.x, m.w, m.sort) != (c.x, c.w, c.sort))
                {
                    model.set_row_data(i, c);
                }
            }
        } else {
            model.set_vec(new);
        }
        // header menu: every column with its visibility
        let all: Vec<ColumnData> = ALL
            .iter()
            .map(|&c| ColumnData {
                id: c.id().into(),
                title: if c == Col::Like {
                    "Liked".into()
                } else {
                    c.label().into()
                },
                x: 0.0,
                w: 0.0,
                right: false,
                sort: 0,
                shown: !s.cfg.table.hidden.contains(&c),
            })
            .collect();
        if s.all_cols_model.row_count() == all.len() {
            for (i, c) in all.into_iter().enumerate() {
                if s.all_cols_model
                    .row_data(i)
                    .is_some_and(|m| m.shown != c.shown)
                {
                    s.all_cols_model.set_row_data(i, c);
                }
            }
        } else {
            s.all_cols_model.set_vec(all);
        }
    });
    // header menu entries (ticked when shown)
    let entries: Vec<crate::MenuItemData> = ALL
        .iter()
        .map(|&c| {
            let shown = STATE.with(|s| !s.borrow().cfg.table.hidden.contains(&c));
            crate::MenuItemData {
                label: format!(
                    "{}{}",
                    if shown { "✓ " } else { "    " },
                    if c == Col::Like { "Liked" } else { c.label() }
                )
                .into(),
                id: c.id().into(),
                shortcut: Default::default(),
                enabled: c.can_hide(),
                separator: false,
                sub: Default::default(),
            }
        })
        .collect();
    ui.global::<crate::Menus>()
        .set_column_menu(slint::ModelRc::new(slint::VecModel::from(entries)));
}

fn save_config() {
    STATE.with(|s| {
        let _ = s.borrow().cfg.save();
    });
}

pub fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_table_column_resized(move |id, delta| {
        if let (Some(ui), Some(col)) = (weak.upgrade(), Col::from_id(&id)) {
            // a drag only edits the widths in memory, from the widths at the start of the drag
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                let total = s.table_w.max(300.0);
                let base = match &s.resize_base {
                    Some(b) => b.clone(),
                    None => s.cfg.table.clone(), // no start event seen: fall back to the current widths
                };
                s.cfg.table.resize_from(&base, col, delta, total);
            });
            refresh_columns(&ui);
        }
    });
    ui.on_table_column_resize_start(|| {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.resize_base = Some(s.cfg.table.clone());
        });
    });
    ui.on_table_column_resize_done(|| {
        // persisted once, when the mouse is released
        STATE.with(|s| s.borrow_mut().resize_base = None);
        save_config();
    });
    let weak = ui.as_weak();
    ui.on_table_column_dropped(move |from, x| {
        let Some(ui) = weak.upgrade() else { return };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let layout = s.cfg.table.layout(s.table_w.max(300.0));
            let to = layout
                .iter()
                .position(|p| x >= p.x && x < p.x + p.w)
                .unwrap_or(layout.len().saturating_sub(1));
            s.cfg.table.move_visible(from.max(0) as usize, to);
        });
        refresh_columns(&ui);
        save_config();
    });
    let weak = ui.as_weak();
    ui.on_table_column_toggled(move |id| {
        if let (Some(ui), Some(col)) = (weak.upgrade(), Col::from_id(&id)) {
            STATE.with(|s| s.borrow_mut().cfg.table.toggle(col));
            refresh_columns(&ui);
            save_config();
        }
    });
    wire_width(ui);
}

/// The table reports its real width; the columns are laid out for it (also used by the
/// screenshot fixtures, so those show the layout the real window would have).
pub fn wire_width(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_table_fixed_metrics(move |n, y, l, k| {
        // at the default size the designed widths stand; larger fonts need the measured ones
        let big = weak
            .upgrade()
            .is_some_and(|ui| ui.global::<crate::Theme>().get_font_scale() > 1.0);
        let m = |v: f32| if big { v.ceil() as u32 } else { 0 };
        if crate::core::columns::set_measured(m(n), m(y), m(l), m(k))
            && let Some(ui) = weak.upgrade()
        {
            refresh_columns(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_table_width(move |w| {
        STATE.with(|s| s.borrow_mut().table_w = w);
        if let Some(ui) = weak.upgrade() {
            refresh_columns(&ui);
        }
    });
}
