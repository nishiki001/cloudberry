//! The network copy of a playlist that is already shown in a tab (stale-while-revalidate): applied
//! in place when the user is idle, otherwise kept and retried every half second.
use super::state::*;
use super::{table, tabs};
use crate::AppWindow;
use crate::core::model::Item;
use slint::ComponentHandle;

thread_local! {
    static WAIT: slint::Timer = slint::Timer::default();
}

/// A fresh copy of a playlist shown in a tab: apply it now when the user is idle, otherwise keep
/// it and try again every half second.
pub(super) fn offer(ui: &AppWindow, uid: u64, songs: Vec<Item>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let base = s
            .tabs
            .iter()
            .find(|t| t.uid == uid)
            .map(|t| t.items.clone())
            .unwrap_or_default();
        s.deferred_refresh.retain(|d| d.0 != uid);
        s.deferred_refresh.push((uid, base, songs));
    });
    if !try_apply(ui) {
        tracing::info!("playlist refresh waits for the user to be idle");
        let weak = ui.as_weak();
        WAIT.with(|t| {
            t.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(500),
                move || {
                    if let Some(ui) = weak.upgrade()
                        && try_apply(&ui)
                    {
                        WAIT.with(|t| t.stop());
                    }
                },
            );
        });
    }
}

/// Apply the waiting refresh if the user is idle. True = nothing is waiting any more.
fn try_apply(ui: &AppWindow) -> bool {
    let menu_open = ui.global::<crate::Menus>().get_table_menu_open();
    if !table::gesture::quiet(menu_open) {
        return false;
    }
    let waiting = STATE.with(|s| std::mem::take(&mut s.borrow_mut().deferred_refresh));
    for (uid, base, songs) in waiting {
        apply_one(ui, uid, base, songs);
    }
    true
}

fn apply_one(ui: &AppWindow, uid: u64, base: Vec<Item>, songs: Vec<Item>) {
    let (changed, cur) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(t) = s.tabs.iter().position(|t| t.uid == uid) else {
            return (false, false); // the tab was closed meanwhile
        };
        if s.tabs[t].items == songs {
            return (false, false); // identical: nothing to do
        }
        if s.tabs[t].items != base {
            // removed / moved / sorted / added since the copy was fetched: the edit wins
            tracing::info!("playlist refresh dropped: the list was edited meanwhile");
            return (false, false);
        }
        let cur = t + 1 == s.cur_tab;
        // keep the selection on the same tracks
        let keys: Vec<String> = s
            .sel
            .iter()
            .filter_map(|&n| s.tabs[t].items.get(n).map(|i| table::gesture::key(i, n)))
            .collect();
        s.tabs[t].items = songs;
        if cur {
            let new_sel: Vec<usize> = s.tabs[t]
                .items
                .iter()
                .enumerate()
                .filter(|(n, i)| keys.contains(&table::gesture::key(i, *n)))
                .map(|(n, _)| n)
                .collect();
            s.anchor = new_sel.first().copied().unwrap_or(0);
            s.sel = new_sel;
        }
        (true, cur)
    });
    tracing::info!(changed, "playlist refresh applied");
    if changed {
        if cur {
            table::refresh(ui);
        }
        tabs::save();
    }
}
