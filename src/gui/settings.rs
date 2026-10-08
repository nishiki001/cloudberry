//! First-run sign-in panel and the settings dialog.
use super::state::{STATE, show_toast};
use crate::auth_cmd;
use crate::config::{self, Config};
use crate::core::api::client::Client;
use crate::core::msg::Command;
use crate::{AppWindow, Dialogs, Theme};
use slint::ComponentHandle;

const FORMATS: [&str; 2] = ["bestaudio/best", "bestaudio[abr<=128]/bestaudio/best"];

fn dir_size(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

fn fmt_size(b: u64) -> String {
    if b >= 1 << 20 {
        format!("{:.1} MB", b as f64 / 1048576.0)
    } else {
        format!("{} KB", b / 1024)
    }
}

/// Directory walks happen off the UI thread.
fn refresh_cache_size(ui: &AppWindow) {
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let size = fmt_size(dir_size(&thumbs_dir()));
        let _ = weak
            .upgrade_in_event_loop(move |ui| ui.global::<Dialogs>().set_cache_size(size.into()));
    });
}

fn thumbs_dir() -> std::path::PathBuf {
    config::cache_dir().join("thumbs")
}

pub(super) fn save(f: impl FnOnce(&mut Config)) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        f(&mut s.cfg);
        let _ = s.cfg.save();
    });
}

fn quality_index(fmt: &str) -> i32 {
    FORMATS.iter().position(|f| *f == fmt).unwrap_or(0) as i32
}

pub fn init(ui: &AppWindow, cfg: &Config) {
    ui.global::<Dialogs>()
        .set_setup_browser(cfg.browser.clone().into());
    ui.global::<Dialogs>()
        .set_audio_quality(quality_index(&cfg.audio_format));
    ui.global::<Dialogs>()
        .set_backend(cfg.backend.clone().into());
    ui.global::<Dialogs>().set_spinning_disc(cfg.spinning_disc);
    ui.global::<Dialogs>()
        .set_disc_speed(cfg.disc_speed.clone().into());
    ui.global::<Theme>()
        .set_disc_speed(crate::core::disc::degrees_per_sec(&cfg.disc_speed));
    refresh_cache_size(ui);
    // first run: no cookies yet and the panel was never dismissed
    let signed = config::cookies_path().exists();
    ui.set_status_account(if signed { "signed in" } else { "not signed in" }.into());
    ui.global::<Dialogs>()
        .set_setup_visible(!cfg.setup_done && !signed);
}

/// Export cookies in a worker thread, then verify with an account call.
fn run_setup(ui: &AppWindow, browser: String) {
    if ui.global::<Dialogs>().get_setup_busy() {
        return; // an export is already running (they would race on the temp file)
    }
    ui.global::<Dialogs>().set_setup_busy(true);
    ui.global::<Dialogs>()
        .set_setup_status("Asking yt-dlp for your cookies…".into());
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = auth_cmd::export_cookies(&browser).and_then(|()| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(async { Client::new(Some(config::cookies_path()))?.signed_in().await })
        });
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.global::<Dialogs>().set_setup_busy(false);
            match result {
                Ok(true) => {
                    ui.global::<Dialogs>().set_setup_status("Signed in ✓".into());
                    ui.global::<Dialogs>().set_setup_visible(false);
                    ui.set_status_account("signed in".into());
                    save(|c| {
                        c.browser = browser;
                        c.setup_done = true;
                    });
                    if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
                        core.send(Command::ReloadCookies(Some(config::cookies_path())));
                    }
                    show_toast(&ui, "signed in");
                }
                Ok(false) => ui.global::<Dialogs>().set_setup_status(format!("Cookies exported, but {browser} does not look signed in to YouTube Music. Sign in there first, then retry.").into()),
                Err(e) => ui.global::<Dialogs>().set_setup_status(format!("Setup failed: {e:#}").into()),
            }
        });
    });
}

/// "Import cookies.txt…": the native / portal file chooser, then the same check as the browser route.
fn import_cookies(ui: &AppWindow) {
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let picked = rfd::FileDialog::new()
            .set_title("Choose a cookies.txt exported from your browser")
            .add_filter("Cookies", &["txt"])
            .pick_file();
        let Some(path) = picked else { return };
        let result = auth_cmd::import_cookie_file(&path).and_then(|_| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(async { Client::new(Some(config::cookies_path()))?.signed_in().await })
        });
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let d = ui.global::<Dialogs>();
            match result {
                Ok(true) => {
                    d.set_setup_status("Signed in ✓".into());
                    d.set_setup_visible(false);
                    ui.set_status_account("signed in".into());
                    save(|c| c.setup_done = true);
                    if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
                        core.send(Command::ReloadCookies(Some(config::cookies_path())));
                    }
                    show_toast(&ui, "signed in");
                }
                Ok(false) => d.set_setup_status(
                    "Cookies imported, but they do not look signed in to YouTube Music.".into(),
                ),
                Err(e) => d.set_setup_status(format!("Import failed: {e:#}").into()),
            }
        });
    });
}

pub fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_import_cookies(move || {
        if let Some(ui) = weak.upgrade() {
            import_cookies(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_setup_run(move |b| {
        if let Some(ui) = weak.upgrade() {
            run_setup(&ui, b.to_string());
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_setup_skip(move || {
        save(|c| c.setup_done = true);
        if let Some(ui) = weak.upgrade() {
            ui.global::<Dialogs>().set_setup_visible(false);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_theme(move |m| {
        save(|c| c.theme = ["system", "light", "dark"][m.clamp(0, 2) as usize].into());
        if let Some(ui) = weak.upgrade() {
            super::appearance::apply_theme(&ui, m);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_skin(move |id| {
        let skin = crate::core::skin::Skin::parse(&id).name();
        save(|c| c.skin = skin.into());
        if let Some(ui) = weak.upgrade() {
            let mode = ui.global::<Theme>().get_mode();
            super::appearance::apply_theme(&ui, mode);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_icon_set(move |id| {
        let set = crate::core::icons::parse(&id);
        save(|c| c.icon_set = set.into());
        if let Some(ui) = weak.upgrade() {
            super::icons::apply(&ui, set);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_analyzer(move |id| {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_analyzer_menu(id);
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_spinning_disc(move |v| {
        save(|c| c.spinning_disc = v);
        if let Some(ui) = weak.upgrade() {
            ui.global::<Dialogs>().set_spinning_disc(v);
            show_toast(&ui, "the cover style applies after a restart");
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_disc_speed(move |id| {
        let id = if matches!(id.as_str(), "slow" | "fast") {
            id.to_string()
        } else {
            "normal".to_string()
        };
        save(|c| c.disc_speed.clone_from(&id));
        if let Some(ui) = weak.upgrade() {
            ui.global::<Dialogs>().set_disc_speed(id.as_str().into());
            ui.global::<Theme>()
                .set_disc_speed(crate::core::disc::degrees_per_sec(&id));
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_backend(move |id| {
        let id = if id == "mpv" { "mpv" } else { "native" };
        save(|c| c.backend = id.into());
        if let Some(ui) = weak.upgrade() {
            ui.global::<Dialogs>().set_backend(id.into());
            show_toast(&ui, "the audio engine applies after a restart");
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_reduce_motion(move |v| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<Theme>().set_reduce_motion(v);
            super::analyzer::ensure_running(&ui);
        }
        save(|c| c.reduce_motion = v);
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_quality(move |q| {
        let fmt = FORMATS[q.clamp(0, 1) as usize];
        if let Some(ui) = weak.upgrade() {
            ui.global::<Dialogs>().set_audio_quality(q);
            show_toast(&ui, "audio quality applies to the next track");
        }
        if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
            core.send(Command::SetAudioFormat(fmt.into()));
        }
        save(|c| c.audio_format = fmt.into());
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_clear_cache(move || {
        let weak = weak.clone();
        std::thread::spawn(move || {
            if let Ok(rd) = std::fs::read_dir(thumbs_dir()) {
                for e in rd.flatten() {
                    let _ = std::fs::remove_file(e.path());
                }
            }
            let _ = weak.upgrade_in_event_loop(|ui| refresh_cache_size(&ui));
        });
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_settings_opened(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_cache_size(&ui);
            super::tools_ui::refresh(&ui);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes_and_quality() {
        assert_eq!(fmt_size(2048), "2 KB");
        assert_eq!(fmt_size(3 * 1048576), "3.0 MB");
        assert_eq!(quality_index("bestaudio[abr<=128]/bestaudio/best"), 1);
        assert_eq!(quality_index("whatever"), 0);
    }
}
