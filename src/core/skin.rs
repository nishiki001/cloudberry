//! Skins: a skin is a set of tokens plus a restyling of the derived colour gradients, not code.
//! Classic is the light-gradient look; Aero is glossy glass (strong gloss highlight, bright rim,
//! inner glow, round orb / pill transport buttons, glass tubes). Colours always come from the
//! active tone, so every preset and dark mode work (dark = smoked glass).
use super::theme::{Grad, Rgb, Tokens, contrast};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skin {
    Classic,
    Aero,
}

impl Skin {
    pub fn parse(s: &str) -> Skin {
        if s == "aero" {
            Skin::Aero
        } else {
            Skin::Classic
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Skin::Classic => "classic",
            Skin::Aero => "aero",
        }
    }
}

/// What the components read (see `Theme` in ui/theme.slint).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SkinTokens {
    /// Strength of the curved gloss highlight on the top half (0 = none).
    pub gloss: f32,
    /// Alpha of the bright 1 px rim.
    pub rim: f32,
    /// Strength of the inner glow along the bottom edge.
    pub glow: f32,
    pub radius: f32,
    /// Play / pause as a round glass orb.
    pub orb: bool,
    /// Transport buttons as pills.
    pub pill: bool,
    /// Sliders as glass tubes with a liquid fill and a bead thumb.
    pub tube: bool,
}

pub fn tokens(skin: Skin) -> SkinTokens {
    match skin {
        Skin::Classic => SkinTokens {
            gloss: 0.0,
            rim: 0.0,
            glow: 0.0,
            radius: 3.0,
            orb: false,
            pill: false,
            tube: false,
        },
        Skin::Aero => SkinTokens {
            gloss: 0.62,
            rim: 0.85,
            glow: 0.55,
            radius: 7.0,
            orb: true,
            pill: true,
            tube: true,
        },
    }
}

/// Keep text readable on a face: pull both ends of the gradient towards `toward` until the
/// text reaches 4.5:1 on each.
fn readable(g: Grad, text: Rgb, toward: Rgb) -> Grad {
    let fix = |mut c: Rgb| {
        for _ in 0..12 {
            if contrast(text, c) >= 4.5 {
                break;
            }
            c = c.mix(toward, 0.15);
        }
        c
    };
    Grad {
        top: fix(g.top),
        bottom: fix(g.bottom),
    }
}

/// Re-derive the gradients of `k` for the skin (Classic keeps what the theme engine made).
pub fn restyle(k: &mut Tokens, skin: Skin) {
    if skin == Skin::Classic {
        return;
    }
    let (w, a, dark, text) = (k.window, k.accent, k.dark, k.text);
    let toward = if dark { Rgb::BLACK } else { Rgb::WHITE };
    let pair = |top: Rgb, bottom: Rgb| readable(Grad { top, bottom }, text, toward);
    if dark {
        // smoked glass: lighter glassy top, deep tinted bottom
        k.button = pair(w.lighten(0.22), w.darken(0.38).mix(a, 0.10));
        k.button_hover = pair(w.lighten(0.36), w.darken(0.2).mix(a, 0.26));
        k.header = pair(w.lighten(0.14), w.darken(0.22).mix(a, 0.08));
        k.tab = pair(w.lighten(0.10), w.darken(0.25).mix(a, 0.06));
        k.tab_active = pair(w.lighten(0.26), w.darken(0.1).mix(a, 0.14));
        k.toolbar = pair(w.lighten(0.18), w.darken(0.30).mix(a, 0.10));
        k.sidebar = Grad {
            top: w.darken(0.38).mix(a, 0.10),
            bottom: w.darken(0.58).mix(a, 0.06),
        };
        k.selected_row = Grad {
            top: k.selection.lighten(0.10),
            bottom: k.selection.mix(a, 0.25),
        };
    } else {
        // pearly glass: near-white top, a hint of the accent at the bottom
        k.button = pair(w.lighten(0.88), w.lighten(0.2).mix(a, 0.24));
        k.button_hover = pair(Rgb::WHITE, w.lighten(0.2).mix(a, 0.40));
        k.header = pair(w.lighten(0.78), w.lighten(0.1).mix(a, 0.12));
        k.tab = pair(w.lighten(0.7), w.mix(a, 0.14));
        k.tab_active = pair(Rgb::WHITE, k.base);
        k.toolbar = pair(w.lighten(0.82), w.lighten(0.1).mix(a, 0.16));
        k.sidebar = Grad {
            top: w.mix(a, 0.16),
            bottom: w.mix(a, 0.30).darken(0.08),
        };
        k.selected_row = Grad {
            top: k.selection.lighten(0.18),
            bottom: k.selection.mix(a, 0.30),
        };
    }
    k.bevel_alpha = if dark { 0.16 } else { 0.9 };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::theme::{PRESETS, derive, system_inputs};

    #[test]
    fn classic_changes_nothing_and_aero_is_glossier() {
        assert_eq!(Skin::parse("aero"), Skin::Aero);
        assert_eq!(Skin::parse("whatever"), Skin::Classic);
        let t = tokens(Skin::Classic);
        assert_eq!(t.gloss, 0.0);
        assert!(!t.orb && !t.pill && !t.tube);
        let a = tokens(Skin::Aero);
        assert!(a.gloss > 0.4 && a.rim > 0.5 && a.orb && a.pill && a.tube && a.radius > t.radius);
        let k = derive(system_inputs(false), false);
        let mut same = k;
        restyle(&mut same, Skin::Classic);
        assert_eq!(same.button, k.button);
        let mut aero = k;
        restyle(&mut aero, Skin::Aero);
        assert_ne!(aero.button, k.button);
    }

    #[test]
    fn aero_text_stays_readable_for_every_preset_and_mode() {
        for dark in [false, true] {
            for (_, accent) in PRESETS {
                let mut i = system_inputs(dark);
                i.accent = accent;
                let mut k = derive(i, dark);
                restyle(&mut k, Skin::Aero);
                for g in [
                    k.button,
                    k.button_hover,
                    k.header,
                    k.tab,
                    k.tab_active,
                    k.toolbar,
                ] {
                    for c in [g.top, g.bottom] {
                        assert!(contrast(k.text, c) >= 4.4, "{dark} {accent:?}: {c:?}");
                    }
                }
            }
        }
    }
}
