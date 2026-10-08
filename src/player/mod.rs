//! Playback abstraction. The GUI and CLI only see the `Player` trait and `PlayerEvent`s.
#![allow(dead_code)] // transport API is consumed by the GUI from M5 on
#[cfg(feature = "mpv")]
pub mod mpv;
pub mod native;

use anyhow::Result;
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadMode {
    Replace,
    Append,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Eof,
    Stop,
    Error,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    TimePos(f64),
    Duration(f64),
    Paused(bool),
    Idle(bool),
    /// End of the demuxer cache in seconds (buffered range end).
    Buffered(f64),
    Volume(f64),
    /// Stream format for the status bar, e.g. "AAC 129 kbps".
    Format(String),
    StartFile,
    FileLoaded,
    EndFile(EndReason),
    Error(String),
}

pub trait Player: Send + Sync {
    fn load(&self, url: &str, mode: LoadMode) -> Result<()>;
    fn play(&self) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn toggle(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    /// Seek to an absolute position in seconds.
    fn seek(&self, secs: f64) -> Result<()>;
    /// Volume 0..=100.
    fn set_volume(&self, vol: u8) -> Result<()>;
    fn next(&self) -> Result<()>;
    fn prev(&self) -> Result<()>;
    /// Options applied to the next load (settings dialog / first-run sign-in).
    fn set_audio_format(&self, format: &str) -> Result<()>;
    fn set_cookies(&self, path: Option<&std::path::Path>) -> Result<()>;
    /// Drop everything queued behind the current entry (prefetched next tracks).
    fn clear_pending(&self) -> Result<()>;
}

pub type PlayerEvents = UnboundedReceiver<PlayerEvent>;

pub fn watch_url(id_or_url: &str) -> String {
    if id_or_url.contains("://") {
        id_or_url.to_string()
    } else {
        format!("https://music.youtube.com/watch?v={id_or_url}")
    }
}

/// A ready playback backend plus what the app needs from it.
pub struct Backend {
    pub player: std::sync::Arc<dyn Player>,
    pub events: PlayerEvents,
    /// Shown to the user, e.g. when the native backend failed and mpv took over.
    pub notice: Option<String>,
    /// Decoded-sample tap for the analyzer (native backend only).
    pub tap: Option<std::sync::Arc<native::Shared>>,
}

pub struct BackendOptions {
    /// "native" (default) or "mpv".
    pub backend: String,
    pub ytdl_path: Option<std::path::PathBuf>,
    pub cookies: Option<std::path::PathBuf>,
    pub audio_format: String,
    pub volume: u8,
}

/// Native first (unless configured otherwise); falls back to mpv when native cannot start.
pub fn create(o: BackendOptions) -> Result<Backend> {
    let mut notice = None;
    if o.backend != "mpv" || !cfg!(feature = "mpv") {
        let ytdl = o
            .ytdl_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("yt-dlp not found"))?;
        match native::NativePlayer::new(native::NativeOptions {
            ytdl_path: ytdl,
            cookies: o.cookies.clone(),
            volume: o.volume,
        }) {
            Ok((p, events)) => {
                let tap = Some(p.tap());
                return Ok(Backend {
                    player: std::sync::Arc::new(p),
                    events,
                    notice: None,
                    tap,
                });
            }
            Err(e) => notice = Some(format!("native audio unavailable ({e:#}); using mpv")),
        }
    }
    #[cfg(feature = "mpv")]
    {
        let (p, events) = mpv::MpvPlayer::new(mpv::MpvOptions {
            ytdl_path: o.ytdl_path,
            audio_format: o.audio_format,
            cookies: o.cookies,
            volume: o.volume,
        })?;
        Ok(Backend {
            player: std::sync::Arc::new(p),
            events,
            notice,
            tap: None,
        })
    }
    #[cfg(not(feature = "mpv"))]
    {
        let _ = o.audio_format;
        anyhow::bail!(
            "{}",
            notice.unwrap_or_else(|| "no usable audio backend".into())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn url_building() {
        assert_eq!(watch_url("abc"), "https://music.youtube.com/watch?v=abc");
        assert_eq!(watch_url("https://x.y/z"), "https://x.y/z");
    }
}
