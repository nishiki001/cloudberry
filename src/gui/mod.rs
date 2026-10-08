//! GUI glue: Slint window ⇄ core runtime. Everything here runs on the UI thread except the
//! event sink, which hops over with `invoke_from_event_loop`.
pub mod analyzer;
pub mod analyzer_menu;
pub mod appearance;
pub mod artist;
mod backdrop;
mod colorpicker;
pub mod discover;
mod events;
mod fonts;
mod icons;
#[cfg(test)]
mod interaction_tests;
mod library_refresh;
mod library_stream;
mod panels;
mod playlist_add;
mod playlist_result;
mod search_results;
mod settings;
mod smoke;
pub mod state;
mod table;
mod tabs;
pub mod theme;
mod tools_ui;
mod wire;
mod wire_lib;

use crate::cli::Cli;
use crate::config::{self, Config};
use crate::core::model::{Item, Kind};
use crate::core::msg::Command;
use crate::core::runtime::{self, CoreDeps, PlayerFactory};
use crate::deps;
use crate::{AppWindow, Panels, Theme};
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use state::STATE;
use std::sync::Arc;
use std::time::Duration;

use events::handle_event;
pub use state::install_art;
use wire::wire;

pub fn run(cli: &Cli) -> anyhow::Result<i32> {
    let found = deps::locate();
    let missing = deps::missing(&found, &Config::load().backend);
    let downloadable = missing.iter().any(|m| matches!(*m, "yt-dlp" | "deno"))
        && crate::tools::managed_enabled()
        && crate::tools::ytdlp_asset().is_some();
    if !missing.is_empty() && downloadable && cli.command.is_none() {
        return tools_ui::first_run(&missing);
    }
    if !missing.is_empty() {
        let text = deps::install_help(&missing);
        eprint!("{text}");
        let dlg = crate::DepsDialog::new()?;
        dlg.set_message(text.into());
        dlg.on_quit(|| {
            let _ = slint::quit_event_loop();
        });
        dlg.run()?;
        return Ok(1);
    }
    let cfg = Config::load();
    select_renderer(cfg.spinning_disc);
    let ui = AppWindow::new()?;
    install_art(&ui);
    let mode = match cfg.theme.as_str() {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    };
    theme::apply(&ui, mode, None);
    ui.global::<Theme>().set_reduce_motion(cfg.reduce_motion);
    ui.global::<Theme>().set_spinning_disc(cfg.spinning_disc);
    ui.set_volume(cfg.volume as f32 / 100.0);
    ui.global::<Panels>()
        .set_results(ModelRc::new(VecModel::default()));

    let weak = ui.as_weak();
    let sink: runtime::EventSink = Arc::new(move |ev| {
        let _ = weak.upgrade_in_event_loop(move |ui| handle_event(&ui, ev));
    });
    let ytdlp = deps::path_of(&found, "yt-dlp");
    let (fmt, vol) = (cfg.audio_format.clone(), cfg.volume);
    let backend = cfg.backend.clone();
    let tap_slot: Arc<std::sync::OnceLock<Arc<crate::player::native::Shared>>> = Arc::default();
    let slot2 = tap_slot.clone();
    let factory: PlayerFactory = Box::new(move || {
        let b = crate::player::create(crate::player::BackendOptions {
            backend,
            ytdl_path: ytdlp,
            cookies: Some(config::cookies_path()),
            audio_format: fmt,
            volume: vol,
        })?;
        if let Some(t) = &b.tap {
            let _ = slot2.set(t.clone());
        }
        Ok(b)
    });
    let core = runtime::spawn(
        sink,
        factory,
        CoreDeps {
            cookie_path: Some(config::cookies_path()),
            thumb_dir: Some(config::cache_dir().join("thumbs")),
            volume: cfg.volume,
            lrclib: cfg.lyrics_lrclib,
            #[cfg(test)]
            api_base: None,
        },
    );
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.cfg = cfg.clone();
        s.core = Some(core.clone());
    });
    analyzer::init(&ui, &cfg, tap_slot);
    settings::init(&ui, &cfg);
    settings::wire(&ui);
    tools_ui::init(&ui, &cfg);
    tools_ui::wire(&ui);
    appearance::init(&ui, &cfg);
    appearance::wire(&ui);
    fonts::init(&ui, &cfg);
    fonts::wire(&ui);
    tabs::restore();
    panels::init_tree();
    panels::wire(&ui, &core);
    discover::wire(&ui, &core);
    artist::wire(&ui, &core);
    STATE.with(|s| s.borrow_mut().ui = Some(ui.as_weak()));
    playlist_add::wire(&ui, &core);
    panels::refresh_tree(&ui);
    table::wire(&ui, &core);
    tabs::wire(&ui);
    table::refresh_columns(&ui);
    table::refresh(&ui);
    wire(&ui, &core, cfg.clone());
    artist::on_select(&ui); // the restored tab may be an artist / album page
    let weak = ui.as_weak();
    ui.on_system_theme_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            let mode = ui.global::<Theme>().get_mode();
            if mode == 0 {
                appearance::apply_theme(&ui, 0); // follow the OS change
            }
        }
    });
    STATE.with(|s| s.borrow_mut().media = crate::mediactl::MediaCtl::new(core.clone(), None));

    if let Some(id) = &cli.autoplay {
        let item = Item {
            kind: Kind::Song,
            title: id.clone(),
            video_id: Some(id.clone()),
            browse_id: None,
            artists: vec![],
            album: None,
            album_id: None,
            duration: None,
            duration_secs: None,
            thumbnail: Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
            subtitle: String::new(),
        };
        core.send(Command::PlayItems {
            items: vec![item],
            index: 0,
        });
    }
    if let Some(id) = &cli.open_playlist {
        wire_lib::open_as_tab(
            &core,
            crate::core::msg::LibraryKind::Playlist,
            id.clone(),
            "Playlist".into(),
        );
    }
    // debugging aid: CLOUDBERRY_WINDOW_SIZE=WxH sizes the window (for measurements)
    if let Some((w, h)) = std::env::var("CLOUDBERRY_WINDOW_SIZE")
        .ok()
        .and_then(|v| {
            v.split_once('x')
                .map(|(w, h)| (w.to_string(), h.to_string()))
        })
        .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
    {
        ui.window().set_size(slint::LogicalSize::new(w, h));
    }
    if let Some(section) = &cli.open_discover {
        discover::choose(&ui, &core, section, None, false);
    }
    let _smoke = cli.smoke.then(|| smoke::run(&ui));
    let mini_timer = Timer::default();
    if cli.mini {
        let weak = ui.as_weak();
        mini_timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(700),
            move || {
                if let Some(ui) = weak.upgrade() {
                    wire::toggle_mini(&ui);
                }
            },
        );
    }
    // debugging aid: CLOUDBERRY_SNAPSHOT=<png> saves the real window's pixels shortly before quitting
    let snap_timer = Timer::default();
    if let (Some(path), Some(n)) = (std::env::var_os("CLOUDBERRY_SNAPSHOT"), cli.quit_after) {
        let weak = ui.as_weak();
        snap_timer.start(
            TimerMode::SingleShot,
            Duration::from_secs(n.saturating_sub(1)),
            move || {
                if let Some(ui) = weak.upgrade()
                    && let Ok(buf) = ui.window().take_snapshot()
                {
                    let _ = image::save_buffer(
                        &path,
                        buf.as_bytes(),
                        buf.width(),
                        buf.height(),
                        image::ColorType::Rgba8,
                    );
                }
            },
        );
    }
    let quit_timer = Timer::default();
    if let Some(n) = cli.quit_after {
        quit_timer.start(TimerMode::SingleShot, Duration::from_secs(n), || {
            let _ = slint::quit_event_loop();
        });
    }
    ui.run()?;
    // the live config (menus and settings saved into it during the session), plus the volume
    let mut cfg = STATE.with(|s| s.borrow().cfg.clone());
    cfg.volume = (ui.get_volume() * 100.0).round() as u8;
    let _ = cfg.save();
    core.send(Command::Quit);
    let progressed = STATE.with(|s| s.borrow().progressed);
    if cli.autoplay.is_some() && !progressed {
        eprintln!("autoplay: time-pos never advanced");
        return Ok(1);
    }
    Ok(0)
}

/// Software rendering repaints only what changed (analyzer, time, slider): far cheaper than GL
/// for this UI. The spinning disc rotates an image, which only the GL renderer can do.
/// `SLINT_BACKEND` in the environment always wins.
fn select_renderer(spinning_disc: bool) {
    if std::env::var_os("SLINT_BACKEND").is_some() {
        set_app_id();
        return;
    }
    let renderer = if spinning_disc { "femtovg" } else { "software" };
    if let Err(e) = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.into())
        .select()
    {
        tracing::warn!("renderer {renderer} unavailable ({e}); using the default");
    }
    set_app_id();
}

/// Wayland app id / X11 class: KDE matches it to `<id>.desktop` for the launcher icon. Needs the
/// platform to exist, so it comes after the backend selection (and before the first window).
fn set_app_id() {
    if let Err(e) = slint::set_xdg_app_id(crate::deps::APP_ID) {
        tracing::warn!("app id not set: {e}");
    }
}

/// Screenshot mode: a known table width and a sorted header so the arrow shows.
pub fn table_fixture_columns(ui: &AppWindow, sorted: bool) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.sort = sorted.then_some((crate::core::columns::Col::Title, true));
    });
    table::wire_width(ui);
    table::refresh_columns(ui);
}

/// Screenshot mode: a library tree with two expanded sections and thumbnails.
pub fn fixture_tree(ui: &AppWindow) {
    panels::init_tree();
    panels::fixture_tree(ui);
}

/// Screenshot mode: the table reads the same persistent models as the real app.
pub fn install_fixture_rows(ui: &AppWindow, rows: Vec<crate::TrackData>) {
    table::install_models(ui);
    STATE.with(|s| s.borrow().rows_model.set_vec(rows));
}
