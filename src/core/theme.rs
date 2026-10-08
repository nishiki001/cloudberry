//! Theme engine: derives every colour the UI needs from three inputs (accent, window, text),
//! separately for light and dark. Pure and unit-tested; the GUI only copies the result into
//! Slint's `Theme` global. Text colours are corrected until they reach WCAG contrast 4.5:1.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const WHITE: Rgb = Rgb(255, 255, 255);
    pub const BLACK: Rgb = Rgb(0, 0, 0);

    /// Linear mix towards `o` by `t` (0 = self, 1 = other), per sRGB channel.
    pub fn mix(self, o: Rgb, t: f32) -> Rgb {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)).round() as u8;
        Rgb(m(self.0, o.0), m(self.1, o.1), m(self.2, o.2))
    }
    pub fn lighten(self, t: f32) -> Rgb {
        self.mix(Rgb::WHITE, t)
    }
    pub fn darken(self, t: f32) -> Rgb {
        self.mix(Rgb::BLACK, t)
    }
    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let lin = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.0) + 0.7152 * lin(self.1) + 0.0722 * lin(self.2)
    }
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }
    pub fn from_hex(s: &str) -> Option<Rgb> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }
}

/// WCAG contrast ratio, 1..=21.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (a.luminance(), b.luminance());
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Inputs {
    pub accent: Rgb,
    pub window: Rgb,
    pub text: Rgb,
}

/// The "system colors" defaults (KDE-Breeze-like).
pub fn system_inputs(dark: bool) -> Inputs {
    if dark {
        Inputs {
            accent: Rgb(0x3D, 0xAE, 0xE9),
            window: Rgb(0x2A, 0x2E, 0x32),
            text: Rgb(0xEF, 0xF0, 0xF1),
        }
    } else {
        Inputs {
            accent: Rgb(0x3D, 0xAE, 0xE9),
            window: Rgb(0xEF, 0xF0, 0xF1),
            text: Rgb(0x23, 0x26, 0x29),
        }
    }
}

pub const PRESETS: [(&str, Rgb); 6] = [
    ("Blue", Rgb(0x3D, 0xAE, 0xE9)),
    ("Teal", Rgb(0x1A, 0xBC, 0x9C)),
    ("Rose", Rgb(0xE0, 0x45, 0x7B)),
    ("Amber", Rgb(0xF5, 0xA6, 0x23)),
    ("Violet", Rgb(0x8E, 0x5C, 0xF7)),
    ("Graphite", Rgb(0x7A, 0x85, 0x91)),
];

/// Top/bottom colours of a gradient widget face.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grad {
    pub top: Rgb,
    pub bottom: Rgb,
}

fn grad(base: Rgb, dark: bool) -> Grad {
    // top ~5% lighter than the face, bottom = face (DESIGN: soft gradient)
    Grad {
        top: base.lighten(if dark { 0.04 } else { 0.05 }),
        bottom: base,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tokens {
    pub dark: bool,
    pub window: Rgb,
    pub base: Rgb,
    pub alt_row: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub text_dim: Rgb,
    pub accent: Rgb,
    /// Selection tint already blended over `base` (opaque).
    pub selection: Rgb,
    /// Accent usable as text / thin icons on `base` (≥ 4.5:1).
    pub accent_text: Rgb,
    /// Text / icon colour on a solid accent fill (white or black, whichever contrasts more).
    pub on_accent: Rgb,
    pub button: Grad,
    pub button_hover: Grad,
    pub header: Grad,
    pub tab: Grad,
    pub tab_active: Grad,
    pub selected_row: Grad,
    pub sidebar: Grad,
    pub toolbar: Grad,
    pub analyzer_bg: Rgb,
    /// 1 px light line along the top edge of raised widgets (alpha 0..1).
    pub bevel_alpha: f32,
}

fn worst(c: Rgb, bgs: &[Rgb]) -> f32 {
    bgs.iter().map(|b| contrast(c, *b)).fold(f32::MAX, f32::min)
}

/// Move `fg` towards black or white until it has `min` contrast against every background.
/// The theme's natural direction is tried first; the other one is used when that cannot
/// reach `min` (e.g. a mid-grey custom window), and the best result wins.
fn fix_contrast(fg: Rgb, bgs: &[Rgb], dark: bool, min: f32) -> Rgb {
    let (first, second) = if dark {
        (Rgb::WHITE, Rgb::BLACK)
    } else {
        (Rgb::BLACK, Rgb::WHITE)
    };
    let mut best = fg;
    for target in [first, second] {
        let mut c = fg;
        for step in 0..=20 {
            c = fg.mix(target, step as f32 * 0.05);
            if worst(c, bgs) >= min {
                return c;
            }
        }
        if worst(c, bgs) > worst(best, bgs) {
            best = c;
        }
    }
    best
}

/// A "light" theme needs a light window and a "dark" theme a dark one, whatever was picked:
/// pull extreme custom window colours back towards the mode's range.
fn normalize_window(w: Rgb, dark: bool) -> Rgb {
    let mut w = w;
    for _ in 0..40 {
        if dark && w.luminance() > 0.10 {
            w = w.darken(0.15);
        } else if !dark && w.luminance() < 0.55 {
            w = w.lighten(0.15);
        } else {
            break;
        }
    }
    w
}

pub fn derive(i: Inputs, dark: bool) -> Tokens {
    let window = normalize_window(i.window, dark);
    let base = if dark {
        window.darken(0.36)
    } else {
        window.lighten(0.8)
    };
    let alt_row = base.mix(window, 0.4);
    let header = if dark {
        window.lighten(0.07)
    } else {
        window.darken(0.05)
    };
    let border = if dark {
        window.lighten(0.1)
    } else {
        window.darken(0.16)
    };
    let selection = base.mix(i.accent, if dark { 0.30 } else { 0.25 });
    let bgs = [base, alt_row, selection, window];
    let text = fix_contrast(i.text, &bgs, dark, 4.5);
    // dim text: between text and window, but never below 4.5:1 on the reading surfaces
    let dim_bgs = [base, alt_row, window, selection];
    // fade towards the window colour as far as the contrast allows; `text` itself always passes
    let mut text_dim = text;
    for step in (0..=21).rev() {
        let cand = text.mix(window, step as f32 * 0.02);
        if dim_bgs.iter().all(|b| contrast(cand, *b) >= 4.5) {
            text_dim = cand;
            break;
        }
    }
    let face = window.mix(base, 0.45);
    let hover_face = face.lighten(if dark { 0.06 } else { 0.5 });
    Tokens {
        dark,
        window,
        base,
        alt_row,
        border,
        text,
        text_dim,
        accent: i.accent,
        selection,
        accent_text: fix_contrast(i.accent, &[base, alt_row], dark, 4.5),
        on_accent: if contrast(Rgb::WHITE, i.accent) >= contrast(Rgb::BLACK, i.accent) {
            Rgb::WHITE
        } else {
            Rgb::BLACK
        },
        button: grad(face, dark),
        button_hover: grad(hover_face, dark),
        header: grad(header, dark),
        tab: grad(window.mix(header, 0.5), dark),
        tab_active: grad(base, dark),
        selected_row: Grad {
            top: selection.lighten(0.06),
            bottom: selection,
        },
        sidebar: if dark {
            Grad {
                top: window.darken(0.25),
                bottom: window.darken(0.42),
            }
        } else {
            Grad {
                top: window.darken(0.07),
                bottom: window.darken(0.16),
            }
        },
        toolbar: grad(window.mix(base, 0.3), dark),
        analyzer_bg: if dark {
            window.darken(0.5)
        } else {
            window.darken(0.07)
        },
        bevel_alpha: if dark { 0.06 } else { 0.5 },
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
