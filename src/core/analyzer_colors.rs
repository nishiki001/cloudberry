//! Analyzer colour schemes: follow the theme accent (default), a few presets, or custom
//! gradient stops. Pure; the GUI feeds in the theme colours and the user's picks.
use super::analyzer_render::Colors;
use super::theme::Rgb;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    Accent,
    Classic,
    Ice,
    Sunset,
    Mono,
    Custom,
}

impl Scheme {
    pub fn parse(s: &str) -> Scheme {
        match s {
            "classic" => Scheme::Classic,
            "ice" => Scheme::Ice,
            "sunset" => Scheme::Sunset,
            "mono" => Scheme::Mono,
            "custom" => Scheme::Custom,
            _ => Scheme::Accent,
        }
    }
}

pub struct Inputs<'a> {
    pub accent: Rgb,
    pub text: Rgb,
    pub bg: Rgb,
    /// 1..=3 gradient stops (bottom → top) for `Scheme::Custom`.
    pub custom: &'a [Rgb],
    /// Peak-cap colour; `None` derives it from the top stop.
    pub peak: Option<Rgb>,
}

fn px(c: Rgb) -> [u8; 4] {
    [c.0, c.1, c.2, 255]
}

fn hex(h: u32) -> Rgb {
    Rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

pub fn colors(s: Scheme, i: &Inputs) -> Colors {
    let stops: [Rgb; 3] = match s {
        Scheme::Accent => [i.accent.darken(0.25), i.accent, i.accent.lighten(0.45)],
        Scheme::Classic => [hex(0x2ECC71), hex(0xF1C40F), hex(0xE74C3C)],
        Scheme::Ice => [hex(0x1B6CA8), hex(0x3DAEE9), hex(0xE6F7FF)],
        Scheme::Sunset => [hex(0x7B2FF7), hex(0xF0436B), hex(0xFFB347)],
        Scheme::Mono => [i.bg.mix(i.text, 0.35), i.bg.mix(i.text, 0.7), i.text],
        Scheme::Custom => match i.custom {
            [] => [i.accent.darken(0.25), i.accent, i.accent.lighten(0.45)],
            [a] => [*a, *a, *a],
            [a, b] => [*a, a.mix(*b, 0.5), *b],
            [a, b, c, ..] => [*a, *b, *c],
        },
    };
    let peak = i.peak.unwrap_or_else(|| stops[2].lighten(0.4));
    Colors {
        bg: px(i.bg),
        stops: stops.map(px),
        peak: px(peak),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(custom: &[Rgb]) -> Inputs<'_> {
        Inputs {
            accent: Rgb(0x3D, 0xAE, 0xE9),
            text: Rgb(0x20, 0x20, 0x20),
            bg: Rgb(0xE4, 0xE7, 0xEA),
            custom,
            peak: None,
        }
    }

    #[test]
    fn default_follows_the_accent() {
        let c = colors(Scheme::parse("whatever"), &inputs(&[]));
        assert_eq!(c.stops[1], [0x3D, 0xAE, 0xE9, 255]);
        assert_eq!(Scheme::parse("accent"), Scheme::Accent);
    }

    #[test]
    fn presets_differ_and_custom_stops_are_used() {
        let all = [Scheme::Classic, Scheme::Ice, Scheme::Sunset, Scheme::Mono];
        let ramps: Vec<_> = all.iter().map(|s| colors(*s, &inputs(&[])).stops).collect();
        for i in 0..ramps.len() {
            for j in i + 1..ramps.len() {
                assert_ne!(ramps[i], ramps[j]);
            }
        }
        let red = Rgb(255, 0, 0);
        let blue = Rgb(0, 0, 255);
        let c = colors(Scheme::Custom, &inputs(&[red, blue]));
        assert_eq!(c.stops[0], [255, 0, 0, 255]);
        assert_eq!(c.stops[2], [0, 0, 255, 255]);
        assert_eq!(c.stops[1], [128, 0, 128, 255]);
        assert_eq!(
            colors(Scheme::Custom, &inputs(&[red])).stops[2],
            [255, 0, 0, 255]
        );
        // no custom stops: falls back to the accent scheme
        assert_eq!(
            colors(Scheme::Custom, &inputs(&[])).stops[1],
            [0x3D, 0xAE, 0xE9, 255]
        );
    }

    #[test]
    fn peak_is_separate() {
        let mut i = inputs(&[]);
        i.peak = Some(Rgb(1, 2, 3));
        assert_eq!(colors(Scheme::Ice, &i).peak, [1, 2, 3, 255]);
        let auto = colors(Scheme::Ice, &inputs(&[])).peak;
        assert!(auto[0] > 0xE6 - 1);
    }
}
