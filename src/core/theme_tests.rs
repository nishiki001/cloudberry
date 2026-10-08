use super::*;

fn assert_readable(t: &Tokens, what: &str) {
    for (name, bg) in [("base", t.base), ("alt-row", t.alt_row)] {
        let c = contrast(t.accent_text, bg);
        assert!(c >= 4.5, "{what}: accent text on {name} = {c:.2}");
    }
    for (name, bg) in [
        ("base", t.base),
        ("alt-row", t.alt_row),
        ("selection", t.selection),
        ("window", t.window),
    ] {
        let c = contrast(t.text, bg);
        assert!(c >= 4.5, "{what}: text on {name} = {c:.2}");
    }
    for (name, bg) in [
        ("base", t.base),
        ("alt-row", t.alt_row),
        ("window", t.window),
        ("selection", t.selection),
    ] {
        let c = contrast(t.text_dim, bg);
        assert!(c >= 4.5, "{what}: dim text on {name} = {c:.2}");
    }
}

#[test]
fn system_defaults_are_readable_in_both_modes() {
    for dark in [false, true] {
        assert_readable(
            &derive(system_inputs(dark), dark),
            if dark { "dark" } else { "light" },
        );
    }
}

#[test]
fn every_preset_is_readable_in_both_modes() {
    for (name, accent) in PRESETS {
        for dark in [false, true] {
            let t = derive(
                Inputs {
                    accent,
                    ..system_inputs(dark)
                },
                dark,
            );
            assert_readable(
                &t,
                &format!("{name} {}", if dark { "dark" } else { "light" }),
            );
        }
    }
}

#[test]
fn hostile_custom_colours_get_corrected() {
    // text almost equal to the window colour, and a very light accent on a dark window
    for dark in [false, true] {
        let w = system_inputs(dark).window;
        let i = Inputs {
            accent: Rgb(250, 250, 120),
            window: w,
            text: w.mix(Rgb(128, 128, 128), 0.1),
        };
        assert_readable(&derive(i, dark), "hostile");
    }
}

#[test]
fn defaults_match_the_design_table() {
    let l = derive(system_inputs(false), false);
    // DESIGN tokens: base #FCFCFC, header #E3E5E7, border #C9CCD0 (derived values stay within a few units (tint differences ignored))
    let near = |a: Rgb, b: Rgb| {
        (a.0 as i32 - b.0 as i32).abs() <= 8
            && (a.1 as i32 - b.1 as i32).abs() <= 8
            && (a.2 as i32 - b.2 as i32).abs() <= 8
    };
    assert!(near(l.base, Rgb(0xFC, 0xFC, 0xFC)), "{:?}", l.base);
    assert!(
        near(l.header.bottom, Rgb(0xE3, 0xE5, 0xE7)),
        "{:?}",
        l.header.bottom
    );
    assert!(near(l.border, Rgb(0xC9, 0xCC, 0xD0)), "{:?}", l.border);
    let d = derive(system_inputs(true), true);
    assert!(near(d.base, Rgb(0x1B, 0x1E, 0x20)), "{:?}", d.base);
    assert!(
        near(d.header.bottom, Rgb(0x31, 0x36, 0x3B)),
        "{:?}",
        d.header.bottom
    );
}

#[test]
fn gradients_are_top_lighter() {
    for dark in [false, true] {
        let t = derive(system_inputs(dark), dark);
        for g in [t.button, t.header, t.tab, t.toolbar] {
            assert!(g.top.luminance() >= g.bottom.luminance());
        }
    }
}

#[test]
fn hex_roundtrip_and_contrast_range() {
    assert_eq!(Rgb::from_hex("#3DAEE9"), Some(Rgb(0x3D, 0xAE, 0xE9)));
    assert_eq!(Rgb(1, 2, 3).hex(), "#010203");
    assert!(Rgb::from_hex("12345").is_none());
    assert!((contrast(Rgb::BLACK, Rgb::WHITE) - 21.0).abs() < 0.01);
    assert!((contrast(Rgb::WHITE, Rgb::WHITE) - 1.0).abs() < 0.01);
}

#[test]
fn mid_grey_and_extreme_windows_stay_readable() {
    for dark in [false, true] {
        for g in [0u8, 40, 100, 150, 200, 255] {
            let w = Rgb(g, g, g);
            let i = Inputs {
                accent: Rgb(0x3D, 0xAE, 0xE9),
                window: w,
                text: system_inputs(dark).text,
            };
            assert_readable(&derive(i, dark), &format!("window {g} dark={dark}"));
        }
    }
}
