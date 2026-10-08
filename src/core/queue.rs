#![allow(dead_code)] // peek_next/remove/move_item are used by M6 (prefetch, queue tab)
//! Play queue: add, play next, remove, reorder, shuffle, repeat. Pure logic, no I/O.
use super::model::Item;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Off,
    All,
    One,
}

impl Repeat {
    pub fn cycle(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::All => "all",
            Repeat::One => "one",
        }
    }
}

#[derive(Debug)]
pub struct Queue {
    items: Vec<Item>,
    /// Play order as indices into `items` (identity unless shuffled).
    order: Vec<usize>,
    /// Position in `order` of the current track.
    pos: Option<usize>,
    shuffle: bool,
    repeat: Repeat,
    rng: u64,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new(0x9E37_79B9_7F4A_7C15)
    }
}

impl Queue {
    pub fn new(seed: u64) -> Self {
        Self {
            items: vec![],
            order: vec![],
            pos: None,
            shuffle: false,
            repeat: Repeat::Off,
            rng: seed | 1,
        }
    }

    fn rand(&mut self) -> u64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Rebuild `order` keeping the current track first when shuffled.
    fn rebuild(&mut self) {
        let cur = self.current_index();
        let mut order: Vec<usize> = (0..self.items.len()).collect();
        if self.shuffle {
            order.retain(|&i| Some(i) != cur);
            for i in (1..order.len()).rev() {
                let j = (self.rand() % (i as u64 + 1)) as usize;
                order.swap(i, j);
            }
            if let Some(c) = cur {
                order.insert(0, c);
            }
        }
        self.order = order;
        self.pos = cur.and_then(|c| self.order.iter().position(|&i| i == c));
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }
    pub fn set_repeat(&mut self, r: Repeat) {
        self.repeat = r;
    }

    /// Index into `items()` of the current track.
    pub fn current_index(&self) -> Option<usize> {
        self.pos.and_then(|p| self.order.get(p)).copied()
    }
    pub fn current(&self) -> Option<&Item> {
        self.current_index().and_then(|i| self.items.get(i))
    }

    /// Replace the queue and start at `start` (index into `items`).
    pub fn set(&mut self, items: Vec<Item>, start: usize) {
        self.items = items;
        self.order = (0..self.items.len()).collect();
        self.pos = (start < self.items.len()).then_some(start);
        self.rebuild();
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
        self.rebuild();
    }

    pub fn add(&mut self, item: Item) {
        self.items.push(item);
        // end of the play order: already-played tracks are not reshuffled
        self.order.push(self.items.len() - 1);
    }

    /// Insert right after the current track (plays next).
    pub fn play_next(&mut self, item: Item) {
        let at = self.current_index().map_or(self.items.len(), |c| c + 1);
        self.items.insert(at, item);
        // keep the inserted track directly after the current one even when shuffled
        let cur = self.current_index().filter(|&c| c < at);
        self.rebuild_keeping(cur);
        if self.shuffle
            && let (Some(p), Some(_)) = (self.pos, cur)
            && let Some(q) = self.order.iter().position(|&i| i == at)
        {
            let v = self.order.remove(q);
            self.order.insert(p + 1, v);
        }
    }

    fn rebuild_keeping(&mut self, cur: Option<usize>) {
        // `cur` is the (already adjusted) items index of the current track
        self.order = (0..self.items.len()).collect();
        self.pos = cur;
        self.rebuild();
    }

    pub fn remove(&mut self, idx: usize) {
        if idx >= self.items.len() {
            return;
        }
        let cur = self.current_index();
        self.items.remove(idx);
        let new_cur = match cur {
            Some(c) if c == idx => None,
            Some(c) if c > idx => Some(c - 1),
            other => other,
        };
        self.order = (0..self.items.len()).collect();
        self.pos = new_cur;
        self.rebuild();
    }

    /// Move an item (reorder). The current track keeps playing.
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return;
        }
        let cur = self.current_index();
        let it = self.items.remove(from);
        self.items.insert(to, it);
        let new_cur = cur.map(|c| {
            if c == from {
                to
            } else if from < c && to >= c {
                c - 1
            } else if from > c && to <= c {
                c + 1
            } else {
                c
            }
        });
        self.order = (0..self.items.len()).collect();
        self.pos = new_cur;
        self.rebuild();
    }

    /// Rearrange the queue: new position `n` holds old item `order[n]` (a permutation). The
    /// current track keeps playing wherever it ends up.
    pub fn reorder(&mut self, order: &[usize]) {
        let mut check: Vec<usize> = order.to_vec();
        check.sort_unstable();
        if check.len() != self.items.len() || check.iter().enumerate().any(|(i, v)| i != *v) {
            return; // not a permutation of 0..len
        }
        let cur = self.current_index();
        self.items = order
            .iter()
            .filter_map(|&i| self.items.get(i).cloned())
            .collect();
        let new_cur = cur.and_then(|c| order.iter().position(|&i| i == c));
        self.order = (0..self.items.len()).collect();
        self.pos = new_cur;
        self.rebuild();
    }

    /// Insert items before position `at` (clamped); the current track keeps playing.
    pub fn insert_at(&mut self, at: usize, new: Vec<Item>) {
        let at = at.min(self.items.len());
        let cur = self.current_index();
        let n = new.len();
        for (k, it) in new.into_iter().enumerate() {
            self.items.insert(at + k, it);
        }
        let new_cur = cur.map(|c| if c >= at { c + n } else { c });
        self.order = (0..self.items.len()).collect();
        self.pos = new_cur;
        self.rebuild();
    }

    /// Remove several items at once; if the current track is among them nothing is current
    /// afterwards (the controller decides what plays next).
    pub fn remove_many(&mut self, remove: &[usize]) {
        let cur = self.current_index();
        let keep: Vec<usize> = (0..self.items.len())
            .filter(|i| !remove.contains(i))
            .collect();
        let new_cur = cur.and_then(|c| keep.iter().position(|&i| i == c));
        self.items = keep
            .iter()
            .filter_map(|&i| self.items.get(i).cloned())
            .collect();
        self.order = (0..self.items.len()).collect();
        self.pos = new_cur;
        self.rebuild();
    }

    /// Jump to a specific item index.
    pub fn jump(&mut self, idx: usize) -> Option<&Item> {
        if idx >= self.items.len() {
            return None;
        }
        self.pos = self.order.iter().position(|&i| i == idx);
        self.current()
    }

    fn next_pos(&self, auto: bool) -> Option<usize> {
        let p = self.pos?;
        if auto && self.repeat == Repeat::One {
            return Some(p);
        }
        if p + 1 < self.order.len() {
            Some(p + 1)
        } else if self.repeat == Repeat::All && !self.order.is_empty() {
            Some(0)
        } else {
            None
        }
    }

    /// Item index that would play after the current one (for prefetch); does not move.
    pub fn peek_next(&self, auto: bool) -> Option<usize> {
        self.next_pos(auto).and_then(|p| self.order.get(p)).copied()
    }

    /// Advance. `auto` = track ended by itself (honors repeat-one).
    pub fn next(&mut self, auto: bool) -> Option<&Item> {
        let p = self.next_pos(auto)?;
        self.pos = Some(p);
        self.current()
    }

    pub fn prev(&mut self) -> Option<&Item> {
        let p = self.pos?;
        let np = if p > 0 {
            p - 1
        } else if self.repeat == Repeat::All {
            self.order.len() - 1
        } else {
            return self.current();
        };
        self.pos = Some(np);
        self.current()
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
