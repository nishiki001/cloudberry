//! Pushes the derived design tokens (core/theme.rs) into Slint's `Theme` global.
use crate::core::skin::Skin;
use crate::core::theme::{Grad, Inputs, Rgb, Tokens, derive, system_inputs};
use crate::{AppWindow, Theme};
use slint::{Color, ComponentHandle};

fn c(v: Rgb) -> Color {
    Color::from_rgb_u8(v.0, v.1, v.2)
}

/// Light, dark, or follow the system (`system_dark` = what the OS currently prefers).
pub fn resolve_dark(mode: i32, system_dark: bool) -> bool {
    match mode {
        1 => false,
        2 => true,
        _ => system_dark,
    }
}

/// Apply the theme for `mode` (0 system, 1 light, 2 dark); `custom` overrides the colour inputs.
pub fn apply(ui: &AppWindow, mode: i32, custom: Option<Inputs>) -> Tokens {
    let t = ui.global::<Theme>();
    t.set_mode(mode);
    let dark = resolve_dark(mode, t.get_system_dark());
    let mut tokens = derive(custom.unwrap_or_else(|| system_inputs(dark)), dark);
    let skin = Skin::parse(&super::state::STATE.with(|s| s.borrow().cfg.skin.clone()));
    crate::core::skin::restyle(&mut tokens, skin);
    set_tokens(ui, &tokens);
    let sk = crate::core::skin::tokens(skin);
    t.set_skin(skin.name().into());
    t.set_gloss(sk.gloss);
    t.set_rim_alpha(sk.rim);
    t.set_glow(sk.glow);
    t.set_btn_radius(sk.radius);
    t.set_orb(sk.orb);
    t.set_pill(sk.pill);
    t.set_tube(sk.tube);
    super::analyzer::redraw(ui); // the analyzer image is pre-rendered in the old colours
    tokens
}

pub fn set_tokens(ui: &AppWindow, k: &Tokens) {
    let t = ui.global::<Theme>();
    t.set_window(c(k.window));
    t.set_base(c(k.base));
    t.set_alt_row(c(k.alt_row));
    t.set_border(c(k.border));
    t.set_text(c(k.text));
    t.set_text_dim(c(k.text_dim));
    t.set_accent(c(k.accent));
    t.set_accent_text(c(k.accent_text));
    t.set_on_accent(c(k.on_accent));
    t.set_selection(c(k.selection));
    let g = |g: Grad| (c(g.top), c(g.bottom));
    let (a, b) = g(k.button);
    t.set_btn_top(a);
    t.set_btn_bottom(b);
    let (a, b) = g(k.button_hover);
    t.set_btn_hover_top(a);
    t.set_btn_hover_bottom(b);
    let (a, b) = g(k.header);
    t.set_header_top(a);
    t.set_header_bottom(b);
    let (a, b) = g(k.tab);
    t.set_tab_top(a);
    t.set_tab_bottom(b);
    let (a, b) = g(k.tab_active);
    t.set_tab_active_top(a);
    t.set_tab_active_bottom(b);
    let (a, b) = g(k.selected_row);
    t.set_sel_top(a);
    t.set_sel_bottom(b);
    let (a, b) = g(k.sidebar);
    t.set_side_top(a);
    t.set_side_bottom(b);
    let (a, b) = g(k.toolbar);
    t.set_tool_top(a);
    t.set_tool_bottom(b);
    t.set_analyzer_bg(c(k.analyzer_bg));
    t.set_bevel_alpha(k.bevel_alpha);
}
