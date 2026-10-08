//! Managed helper tools in the GUI: first-run download, daily yt-dlp update, Settings → Playback.
use crate::tools;
use crate::{AppWindow, DepsDialog, Dialogs};
use slint::ComponentHandle;

/// First run: yt-dlp / deno are missing and the managed copies are on. Shows the download dialog;
/// on success the app starts again (PATH must be set before any thread exists).
pub fn first_run(missing: &[&str]) -> anyhow::Result<i32> {
    let help = crate::deps::install_help(missing);
    let dlg = DepsDialog::new()?;
    dlg.set_can_retry(true);
    dlg.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    let weak = dlg.as_weak();
    let start = move || {
        let w = weak.clone();
        let help = help.clone();
        let _ = w.upgrade_in_event_loop(|d| {
            d.set_busy(true);
            d.set_message("Downloading playback helpers…".into());
        });
        std::thread::spawn(move || {
            let w2 = w.clone();
            let res = tools::ensure(&move |p| {
                let text = p.text();
                let _ = w2.upgrade_in_event_loop(move |d| d.set_message(text.into()));
            });
            let _ = w.upgrade_in_event_loop(move |d| match res {
                Ok(()) => {
                    relaunch();
                    let _ = slint::quit_event_loop();
                }
                Err(e) => {
                    d.set_message(format!("Download failed: {e:#}\n\nCheck the connection and retry, or install by hand:\n{help}").into());
                    d.set_busy(false);
                }
            });
        });
    };
    dlg.on_retry({
        let start = start.clone();
        move || start()
    });
    start();
    dlg.run()?;
    Ok(0)
}

/// Start this program again with the same arguments.
fn relaunch() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .spawn();
    }
}

fn status_line() -> String {
    let one = |t: &str| {
        let found = if tools::managed_enabled() {
            tools::managed_path(t)
        } else {
            None
        }
        .or_else(|| crate::deps::path_of(&crate::deps::locate(), t));
        // "2026.08.19" (yt-dlp) / "deno 2.9.7 (stable, …)"
        let ver = found.and_then(|p| tools::version_of(&p)).and_then(|v| {
            let mut w = v.split_whitespace();
            let first = w.next()?.to_string();
            Some(if first == t {
                w.next()?.to_string()
            } else {
                first
            })
        });
        match ver {
            Some(v) => format!("{t} {v}"),
            None => format!("{t} missing"),
        }
    };
    format!("{} · {}", one("yt-dlp"), one("deno"))
}

/// Fill the Settings line (runs the tools, so off the UI thread).
pub fn refresh(ui: &AppWindow) {
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let line = status_line();
        let _ = weak
            .upgrade_in_event_loop(move |ui| ui.global::<Dialogs>().set_tools_status(line.into()));
    });
}

pub fn init(ui: &AppWindow, cfg: &crate::config::Config) {
    let d = ui.global::<Dialogs>();
    d.set_mpv_available(cfg!(feature = "mpv"));
    d.set_tools_locked(tools::SYSTEM_ONLY_BUILD);
    let managed = tools::managed_enabled();
    d.set_tools_source(if managed { "managed" } else { "system" }.into());
    refresh(ui);
    let _ = cfg;
    // at most one update check a day, in the background
    if managed && tools::managed_path("yt-dlp").is_some() && tools::update_due() {
        std::thread::spawn(|| match tools::update_ytdlp() {
            Ok(t) => tracing::info!("yt-dlp update check: {t}"),
            Err(e) => tracing::warn!("yt-dlp update check failed: {e:#}"),
        });
    }
}

pub fn wire(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_set_tools_source(move |id| {
        let system = id == "system";
        super::settings::save(|c| c.use_system_tools = system);
        if let Some(ui) = weak.upgrade() {
            ui.global::<Dialogs>().set_tools_source(id);
            super::state::show_toast(&ui, "applies after a restart");
        }
    });
    let weak = ui.as_weak();
    ui.global::<Dialogs>().on_update_tools(move || {
        let Some(ui) = weak.upgrade() else { return };
        ui.global::<Dialogs>().set_tools_busy(true);
        let w = weak.clone();
        std::thread::spawn(move || {
            let res = tools::ensure(&|_| {}).and_then(|()| tools::update_ytdlp());
            let line = status_line();
            let _ = w.upgrade_in_event_loop(move |ui| {
                let d = ui.global::<Dialogs>();
                d.set_tools_busy(false);
                d.set_tools_status(line.into());
                let msg = match res {
                    Ok(t) => t.lines().last().unwrap_or("up to date").to_string(),
                    Err(e) => format!("update failed: {e:#}"),
                };
                super::state::show_toast(&ui, &msg);
            });
        });
    });
}
