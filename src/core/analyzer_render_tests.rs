use super::*;

const C: Colors = Colors {
    bg: [20, 22, 24, 255],
    stops: [[61, 174, 233, 255], [46, 204, 113, 255], [246, 116, 0, 255]],
    peak: [255, 255, 255, 255],
};

const ALL: [Style; 6] = [
    Style::Block,
    Style::Bars,
    Style::Wave,
    Style::Curve,
    Style::Mirror,
    Style::Dots,
];

fn frame<'a>(l: &'a [f32], p: &'a [f32], w: &'a [f32]) -> Frame<'a> {
    Frame {
        levels: l,
        peaks: p,
        wave: w,
    }
}

fn count_lit(img: &RgbaImage) -> usize {
    img.pixels()
        .filter(|p| p.0[0] > 40 || p.0[1] > 60 || p.0[2] > 60)
        .count()
}

fn tone(n: usize, amp: f32) -> Vec<f32> {
    (0..n).map(|i| amp * (i as f32 * 0.5).sin()).collect()
}

#[test]
fn size_is_exact_for_every_style() {
    for s in ALL {
        let b = bands_for(s, 260);
        let img = render(
            &frame(&vec![0.5; b], &vec![0.6; b], &tone(512, 0.5)),
            s,
            260,
            40,
            &C,
        );
        assert_eq!((img.width(), img.height()), (260, 40), "{s:?}");
    }
}

#[test]
fn silence_shows_only_the_dim_well() {
    for s in [
        Style::Block,
        Style::Bars,
        Style::Mirror,
        Style::Dots,
        Style::Curve,
    ] {
        let b = bands_for(s, 240);
        let z = vec![0.0; b];
        let img = render(&frame(&z, &z, &[]), s, 240, 36, &C);
        assert_eq!(count_lit(&img), 0, "{s:?}");
    }
}

#[test]
fn louder_means_more_pixels() {
    for s in [
        Style::Block,
        Style::Bars,
        Style::Mirror,
        Style::Dots,
        Style::Curve,
    ] {
        let b = bands_for(s, 240);
        let q = vec![0.2; b];
        let l = vec![0.9; b];
        let quiet = count_lit(&render(&frame(&q, &q, &[]), s, 240, 36, &C));
        let loud = count_lit(&render(&frame(&l, &l, &[]), s, 240, 36, &C));
        assert!(loud > quiet * 2, "{s:?}: {quiet} vs {loud}");
    }
    let quiet = count_lit(&render(
        &frame(&[], &[], &tone(512, 0.05)),
        Style::Wave,
        240,
        36,
        &C,
    ));
    let loud = count_lit(&render(
        &frame(&[], &[], &tone(512, 0.9)),
        Style::Wave,
        240,
        36,
        &C,
    ));
    assert!(loud > quiet, "wave: {quiet} vs {loud}");
}

#[test]
fn cell_size_is_constant_and_bands_follow_the_width() {
    assert_eq!(bands_for(Style::Block, 20), MIN_BANDS);
    assert_eq!(bands_for(Style::Block, 10_000), MAX_BANDS);
    let narrow = bands_for(Style::Block, 300);
    let wide = bands_for(Style::Block, 900);
    assert!(wide > narrow * 2);
    // every column of a full-level block frame is exactly `pitch - 1` px wide
    for width in [300u32, 900] {
        let b = bands_for(Style::Block, width);
        let img = render(
            &frame(&vec![1.0; b], &vec![1.0; b], &[]),
            Style::Block,
            width,
            12,
            &C,
        );
        let y = 10; // inside the bottom block row
        let mut runs = Vec::new();
        let mut run = 0;
        for x in 0..width {
            if img.get_pixel(x, y).0 != C.bg
                && img.get_pixel(x, y).0[0] != lerp(C.bg, C.stops[0], 0.07)[0]
            {
                run += 1;
            } else if run > 0 {
                runs.push(run);
                run = 0;
            }
        }
        if run > 0 {
            runs.push(run);
        }
        assert!(runs.iter().all(|&r| r == 6), "{width}: {runs:?}");
        assert_eq!(runs.len(), b);
    }
}

#[test]
fn peak_cap_floats_above_the_bar() {
    let img = render(&frame(&[0.2], &[0.8], &[]), Style::Block, 8, 60, &C);
    let rows = 60 / 3;
    let cap_row = (0.8f32 * rows as f32).round() as u32;
    let y = 60 - cap_row * 3;
    assert_eq!(img.get_pixel(1, y).0, C.peak);
}

#[test]
fn mirror_grows_both_ways_and_dots_are_round() {
    let b = bands_for(Style::Mirror, 100);
    let l = vec![0.8; b];
    let img = render(&frame(&l, &l, &[]), Style::Mirror, 100, 40, &C);
    let x = 52;
    assert_ne!(img.get_pixel(x, 5).0, C.bg);
    assert_ne!(img.get_pixel(x, 34).0, C.bg);
    let b = bands_for(Style::Dots, 60);
    let l = vec![1.0; b];
    let img = render(&frame(&l, &l, &[]), Style::Dots, 60, 12, &C);
    // corners of a 4x4 dot are background, the middle is lit
    let x0 = 60u32.saturating_sub(6 * b as u32) / 2;
    assert_eq!(img.get_pixel(x0, 0).0, C.bg);
    assert_ne!(img.get_pixel(x0 + 1, 1).0, C.bg);
}

#[test]
fn style_parsing() {
    for (s, v) in [
        ("block", Style::Block),
        ("bars", Style::Bars),
        ("wave", Style::Wave),
        ("curve", Style::Curve),
        ("mirror", Style::Mirror),
        ("dots", Style::Dots),
    ] {
        assert_eq!(Style::parse(s), Some(v));
    }
    assert_eq!(Style::parse("off"), None);
}

#[test]
fn cell_styles_reach_both_edges_at_any_width() {
    for style in [Style::Block, Style::Bars, Style::Dots] {
        for w in [860u32, 1200, 1920] {
            let b = bands_for(style, w);
            let lay = layout(style, w);
            let img = render(&frame(&vec![1.0; b], &vec![1.0; b], &[]), style, w, 24, &C);
            let lit_cols: Vec<u32> = (0..w)
                .filter(|&x| {
                    (0..24).any(|y| {
                        img.get_pixel(x, y).0 != C.bg
                            && img.get_pixel(x, y).0[0] != lerp(C.bg, C.stops[0], 0.07)[0]
                    })
                })
                .collect();
            let (first, last) = (*lit_cols.first().unwrap(), *lit_cols.last().unwrap());
            assert!(
                first.abs_diff(lay.margin) <= 1,
                "{style:?} {w}: first lit {first}, margin {}",
                lay.margin
            );
            assert!(
                (w - 1 - last).abs_diff(lay.margin) <= 2,
                "{style:?} {w}: last lit {last}"
            );
        }
    }
}
