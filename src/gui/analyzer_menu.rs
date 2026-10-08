//! Analyzer settings glue: custom colour stops / peak colour, and the style / scheme names.
use super::analyzer::{draw, parse_stops};
use super::state::STATE;
use crate::core::analyzer_colors::Scheme;
use crate::core::analyzer_render::Style;
use crate::core::color::parse_hex;
use crate::{AppWindow, Appearance};
use slint::ComponentHandle;

pub fn wire_colors(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<Appearance>().on_analyzer_changed(move || {
        let Some(ui) = weak.upgrade() else { return };
        let g = ui.global::<Appearance>();
        let stops: Vec<String> = [
            g.get_analyzer_c1(),
            g.get_analyzer_c2(),
            g.get_analyzer_c3(),
        ]
        .iter()
        .map(|s| s.trim().to_string())
        .collect();
        let peak = g.get_analyzer_peak().trim().to_string();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(a) = s.analyzer.as_mut() {
                a.custom = parse_stops(&stops);
                a.peak = parse_hex(&peak);
            }
            s.cfg.analyzer_custom = stops;
            s.cfg.analyzer_peak = peak;
        });
        super::appearance::save_soon();
        draw(&ui);
    });
}

pub fn style_name(s: Style) -> &'static str {
    match s {
        Style::Block => "block",
        Style::Bars => "bars",
        Style::Wave => "wave",
        Style::Curve => "curve",
        Style::Mirror => "mirror",
        Style::Dots => "dots",
    }
}

pub fn scheme_name(s: Scheme) -> &'static str {
    match s {
        Scheme::Accent => "accent",
        Scheme::Classic => "classic",
        Scheme::Ice => "ice",
        Scheme::Sunset => "sunset",
        Scheme::Mono => "mono",
        Scheme::Custom => "custom",
    }
}
