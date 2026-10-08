//! Headless `play` subcommand.
use crate::config::{self, Config};
use crate::deps;
use crate::player::{EndReason, LoadMode, PlayerEvent, watch_url};
use anyhow::Result;
use std::time::{Duration, Instant};

pub fn run(target: &str, seconds: Option<u64>, seek: Option<u64>) -> Result<i32> {
    let found = deps::locate();
    let missing = deps::missing(&found, &Config::load().backend);
    if !missing.is_empty() {
        eprint!("{}", deps::install_help(&missing));
        return Ok(1);
    }
    let cfg = Config::load();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let crate::player::Backend {
            player,
            mut events,
            notice,
            ..
        } = crate::player::create(crate::player::BackendOptions {
            backend: cfg.backend.clone(),
            ytdl_path: deps::path_of(&found, "yt-dlp"),
            cookies: Some(config::cookies_path()),
            audio_format: cfg.audio_format.clone(),
            volume: cfg.volume,
        })?;
        if let Some(n) = notice {
            eprintln!("{n}");
        }
        player.load(&watch_url(target), LoadMode::Replace)?;
        let overall = Instant::now() + Duration::from_secs(seconds.unwrap_or(0) + 60);
        let mut last_print = -1.0f64;
        let mut advanced = false;
        let mut seeked = false;
        let mut seek_ok = seek.is_none();
        loop {
            let ev = tokio::time::timeout_at(overall.into(), events.recv()).await;
            let ev = match ev {
                Ok(Some(e)) => e,
                _ => break,
            };
            match ev {
                PlayerEvent::TimePos(t) => {
                    if t >= 1.0 {
                        advanced = true;
                    }
                    if let Some(target_pos) = seek {
                        if !seeked && t >= 2.0 {
                            seeked = true;
                            println!("seeking to {target_pos}");
                            player.seek(target_pos as f64)?;
                        } else if seeked
                            && t >= target_pos as f64 - 0.3
                            && t < target_pos as f64 + 3.0
                        {
                            println!("position after seek: {t:.2}");
                            seek_ok = (t - target_pos as f64).abs() < 1.5;
                            break;
                        }
                    }
                    if t - last_print >= 1.0 {
                        last_print = t;
                        println!("time-pos {t:.1}");
                    }
                    if let Some(s) = seconds
                        && t >= s as f64
                    {
                        break;
                    }
                }
                PlayerEvent::Duration(d) => println!("duration {d:.1}"),
                PlayerEvent::Error(_) if advanced => {}
                PlayerEvent::Error(_) | PlayerEvent::EndFile(EndReason::Error) => {
                    eprintln!("playback failed — try `yt-dlp -U`");
                    return Ok(1);
                }
                PlayerEvent::EndFile(EndReason::Eof) => break,
                _ => {}
            }
        }
        let _ = player.stop();
        if advanced && seek_ok {
            Ok(0)
        } else {
            eprintln!("time-pos never advanced");
            Ok(1)
        }
    })
}
