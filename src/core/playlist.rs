//! Playlist table model operations: stable sorting and multi-selection moves, as pure
//! permutations so the queue, opened playlists and tests all share one implementation.
use super::model::Item;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Number/Year: kept for the model; the table does not offer them yet
pub enum SortKey {
    Number,
    Title,
    Artist,
    Album,
    Year,
    Length,
}

fn artist_key(i: &Item) -> String {
    i.artists
        .iter()
        .map(|a| a.name.to_lowercase())
        .collect::<Vec<_>>()
        .join(", ")
}

fn cmp(a: &Item, b: &Item, key: SortKey) -> Ordering {
    match key {
        SortKey::Number => Ordering::Equal,
        SortKey::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        SortKey::Artist => artist_key(a).cmp(&artist_key(b)),
        SortKey::Album => a
            .album
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b.album.as_deref().unwrap_or("").to_lowercase()),
        SortKey::Year => Ordering::Equal, // no year in the data model yet
        SortKey::Length => a
            .duration_secs
            .unwrap_or(0)
            .cmp(&b.duration_secs.unwrap_or(0)),
    }
}

/// New order (indices into `items`) sorted by `key`. Stable: equal items keep their order, in
/// both directions (descending reverses the comparison, not the result).
pub fn sort_indices(items: &[Item], key: SortKey, ascending: bool) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..items.len()).collect();
    if key == SortKey::Number {
        if !ascending {
            idx.reverse();
        }
        return idx;
    }
    idx.sort_by(|&a, &b| {
        let o = cmp(&items[a], &items[b], key);
        if ascending { o } else { o.reverse() }
    });
    idx
}

/// Permutation that moves the rows in `selected` (any order, duplicates ignored) so that they
/// end up contiguous *before the row that is currently at `to`* (`to == len` = to the end),
/// keeping their relative order. The result lists old indices in the new order.
pub fn move_selection(len: usize, selected: &[usize], to: usize) -> Vec<usize> {
    let mut sel: Vec<usize> = selected.iter().copied().filter(|&i| i < len).collect();
    sel.sort_unstable();
    sel.dedup();
    let to = to.min(len);
    let mut before: Vec<usize> = Vec::new();
    let mut after: Vec<usize> = Vec::new();
    for i in 0..len {
        if sel.binary_search(&i).is_ok() {
            continue;
        }
        if i < to {
            before.push(i)
        } else {
            after.push(i)
        }
    }
    before.into_iter().chain(sel).chain(after).collect()
}

/// `items` rearranged so that new position `n` holds old item `order[n]`.
pub fn apply_order<T: Clone>(items: &[T], order: &[usize]) -> Vec<T> {
    order
        .iter()
        .filter_map(|&i| items.get(i).cloned())
        .collect()
}

/// Remove the given indices (any order, duplicates ignored).
pub fn remove_indices<T: Clone>(items: &[T], remove: &[usize]) -> Vec<T> {
    items
        .iter()
        .enumerate()
        .filter(|(i, _)| !remove.contains(i))
        .map(|(_, t)| t.clone())
        .collect()
}

#[cfg(test)]
#[path = "playlist_tests.rs"]
mod tests;
