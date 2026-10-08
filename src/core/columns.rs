//! Playlist table columns: order, widths, visibility, sort state and the pixel layout.
//! Pure logic (persisted in config.toml); the GUI only draws what `layout` returns.
use super::playlist::SortKey;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Col {
    Num,
    Title,
    Artist,
    Album,
    Year,
    Length,
    Like,
}

pub const ALL: [Col; 7] = [
    Col::Num,
    Col::Title,
    Col::Artist,
    Col::Album,
    Col::Year,
    Col::Length,
    Col::Like,
];

impl Col {
    pub fn id(self) -> &'static str {
        match self {
            Col::Num => "num",
            Col::Title => "title",
            Col::Artist => "artist",
            Col::Album => "album",
            Col::Year => "year",
            Col::Length => "length",
            Col::Like => "like",
        }
    }
    pub fn from_id(s: &str) -> Option<Col> {
        ALL.into_iter().find(|c| c.id() == s)
    }
    pub fn label(self) -> &'static str {
        match self {
            Col::Num => "#",
            Col::Title => "Title",
            Col::Artist => "Artist",
            Col::Album => "Album",
            Col::Year => "Year",
            Col::Length => "Length",
            Col::Like => "♥",
        }
    }
    pub fn right_aligned(self) -> bool {
        matches!(self, Col::Num | Col::Length)
    }
    pub fn min_width(self) -> u32 {
        let base = self.base_min_width();
        match self.fixed_slot() {
            Some(i) => base.max(MEASURED[i].load(Ordering::Relaxed)),
            None => base,
        }
    }
    fn fixed_slot(self) -> Option<usize> {
        match self {
            Col::Num => Some(0),
            Col::Year => Some(1),
            Col::Length => Some(2),
            Col::Like => Some(3),
            _ => None,
        }
    }
    fn base_min_width(self) -> u32 {
        match self {
            Col::Num => 30,
            Col::Title => 120,
            Col::Artist | Col::Album => 80,
            Col::Length => 52,
            Col::Like => 24,
            Col::Year => 44,
        }
    }
    pub fn default_width(self) -> u32 {
        match self {
            Col::Num => 36,
            // flexible columns: proportions (40 : 22 : 30), see `ColumnConfig::layout`
            Col::Title => 400,
            Col::Artist => 220,
            Col::Album => 300,
            Col::Year => 52,
            Col::Length => 62,
            Col::Like => 28,
        }
    }
    pub fn sort_key(self) -> Option<SortKey> {
        match self {
            Col::Title => Some(SortKey::Title),
            Col::Artist => Some(SortKey::Artist),
            Col::Album => Some(SortKey::Album),
            Col::Length => Some(SortKey::Length),
            Col::Num | Col::Year | Col::Like => None, // no year in the data model yet; # is the row order
        }
    }
    pub fn can_hide(self) -> bool {
        self != Col::Title
    }
}

/// Widths the fixed columns (#, Year, Length, ♥) need for the current font, measured by the UI
/// (widest real content and header with its sort arrow). 0 = not measured yet.
static MEASURED: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

/// Record the measured widths (px) of `#`, Year, Length and ♥. Returns true if they changed.
pub fn set_measured(num: u32, year: u32, length: u32, like: u32) -> bool {
    let new = [num, year, length, like];
    let mut changed = false;
    for (slot, v) in MEASURED.iter().zip(new) {
        changed |= slot.swap(v, Ordering::Relaxed) != v;
    }
    changed
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ColWidth {
    pub col: Col,
    pub w: u32,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnConfig {
    pub order: Vec<Col>,
    pub hidden: Vec<Col>,
    pub widths: Vec<ColWidth>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            order: ALL.to_vec(),
            hidden: Vec::new(),
            widths: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placed {
    pub col: Col,
    pub x: f32,
    pub w: f32,
}

/// Width of the leading ▶ state column.
pub const STATE_W: f32 = 26.0;
/// Room kept free on the right for the scrollbar.
pub const SCROLL_W: f32 = 14.0;

impl Col {
    /// Title, Artist and Album share the free width in proportion; the rest keep their width.
    pub fn flexible(self) -> bool {
        matches!(self, Col::Title | Col::Artist | Col::Album)
    }
}

/// Columns dropped automatically (first to last) when the table is too narrow for the minimum
/// widths of the visible ones. The saved visibility is not touched.
pub const HIDE_ORDER: [Col; 6] = [
    Col::Year,
    Col::Album,
    Col::Artist,
    Col::Length,
    Col::Num,
    Col::Like,
];

impl ColumnConfig {
    /// Fix up a config loaded from disk: unknown/duplicate entries dropped, missing appended.
    pub fn sanitized(mut self) -> Self {
        let mut seen = Vec::new();
        self.order.retain(|c| {
            let fresh = !seen.contains(c);
            seen.push(*c);
            fresh
        });
        for c in ALL {
            if !self.order.contains(&c) {
                self.order.push(c);
            }
        }
        self.hidden.retain(|c| c.can_hide());
        self
    }

    pub fn visible(&self) -> Vec<Col> {
        self.order
            .iter()
            .copied()
            .filter(|c| !self.hidden.contains(c))
            .collect()
    }

    pub fn width(&self, c: Col) -> u32 {
        self.widths
            .iter()
            .find(|w| w.col == c)
            .map_or(c.default_width(), |w| w.w)
            .max(c.min_width())
    }

    pub fn set_width(&mut self, c: Col, w: u32) {
        let w = w.max(c.min_width());
        match self.widths.iter_mut().find(|e| e.col == c) {
            Some(e) => e.w = w,
            None => self.widths.push(ColWidth { col: c, w }),
        }
    }

    pub fn toggle(&mut self, c: Col) {
        if !c.can_hide() {
            return;
        }
        if let Some(i) = self.hidden.iter().position(|h| *h == c) {
            self.hidden.remove(i);
        } else {
            self.hidden.push(c);
        }
    }

    /// Move the visible column at `from` to visible position `to`.
    pub fn move_visible(&mut self, from: usize, to: usize) {
        let vis = self.visible();
        let (Some(&c), Some(&target)) = (vis.get(from), vis.get(to)) else {
            return;
        };
        if c == target {
            return;
        }
        self.order.retain(|x| *x != c);
        let at = self
            .order
            .iter()
            .position(|x| *x == target)
            .unwrap_or(self.order.len());
        // moving right lands after the target, moving left before it
        self.order.insert(if to > from { at + 1 } else { at }, c);
    }
}

#[path = "columns_layout.rs"]
mod layout;

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
