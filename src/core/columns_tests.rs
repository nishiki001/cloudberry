use super::*;

#[test]
fn hiding_and_showing_columns() {
    let mut c = ColumnConfig::default();
    c.toggle(Col::Album);
    c.toggle(Col::Title); // cannot be hidden
    assert!(!c.visible().contains(&Col::Album));
    assert!(c.visible().contains(&Col::Title));
    assert_eq!(c.layout(1200.0).len(), 6);
    c.toggle(Col::Album);
    assert!(c.visible().contains(&Col::Album));
}

#[test]
fn resizing_clamps_to_the_minimum() {
    let mut c = ColumnConfig::default();
    c.set_width(Col::Artist, 10);
    assert_eq!(c.width(Col::Artist), Col::Artist.min_width());
    c.set_width(Col::Artist, 240);
    assert_eq!(c.width(Col::Artist), 240);
}

#[test]
fn moving_visible_columns_both_directions() {
    let mut c = ColumnConfig::default();
    // visible: Num Title Artist Album Year Length Like
    c.move_visible(2, 4); // Artist after Year
    assert_eq!(
        c.visible()[..5],
        [Col::Num, Col::Title, Col::Album, Col::Year, Col::Artist]
    );
    c.move_visible(4, 1); // back before Title... lands at Title's slot
    assert_eq!(c.visible()[..3], [Col::Num, Col::Artist, Col::Title]);
}

#[test]
fn moving_around_a_hidden_column() {
    let mut c = ColumnConfig::default();
    c.toggle(Col::Artist);
    // visible: Num Title Album Year Length Like
    c.move_visible(2, 1);
    assert_eq!(c.visible()[..3], [Col::Num, Col::Album, Col::Title]);
}

#[test]
fn sanitize_repairs_hand_edited_configs() {
    let c = ColumnConfig {
        order: vec![Col::Album, Col::Album, Col::Title],
        hidden: vec![Col::Title, Col::Year],
        widths: vec![],
    }
    .sanitized();
    assert_eq!(c.order.len(), 7);
    assert_eq!(c.order[0], Col::Album);
    assert!(!c.hidden.contains(&Col::Title));
}

#[test]
fn config_roundtrips_through_toml() {
    let mut c = ColumnConfig::default();
    c.set_width(Col::Album, 222);
    c.toggle(Col::Year);
    c.move_visible(2, 3);
    let s = toml::to_string(&c).unwrap();
    let back: ColumnConfig = toml::from_str(&s).unwrap();
    assert_eq!(back, c);
}

fn px(l: &[Placed], c: Col) -> f32 {
    l.iter().find(|p| p.col == c).map_or(0.0, |p| p.w)
}

#[test]
fn columns_always_fit_the_viewport_exactly() {
    let cfg = ColumnConfig::default();
    for total in (300..=2000).step_by(7) {
        let total = total as f32;
        let l = cfg.layout(total);
        let right = l.last().map_or(0.0, |p| p.x + p.w);
        assert!(
            right <= total - SCROLL_W + 0.01,
            "{total}: right edge {right}"
        );
        // when the minimums fit, the columns fill the width to the pixel
        assert!(
            (right - (total - SCROLL_W)).abs() < 0.01,
            "{total}: {right}"
        );
        for p in &l {
            assert!(p.w >= p.col.min_width() as f32 - 0.01, "{total}: {p:?}");
            assert_eq!(p.w, p.w.floor(), "whole pixels: {p:?}");
        }
        // columns follow each other without gaps or overlap
        for w in l.windows(2) {
            assert!((w[0].x + w[0].w - w[1].x).abs() < 0.01);
        }
    }
}

#[test]
fn flexible_columns_share_in_proportion() {
    let l = ColumnConfig::default().layout(1920.0);
    let (t, a, b) = (px(&l, Col::Title), px(&l, Col::Artist), px(&l, Col::Album));
    // 40 : 22 : 30
    assert!((t / a - 40.0 / 22.0).abs() < 0.05, "{t} {a}");
    assert!((b / a - 30.0 / 22.0).abs() < 0.05, "{b} {a}");
    assert!(
        t < 2.0 * b,
        "the title must not dwarf the others at full screen"
    );
    // fixed columns keep their width at any size
    assert_eq!(px(&l, Col::Year), 52.0);
    assert_eq!(px(&ColumnConfig::default().layout(1000.0), Col::Year), 52.0);
}

#[test]
fn narrow_tables_drop_low_priority_columns_automatically() {
    let cfg = ColumnConfig::default();
    let cols = |w| -> Vec<Col> { cfg.layout(w).iter().map(|p| p.col).collect() };
    assert_eq!(cols(1000.0).len(), 7);
    // 550 px (860 px window, default sidebar): everything still fits
    assert_eq!(cols(550.0).len(), 7);
    let mid = cols(430.0);
    assert!(!mid.contains(&Col::Year), "{mid:?}");
    assert!(mid.contains(&Col::Title));
    let tiny = cols(300.0);
    assert!(tiny.contains(&Col::Title));
    assert!(!tiny.contains(&Col::Album) && !tiny.contains(&Col::Year));
    // the saved visibility is untouched
    assert!(cfg.visible().contains(&Col::Year));
    // nothing ever hangs over the edge
    for w in [300.0, 360.0, 430.0, 500.0] {
        let l = cfg.layout(w);
        let right = l.last().map_or(0.0, |p| p.x + p.w);
        assert!(right <= w - SCROLL_W + 0.01, "{w}: {right}");
    }
}

#[test]
fn saved_widths_act_as_proportions_at_any_size() {
    let mut c = ColumnConfig::default();
    c.set_width(Col::Title, 200);
    c.set_width(Col::Artist, 200);
    c.set_width(Col::Album, 200);
    for total in [800.0, 1920.0] {
        let l = c.layout(total);
        let (t, a, b) = (px(&l, Col::Title), px(&l, Col::Artist), px(&l, Col::Album));
        assert!(
            (t - a).abs() <= 1.0 && (a - b).abs() <= 2.0,
            "{total}: {t} {a} {b}"
        );
    }
}

#[test]
fn resize_drag_is_stable_clamped_and_local() {
    let base = ColumnConfig::default();
    let total = 1000.0;
    let before = base.layout(total);
    let mut a = base.clone();
    a.resize_from(&base, Col::Title, 37.0, total);
    let once = a.layout(total);
    // the same drag event applied again gives exactly the same widths
    for _ in 0..3 {
        a.resize_from(&base, Col::Title, 37.0, total);
        assert_eq!(a.layout(total), once);
    }
    // the dragged column and its flexible neighbour trade width; nothing else moves
    for (p, q) in once.iter().zip(&before) {
        match p.col {
            Col::Title => assert!((p.w - q.w - 37.0).abs() <= 1.0, "{p:?} {q:?}"),
            Col::Artist => assert!((q.w - p.w - 37.0).abs() <= 1.0, "{p:?} {q:?}"),
            _ => assert_eq!(p.w, q.w, "{:?}", p.col),
        }
    }
    // dragging back to zero restores the original layout
    a.resize_from(&base, Col::Title, 0.0, total);
    assert_eq!(a.layout(total), before);
    // clamping: never below the minimum on either side
    a.resize_from(&base, Col::Title, -5000.0, total);
    assert!(px(&a.layout(total), Col::Title) >= Col::Title.min_width() as f32);
    a.resize_from(&base, Col::Title, 5000.0, total);
    assert!(px(&a.layout(total), Col::Artist) >= Col::Artist.min_width() as f32);
    // the last flexible column trades with the previous one
    let mut l = base.clone();
    l.resize_from(&base, Col::Album, 20.0, total);
    assert!(px(&l.layout(total), Col::Album) > px(&before, Col::Album));
    // a fixed column changes its own width and the flexible ones absorb it
    let mut f = base.clone();
    f.resize_from(&base, Col::Year, 10.0, total);
    assert_eq!(px(&f.layout(total), Col::Year), 62.0);
    // fractional pointer positions do not make the width flicker between two values
    let mut g = base.clone();
    let widths: Vec<f32> = [10.4, 10.6, 10.4, 10.6]
        .iter()
        .map(|d| {
            g.resize_from(&base, Col::Artist, *d, total);
            px(&g.layout(total), Col::Artist)
        })
        .collect();
    assert_eq!(widths[0], widths[2]);
    assert_eq!(widths[1], widths[3]);
}
