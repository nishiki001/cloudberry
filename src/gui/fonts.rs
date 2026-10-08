//! Fonts: UI and lyrics family, size factor, and the list of installed families.

use super::state::STATE;
use crate::config::Config;
use crate::{AppWindow, Dialogs, Theme};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

/// Bundled with the app (OFL, assets/fonts/), registered by `import` in ui/app.slint.
const BUNDLED: &str = "Atkinson Hyperlegible";
const BASE_SIZE: f32 = 13.0;

/// Sorted, de-duplicated installed family names plus the bundled one.
pub fn families() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names: Vec<String> = db
        .faces()
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .chain(std::iter::once(BUNDLED.to_string()))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

/// Size setting (px, 0 = default) → factor relative to the 13 px design size.
pub fn scale(size: u32) -> f32 {
    if size == 0 {
        1.0
    } else {
        size.clamp(10, 24) as f32 / BASE_SIZE
    }
}

fn push(ui: &AppWindow, cfg: &Config) {
    let t = ui.global::<Theme>();
    t.set_font_family(cfg.font_family.as_str().into());
    let lyrics = if cfg.lyrics_font.is_empty() {
        &cfg.font_family
    } else {
        &cfg.lyrics_font
    };
    t.set_lyrics_family(lyrics.as_str().into());
    t.set_font_scale(scale(cfg.font_size));
    let d = ui.global::<Dialogs>();
    d.set_font_family(cfg.font_family.as_str().into());
    d.set_lyrics_font(cfg.lyrics_font.as_str().into());
    let px = if cfg.font_size == 0 {
        13
    } else {
        cfg.font_size
    };
    d.set_font_size(px.to_string().into());
}

pub fn init(ui: &AppWindow, cfg: &Config) {
    push(ui, cfg);
    // scanning the system fonts takes a moment: off the UI thread
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let mut list = vec!["System default".to_string()];
        list.extend(families());
        let _ = weak.upgrade_in_event_loop(move |ui| set_list(&ui, list));
    });
}

/// Screenshot fixtures have no event loop: apply the settings and fill the list directly.
pub fn init_sync(ui: &AppWindow, cfg: &Config) {
    push(ui, cfg);
    let mut list = vec!["System default".to_string()];
    list.extend(families());
    set_list(ui, list);
}

/// Fill the family list (also used by screenshot fixtures).
pub fn set_list(ui: &AppWindow, list: Vec<String>) {
    let model: Vec<SharedString> = list.into_iter().map(Into::into).collect();
    ui.global::<Dialogs>()
        .set_fonts(ModelRc::new(VecModel::from(model)));
}

fn save(f: impl FnOnce(&mut Config)) -> Config {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        f(&mut s.cfg);
        let _ = s.cfg.save();
        s.cfg.clone()
    })
}

pub fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_font(move |target, family| {
        let cfg = save(|c| {
            if target == "lyrics" {
                c.lyrics_font = family.to_string();
            } else {
                c.font_family = family.to_string();
            }
        });
        if let Some(ui) = weak.upgrade() {
            push(&ui, &cfg);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_font_size(move |size| {
        let px: u32 = size.parse().unwrap_or(13);
        let cfg = save(|c| c.font_size = if px == 13 { 0 } else { px.clamp(10, 24) });
        if let Some(ui) = weak.upgrade() {
            push(&ui, &cfg);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_scale() {
        assert_eq!(scale(0), 1.0);
        assert_eq!(scale(13), 1.0);
        assert!((scale(26) - 24.0 / 13.0).abs() < 1e-6, "clamped to 24 px");
        assert!(scale(15) > 1.0 && scale(2) < 1.0);
    }
}
