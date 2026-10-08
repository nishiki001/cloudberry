use super::*;
use crate::core::model::{Artist, Kind};

fn it(title: &str, artist: &str, album: &str, secs: u32) -> Item {
    Item {
        kind: Kind::Song,
        title: title.into(),
        video_id: Some(title.into()),
        browse_id: None,
        artists: vec![Artist {
            name: artist.into(),
            id: None,
        }],
        album: Some(album.into()),
        album_id: None,
        duration: None,
        duration_secs: Some(secs),
        thumbnail: None,
        subtitle: String::new(),
    }
}

fn titles(items: &[Item], order: &[usize]) -> Vec<String> {
    order.iter().map(|&i| items[i].title.clone()).collect()
}

#[test]
fn sort_is_stable_and_case_insensitive() {
    let items = vec![
        it("b", "X", "z", 1),
        it("A", "Y", "z", 2),
        it("c", "X", "y", 3),
        it("a", "X", "y", 4),
    ];
    // equal titles "A"/"a" keep their original order
    assert_eq!(
        titles(&items, &sort_indices(&items, SortKey::Title, true)),
        ["A", "a", "b", "c"]
    );
    // same artist keeps the original order inside the group
    assert_eq!(
        titles(&items, &sort_indices(&items, SortKey::Artist, true)),
        ["b", "c", "a", "A"]
    );
    // descending is stable too: ties are NOT reversed
    assert_eq!(
        titles(&items, &sort_indices(&items, SortKey::Artist, false)),
        ["A", "b", "c", "a"]
    );
    assert_eq!(
        titles(&items, &sort_indices(&items, SortKey::Length, false)),
        ["a", "c", "A", "b"]
    );
}

#[test]
fn number_sort_is_original_or_reversed() {
    let items = vec![it("1", "", "", 0), it("2", "", "", 0), it("3", "", "", 0)];
    assert_eq!(sort_indices(&items, SortKey::Number, true), [0, 1, 2]);
    assert_eq!(sort_indices(&items, SortKey::Number, false), [2, 1, 0]);
}

#[test]
fn move_selection_keeps_relative_order() {
    // rows 1 and 3 dropped before row 5 of 6
    assert_eq!(move_selection(6, &[3, 1], 5), [0, 2, 4, 1, 3, 5]);
    // to the very end
    assert_eq!(move_selection(5, &[0, 1], 5), [2, 3, 4, 0, 1]);
    // to the top
    assert_eq!(move_selection(5, &[3, 4], 0), [3, 4, 0, 1, 2]);
    // dropping inside the selection's own span keeps everything in place
    assert_eq!(move_selection(5, &[1, 2], 2), [0, 1, 2, 3, 4]);
}

#[test]
fn move_selection_ignores_junk() {
    assert_eq!(move_selection(3, &[7, 1, 1], 0), [1, 0, 2]);
    assert_eq!(move_selection(0, &[0], 0), Vec::<usize>::new());
    assert_eq!(move_selection(3, &[], 1), [0, 1, 2]);
}

#[test]
fn every_move_is_a_permutation() {
    for len in 0..8usize {
        for to in 0..=len {
            for mask in 0..(1u32 << len) {
                let sel: Vec<usize> = (0..len).filter(|i| mask >> i & 1 == 1).collect();
                let mut p = move_selection(len, &sel, to);
                p.sort_unstable();
                assert_eq!(
                    p,
                    (0..len).collect::<Vec<_>>(),
                    "len {len} to {to} sel {sel:?}"
                );
            }
        }
    }
}

#[test]
fn apply_and_remove() {
    let v = vec!["a", "b", "c", "d"];
    assert_eq!(apply_order(&v, &[3, 0, 2, 1]), ["d", "a", "c", "b"]);
    assert_eq!(remove_indices(&v, &[1, 3, 3]), ["a", "c"]);
}
