//! Playlist background: renders the picture off the UI thread (decode, fit, blur, tint),
//! caches the decoded source, and crossfades between two image layers.
use super::state::STATE;
use crate::config::Config;
use crate::core::background::{Fit, Params, Pos, pattern, render};
use crate::core::theme::Rgb;
use crate::{AppWindow, Appearance, Backdrop, Theme};
use image::RgbaImage;
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
struct Bd {
    generation: u64,
    show_b: bool,
    shown: bool,
    cover: Option<Arc<RgbaImage>>,
    cover_seq: u64,
    /// Decoded custom file, by path.
    custom: Option<(String, Arc<RgbaImage>)>,
    size: (u32, u32),
    last_key: String,
    timer: Timer,
    sync: bool,
}

thread_local! {
    static BD: RefCell<Bd> = RefCell::new(Bd::default());
}

/// True when a background picture is (or will be) drawn behind the table.
pub fn wants_image(cfg: &Config) -> bool {
    matches!(cfg.bg_mode.as_str(), "pattern" | "cover" | "image")
}

pub fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<Backdrop>().on_resized(move |w, h| {
        let size = (w.max(0) as u32, h.max(0) as u32);
        if size.0 < 8 || size.1 < 8 {
            return;
        }
        let changed = BD.with(|b| std::mem::replace(&mut b.borrow_mut().size, size) != size);
        if changed && let Some(ui) = weak.upgrade() {
            update(&ui);
        }
    });
}

/// A new cover arrived (used by the "cover" mode).
pub fn set_cover(ui: &AppWindow, img: Arc<RgbaImage>) {
    let cover_mode = STATE.with(|s| s.borrow().cfg.bg_mode == "cover");
    BD.with(|b| {
        let mut b = b.borrow_mut();
        b.cover = Some(img);
        b.cover_seq += 1;
    });
    if !cover_mode {
        return; // other modes do not depend on the cover
    }
    update(ui);
}

fn rgb(c: slint::Color) -> Rgb {
    Rgb(c.red(), c.green(), c.blue())
}

/// Schedule a (debounced) re-render for the current settings and colours.
pub fn update(ui: &AppWindow) {
    let cfg = STATE.with(|s| s.borrow().cfg.clone());
    if !wants_image(&cfg) {
        ui.global::<Backdrop>().set_active(false);
        BD.with(|b| {
            let mut b = b.borrow_mut();
            b.generation += 1;
            b.shown = false;
            b.last_key.clear();
            b.timer.stop(); // a pending render must not bring the picture back
        });
        return;
    }
    let theme = ui.global::<Theme>();
    let (base, accent) = (rgb(theme.get_base()), rgb(theme.get_accent()));
    ui.global::<Backdrop>().set_opacity(cfg.bg_opacity);
    let weak = ui.as_weak();
    BD.with(|b| {
        let b = b.borrow();
        b.timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(120),
            move || {
                if let Some(ui) = weak.upgrade() {
                    let cfg = STATE.with(|s| s.borrow().cfg.clone());
                    if wants_image(&cfg) {
                        start_render(&ui, &cfg, base, accent);
                    }
                }
            },
        );
    });
}

type Rendered = Result<RgbaImage, String>;
type Loaded = (String, Arc<RgbaImage>);

enum Source {
    Pattern,
    Picture(Arc<RgbaImage>),
    File(String, Option<Arc<RgbaImage>>),
}

pub(super) fn load_file(path: &str) -> Result<Arc<RgbaImage>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("cannot read file: {e}"))?;
    if meta.len() > 40 << 20 {
        return Err("file is larger than 40 MB".into());
    }
    if !meta.is_file() {
        return Err("not a regular file".into());
    }
    let mut reader = image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| format!("cannot open: {e}"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 << 20);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|e| format!("not a supported picture: {e}"))?;
    let img = if img.width().max(img.height()) > 2400 {
        img.resize(2400, 2400, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    Ok(Arc::new(img.to_rgba8()))
}

fn start_render(ui: &AppWindow, cfg: &Config, base: Rgb, accent: Rgb) {
    let (generation, size, source, key) = BD.with(|b| {
        let mut b = b.borrow_mut();
        let source = match cfg.bg_mode.as_str() {
            "cover" => b.cover.clone().map(Source::Picture),
            "image" if !cfg.bg_path.is_empty() => {
                let cached = b
                    .custom
                    .as_ref()
                    .filter(|(p, _)| *p == cfg.bg_path)
                    .map(|(_, i)| i.clone());
                Some(Source::File(cfg.bg_path.clone(), cached))
            }
            "pattern" => Some(Source::Pattern),
            _ => None,
        };
        let key = format!(
            "{}|{}|{}|{}|{}|{}|{}|{:?}|{:?}|{:?}|{}",
            cfg.bg_mode,
            cfg.bg_path,
            cfg.bg_fit,
            cfg.bg_pos,
            cfg.bg_blur,
            cfg.bg_tint,
            b.size.0 * 10_000 + b.size.1,
            base,
            accent,
            (cfg.bg_mode == "cover").then_some(b.cover_seq),
            cfg.bg_tint_color,
        );
        if key == b.last_key {
            return (0, b.size, None, key);
        }
        b.generation += 1;
        (b.generation, b.size, source, key)
    });
    let Some(source) = source else {
        if generation != 0 {
            ui.global::<Backdrop>().set_active(false);
            ui.global::<Appearance>().set_bg_error(
                if cfg.bg_mode == "cover" {
                    "waiting for a cover…"
                } else {
                    ""
                }
                .into(),
            );
        }
        return;
    };
    if size.0 < 8 {
        return;
    }
    let params = Params {
        fit: Fit::parse(&cfg.bg_fit),
        pos: Pos::parse(&cfg.bg_pos),
        blur: cfg.bg_blur * 24.0,
        tint: crate::core::color::parse_hex(&cfg.bg_tint_color).unwrap_or(base),
        tint_alpha: cfg.bg_tint * 0.8,
    };
    let work = move || -> (Rendered, Option<Loaded>) {
        let (w, h) = size;
        let mut loaded = None;
        let out = match source {
            Source::Pattern => Ok(render(
                &pattern(w, h, accent, base),
                w,
                h,
                &Params {
                    blur: 0.0,
                    tint_alpha: 0.0,
                    ..params
                },
            )),
            Source::Picture(img) => Ok(render(&img, w, h, &params)),
            Source::File(path, cached) => match cached.map_or_else(|| load_file(&path), Ok) {
                Ok(img) => {
                    loaded = Some((path, img.clone()));
                    Ok(render(&img, w, h, &params))
                }
                Err(e) => Err(e),
            },
        };
        (out, loaded)
    };
    let finish = move |ui: &AppWindow, out, loaded| {
        if BD.with(|b| b.borrow().generation) != generation {
            return; // superseded while rendering
        }
        show(ui, out, loaded, key);
    };
    if BD.with(|b| b.borrow().sync) {
        let (out, loaded) = work();
        finish(ui, out, loaded);
        return;
    }
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let (out, loaded) = work();
        let _ = weak.upgrade_in_event_loop(move |ui| finish(&ui, out, loaded));
    });
}

/// Screenshot mode has no event loop: render inline instead of on a worker thread.
pub fn set_sync() {
    BD.with(|b| b.borrow_mut().sync = true);
}

fn show(ui: &AppWindow, out: Rendered, loaded: Option<Loaded>, key: String) {
    let a = ui.global::<Appearance>();
    match out {
        Ok(img) => {
            a.set_bg_error("".into());
            let image = crate::art::to_slint(&img);
            let bd = ui.global::<Backdrop>();
            // decide under the borrow, touch Slint after it is released
            let (set_a, set_b, show_b) = BD.with(|b| {
                let mut b = b.borrow_mut();
                if let Some(l) = loaded {
                    b.custom = Some(l);
                }
                // write the hidden layer, then fade to it
                let plan = if !b.shown {
                    b.show_b = false;
                    (Some(image.clone()), Some(image), false)
                } else if b.show_b {
                    b.show_b = false;
                    (Some(image), None, false)
                } else {
                    b.show_b = true;
                    (None, Some(image), true)
                };
                b.shown = true;
                b.last_key = key;
                plan
            });
            if let Some(i) = set_a {
                bd.set_a(i);
            }
            if let Some(i) = set_b {
                bd.set_b(i);
            }
            bd.set_show_b(show_b);
            bd.set_active(true);
        }
        Err(e) => {
            a.set_bg_error(e.into());
            ui.global::<Backdrop>().set_active(false);
        }
    }
}
