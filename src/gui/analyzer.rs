//! Drives the analyzer: a UI-thread timer that runs only while there is something to animate
//! (playing, or bars/caps still falling), then stops so a paused player costs nothing.
use super::analyzer_menu::{scheme_name, style_name};
use super::state::STATE;
use crate::art::to_slint;
use crate::config::Config;
use crate::core::analyzer::Analyzer;
use crate::core::analyzer_colors::{self, Scheme};
use crate::core::analyzer_render::{self, Colors, Frame, Style};
use crate::core::color::parse_hex;
use crate::core::theme::Rgb;
use crate::player::native::Shared;
use crate::{AppWindow, Appearance, Theme};
use slint::{Color, ComponentHandle, Timer, TimerMode};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

pub struct AnalyzerState {
    an: Analyzer,
    tap: Option<Arc<Shared>>,
    /// Filled by the playback backend (created on the core thread) once it is up.
    slot: Arc<OnceLock<Arc<Shared>>>,
    rate: f32,
    timer: Timer,
    /// Debounces band-layout changes while the window is being resized.
    fit_timer: Timer,
    last: Instant,
    pub playing: bool,
    /// Spinning discs: eased speed (0..1) and angle in degrees, advanced by the same tick.
    disc_speed: f32,
    disc_angle: f32,
    size: (u32, u32),
    style: Option<Style>,
    fps: u32,
    scheme: Scheme,
    pub(super) custom: Vec<Rgb>,
    pub(super) peak: Option<Rgb>,
}

fn rgb(c: Color) -> Rgb {
    Rgb(c.red(), c.green(), c.blue())
}

/// Colours for the current theme and the analyzer's scheme.
pub fn colors(ui: &AppWindow, a: &AnalyzerState) -> Colors {
    colors_with(ui, a.scheme, &a.custom, a.peak)
}

pub fn colors_with(ui: &AppWindow, scheme: Scheme, custom: &[Rgb], peak: Option<Rgb>) -> Colors {
    let t = ui.global::<Theme>();
    analyzer_colors::colors(
        scheme,
        &analyzer_colors::Inputs {
            accent: rgb(t.get_accent()),
            text: rgb(t.get_text()),
            bg: rgb(t.get_analyzer_bg()),
            custom,
            peak,
        },
    )
}

pub(super) fn parse_stops(list: &[String]) -> Vec<Rgb> {
    list.iter().filter_map(|h| parse_hex(h)).take(3).collect()
}

pub fn init(ui: &AppWindow, cfg: &Config, slot: Arc<OnceLock<Arc<Shared>>>) {
    let style = Style::parse(&cfg.analyzer_style);
    let st = AnalyzerState {
        an: Analyzer::new(
            analyzer_render::bands_for(style.unwrap_or(Style::Block), 240),
            44_100.0,
        ),
        tap: None,
        slot,
        rate: 44_100.0,
        timer: Timer::default(),
        fit_timer: Timer::default(),
        last: Instant::now(),
        playing: false,
        disc_speed: 0.0,
        disc_angle: 0.0,
        size: (240, 34),
        style,
        fps: if cfg.analyzer_fps >= 60 { 60 } else { 30 },
        scheme: Scheme::parse(&cfg.analyzer_scheme),
        custom: parse_stops(&cfg.analyzer_custom),
        peak: parse_hex(&cfg.analyzer_peak),
    };
    ui.set_analyzer_style(cfg.analyzer_style.clone().into());
    ui.set_analyzer_fps(st.fps as i32);
    let a = ui.global::<Appearance>();
    a.set_analyzer_scheme(cfg.analyzer_scheme.clone().into());
    let c = |i: usize| {
        cfg.analyzer_custom
            .get(i)
            .cloned()
            .unwrap_or_default()
            .into()
    };
    a.set_analyzer_c1(c(0));
    a.set_analyzer_c2(c(1));
    a.set_analyzer_c3(c(2));
    a.set_analyzer_peak(cfg.analyzer_peak.clone().into());
    STATE.with(|s| s.borrow_mut().analyzer = Some(st));
    wire(ui);
}

/// Rebuild the band table when the width (or style) asks for a different band count.
fn fit_bands(a: &mut AnalyzerState) {
    let Some(style) = a.style else { return };
    let want = analyzer_render::bands_for(style, a.size.0);
    if want != a.an.bands() {
        a.an = Analyzer::new(want, a.rate);
    }
}

fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_analyzer_resized(move |w, h| {
        let Some(ui) = weak.upgrade() else { return };
        // physical pixels: crisp cells on scaled displays
        let k = ui.window().scale_factor();
        STATE.with(|s| {
            if let Some(a) = s.borrow_mut().analyzer.as_mut() {
                a.size = (
                    ((w.max(8) as f32) * k).round() as u32,
                    ((h.max(8) as f32) * k).round() as u32,
                );
                // the band layout follows ~100 ms after the last resize event, never per frame
                let weak = weak.clone();
                a.fit_timer.start(
                    TimerMode::SingleShot,
                    Duration::from_millis(100),
                    move || {
                        let Some(ui) = weak.upgrade() else { return };
                        STATE.with(|s| {
                            if let Some(a) = s.borrow_mut().analyzer.as_mut() {
                                fit_bands(a);
                            }
                        });
                        draw(&ui);
                    },
                );
            }
        });
        draw(&ui);
    });
    let weak = ui.as_weak();
    ui.on_analyzer_menu(move |m| {
        let Some(ui) = weak.upgrade() else { return };
        let (style_s, fps) = STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(a) = s.analyzer.as_mut() {
                match m.as_str() {
                    "fps30" => a.fps = 30,
                    "fps60" => a.fps = 60,
                    other => {
                        if let Some(sc) = other.strip_prefix("scheme-") {
                            a.scheme = Scheme::parse(sc);
                        } else if other == "off" || Style::parse(other).is_some() {
                            a.style = Style::parse(other);
                            fit_bands(a);
                        }
                    }
                }
            }
            let a = s.analyzer.as_ref().unwrap();
            let style = a.style.map_or("off", style_name).to_string();
            let scheme = scheme_name(a.scheme).to_string();
            let fps = a.fps;
            s.cfg.analyzer_style.clone_from(&style);
            s.cfg.analyzer_scheme = scheme;
            s.cfg.analyzer_fps = fps;
            let _ = s.cfg.save();
            (style, fps)
        });
        ui.set_analyzer_style(style_s.into());
        ui.set_analyzer_fps(fps as i32);
        let scheme = STATE.with(|s| s.borrow().cfg.analyzer_scheme.clone());
        ui.global::<Appearance>().set_analyzer_scheme(scheme.into());
        STATE.with(|s| {
            if let Some(a) = s.borrow().analyzer.as_ref() {
                a.timer.stop();
            }
        });
        ensure_running(&ui);
        draw(&ui);
    });
    super::analyzer_menu::wire_colors(ui);
}

/// Called whenever playback state changes.
pub fn set_playing(ui: &AppWindow, playing: bool) {
    STATE.with(|s| {
        if let Some(a) = s.borrow_mut().analyzer.as_mut() {
            a.playing = playing;
        }
    });
    ensure_running(ui);
}

/// The discs should be turning (setting on, motion allowed, playing).
fn disc_wanted(ui: &AppWindow, a: &AnalyzerState) -> bool {
    let t = ui.global::<Theme>();
    a.playing && t.get_spinning_disc() && !t.get_reduce_motion()
}

pub(super) fn ensure_running(ui: &AppWindow) {
    let fps = STATE.with(|s| {
        let s = s.borrow();
        let a = s.analyzer.as_ref()?;
        let disc = disc_wanted(ui, a) || a.disc_speed > 0.001;
        let has_input = a.tap.is_some() || a.slot.get().is_some();
        let animating = a.playing || !a.an.settled() || disc;
        let wanted = (a.style.is_some() && has_input) || disc;
        // the GL renderer (spinning disc) repaints the whole window per frame: 30 at most
        let fps = if ui.global::<Theme>().get_spinning_disc() {
            a.fps.min(30)
        } else {
            a.fps
        };
        (wanted && !a.timer.running() && animating).then_some(fps)
    });
    let Some(fps) = fps else { return };
    let weak = ui.as_weak();
    STATE.with(|s| {
        if let Some(a) = s.borrow_mut().analyzer.as_mut() {
            a.last = Instant::now();
            a.timer.start(
                TimerMode::Repeated,
                Duration::from_millis(1000 / fps as u64),
                move || {
                    if let Some(ui) = weak.upgrade() {
                        tick(&ui);
                    }
                },
            );
        }
    });
}

fn tick(ui: &AppWindow) {
    draw(ui);
    STATE.with(|s| {
        if let Some(a) = s.borrow().analyzer.as_ref()
            && !a.playing
            && a.an.settled()
            && a.disc_speed <= 0.001
        {
            a.timer.stop(); // everything is at rest: no more redraws
        }
    });
}

/// Ease the disc speed toward its target and advance the angle by `dt` seconds; returns the new
/// angle to set once the state borrow is released.
fn step_disc(ui: &AppWindow, a: &mut AnalyzerState, dt: f32) -> Option<f32> {
    let target = if disc_wanted(ui, a) { 1.0 } else { 0.0 };
    // ~0.25 s time constant, like the old per-disc ease
    a.disc_speed += (target - a.disc_speed) * (1.0 - (-dt / 0.25).exp());
    if a.disc_speed < 0.001 && target == 0.0 {
        a.disc_speed = 0.0;
        return None;
    }
    let dps = ui.global::<Theme>().get_disc_speed();
    a.disc_angle = (a.disc_angle + dps * a.disc_speed * dt) % 360.0;
    Some(a.disc_angle)
}

/// Re-render the current frame (e.g. after a theme change while paused).
pub fn redraw(ui: &AppWindow) {
    draw(ui);
}

/// Update the analyzer with the newest samples and push a frame to the UI.
pub(super) fn draw(ui: &AppWindow) {
    let (img, angle) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(a) = s.analyzer.as_mut() else {
            return (None, None);
        };
        if a.tap.is_none()
            && let Some(t) = a.slot.get()
        {
            // device rate is known now: rebuild the band table for it
            a.tap = Some(t.clone());
            a.rate = t.rate as f32;
            a.an = Analyzer::new(a.an.bands(), a.rate);
        }
        let now = Instant::now();
        let dt = now.duration_since(a.last).as_secs_f32().clamp(0.001, 0.25);
        a.last = now;
        let mut buf = [0.0f32; crate::player::native::TAP_LEN];
        match (&a.tap, a.playing) {
            (Some(t), true) => t.tap.snapshot(&mut buf),
            _ => buf.fill(0.0),
        }
        let angle = step_disc(ui, a, dt);
        let samples: &[f32] = if a.playing {
            &buf[buf.len() - a.an.fft_len().min(buf.len())..]
        } else {
            &[]
        };
        let Some(style) = a.style else {
            return (None, angle); // disc only: no FFT
        };
        a.an.update(samples, dt);
        let c = colors(ui, a);
        let frame = Frame {
            levels: a.an.levels(),
            peaks: a.an.peaks(),
            wave: a.an.wave(),
        };
        let img = analyzer_render::render(&frame, style, a.size.0, a.size.1, &c);
        (Some(img), angle)
    });
    if let Some(deg) = angle {
        ui.global::<Theme>().set_disc_angle(deg);
    }
    if let Some(img) = img {
        ui.set_analyzer_frame(to_slint(&img));
    }
}
