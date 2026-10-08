//! Appearance settings → theme inputs, and the contrast guard for rows drawn over a
//! background image. Pure and unit-tested.
use super::theme::{Inputs, Rgb, Tokens, contrast, system_inputs};

/// Colour inputs for the theme: system colours, or the custom ones where they parse
/// (an empty / invalid field keeps the system value).
pub fn inputs(custom: bool, accent: &str, window: &str, text: &str, dark: bool) -> Option<Inputs> {
    if !custom {
        return None;
    }
    let sys = system_inputs(dark);
    Some(Inputs {
        accent: Rgb::from_hex(accent).unwrap_or(sys.accent),
        window: Rgb::from_hex(window).unwrap_or(sys.window),
        text: Rgb::from_hex(text).unwrap_or(sys.text),
    })
}

/// Opacity of the row/base layer drawn over the background image so that text stays readable
/// whatever the picture contains. The worst case is a pure white or black pixel, shown at
/// `image_opacity` over the base colour; `text` must keep 4.5:1 and dim text 3:1.
pub fn base_alpha(k: &Tokens, has_image: bool, image_opacity: f32) -> f32 {
    if !has_image {
        return 1.0;
    }
    let o = image_opacity.clamp(0.0, 1.0);
    let ok = |a: f32| {
        [Rgb::WHITE, Rgb::BLACK].into_iter().all(|worst| {
            let behind = k.base.mix(worst, o);
            // the selection and alternate rows are the other row colours on top
            [k.base, k.alt_row].into_iter().all(|row| {
                let eff = behind.mix(row, a);
                contrast(k.text, eff) >= 4.5 && contrast(k.text_dim, eff) >= 3.0
            })
        })
    };
    (11..=20)
        .map(|n| n as f32 * 0.05)
        .find(|&a| ok(a))
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::theme::{PRESETS, derive};

    #[test]
    fn custom_inputs_fall_back_per_field() {
        assert!(inputs(false, "#FF0000", "", "", false).is_none());
        let i = inputs(true, "#FF0000", "nonsense", "", true).unwrap();
        assert_eq!(i.accent, Rgb(255, 0, 0));
        assert_eq!(i.window, system_inputs(true).window);
    }

    #[test]
    fn rows_over_any_image_stay_readable() {
        for dark in [false, true] {
            for (_, accent) in PRESETS {
                let mut i = system_inputs(dark);
                i.accent = accent;
                let k = derive(i, dark);
                for o in [0.1, 0.35, 0.6, 1.0] {
                    let a = base_alpha(&k, true, o);
                    assert!((0.55..=1.0).contains(&a));
                    for worst in [Rgb::WHITE, Rgb::BLACK] {
                        let eff = k.base.mix(worst, o).mix(k.alt_row, a);
                        assert!(contrast(k.text, eff) >= 4.4, "{dark} {accent:?} {o}");
                    }
                }
            }
        }
        let k = derive(system_inputs(false), false);
        assert_eq!(base_alpha(&k, false, 1.0), 1.0);
    }
}
