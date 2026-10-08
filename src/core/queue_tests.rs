use super::*;
use crate::core::model::Kind;

fn it(t: &str) -> Item {
    Item {
        kind: Kind::Song,
        title: t.into(),
        video_id: Some(t.into()),
        browse_id: None,
        artists: vec![],
        album: None,
        album_id: None,
        duration: None,
        duration_secs: None,
        thumbnail: None,
        subtitle: String::new(),
    }
}
fn q(n: usize) -> Queue {
    let mut q = Queue::new(7);
    q.set((0..n).map(|i| it(&i.to_string())).collect(), 0);
    q
}
fn titles(q: &Queue) -> Vec<String> {
    q.items().iter().map(|i| i.title.clone()).collect()
}

#[test]
fn next_prev_and_end() {
    let mut q = q(3);
    assert_eq!(q.next(false).unwrap().title, "1");
    assert_eq!(q.next(false).unwrap().title, "2");
    assert!(q.next(false).is_none());
    assert_eq!(q.prev().unwrap().title, "1");
}

#[test]
fn repeat_modes() {
    let mut q = q(2);
    q.set_repeat(Repeat::One);
    assert_eq!(q.next(true).unwrap().title, "0");
    assert_eq!(q.next(false).unwrap().title, "1");
    q.set_repeat(Repeat::All);
    assert_eq!(q.next(true).unwrap().title, "0");
    q.set_repeat(Repeat::Off);
    q.jump(1);
    assert!(q.next(true).is_none());
}

#[test]
fn add_and_play_next() {
    let mut q = q(3);
    q.add(it("x"));
    q.play_next(it("y"));
    assert_eq!(titles(&q), ["0", "y", "1", "2", "x"]);
    assert_eq!(q.current().unwrap().title, "0");
    assert_eq!(q.next(false).unwrap().title, "y");
}

#[test]
fn remove_adjusts_current() {
    let mut q = q(4);
    q.jump(2);
    q.remove(0);
    assert_eq!(q.current().unwrap().title, "2");
    q.remove(2);
    assert_eq!(titles(&q), ["1", "2"]);
    assert_eq!(q.current().unwrap().title, "2");
    q.remove(1);
    assert!(q.current().is_none());
}

#[test]
fn reorder_keeps_current() {
    let mut q = q(4);
    q.jump(1);
    q.move_item(0, 3);
    assert_eq!(titles(&q), ["1", "2", "3", "0"]);
    assert_eq!(q.current().unwrap().title, "1");
    q.move_item(3, 0);
    assert_eq!(q.current().unwrap().title, "1");
    q.move_item(1, 2);
    assert_eq!(q.current().unwrap().title, "1");
}

#[test]
fn shuffle_visits_each_once_current_first() {
    let mut q = q(8);
    q.jump(3);
    q.set_shuffle(true);
    assert_eq!(q.current().unwrap().title, "3");
    let mut seen = vec![q.current().unwrap().title.clone()];
    while let Some(i) = q.next(false) {
        seen.push(i.title.clone());
    }
    seen.sort();
    assert_eq!(seen.len(), 8);
    seen.dedup();
    assert_eq!(seen.len(), 8);
    q.set_shuffle(false);
    assert!(q.current().is_some());
}

#[test]
fn play_next_while_shuffled_is_next() {
    let mut q = q(6);
    q.set_shuffle(true);
    q.play_next(it("y"));
    assert_eq!(q.next(false).unwrap().title, "y");
}

#[test]
fn add_while_shuffled_keeps_order_and_goes_last() {
    let mut q = q(4);
    q.set_shuffle(true);
    let before: Vec<usize> = q.order.clone();
    q.add(it("z"));
    assert_eq!(&q.order[..4], &before[..]);
    assert_eq!(q.order[4], 4);
}

#[test]
fn peek_matches_next() {
    let mut q = q(3);
    let p = q.peek_next(true).unwrap();
    let expect = q.items()[p].title.clone();
    assert_eq!(expect, q.next(true).unwrap().title);
}

#[test]
fn reorder_follows_the_current_track() {
    let mut q = q(4);
    q.jump(2);
    q.reorder(&[3, 2, 1, 0]);
    assert_eq!(titles(&q), ["3", "2", "1", "0"]);
    assert_eq!(q.current().unwrap().title, "2");
    assert_eq!(q.current_index(), Some(1));
    // wrong length is ignored
    q.reorder(&[0, 1]);
    assert_eq!(titles(&q), ["3", "2", "1", "0"]);
}

#[test]
fn remove_many_keeps_or_drops_current() {
    let mut q = q(6);
    q.jump(4);
    q.remove_many(&[0, 2, 5]);
    assert_eq!(titles(&q), ["1", "3", "4"]);
    assert_eq!(q.current().unwrap().title, "4");
    q.remove_many(&[2]);
    assert!(q.current().is_none());
    assert_eq!(titles(&q), ["1", "3"]);
}

#[test]
fn insert_at_shifts_the_current_track() {
    let mut q = q(4);
    q.jump(2);
    q.insert_at(1, vec![it("x"), it("y")]);
    assert_eq!(titles(&q), ["0", "x", "y", "1", "2", "3"]);
    assert_eq!(q.current().unwrap().title, "2");
    q.insert_at(99, vec![it("end")]);
    assert_eq!(titles(&q).last().unwrap(), "end");
    q.insert_at(5, vec![it("after")]);
    assert_eq!(q.current().unwrap().title, "2");
}

#[test]
fn reorder_rejects_non_permutations() {
    let mut q = q(3);
    q.reorder(&[0, 0, 1]);
    q.reorder(&[0, 1, 5]);
    assert_eq!(titles(&q), ["0", "1", "2"]);
}
