mod art;
mod artist_cmd;
mod auth_cmd;
mod cli;
mod config;
mod core;
mod deps;
mod desktop_cmd;
mod discover_cmd;
mod fixtures;
mod fixtures_artist;
mod fixtures_discover;
mod fixtures_extra;
mod fixtures_views;
mod gui;
mod lib_cmd;
mod lyrics_cmd;
mod mediactl;
mod panic_hook;
mod play;
mod player;
mod radio_cmd;
mod screenshot;
mod search_cmd;
mod tools;

use clap::Parser;
use cli::{Cli, Cmd};

slint::include_modules!();

fn main() {
    let cli = Cli::parse();
    // PATH must be set up before any other thread (logging worker) exists.
    let _ = deps::locate();
    let log = config::init_logging();
    panic_hook::install(cli.command.is_none() && cli.screenshot.is_none());
    let code = match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    };
    drop(log); // flush the file logger: process::exit would skip it
    std::process::exit(code);
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    if let Some(out) = &cli.screenshot {
        if cli.view == "deps" {
            screenshot::render_deps(out, cli.theme)?;
            println!("wrote {}", out.display());
            return Ok(0);
        }
        let size = cli
            .size
            .as_deref()
            .and_then(|s| s.split_once('x'))
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .unwrap_or((1200, 760));
        config::NO_PERSIST.store(true, std::sync::atomic::Ordering::Relaxed);
        screenshot::render(out, size, |ui| {
            let mode = match cli.theme {
                cli::ThemeArg::Light => 1,
                cli::ThemeArg::Dark => 2,
            };
            gui::theme::apply(ui, mode, None);
            gui::install_art(ui);
            gui::appearance::fixture(ui, &cli.fixture);
            ui.set_view(cli.view.clone().into());
            fixtures::apply(ui, &cli.view, &cli.fixture);
        })?;
        println!("wrote {}", out.display());
        return Ok(0);
    }
    match cli.command {
        Some(Cmd::Search {
            query,
            filter,
            limit,
            json,
        }) => search_cmd::run(&query, filter.as_deref(), limit, json),
        Some(Cmd::Liked { json }) => lib_cmd::liked(json),
        Some(Cmd::Playlist { id, json }) => lib_cmd::playlist(id.as_deref(), json),
        Some(Cmd::Lyrics { id, json }) => lyrics_cmd::run(&id, json),
        Some(Cmd::Discover {
            section,
            country,
            params,
            raw,
            json,
        }) => discover_cmd::run(&section, &country, params.as_deref(), raw, json),
        Some(Cmd::Artist { target, json }) => artist_cmd::run(&target, json),
        Some(Cmd::Raw { endpoint, body }) => discover_cmd::raw(&endpoint, &body),
        Some(Cmd::Radio { id, json }) => radio_cmd::run(&id, json),
        Some(Cmd::Auth(a)) => auth_cmd::run(a),
        Some(Cmd::Tools { action }) => tools::cli(&action),
        Some(Cmd::Doctor) => Ok(deps::doctor()),
        Some(Cmd::InstallDesktop { prefix }) => desktop_cmd::install(prefix.as_deref()),
        Some(Cmd::UninstallDesktop { prefix }) => desktop_cmd::uninstall(prefix.as_deref()),
        Some(Cmd::Play {
            target,
            seconds,
            seek,
        }) => play::run(&target, seconds, seek),
        None => gui::run(&cli),
    }
}
