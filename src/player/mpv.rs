//! libmpv2 implementation of `Player`.
use super::{EndReason, LoadMode, Player, PlayerEvent, PlayerEvents};
use anyhow::{Context, Result, anyhow};
use libmpv2::events::{Event, PropertyData};
use libmpv2::{Format, Mpv};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

pub struct MpvOptions {
    pub ytdl_path: Option<PathBuf>,
    pub audio_format: String,
    pub cookies: Option<PathBuf>,
    pub volume: u8,
}

pub struct MpvPlayer {
    mpv: Arc<Mpv>,
    stop: Arc<AtomicBool>,
}

fn e<T>(r: libmpv2::Result<T>, what: &str) -> Result<T> {
    r.map_err(|err| anyhow!("mpv {what}: {err:?}"))
}

/// mpv key-value list value with explicit length, safe for commas and `=`.
fn kv_escape(v: &str) -> String {
    format!("%{}%{}", v.len(), v)
}

impl MpvPlayer {
    pub fn new(opts: MpvOptions) -> Result<(Self, PlayerEvents)> {
        let null_ao = std::env::var("APP_AO")
            .map(|v| v == "null")
            .unwrap_or(false);
        let mpv = e(
            Mpv::with_initializer(|init| {
                for (k, v) in [
                    ("config", "no"),
                    ("terminal", "no"),
                    ("input-default-bindings", "no"),
                    ("input-vo-keyboard", "no"),
                    ("vid", "no"),
                    ("audio-display", "no"),
                    ("force-window", "no"),
                    ("idle", "yes"),
                    ("ytdl", "yes"),
                    ("prefetch-playlist", "yes"),
                    ("gapless-audio", "weak"),
                    ("cache", "yes"),
                    ("volume-max", "100"),
                ] {
                    init.set_property(k, v)?;
                }
                init.set_property("ytdl-format", opts.audio_format.as_str())?;
                let mut so = String::from("ytdl_hook-try_ytdl_first=yes");
                if let Some(p) = &opts.ytdl_path {
                    so.push_str(&format!(
                        ",ytdl_hook-ytdl_path={}",
                        kv_escape(&p.to_string_lossy())
                    ));
                }
                init.set_property("script-opts", so.as_str())?;
                if let Some(c) = opts.cookies.as_ref().filter(|c| c.exists()) {
                    let raw = format!("cookies={}", kv_escape(&c.to_string_lossy()));
                    init.set_property("ytdl-raw-options", raw.as_str())?;
                }
                init.set_property("volume", opts.volume as i64)?;
                if null_ao {
                    init.set_property("ao", "null")?;
                }
                Ok(())
            }),
            "init",
        )
        .context("libmpv init (is libmpv installed?)")?;
        let mpv = Arc::new(mpv);
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::unbounded_channel();

        let client = e(mpv.create_client(Some("events")), "create_client")?;
        e(client.disable_deprecated_events(), "events")?;
        for (i, (name, fmt)) in [
            ("time-pos", Format::Double),
            ("duration", Format::Double),
            ("pause", Format::Flag),
            ("idle-active", Format::Flag),
            ("volume", Format::Double),
            ("demuxer-cache-time", Format::Double),
        ]
        .into_iter()
        .enumerate()
        {
            e(client.observe_property(name, fmt, i as u64 + 1), "observe")?;
        }
        let stop2 = stop.clone();
        std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || event_loop(client, tx, stop2))
            .context("spawn mpv event thread")?;
        Ok((Self { mpv, stop }, rx))
    }

    fn cmd(&self, name: &str, args: &[&str]) -> Result<()> {
        e(self.mpv.command(name, args), name)
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wakes the event thread (blocking wait) with a Shutdown event.
        let _ = self.mpv.command("quit", &[]);
    }
}

fn event_loop(client: Mpv, tx: mpsc::UnboundedSender<PlayerEvent>, stop: Arc<AtomicBool>) {
    use libmpv2_sys as sys;
    let mut last_pos = f64::NEG_INFINITY;
    while !stop.load(Ordering::Relaxed) {
        let Some(ev) = client.wait_event(-1.0) else {
            continue;
        };
        let out = match ev {
            Ok(Event::Shutdown) => break,
            Ok(Event::StartFile) => Some(PlayerEvent::StartFile),
            Ok(Event::FileLoaded) => Some(PlayerEvent::FileLoaded),
            Ok(Event::EndFile(r)) => Some(PlayerEvent::EndFile(match r {
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_EOF => EndReason::Eof,
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_STOP => EndReason::Stop,
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_ERROR => EndReason::Error,
                _ => EndReason::Other,
            })),
            Ok(Event::PropertyChange { name, change, .. }) => match (name, change) {
                ("time-pos", PropertyData::Double(v)) if (v - last_pos).abs() >= 0.25 => {
                    last_pos = v;
                    Some(PlayerEvent::TimePos(v))
                }
                ("time-pos", _) => None,
                ("duration", PropertyData::Double(v)) => Some(PlayerEvent::Duration(v)),
                ("pause", PropertyData::Flag(v)) => Some(PlayerEvent::Paused(v)),
                ("idle-active", PropertyData::Flag(v)) => Some(PlayerEvent::Idle(v)),
                ("volume", PropertyData::Double(v)) => Some(PlayerEvent::Volume(v)),
                ("demuxer-cache-time", PropertyData::Double(v)) => Some(PlayerEvent::Buffered(v)),
                _ => None,
            },
            Ok(_) => None,
            Err(err) => {
                // libmpv2 reports END_FILE with reason error as Err.
                tracing::debug!("mpv event error: {err:?}");
                Some(PlayerEvent::Error(format!("{err:?}")))
            }
        };
        if let Some(o) = out
            && tx.send(o).is_err()
        {
            break;
        }
    }
}

impl Player for MpvPlayer {
    fn load(&self, url: &str, mode: LoadMode) -> Result<()> {
        let m = match mode {
            LoadMode::Replace => "replace",
            LoadMode::Append => "append",
        };
        self.cmd("loadfile", &[url, m])?;
        if mode == LoadMode::Replace {
            e(self.mpv.set_property("pause", false), "pause")?;
        }
        Ok(())
    }
    fn play(&self) -> Result<()> {
        e(self.mpv.set_property("pause", false), "pause")
    }
    fn pause(&self) -> Result<()> {
        e(self.mpv.set_property("pause", true), "pause")
    }
    fn toggle(&self) -> Result<()> {
        self.cmd("cycle", &["pause"])
    }
    fn stop(&self) -> Result<()> {
        self.cmd("stop", &[])
    }
    fn seek(&self, secs: f64) -> Result<()> {
        self.cmd("seek", &[&format!("{secs:.2}"), "absolute"])
    }
    fn set_volume(&self, vol: u8) -> Result<()> {
        e(
            self.mpv.set_property("volume", vol.min(100) as i64),
            "volume",
        )
    }
    fn next(&self) -> Result<()> {
        self.cmd("playlist-next", &[])
    }
    fn prev(&self) -> Result<()> {
        self.cmd("playlist-prev", &[])
    }
    fn set_audio_format(&self, format: &str) -> Result<()> {
        e(self.mpv.set_property("ytdl-format", format), "ytdl-format")
    }
    fn set_cookies(&self, path: Option<&std::path::Path>) -> Result<()> {
        let raw = match path.filter(|p| p.exists()) {
            Some(p) => format!("cookies={}", kv_escape(&p.to_string_lossy())),
            None => String::new(),
        };
        e(
            self.mpv.set_property("ytdl-raw-options", raw.as_str()),
            "ytdl-raw-options",
        )
    }
    fn clear_pending(&self) -> Result<()> {
        // keeps the playing entry
        self.cmd("playlist-clear", &[])
    }
}
