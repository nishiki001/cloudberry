//! Appearance settings: custom colours, presets and the playlist background.
use super::state::STATE;
use crate::config::Config;
use crate::core::appearance::{base_alpha, inputs};
use crate::core::theme::PRESETS;
use crate::{AppWindow, Appearance, Theme};
use image::{Rgba, RgbaImage};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;

thread_local! {
    static SAVE: slint::Timer = slint::Timer::default();
}

/// Write the config once the user pauses (sliders, typing and picker drags fire per event).
pub fn save_soon() {
    SAVE.with(|t| {
        t.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(400),
            || {
                STATE.with(|s| {
                    let _ = s.borrow().cfg.save();
                });
            },
        )
    });
}

/// (Re)derive the theme from the saved settings and refresh what depends on its colours.
pub fn apply_theme(ui: &AppWindow, mode: i32) {
    let cfg = STATE.with(|s| s.borrow().cfg.clone());
    let dark = super::theme::resolve_dark(mode, ui.global::<Theme>().get_system_dark());
    let custom = inputs(
        cfg.colors_custom,
        &cfg.accent,
        &cfg.window_color,
        &cfg.text_color,
        dark,
    );
    let tokens = super::theme::apply(ui, mode, custom);
    let has_bg = super::backdrop::wants_image(&cfg);
    ui.global::<Theme>()
        .set_base_alpha(base_alpha(&tokens, has_bg, cfg.bg_opacity));
    super::backdrop::update(ui);
}

pub fn init(ui: &AppWindow, cfg: &Config) {
    super::icons::init(ui, &cfg.icon_set);
    let a = ui.global::<Appearance>();
    a.set_custom(cfg.colors_custom);
    a.set_accent(cfg.accent.clone().into());
    a.set_window_color(cfg.window_color.clone().into());
    a.set_text_color(cfg.text_color.clone().into());
    a.set_bg_mode(cfg.bg_mode.clone().into());
    a.set_bg_path(cfg.bg_path.clone().into());
    show_picture(ui, cfg.bg_path.clone());
    a.set_bg_fit(cfg.bg_fit.clone().into());
    a.set_bg_pos(cfg.bg_pos.clone().into());
    a.set_bg_opacity(cfg.bg_opacity);
    a.set_bg_blur(cfg.bg_blur);
    a.set_bg_tint(cfg.bg_tint);
    a.set_bg_tint_color(cfg.bg_tint_color.clone().into());
    // anonymous struct {name, color, hex} is a tuple in field order: (color, hex, name)
    let presets: Vec<(slint::Color, slint::SharedString, slint::SharedString)> = PRESETS
        .iter()
        .map(|(name, c)| {
            (
                slint::Color::from_rgb_u8(c.0, c.1, c.2),
                c.hex().into(),
                (*name).into(),
            )
        })
        .collect();
    a.set_presets(ModelRc::new(VecModel::from(presets)));
    super::colorpicker::init(ui);
    let mode = ui.global::<Theme>().get_mode();
    apply_theme(ui, mode);
}

/// File name and a small preview of the chosen picture (decoded off the UI thread).
fn show_picture(ui: &AppWindow, path: String) {
    let a = ui.global::<Appearance>();
    if path.is_empty() {
        a.set_bg_name("".into());
        a.set_bg_has_preview(false);
        return;
    }
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    a.set_bg_name(name.into());
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let preview = super::backdrop::load_file(&path)
            .ok()
            .map(|img| image::imageops::thumbnail(&*img, 112, 80));
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let a = ui.global::<Appearance>();
            if a.get_bg_path() != path.as_str() {
                return; // another picture was chosen meanwhile
            }
            match preview {
                Some(p) => {
                    a.set_bg_preview(crate::art::to_slint(&p));
                    a.set_bg_has_preview(true);
                }
                None => a.set_bg_has_preview(false),
            }
        });
    });
}

pub fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<Appearance>().on_browse_bg(move || {
        // the native dialog blocks: run it on a helper thread
        let weak = weak.clone();
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new()
                .set_title("Choose a background picture")
                .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                .pick_file();
            let Some(path) = picked else { return };
            let path = path.to_string_lossy().into_owned();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                let a = ui.global::<Appearance>();
                a.set_bg_path(path.clone().into());
                a.set_bg_mode("image".into());
                show_picture(&ui, path);
                a.invoke_changed();
            });
        });
    });
    let weak = ui.as_weak();
    ui.global::<Appearance>().on_clear_bg(move || {
        let Some(ui) = weak.upgrade() else { return };
        let a = ui.global::<Appearance>();
        a.set_bg_path("".into());
        a.set_bg_mode("none".into());
        show_picture(&ui, String::new());
        a.invoke_changed();
    });
    let weak = ui.as_weak();
    ui.global::<Appearance>().on_changed(move || {
        let Some(ui) = weak.upgrade() else { return };
        let a = ui.global::<Appearance>();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let c = &mut s.cfg;
            c.colors_custom = a.get_custom();
            c.accent = a.get_accent().trim().to_string();
            c.window_color = a.get_window_color().trim().to_string();
            c.text_color = a.get_text_color().trim().to_string();
            c.bg_mode = a.get_bg_mode().to_string();
            c.bg_path = a.get_bg_path().trim().to_string();
            c.bg_fit = a.get_bg_fit().to_string();
            c.bg_pos = a.get_bg_pos().to_string();
            c.bg_opacity = a.get_bg_opacity().clamp(0.0, 1.0);
            c.bg_blur = a.get_bg_blur().clamp(0.0, 1.0);
            c.bg_tint = a.get_bg_tint().clamp(0.0, 1.0);
            c.bg_tint_color = a.get_bg_tint_color().trim().to_string();
        });
        save_soon();
        apply_theme(&ui, ui.global::<Theme>().get_mode());
    });
    super::backdrop::wire(ui);
}

/// Screenshot fixtures: `rose` / `amber` / ... (custom accent preset), `bg-pattern`, `bg-cover`,
/// `bg-image`, `bg-blur`. Rendering is synchronous there (no event loop).
pub fn fixture(ui: &AppWindow, name: &str) {
    let mut cfg = Config::default();
    if let Some((_, c)) = PRESETS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        cfg.colors_custom = true;
        cfg.accent = c.hex();
    }
    let picture = Arc::new(RgbaImage::from_fn(640, 480, |x, y| {
        let (fx, fy) = (x as f32 / 640.0, y as f32 / 480.0);
        let d = ((fx - 0.65).powi(2) + (fy - 0.4).powi(2)).sqrt();
        let ring = ((d * 28.0).sin() * 0.5 + 0.5) * (1.0 - d).max(0.0);
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
        Rgba([
            c(0.2 + 0.7 * fx),
            c(0.3 + 0.5 * ring),
            c(0.9 - 0.6 * fy),
            255,
        ])
    }));
    // fixture names are dash-separated flags: `aero` skin, `aeroicons` / `pixel` icon sets
    let flags: Vec<&str> = name.split('-').collect();
    if flags.contains(&"aero") {
        cfg.skin = "aero".into();
    }
    if flags.contains(&"aeroicons") {
        cfg.icon_set = "aero".into();
    }
    if flags.contains(&"atkinson") {
        cfg.font_family = "Atkinson Hyperlegible".into();
    }
    if flags.contains(&"big") {
        cfg.font_size = 17;
    }
    if flags.contains(&"huge") {
        cfg.font_size = 24;
    }
    if flags.contains(&"pixel") {
        cfg.icon_set = "pixel".into();
    }
    if name.ends_with("-tone") {
        // a custom tone: rose accent on a rosy window colour
        cfg.colors_custom = true;
        cfg.accent = "#E0457B".into();
        cfg.window_color = "#F3DDE6".into();
    }
    match name {
        "bg-pattern" => cfg.bg_mode = "pattern".into(),
        "bg-cover" => {
            cfg.bg_mode = "cover".into();
            cfg.bg_blur = 0.4;
        }
        "bg-image" | "bg-blur" | "bg-left" | "bg-right" => {
            let path = std::env::temp_dir().join("cloudberry-fixture-bg.png");
            let _ = picture.save(&path);
            cfg.bg_mode = "image".into();
            cfg.bg_path = path.to_string_lossy().into_owned();
            cfg.bg_opacity = 0.7;
            cfg.bg_blur = if name == "bg-blur" { 0.6 } else { 0.0 };
            match name {
                "bg-left" => cfg.bg_pos = "left".into(),
                "bg-right" => cfg.bg_pos = "right".into(),
                _ => {}
            }
        }
        _ => {}
    }
    STATE.with(|s| s.borrow_mut().cfg = cfg.clone());
    super::backdrop::set_sync();
    super::backdrop::wire(ui);
    if name == "bg-cover" {
        super::backdrop::set_cover(ui, picture.clone());
    }
    init(ui, &cfg);
    super::fonts::init_sync(ui, &cfg);
    if cfg.bg_mode == "image" {
        // no event loop in screenshot mode: fill the picture row directly
        let a = ui.global::<Appearance>();
        a.set_bg_name("fixture-picture.png".into());
        a.set_bg_preview(crate::art::to_slint(&image::imageops::thumbnail(
            &*picture, 112, 80,
        )));
        a.set_bg_has_preview(true);
    }
}
