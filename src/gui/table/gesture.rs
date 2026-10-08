//! Pointer gestures on table rows: which track a press / right-click / double-click is about, and
//! whether the user is busy (menu open, recent interaction) so that a background refresh of the
//! list has to wait. Rows move when a list is refreshed; a track does not.
use super::super::state::STATE;
use super::current_items;
use crate::core::model::Item;
use std::time::{Duration, Instant};

/// Stable key of a track: its video id (position-based when it has none).
pub fn key(item: &Item, n: usize) -> String {
    item.video_id
        .clone()
        .unwrap_or_else(|| format!("#{n}:{}", item.title))
}

/// A gesture is only trusted for this long (a stale press must not redirect a later action; longer than a menu may hold a refresh back).
const GESTURE_TTL: Duration = Duration::from_secs(600);

/// The user touched the table now.
pub fn touch() {
    STATE.with(|s| s.borrow_mut().touched = Some(Instant::now()));
}

/// A press / right-click started on row `idx`: remember which track that is.
pub fn press(idx: usize) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let k = current_items(&s).get(idx).map(|i| key(i, idx));
        s.gesture = k.map(|k| (idx, k, Instant::now()));
        s.touched = Some(Instant::now());
    });
}

/// The row an action on row `idx` is really about: the row of the track the gesture started on,
/// wherever it is now. `None` = that track is gone (do nothing rather than act on another one).
pub fn resolve(idx: usize) -> Option<usize> {
    STATE.with(|s| {
        let s = s.borrow();
        let items = current_items(&s);
        let Some((gi, k, at)) = &s.gesture else {
            return (idx < items.len()).then_some(idx);
        };
        if *gi != idx || at.elapsed() > GESTURE_TTL {
            return (idx < items.len()).then_some(idx);
        }
        // the same row when it still holds the track, else the nearest row that does
        let at_row = |n: usize| {
            items
                .get(n)
                .is_some_and(|i| key(i, n) == *k || i.video_id.as_deref() == Some(k.as_str()))
        };
        if at_row(idx) {
            return Some(idx);
        }
        (0..items.len())
            .filter(|&n| at_row(n))
            .min_by_key(|&n| n.abs_diff(idx))
    })
}

/// True when a refresh may be applied now: no table menu is open and nothing was touched for a
/// few seconds.
pub fn quiet(menu_open: bool) -> bool {
    STATE.with(|s| {
        let s = s.borrow();
        let wait = Duration::from_millis(if s.quiet_ms == 0 { 3000 } else { s.quiet_ms });
        let idle = s.touched.is_none_or(|t| t.elapsed() >= wait);
        // a menu that was left open for a minute is not holding anything back any more
        let menu = menu_open
            && s.touched
                .is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        idle && !menu
    })
}
