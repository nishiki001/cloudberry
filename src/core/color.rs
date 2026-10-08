//! HSV ↔ RGB ↔ hex conversions for the colour picker, and the "recent colours" list.
use super::theme::Rgb;

/// Hue 0..360, saturation and value 0..1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

pub fn rgb_to_hsv(c: Rgb) -> Hsv {
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    Hsv {
        h,
        s: if max == 0.0 { 0.0 } else { d / max },
        v: max,
    }
}

pub fn hsv_to_rgb(c: Hsv) -> Rgb {
    let h = c.h.rem_euclid(360.0);
    let (s, v) = (c.s.clamp(0.0, 1.0), c.v.clamp(0.0, 1.0));
    let k = |n: f32| {
        let k = (n + h / 60.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    let byte = |x: f32| (x * 255.0).round() as u8;
    Rgb(byte(k(5.0)), byte(k(3.0)), byte(k(1.0)))
}

pub fn hsv_to_hex(h: f32, s: f32, v: f32) -> String {
    hsv_to_rgb(Hsv { h, s, v }).hex()
}

/// Parses "#RRGGBB" (the `#` is optional) and the short "#RGB" form.
pub fn parse_hex(s: &str) -> Option<Rgb> {
    let t = s.trim().trim_start_matches('#');
    if t.len() == 3 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        let d = |i: usize| u8::from_str_radix(&t[i..=i], 16).map(|x| x * 17);
        return Some(Rgb(d(0).ok()?, d(1).ok()?, d(2).ok()?));
    }
    Rgb::from_hex(t)
}

/// Move `hex` to the front of the recent list (normalised, no duplicates, at most `max`).
pub fn push_recent(list: &mut Vec<String>, hex: &str, max: usize) {
    let Some(c) = parse_hex(hex) else { return };
    let hex = c.hex();
    list.retain(|h| !h.eq_ignore_ascii_case(&hex));
    list.insert(0, hex);
    list.truncate(max);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hsv_rgb_round_trips_exactly() {
        for r in (0..=255u16).step_by(15) {
            for g in (0..=255u16).step_by(15) {
                for b in (0..=255u16).step_by(15) {
                    let c = Rgb(r as u8, g as u8, b as u8);
                    assert_eq!(hsv_to_rgb(rgb_to_hsv(c)), c, "{c:?}");
                }
            }
        }
    }

    #[test]
    fn known_colours_and_edges() {
        assert_eq!(
            rgb_to_hsv(Rgb(255, 0, 0)),
            Hsv {
                h: 0.0,
                s: 1.0,
                v: 1.0
            }
        );
        assert_eq!(rgb_to_hsv(Rgb(0, 0, 255)).h, 240.0);
        assert_eq!(rgb_to_hsv(Rgb(128, 128, 128)).s, 0.0);
        assert_eq!(
            rgb_to_hsv(Rgb(0, 0, 0)),
            Hsv {
                h: 0.0,
                s: 0.0,
                v: 0.0
            }
        );
        assert_eq!(
            hsv_to_rgb(Hsv {
                h: 360.0,
                s: 1.0,
                v: 1.0
            }),
            Rgb(255, 0, 0)
        );
        assert_eq!(
            hsv_to_rgb(Hsv {
                h: -120.0,
                s: 1.0,
                v: 1.0
            }),
            Rgb(0, 0, 255)
        );
        assert_eq!(hsv_to_hex(120.0, 1.0, 1.0), "#00FF00");
        // out-of-range input is clamped, never panics
        assert_eq!(
            hsv_to_rgb(Hsv {
                h: 10.0,
                s: 3.0,
                v: -1.0
            }),
            Rgb(0, 0, 0)
        );
    }

    #[test]
    fn hex_round_trip_and_forms() {
        for c in [
            Rgb(0, 0, 0),
            Rgb(255, 255, 255),
            Rgb(0x3D, 0xAE, 0xE9),
            Rgb(1, 2, 3),
        ] {
            assert_eq!(parse_hex(&c.hex()), Some(c));
            let hsv = rgb_to_hsv(c);
            assert_eq!(parse_hex(&hsv_to_hex(hsv.h, hsv.s, hsv.v)), Some(c));
        }
        assert_eq!(parse_hex("#f80"), Some(Rgb(255, 136, 0)));
        assert_eq!(parse_hex("3daee9"), Some(Rgb(0x3D, 0xAE, 0xE9)));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn recent_list_is_deduplicated_and_capped() {
        let mut l = Vec::new();
        for h in ["#ff0000", "#00ff00", "#FF0000", "bogus", "#0000ff"] {
            push_recent(&mut l, h, 3);
        }
        assert_eq!(l, ["#0000FF", "#FF0000", "#00FF00"]);
        push_recent(&mut l, "#123456", 3);
        assert_eq!(l, ["#123456", "#0000FF", "#FF0000"]);
    }
}
