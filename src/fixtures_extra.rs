//! Screenshot-fixture helpers: synthetic analyzer levels and thumbnails.
use crate::AppWindow;
use crate::TrackData;
use crate::fixtures::cover;
use slint::ComponentHandle;

/// A plausible-looking spectrum (bass-heavy hill with ripples) for `n` bands.
pub fn synthetic_levels(n: usize) -> (Vec<f32>, Vec<f32>) {
    let levels: Vec<f32> = (0..n)
        .map(|i| {
            let x = i as f32 / n as f32;
            let hill = (1.0 - x).powf(1.4) * 0.85 + 0.1;
            (hill + 0.12 * (i as f32 * 0.9).sin() - 0.05).clamp(0.02, 1.0)
        })
        .collect();
    let peaks = levels
        .iter()
        .enumerate()
        .map(|(i, l)| (l + 0.08 + 0.03 * (i % 3) as f32).min(1.0))
        .collect();
    (levels, peaks)
}

/// A few mixed sines standing in for raw samples.
pub fn synthetic_wave() -> Vec<f32> {
    (0..2048)
        .map(|i| {
            let t = i as f32;
            0.35 * (t * 0.05).sin() + 0.2 * (t * 0.13).sin() + 0.1 * (t * 0.31).sin()
        })
        .collect()
}

/// Fixture `an-<style>[-<scheme>]` (also the older `block` / `bars`): draws a synthetic frame
/// at whatever size the analyzer area reports.
pub fn synthetic_analyzer(ui: &AppWindow, name: &str) {
    use crate::core::analyzer_colors::Scheme;
    use crate::core::analyzer_render::{Frame, Style, bands_for, render};
    let spec = name.strip_prefix("an-").unwrap_or(name);
    let (style_s, scheme_s) = spec.split_once('-').unwrap_or((spec, "accent"));
    let Some(style) = Style::parse(style_s) else {
        return;
    };
    let scheme = Scheme::parse(scheme_s);
    ui.set_analyzer_style(crate::gui::analyzer_menu::style_name(style).into());
    let wave = synthetic_wave();
    let weak = ui.as_weak();
    ui.on_analyzer_resized(move |w, h| {
        if let Some(ui) = weak.upgrade() {
            let colors = crate::gui::analyzer::colors_with(&ui, scheme, &[], None);
            let (w, h) = (w.max(8) as u32, h.max(8) as u32);
            let (levels, peaks) = synthetic_levels(bands_for(style, w));
            let frame = Frame {
                levels: &levels,
                peaks: &peaks,
                wave: &wave,
            };
            ui.set_analyzer_frame(crate::art::to_slint(&render(&frame, style, w, h, &colors)));
        }
    });
}

/// A row with a generated small thumbnail.
pub fn thumbed(mut r: TrackData, hue: f32) -> TrackData {
    r.thumb = cover(64, hue);
    r.has_thumb = true;
    r
}
